use std::borrow::Borrow;
use std::marker::PhantomData;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;

use quote::format_ident;
use quote::ToTokens;

use syn::parse_quote_spanned;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::Attribute;
use syn::Ident;
use syn::ItemImpl;
use syn::ItemStruct;
use syn::TraitBound;

use syn::parse_quote;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::Error;
use syn::Type;

use crate::factory::ComparablePats;
use crate::factory::PenumExpr;
use crate::factory::Subject;
use crate::factory::WherePredicate;

use crate::dispatch::VariantSignature;
use crate::error::Diagnostic;

use crate::utils::get_unique_assertion_statement;
use crate::utils::into_unique_ident;
use crate::utils::lifetime_not_permitted;
use crate::utils::maybe_bounds_not_permitted;
use crate::utils::no_match_found;
use crate::utils::PolymorphicMap;
use crate::utils::TypeId;
use crate::utils::UniqueIdentifier;

pub struct Disassembled;
pub struct Assembled;

/// Top level container type for Penum.
///
/// It contains everything we need to construct our dispatcher and
/// pattern validator.
pub struct Penum<State = Disassembled> {
    pub expr: PenumExpr,
    pub subject: Subject,
    pub error: Diagnostic,
    pub types: PolymorphicMap,
    pub impls: Vec<ItemImpl>,
    _marker: PhantomData<State>,
}

impl Penum<Disassembled> {
    pub fn from(expr: PenumExpr, subject: Subject) -> Self {
        Self {
            expr,
            subject,
            error: Default::default(),
            types: Default::default(),
            impls: Default::default(),
            _marker: Default::default(),
        }
    }

    /// I am using [field / parameter / argument] interchangeably
    pub fn assemble(mut self) -> Penum<Assembled> {
        let variants = &self.subject.get_variants();
        let enum_ident = &self.subject.ident;

        if !variants.is_empty() {
            // The point is that as we check for equality, we also do
            // impl assertions by extending the `subjects` where clause.
            // This is something that we might want to change in the
            // future and instead use `spanned_quote` or some other
            // bound assertion.
            let mut predicates: Punctuated<WherePredicate, Comma> = Default::default();

            // Prepare our patterns by converting them into
            // `Comparables`. This is just a wrapper type that contains
            // commonly used props.
            let comparable_pats: ComparablePats = self.expr.borrow().into();

            // We pre-check for clause because we might be needing this
            // during the dispatch step. Should add
            // `has_dispatchable_member` maybe? let has_clause =
            // self.expr.has_clause(); Turn into iterator instead?
            let mut maybe_blueprints = self.expr.get_blueprints();

            // Expecting failure like `variant doesn't match shape`,
            // hence pre-calling.
            let pattern_fmt = self.expr.pattern_to_string();

            // For each variant:
            // 1. Validate its shape by comparing discriminant and
            //    unit/tuple/struct arity. (OUTER)
            //    - Failure: add a "no_match_found" error and continue
            //      to next variant.
            // 2. Validate each parameter    ...continue... (INNER)
            for (variant_ident, comp_item) in self.subject.get_comparable_fields() {
                // FIXME: This only affects concrete types.. but
                //        `.compare(..)` should return a list of matches
                //        instead of just the first match it finds.
                //
                //        # Uni-matcher -> Multi-matcher
                //        Currently, we can end up returning a pattern that matches in shape, but not
                //        in structure, even though another pattern could satisfy our variant. In a case
                //        like the one below, we have a "catch all" variadic.
                //
                //        e.g. (i32, ..) | (..) => V1(String, i32), V2(String, String)
                //                                    ^^^^^^           ^^^^^^
                //                                    |                |
                //                                    `Found 'String' but expected 'i32'`
                //
                //        Because the first pattern fragment contains a concrete type, it should be possible
                //        mark the error as temporary and then check for other pattern matches. Note, the first
                //        error should always be the default one.
                //
                //        Given our pattern above, `(..)` should be a fallback pattern.
                //
                //        Should we allow concrete types with trait bound at argument position?
                //        e.g.
                //          (i32: Trait,  ..) | (..)
                //          (i32: ^Trait, ..) | (..)
                //
                //        For future reference! This should help with dispach inference.
                //
                //        # "catch-all" syntax
                //        Given the example above, if we were to play with it a little, we could end up with
                //        something like this:
                //        `(i32, ..) | _` that translate to `(i32, ..) | (..) | {..}`
                //
                //        Maybe it's something that would be worth having considering something like this:
                //        `_ where String: ^AsRef<str>`

                // 1. Check if we match in `shape`
                let Some(matched_pair) = comparable_pats.compare(&comp_item) else {
                    self.error.extend(comp_item.value.span(), no_match_found(comp_item.value, &pattern_fmt));
                    continue
                };

                // No support for empty unit iter, yet... NOTE: Make
                // sure to handle composite::unit iterator before
                // removing this
                if matched_pair.as_composite().is_unit() {
                    continue;
                }

                let max_fields_len = comp_item.value.len();

                // 2. Check if we match in `structure`. (We are naively
                // always expecting to never have infixed variadics)
                for (_index_param, (pat_parameter, item_field)) in matched_pair.zip().enumerate() {
                    // If we cannot desctructure a pattern field, then
                    // it must be variadic. This might change later
                    let Some(pat_field) = pat_parameter.get_field() else {
                        break;
                    };

                    let item_ty_string = item_field.ty.get_string();

                    if let Type::ImplTrait(ref ty_impl_trait) = pat_field.ty {
                        let bounds = &ty_impl_trait.bounds;

                        let mut impl_string = String::new();
                        for bound in bounds.iter() {
                            match bound {
                                syn::TypeParamBound::Trait(trait_bound) => {
                                    if let syn::TraitBoundModifier::None = trait_bound.modifier {
                                        impl_string
                                            .push_str(&trait_bound.get_unique_trait_bound_id())
                                    } else {
                                        self.error.extend(
                                            bound.span(),
                                            maybe_bounds_not_permitted(trait_bound),
                                        );
                                    }
                                }
                                syn::TypeParamBound::Lifetime(_) => {
                                    self.error.extend(bound.span(), lifetime_not_permitted());
                                }
                            }
                        }

                        // No point in continuing if we have errors or
                        // unique_impl_id is empty
                        if self.error.has_error() || impl_string.is_empty() {
                            // Add debug logs
                            continue;
                        }

                        // TODO: Add support for impl dispatch
                        let unique_impl_id =
                            into_unique_ident(&impl_string, variant_ident, ty_impl_trait.span());

                        predicates.push(parse_quote!(#unique_impl_id: #bounds));

                        // First we check if pty (T) exists in
                        // polymorphicmap. If it exists, insert new
                        // concrete type.
                        self.types
                            .polymap_insert(unique_impl_id.borrow(), TypeId::from(&item_field.ty));

                        continue;
                    }

                    let pat_ty = pat_field.ty.get_string();
                    // Check if it's a generic or concrete type
                    // - We only accept `_|[A-Z][A-Z0-9]*` as generics.
                    let is_generic =
                        pat_field.ty.is_placeholder() || pat_ty.to_uppercase().eq(&pat_ty);

                    if is_generic {
                        // First we check if pty (T) exists in
                        // polymorphicmap. If it exists, insert new
                        // concrete type.

                        self.types.polymap_insert(
                            &pat_field.ty.get_ident(),
                            TypeId::from(&item_field.ty),
                        );

                        // 3. Dispachable list
                        let Some(blueprints) = maybe_blueprints.as_mut().and_then(|bp| bp.get_mut(&pat_ty)) else {
                            continue
                        };

                        // FIXME: We are only expecting one dispatch per
                        //        generic now, so CHANGE THIS WHEN
                        //        POSSIBLE: where T: ^Trait, T: ^Mate ->
                        //        only ^Trait will be found. :( Fixed?
                        //        where T: ^Trait + ^Mate   -> should be
                        //        just turn this into a poly map
                        //        instead?
                        let variant_sig = VariantSignature::new(
                            enum_ident,
                            variant_ident,
                            item_field,
                            max_fields_len,
                        );

                        for blueprint in blueprints.iter_mut() {
                            blueprint.attach(&variant_sig)
                        }
                    } else if item_ty_string.ne(&pat_ty) {
                        self.error.extend(
                            item_field.ty.span(),
                            format!("Found {item_ty_string} but expected {pat_ty}."),
                        );
                    }
                }
            }

            // Assemble all our impl statements
            if let Some(bp_map) = maybe_blueprints {
                bp_map.for_each_blueprint(|bp| {
                    let path = bp.get_sanatized_impl_path();
                    if let Some(assocs) = bp.get_mapped_bindings() {
                        let methods = bp.get_associated_methods();
                        let implementation: ItemImpl = parse_quote!(
                            impl #path for #enum_ident {
                                #(#assocs)*

                                #(#methods)*
                            }
                        );
                        self.impls.push(implementation);
                    }
                });
            }

            let penum_expr_clause = self.expr.clause.get_or_insert_with(|| parse_quote!(where));

            // Might be a little unnecessary to loop through our
            // predicates again.. But we can refactor later.
            predicates
                .iter()
                .for_each(|pred| penum_expr_clause.predicates.push(parse_quote!(#pred)));
        } else {
            self.error
                .extend(variants.span(), "Expected to find at least one variant.");
        }

        // SAFETY: We are transmuting self into self with a different
        //         ZST marker that is just there to let us decide what
        //         methods should be available during different stages.
        //         So it's safe for us to transmute.
        unsafe { std::mem::transmute(self) }
    }
}

impl Penum<Assembled> {
    pub fn unwrap_or_error(mut self) -> TokenStream {
        let (struct_assertions, assertion_doc) = self.build_assertions();

        self.error
            .map(Error::to_compile_error)
            .unwrap_or_else(|| {
                let enum_item = self.subject;
                let impl_items = self.impls;
                let output = quote::quote!(
                    #(#struct_assertions)*

                    #assertion_doc
                    #enum_item

                    #(#impl_items)*
                );
                output
            })
            .into()
    }

    fn build_assertions(&mut self) -> (Vec<ItemStruct>, Option<Attribute>) {
        let mut assertions: Vec<ItemStruct> = Vec::new();

        if let Some(where_cl) = self.expr.clause.as_ref() {
            for (pred_index, predicate) in where_cl.predicates.iter().enumerate() {
                match predicate {
                    WherePredicate::Type(pred) => {
                        if let Some(pty_set) = self.types.get(&pred.bounded_ty.get_ident()) {
                            for (index, ty_id) in pty_set.iter().enumerate() {
                                let bounded_ty = ty_id.get_type();
                                if let Some(ty) = bounded_ty {
                                    let assert_ident = get_unique_assertion_statement(
                                        &self.subject.ident,
                                        ty,
                                        pred_index,
                                        index,
                                    );
                                    let spanned_bounds = pred
                                        .bounds
                                        .to_token_stream()
                                        .into_iter()
                                        .map(|mut token| {
                                            // This is the only way we
                                            // can change the span of a
                                            // `bound`..
                                            token.set_span(ty.span());
                                            token
                                        })
                                        .collect::<TokenStream2>();

                                    // #[feature(trivial_bounds)]
                                    // #[rustfmt::skip]
                                    assertions.push(parse_quote_spanned!(ty.span()=>
                                        #[allow(non_camel_case_types)]
                                        struct #assert_ident where #bounded_ty: #spanned_bounds;
                                    ))
                                }
                            }
                        }
                    }
                    WherePredicate::Lifetime(pred) => self
                        .error
                        .extend(pred.span(), "lifetime predicates are unsupported"),
                }
            }
        }

        let doc_attribute = if assertions.is_empty() {
            None
        } else {
            let doc_str = format!(
                include_str!("template/struct_assertion.md"),
                assertions
                    .iter()
                    .map(|x| {
                        let st = x.get_string();
                        st.replace("# [allow (non_camel_case_types)]", "\n")
                    })
                    .collect::<String>()
            );

            Some(parse_quote!(#[doc = #doc_str]))
        };

        (assertions, doc_attribute)
    }
}

pub trait Stringify: ToTokens {
    fn get_string(&self) -> String {
        self.to_token_stream().to_string()
    }
}

pub trait TraitBoundUtils {
    fn get_unique_trait_bound_id(&self) -> String;
}

pub trait TypeUtils {
    fn is_placeholder(&self) -> bool;
    fn some_generic(&self) -> Option<String>;
    fn get_ident(&self) -> Ident;
}

impl<T> Stringify for T where T: ToTokens {}

impl TypeUtils for Type {
    fn is_placeholder(&self) -> bool {
        matches!(self, Type::Infer(_))
    }

    fn some_generic(&self) -> Option<String> {
        self.is_placeholder()
            .then(|| {
                let pat_ty = self.get_string();
                pat_ty.to_uppercase().eq(&pat_ty).then_some(pat_ty)
            })
            .flatten()
    }

    /// Only use this when you are sure it's a generic type.
    fn get_ident(&self) -> Ident {
        format_ident!("{}", self.get_string(), span = self.span())
    }
}

impl TraitBoundUtils for TraitBound {
    fn get_unique_trait_bound_id(&self) -> String {
        let mut unique = UniqueIdentifier(vec![]);
        unique.visit_trait_bound(self);
        unique.0.join("_")
    }
}

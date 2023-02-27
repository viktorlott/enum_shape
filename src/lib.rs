use std::{
    borrow::Borrow,
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
};

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote, ToTokens};
use syn::{
    parse::{self, Parse},
    parse_macro_input, parse_quote,
    punctuated::Punctuated,
    spanned::Spanned,
    Data, DeriveInput, Error, Fields, PredicateType, Token, Variant, WhereClause, WherePredicate,
};

/// name(T) where T : Hello
struct VariantPattern {
    variant: Variant,
    where_clause: Option<WhereClause>,
}

impl Parse for VariantPattern {
    fn parse(input: parse::ParseStream) -> syn::Result<Self> {
        Ok(Self {
            variant: input.parse()?,
            where_clause: input.parse()?,
        })
    }
}

struct ErrorStash(Option<Error>);

impl ErrorStash {
    fn extend(&mut self, span: Span, error: impl Display) {
        if let Some(err) = self.0.as_mut() {
            err.combine(Error::new(span, error));
        } else {
            self.0 = Some(Error::new(span, error));
        }
    }

    fn into_or(self, data: impl FnOnce() -> DeriveInput) -> TokenStream {
        if let Some(error) = self.0 {
            error.to_compile_error().into()
        } else {
            data().to_token_stream().into()
        }
    }
}

fn string<T: ToTokens>(x: &T) -> String {
    x.to_token_stream().to_string()
}

fn validate_and_collect(
    pat_fields: &Fields,
    item_fields: &Fields,
    ptype_pairs: &mut PatternTypePairs,
    errors: &mut ErrorStash,
) {
    pat_fields
        .into_iter()
        .zip(item_fields.into_iter())
        .for_each(|(pat, item)| {
            let (pty, ity) = (string(&pat.ty), string(&item.ty));

            let is_generic = pty.eq("_") || pty.to_uppercase().eq(&pty);

            if !is_generic && pty != ity {
                return errors.extend(item.ty.span(), format!("Found {ity} but expected {pty}."));
            }

            if let Some(set) = ptype_pairs.get_mut(&pty) {
                set.insert(ity);
            } else {
                let mut bset = BTreeSet::new();
                bset.insert(ity);
                ptype_pairs.insert(pty, bset);
            }
        });
}

fn construct_bounds_tokens(
    pw_clause: Option<&WhereClause>,
    ppairs: &PatternTypePairs,
    errors: &mut ErrorStash,
) -> TokenStream2 {
    let mut bound_tokens = TokenStream2::new();
    if let Some(where_cl) = pw_clause {
        where_cl
            .predicates
            .iter()
            .for_each(|predicate| match predicate {
                syn::WherePredicate::Type(PredicateType {
                    bounded_ty, bounds, ..
                }) => {
                    if let Some(pty_set) = ppairs.get(&bounded_ty.to_token_stream().to_string()) {
                        pty_set.iter().for_each(|ty| {
                            let ty = format_ident!("{}", ty);
                            let ty_predicate = quote!(#ty: #bounds);
                            bound_tokens = quote!(#bound_tokens #ty_predicate,)
                        });
                    }
                }
                _ => errors.extend(Span::call_site(), "Unsupported `where clause`"),
            });
    }
    bound_tokens
}
// e.g. `T -> [i32, f32]`, `U -> [String, usize, CustomStruct]
type PatternTypePairs = BTreeMap<String, BTreeSet<String>>;
fn matcher(
    variant_pattern: &Variant,
    variant_item: &Variant,
    ptype_pairs: &mut PatternTypePairs,
    errors: &mut ErrorStash,
) {
    let Some((pfields, ifields)) = (match (&variant_pattern.fields, &variant_item.fields) {
        value @ ((Fields::Named(_), Fields::Named(_)) | (Fields::Unnamed(_), Fields::Unnamed(_))) => Some(value),
        _ => None,
    }) else {
        return errors.extend(
            variant_item.fields.span(),
            format!(
                "`{}` doesn't match pattern `{}`",
                variant_item.to_token_stream(),
                variant_pattern.to_token_stream()
            ),
        )
    };

    validate_and_collect(pfields.borrow(), ifields.borrow(), ptype_pairs, errors)
}

#[proc_macro_attribute]
pub fn shape(attr: TokenStream, input: TokenStream) -> TokenStream {
    let derived_input = parse_macro_input!(input as DeriveInput);

    let Data::Enum(enum_definition) = &derived_input.data else {
        return Error::new(derived_input.ident.span(), "Expected an enum.").to_compile_error().into();
    };

    if enum_definition.variants.is_empty() {
        return Error::new(
            enum_definition.variants.span(),
            "Expected to find at least one variant.",
        )
        .to_compile_error()
        .into();
    }

    let mut source = derived_input.clone();

    let variant_pattern = parse_macro_input!(attr as VariantPattern);
    let mut ptype_pairs: PatternTypePairs = PatternTypePairs::new();
    let mut errors: ErrorStash = ErrorStash(None);

    enum_definition.variants.iter().for_each(|variant| {
        matcher(
            &variant_pattern.variant,
            variant,
            &mut ptype_pairs,
            &mut errors,
        )
    });

    let ty_predicate = construct_bounds_tokens(
        variant_pattern.where_clause.as_ref(),
        &ptype_pairs,
        &mut errors,
    );

    // TODO: Fix this shit
    errors.into_or(|| {
        if let Some(ref mut swc) = source.generics.where_clause {
            // TODO: Change this to optional later
            let where_clause: Punctuated<WherePredicate, Token![,]> = parse_quote!(#ty_predicate);
            where_clause
                .iter()
                .for_each(|nwc| swc.predicates.push(nwc.clone()))
        } else {
            source.generics.where_clause = Some(parse_quote!(where #ty_predicate))
        }
        source
    })
}

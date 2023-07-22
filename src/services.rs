use proc_macro::TokenStream;
use quote::ToTokens;
use syn::parse_macro_input;
use syn::ItemTrait;
use syn::Type;

use crate::dispatch::T_SHM;
use crate::factory::PenumExpr;
use crate::factory::Subject;
use crate::penum::Penum;
use crate::penum::Stringify;
use crate::utils::censor_discriminants_get_default;
use crate::utils::variants_to_arms;

pub fn penum_expand(attr: TokenStream, input: TokenStream) -> TokenStream {
    // TODO: Make it bi-directional, meaning it's also possible to register enums and then do
    // the implementations when we tag a trait. (That is actually better).
    if attr.is_empty() {
        let output = input.clone();
        let item_trait = parse_macro_input!(input as ItemTrait);

        // If we cannot find the trait the user wants to dispatch, we need to store it.
        T_SHM.insert(item_trait.ident.get_string(), item_trait.get_string());

        output
    } else {
        let pattern = parse_macro_input!(attr as PenumExpr);
        let input = parse_macro_input!(input as Subject);

        let penum = Penum::from(pattern, input).assemble();

        // Loop through enum definition and match each variant with each
        // shape pattern. for each variant => pattern.find(variant)
        penum.unwrap_or_error()
    }
}

pub fn to_string_expand(input: TokenStream) -> TokenStream {
    let subject = parse_macro_input!(input as Subject);

    let matching_arms: proc_macro2::TokenStream =
        variants_to_arms(subject.get_variants().iter(), |expr| {
            quote::quote!(format!(#expr))
        });

    let (subject, has_default) = censor_discriminants_get_default(subject, None);

    let enum_name = &subject.ident;
    quote::quote!(
        #subject

        impl std::string::ToString for #enum_name {
            fn to_string(&self) -> String {
                match self {
                    #matching_arms
                    _ => #has_default
                }
            }
        }
    )
    .to_token_stream()
    .into()
}

pub fn fmt_expand(input: TokenStream) -> TokenStream {
    let subject = parse_macro_input!(input as Subject);

    let matching_arms: proc_macro2::TokenStream =
        variants_to_arms(subject.get_variants().iter(), |expr| {
            quote::quote!(write!(f, #expr))
        });

    let (subject, has_default) = censor_discriminants_get_default(
        subject,
        Some(|dft| {
            // FIXME: I'm too lazy to make this not look like shit.
            dft.or(Some(quote::quote!(write!(f, "{}", "".to_string()))))
                .unwrap()
        }),
    );

    let enum_name = &subject.ident;
    quote::quote!(
        #subject

        impl std::fmt::Display for #enum_name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    #matching_arms
                    _ => #has_default
                }
            }
        }
    )
    .to_token_stream()
    .into()
}

pub fn into_expand(attr: TokenStream, input: TokenStream) -> TokenStream {
    let ty = parse_macro_input!(attr as Type);
    let mut subject = parse_macro_input!(input as Subject);

    let matching_arms: proc_macro2::TokenStream =
        variants_to_arms(subject.get_variants().iter(), |expr| quote::quote!(#expr));

    let mut has_default = quote::quote!(Default::default()).to_token_stream();

    subject.data.variants = subject
        .data
        .variants
        .into_iter()
        .filter_map(|mut variant| {
            if variant.discriminant.is_some() && variant.ident == "__Default__" {
                let (_, expr) = variant.discriminant.as_ref().unwrap();
                has_default = quote::quote!(#expr);
                return None;
            }

            variant.discriminant = None;
            Some(variant)
        })
        .collect();

    let enum_name = &subject.ident;

    quote::quote!(
        #subject

        impl Into<#ty> for #enum_name {
            fn into(self) -> #ty {
                match self {
                    #matching_arms
                    _ => #has_default
                }
            }
        }
    )
    .to_token_stream()
    .into()
}

pub fn deref_expand(
    attr: TokenStream,
    input: TokenStream,
    extend: Option<fn(&Subject) -> proc_macro2::TokenStream>,
) -> TokenStream {
    let ty = parse_macro_input!(attr as Type);
    let mut subject = parse_macro_input!(input as Subject);

    let matching_arms: proc_macro2::TokenStream =
        variants_to_arms(subject.get_variants().iter(), |expr| quote::quote!(#expr));

    let mut has_default = quote::quote!(Default::default()).to_token_stream();
    subject.data.variants = subject
        .data
        .variants
        .into_iter()
        .filter_map(|mut variant| {
            if variant.discriminant.is_some() && variant.ident == "__Default__" {
                let (_, expr) = variant.discriminant.as_ref().unwrap();
                has_default = quote::quote!(#expr);
                return None;
            }

            variant.discriminant = None;
            Some(variant)
        })
        .collect();

    let enum_name = &subject.ident;

    let extensions = extend.map(|extend| extend(&subject));

    quote::quote!(
        #subject

        impl std::ops::Deref for #enum_name {
            type Target = #ty;
            fn deref(&self) -> &Self::Target {
                match self {
                    #matching_arms
                    _ => #has_default
                }
            }
        }

        #extensions
    )
    .to_token_stream()
    .into()
}

pub fn static_str(input: TokenStream) -> TokenStream {
    deref_expand(
        quote::quote!(str).into(),
        input,
        Some(|subject| {
            let enum_name = &subject.ident;

            quote::quote!(
                impl AsRef<str> for #enum_name {
                    fn as_ref(&self) -> &str { &**self }
                }

                impl #enum_name {
                    fn as_str(&self) -> &str  { &**self }
                    fn static_str(&self) -> &str { &**self }
                }
            )
        }),
    )
}

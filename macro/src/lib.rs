use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;
use syn::spanned::Spanned;
use syn::{Error, Item};

#[proc_macro_attribute]
pub fn state(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as Item);
    let (ident, attrs) = match &input {
        Item::Struct(item) => (&item.ident, &item.attrs),
        Item::Enum(item) => (&item.ident, &item.attrs),
        other => {
            return Error::new(other.span(), "only `struct` and `enum` are allowed!")
                .to_compile_error()
                .into();
        }
    };

    let mut derive_serialize = quote! {
        #[cfg_attr(target_arch = "wasm32", derive(thawing::serde::Serialize))]
        #[cfg_attr(not(target_arch = "wasm32"), derive(::thawing::serde::Serialize))]
    };

    let mut derive_deserialize = quote! {
        #[cfg_attr(target_arch = "wasm32", derive(thawing::serde::Deserialize))]
        #[cfg_attr(not(target_arch = "wasm32"), derive(::thawing::serde::Deserialize))]
    };

    let mut serde_crate = quote! {
        #[cfg_attr(target_arch = "wasm32", serde(crate = "thawing::serde"))]
        #[cfg_attr(not(target_arch = "wasm32"), serde(crate = "::thawing::serde"))]
    };

    let impl_marker = quote! {
        #[cfg(target_arch = "wasm32")]
        impl thawing::Message for #ident {}
        #[cfg(not(target_arch = "wasm32"))]
        impl ::thawing::Message for #ident {}
    };

    for attr in attrs {
        if attr.path().is_ident("derive") {
            let _ = attr.parse_nested_meta(|meta| {
                let segments = meta
                    .path
                    .segments
                    .iter()
                    .map(|p| p.ident.to_string())
                    .collect::<Vec<_>>();
                if meta.path.is_ident("Serialize")
                    || segments
                        .windows(2)
                        .find(|segments| *segments == ["serde", "Serialize"])
                        .is_some()
                {
                    derive_serialize = proc_macro2::TokenStream::new();
                }

                if meta.path.is_ident("Deserialize")
                    || segments
                        .windows(2)
                        .find(|segments| *segments == ["serde", "Deserialize"])
                        .is_some()
                {
                    derive_deserialize = proc_macro2::TokenStream::new();
                }

                Ok(())
            });
        }
    }

    if derive_serialize.is_empty() && derive_deserialize.is_empty() {
        serde_crate = proc_macro2::TokenStream::new();
    }

    let expanded = quote! {
        #derive_serialize
        #derive_deserialize
        #serde_crate
        #input

        #impl_marker
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn message(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as Item);
    let (ident, attrs) = match &input {
        Item::Struct(item) => (&item.ident, &item.attrs),
        Item::Enum(item) => (&item.ident, &item.attrs),
        other => {
            return Error::new(other.span(), "only `struct` and `enum` are allowed!")
                .to_compile_error()
                .into();
        }
    };

    let mut derive_serialize = quote! {
        #[cfg_attr(target_arch = "wasm32", derive(thawing::serde::Serialize))]
        #[cfg_attr(not(target_arch = "wasm32"), derive(::thawing::serde::Serialize))]
    };

    let mut derive_deserialize = quote! {
        #[cfg_attr(target_arch = "wasm32", derive(thawing::serde::Deserialize))]
        #[cfg_attr(not(target_arch = "wasm32"), derive(::thawing::serde::Deserialize))]
    };

    let mut serde_crate = quote! {
        #[cfg_attr(target_arch = "wasm32", serde(crate = "thawing::serde"))]
        #[cfg_attr(not(target_arch = "wasm32"), serde(crate = "::thawing::serde"))]
    };

    let impl_marker = quote! {
        #[cfg(target_arch = "wasm32")]
        impl thawing::Message for #ident {}
        #[cfg(not(target_arch = "wasm32"))]
        impl ::thawing::Message for #ident {}
    };

    for attr in attrs {
        if attr.path().is_ident("derive") {
            let _ = attr.parse_nested_meta(|meta| {
                let segments = meta
                    .path
                    .segments
                    .iter()
                    .map(|p| p.ident.to_string())
                    .collect::<Vec<_>>();
                if meta.path.is_ident("Serialize")
                    || segments
                        .windows(2)
                        .find(|segments| *segments == ["serde", "Serialize"])
                        .is_some()
                {
                    derive_serialize = proc_macro2::TokenStream::new();
                }

                if meta.path.is_ident("Deserialize")
                    || segments
                        .windows(2)
                        .find(|segments| *segments == ["serde", "Deserialize"])
                        .is_some()
                {
                    derive_deserialize = proc_macro2::TokenStream::new();
                }

                Ok(())
            });
        }
    }

    if derive_serialize.is_empty() && derive_deserialize.is_empty() {
        serde_crate = proc_macro2::TokenStream::new();
    }

    let expanded = quote! {
        #derive_serialize
        #derive_deserialize
        #serde_crate
        #input

        #impl_marker
    };

    TokenStream::from(expanded)
}

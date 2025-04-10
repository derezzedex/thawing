use proc_macro::TokenStream;
use quote::quote;
use syn::DeriveInput;
use syn::parse_macro_input;

fn parse_attributes(attrs: &Vec<syn::Attribute>) -> proc_macro2::TokenStream {
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

    quote! {
        #derive_serialize
        #derive_deserialize
        #serde_crate
    }
}

#[proc_macro_attribute]
pub fn data(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let attributes = parse_attributes(&input.attrs);

    let expanded = quote! {
        #attributes
        #input
    };

    TokenStream::from(expanded)
}

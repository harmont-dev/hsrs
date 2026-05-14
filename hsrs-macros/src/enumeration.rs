use proc_macro2::TokenStream;
use quote::quote;
use syn::{Fields, ItemEnum};

pub(crate) fn expand(item: TokenStream) -> syn::Result<TokenStream> {
    let input: ItemEnum = syn::parse2(item)?;

    for variant in &input.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(syn::Error::new_spanned(
                variant,
                "hsrs::enumeration only supports unit variants",
            ));
        }
    }

    let attrs = &input.attrs;
    let vis = &input.vis;
    let ident = &input.ident;
    let variants = &input.variants;

    Ok(quote! {
        #(#attrs)*
        #[::hsrs::safer_ffi::derive_ReprC]
        #[repr(u8)]
        #[derive(Clone, Copy, ::hsrs::borsh::BorshSerialize, ::hsrs::borsh::BorshDeserialize)]
        #[borsh(crate = "::hsrs::borsh")]
        #vis enum #ident {
            #variants
        }
    })
}

use proc_macro2::TokenStream;
use quote::quote;
use syn::ItemStruct;

pub(crate) fn expand(item: TokenStream) -> syn::Result<TokenStream> {
    let input: ItemStruct = syn::parse2(item)?;

    let attrs = &input.attrs;
    let vis = &input.vis;
    let ident = &input.ident;
    let generics = &input.generics;
    let fields = &input.fields;
    let semi = input.semi_token;

    Ok(quote! {
        #(#attrs)*
        #[derive(Copy, Clone, ::hsrs::borsh::BorshSerialize, ::hsrs::borsh::BorshDeserialize)]
        #vis struct #ident #generics #fields #semi
    })
}

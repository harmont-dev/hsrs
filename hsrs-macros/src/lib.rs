#![allow(
    clippy::needless_pass_by_value,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::missing_docs_in_private_items,
    clippy::exhaustive_enums,
    clippy::exhaustive_structs,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::shadow_reuse,
    clippy::shadow_same,
    clippy::shadow_unrelated,
    clippy::wildcard_imports,
    clippy::cargo,
    clippy::redundant_pub_crate,
    clippy::unnecessary_wraps,
    clippy::expect_used,
    clippy::str_to_string,
    clippy::implicit_clone,
)]

mod enumeration;
mod module;

#[proc_macro_attribute]
pub fn enumeration(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    enumeration::expand(item.into())
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

#[proc_macro_attribute]
pub fn module(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    module::expand(item.into())
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

#[proc_macro_attribute]
pub fn data_type(
    _attr: proc_macro::TokenStream,
    _item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    syn::Error::new(
        proc_macro2::Span::call_site(),
        "`#[hsrs::data_type]` must be used inside a `#[hsrs::module]` block",
    )
    .to_compile_error()
    .into()
}

#[proc_macro_attribute]
pub fn function(
    _attr: proc_macro::TokenStream,
    _item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    syn::Error::new(
        proc_macro2::Span::call_site(),
        "`#[hsrs::function]` must be used inside a `#[hsrs::module]` block",
    )
    .to_compile_error()
    .into()
}

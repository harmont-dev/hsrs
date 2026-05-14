use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{FnArg, ImplItem, ImplItemFn, Item, ItemMod, Pat, ReturnType, Type};

pub(crate) fn expand(item: TokenStream) -> syn::Result<TokenStream> {
    let mut input: ItemMod = syn::parse2(item)?;
    let mod_name = input.ident.clone();

    if input.content.is_none() {
        return Err(syn::Error::new_spanned(
            &input,
            "hsrs::module requires an inline module",
        ));
    }
    let content = input.content.as_mut().expect("checked above");
    let items = &mut content.1;

    let struct_ident = find_data_type_struct(items)?;
    process_struct(items, &struct_ident)?;
    let ffi_wrappers = generate_ffi_from_impl(items, &mod_name, &struct_ident)?;

    let free_name = format_ident!("{mod_name}_free");
    let destructor: Item = syn::parse_quote! {
        #[allow(
            clippy::needless_pass_by_value,
            clippy::missing_docs_in_private_items,
        )]
        #[::safer_ffi::ffi_export]
        fn #free_name(it: repr_c::Box<#struct_ident>) {
            drop(it);
        }
    };

    let use_super: Item = syn::parse_quote! {
        #[allow(clippy::wildcard_imports)]
        use super::*;
    };
    let use_ffi: Item = syn::parse_quote! {
        #[allow(clippy::wildcard_imports)]
        use ::safer_ffi::prelude::*;
    };
    items.insert(0, use_super);
    items.insert(1, use_ffi);

    for wrapper in ffi_wrappers {
        items.push(wrapper);
    }
    items.push(destructor);

    let attrs = &input.attrs;
    let vis = &input.vis;
    let mod_items = &content.1;

    Ok(quote! {
        #(#attrs)*
        #vis mod #mod_name {
            #(#mod_items)*
        }
    })
}

fn find_data_type_struct(items: &[Item]) -> syn::Result<Ident> {
    for item in items {
        if let Item::Struct(s) = item {
            if has_hsrs_attr(&s.attrs, "data_type") {
                return Ok(s.ident.clone());
            }
        }
    }
    Err(syn::Error::new(
        proc_macro2::Span::call_site(),
        "hsrs::module requires a #[hsrs::data_type] struct",
    ))
}

fn process_struct(items: &mut [Item], struct_ident: &Ident) -> syn::Result<()> {
    for item in items.iter_mut() {
        if let Item::Struct(s) = item {
            if s.ident == *struct_ident {
                s.attrs.retain(|a| !is_hsrs_path(a, "data_type"));
                let derive: syn::Attribute = syn::parse_quote!(#[::safer_ffi::derive_ReprC]);
                let repr: syn::Attribute = syn::parse_quote!(#[repr(opaque)]);
                s.attrs.insert(0, repr);
                s.attrs.insert(0, derive);
                return Ok(());
            }
        }
    }
    Err(syn::Error::new(
        proc_macro2::Span::call_site(),
        "data_type struct not found",
    ))
}

fn generate_ffi_from_impl(
    items: &mut [Item],
    mod_name: &Ident,
    struct_ident: &Ident,
) -> syn::Result<Vec<Item>> {
    let mut wrappers = Vec::new();

    for item in items.iter_mut() {
        if let Item::Impl(imp) = item {
            if imp.trait_.is_none() {
                if let Type::Path(tp) = &*imp.self_ty {
                    if tp.path.is_ident(struct_ident) {
                        let mut methods_info: Vec<ImplItemFn> = Vec::new();
                        for impl_item in &mut imp.items {
                            if let ImplItem::Fn(method) = impl_item {
                                if has_hsrs_attr(&method.attrs, "function") {
                                    methods_info.push(method.clone());
                                    method.attrs.retain(|a| !is_hsrs_path(a, "function"));
                                }
                            }
                        }
                        for method in &methods_info {
                            wrappers.push(generate_wrapper(method, mod_name, struct_ident)?);
                        }
                        return Ok(wrappers);
                    }
                }
            }
        }
    }

    Ok(wrappers)
}

fn generate_wrapper(
    method: &ImplItemFn,
    mod_name: &Ident,
    struct_ident: &Ident,
) -> syn::Result<Item> {
    let method_name = &method.sig.ident;
    let ffi_name = format_ident!("{mod_name}_{method_name}");
    let params: Vec<_> = non_self_params(&method.sig.inputs);
    let param_names: Vec<_> = param_idents(&method.sig.inputs);

    let first_arg = method.sig.inputs.first();
    let is_self = matches!(first_arg, Some(FnArg::Receiver(_)));
    let is_mut = matches!(first_arg, Some(FnArg::Receiver(r)) if r.mutability.is_some());

    if !is_self {
        Ok(syn::parse_quote! {
            #[allow(clippy::missing_docs_in_private_items)]
            #[::safer_ffi::ffi_export]
            fn #ffi_name(#(#params),*) -> repr_c::Box<#struct_ident> {
                Box::new(#struct_ident::#method_name(#(#param_names),*)).into()
            }
        })
    } else if is_mut {
        match &method.sig.output {
            ReturnType::Default => Ok(syn::parse_quote! {
                #[allow(clippy::missing_docs_in_private_items)]
                #[::safer_ffi::ffi_export]
                fn #ffi_name(this: &mut #struct_ident, #(#params),*) {
                    this.#method_name(#(#param_names),*);
                }
            }),
            ReturnType::Type(_, ret_ty) => Ok(syn::parse_quote! {
                #[allow(clippy::missing_docs_in_private_items)]
                #[::safer_ffi::ffi_export]
                fn #ffi_name(this: &mut #struct_ident, #(#params),*) -> #ret_ty {
                    this.#method_name(#(#param_names),*)
                }
            }),
        }
    } else {
        match &method.sig.output {
            ReturnType::Default => Ok(syn::parse_quote! {
                #[allow(clippy::missing_docs_in_private_items)]
                #[::safer_ffi::ffi_export]
                fn #ffi_name(this: &#struct_ident, #(#params),*) {
                    this.#method_name(#(#param_names),*);
                }
            }),
            ReturnType::Type(_, ret_ty) => Ok(syn::parse_quote! {
                #[allow(clippy::missing_docs_in_private_items)]
                #[::safer_ffi::ffi_export]
                fn #ffi_name(this: &#struct_ident, #(#params),*) -> #ret_ty {
                    this.#method_name(#(#param_names),*)
                }
            }),
        }
    }
}

fn has_hsrs_attr(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().any(|a| is_hsrs_path(a, name))
}

fn is_hsrs_path(attr: &syn::Attribute, name: &str) -> bool {
    let segs: Vec<_> = attr
        .path()
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect();
    segs == vec!["hsrs", name]
}

fn non_self_params(
    inputs: &syn::punctuated::Punctuated<FnArg, syn::Token![,]>,
) -> Vec<&FnArg> {
    inputs
        .iter()
        .filter(|a| !matches!(a, FnArg::Receiver(_)))
        .collect()
}

fn param_idents(
    inputs: &syn::punctuated::Punctuated<FnArg, syn::Token![,]>,
) -> Vec<&Ident> {
    inputs
        .iter()
        .filter_map(|arg| {
            if let FnArg::Typed(pt) = arg {
                if let Pat::Ident(pi) = &*pt.pat {
                    return Some(&pi.ident);
                }
            }
            None
        })
        .collect()
}

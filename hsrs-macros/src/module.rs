use std::collections::HashSet;

use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{
    FnArg,
    ImplItem,
    ImplItemFn,
    Item,
    ItemMod,
    Pat,
    ReturnType,
    Type,
    ext::IdentExt,
    parse::{Parse, ParseStream},
};

struct ModuleAttr {
    value_type_names: HashSet<String>,
}

impl Parse for ModuleAttr {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut value_type_names = HashSet::new();
        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            match ident.to_string().as_str() {
                "value_types" => {
                    let content;
                    syn::parenthesized!(content in input);
                    let names = content.parse_terminated(Ident::parse, syn::Token![,])?;
                    for name in names {
                        value_type_names.insert(name.to_string());
                    }
                },
                "safety" => {
                    let _: syn::Token![=] = input.parse()?;
                    let _safety_value: Ident = input.call(Ident::parse_any)?;
                    // Consumed and discarded — codegen reads this from source
                },
                other => {
                    return Err(syn::Error::new_spanned(
                        ident,
                        format!("unknown module attribute: `{other}`"),
                    ));
                },
            }
            if !input.is_empty() {
                let _: syn::Token![,] = input.parse()?;
            }
        }
        Ok(Self { value_type_names })
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn expand(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let module_attr: ModuleAttr = syn::parse2(attr)?;
    let mut value_type_names = module_attr.value_type_names;

    let mut input: ItemMod = syn::parse2(item)?;
    let mod_name = input.ident.clone();

    if input.content.is_none() {
        return Err(syn::Error::new_spanned(&input, "hsrs::module requires an inline module"));
    }
    let content = input.content.as_mut().expect("checked above");
    let items = &mut content.1;

    for it in items.iter() {
        if let Item::Struct(s) = it {
            if has_hsrs_attr(&s.attrs, "value_type") {
                value_type_names.insert(s.ident.to_string());
            }
        }
    }

    let struct_ident = find_data_type_struct(items)?;
    process_struct(items, &struct_ident)?;
    let ffi_wrappers = generate_ffi_from_impl(items, &mod_name, &struct_ident, &value_type_names)?;

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
    Err(syn::Error::new(proc_macro2::Span::call_site(), "data_type struct not found"))
}

fn generate_ffi_from_impl(
    items: &mut [Item],
    mod_name: &Ident,
    struct_ident: &Ident,
    value_type_names: &HashSet<String>,
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
                            wrappers.push(generate_wrapper(
                                method,
                                mod_name,
                                struct_ident,
                                value_type_names,
                            )?);
                        }
                        return Ok(wrappers);
                    }
                }
            }
        }
    }

    Ok(wrappers)
}

fn is_primitive_type(ty: &Type) -> bool {
    if let Type::Path(tp) = ty {
        if let Some(ident) = tp.path.get_ident() {
            let s = ident.to_string();
            return matches!(
                s.as_str(),
                "i8" | "i16"
                    | "i32"
                    | "i64"
                    | "u8"
                    | "u16"
                    | "u32"
                    | "u64"
                    | "bool"
                    | "usize"
                    | "isize"
            );
        }
    }
    if let Type::Tuple(tt) = ty {
        return tt.elems.is_empty();
    }
    false
}

fn is_self_type(ty: &Type) -> bool {
    if let Type::Path(tp) = ty { tp.path.is_ident("Self") } else { false }
}

fn needs_borsh(ty: &Type, value_type_names: &HashSet<String>) -> bool {
    if is_primitive_type(ty) || is_self_type(ty) {
        return false;
    }
    if let Type::Path(tp) = ty {
        if let Some(ident) = tp.path.get_ident() {
            let s = ident.to_string();
            return value_type_names.contains(&s) || s == "String";
        }
        if let Some(seg) = tp.path.segments.last() {
            let name = seg.ident.to_string();
            return matches!(name.as_str(), "Result" | "Option" | "Vec");
        }
    }
    false
}

fn return_needs_borsh(method: &ImplItemFn, value_type_names: &HashSet<String>) -> bool {
    match &method.sig.output {
        ReturnType::Type(_, ty) => needs_borsh(ty, value_type_names),
        ReturnType::Default => false,
    }
}

struct FfiParams {
    sig_tokens: Vec<TokenStream>,
    call_tokens: Vec<TokenStream>,
}

fn build_ffi_params(method: &ImplItemFn, value_type_names: &HashSet<String>) -> FfiParams {
    let mut sig_tokens = Vec::new();
    let mut call_tokens = Vec::new();

    for arg in &method.sig.inputs {
        if let FnArg::Typed(pt) = arg {
            if let Pat::Ident(pi) = &*pt.pat {
                let name = &pi.ident;
                if needs_borsh(&pt.ty, value_type_names) {
                    let ptr_name = format_ident!("{name}_ptr");
                    let len_name = format_ident!("{name}_len");
                    let ty = &pt.ty;
                    sig_tokens.push(quote! { #ptr_name: *const u8 });
                    sig_tokens.push(quote! { #len_name: u64 });
                    call_tokens.push(quote! {
                        {
                            #[allow(unsafe_code)]
                            let #name: #ty = unsafe { ::hsrs::borsh_deserialize(#ptr_name, #len_name) };
                            #name
                        }
                    });
                } else {
                    sig_tokens.push(quote! { #arg });
                    call_tokens.push(quote! { #name });
                }
            }
        }
    }

    FfiParams { sig_tokens, call_tokens }
}

#[allow(clippy::too_many_lines)]
fn generate_wrapper(
    method: &ImplItemFn,
    mod_name: &Ident,
    struct_ident: &Ident,
    value_type_names: &HashSet<String>,
) -> syn::Result<Item> {
    let method_name = &method.sig.ident;
    let ffi_name = format_ident!("{mod_name}_{method_name}");

    let first_arg = method.sig.inputs.first();
    let is_self = matches!(first_arg, Some(FnArg::Receiver(_)));
    let is_mut = matches!(first_arg, Some(FnArg::Receiver(r)) if r.mutability.is_some());
    let borsh_return = return_needs_borsh(method, value_type_names);

    let ffi = build_ffi_params(method, value_type_names);
    let ffi_params = &ffi.sig_tokens;
    let call_args = &ffi.call_tokens;

    if !is_self {
        if borsh_return {
            Ok(syn::parse_quote! {
                #[allow(clippy::missing_docs_in_private_items)]
                #[::safer_ffi::ffi_export]
                fn #ffi_name(#(#ffi_params),*) -> repr_c::Box<::hsrs::BorshBuffer> {
                    let result = #struct_ident::#method_name(#(#call_args),*);
                    Box::new(::hsrs::BorshBuffer::from_borsh(&result)).into()
                }
            })
        } else {
            let params: Vec<_> = non_self_params(&method.sig.inputs);
            let param_names: Vec<_> = param_idents(&method.sig.inputs);
            Ok(syn::parse_quote! {
                #[allow(clippy::missing_docs_in_private_items)]
                #[::safer_ffi::ffi_export]
                fn #ffi_name(#(#params),*) -> repr_c::Box<#struct_ident> {
                    Box::new(#struct_ident::#method_name(#(#param_names),*)).into()
                }
            })
        }
    } else if is_mut {
        if borsh_return {
            Ok(syn::parse_quote! {
                #[allow(clippy::missing_docs_in_private_items)]
                #[::safer_ffi::ffi_export]
                fn #ffi_name(this: &mut #struct_ident, #(#ffi_params),*) -> repr_c::Box<::hsrs::BorshBuffer> {
                    let result = this.#method_name(#(#call_args),*);
                    Box::new(::hsrs::BorshBuffer::from_borsh(&result)).into()
                }
            })
        } else {
            match &method.sig.output {
                ReturnType::Default => {
                    Ok(syn::parse_quote! {
                        #[allow(clippy::missing_docs_in_private_items)]
                        #[::safer_ffi::ffi_export]
                        fn #ffi_name(this: &mut #struct_ident, #(#ffi_params),*) {
                            this.#method_name(#(#call_args),*);
                        }
                    })
                },
                ReturnType::Type(_, ret_ty) => {
                    Ok(syn::parse_quote! {
                        #[allow(clippy::missing_docs_in_private_items)]
                        #[::safer_ffi::ffi_export]
                        fn #ffi_name(this: &mut #struct_ident, #(#ffi_params),*) -> #ret_ty {
                            this.#method_name(#(#call_args),*)
                        }
                    })
                },
            }
        }
    } else if borsh_return {
        Ok(syn::parse_quote! {
            #[allow(clippy::missing_docs_in_private_items)]
            #[::safer_ffi::ffi_export]
            fn #ffi_name(this: &#struct_ident, #(#ffi_params),*) -> repr_c::Box<::hsrs::BorshBuffer> {
                let result = this.#method_name(#(#call_args),*);
                Box::new(::hsrs::BorshBuffer::from_borsh(&result)).into()
            }
        })
    } else {
        match &method.sig.output {
            ReturnType::Default => {
                Ok(syn::parse_quote! {
                    #[allow(clippy::missing_docs_in_private_items)]
                    #[::safer_ffi::ffi_export]
                    fn #ffi_name(this: &#struct_ident, #(#ffi_params),*) {
                        this.#method_name(#(#call_args),*);
                    }
                })
            },
            ReturnType::Type(_, ret_ty) => {
                Ok(syn::parse_quote! {
                    #[allow(clippy::missing_docs_in_private_items)]
                    #[::safer_ffi::ffi_export]
                    fn #ffi_name(this: &#struct_ident, #(#ffi_params),*) -> #ret_ty {
                        this.#method_name(#(#call_args),*)
                    }
                })
            },
        }
    }
}

fn has_hsrs_attr(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().any(|a| is_hsrs_path(a, name))
}

fn is_hsrs_path(attr: &syn::Attribute, name: &str) -> bool {
    let segs: Vec<_> = attr.path().segments.iter().map(|s| s.ident.to_string()).collect();
    segs == vec!["hsrs", name]
}

fn non_self_params(inputs: &syn::punctuated::Punctuated<FnArg, syn::Token![,]>) -> Vec<&FnArg> {
    inputs.iter().filter(|a| !matches!(a, FnArg::Receiver(_))).collect()
}

fn param_idents(inputs: &syn::punctuated::Punctuated<FnArg, syn::Token![,]>) -> Vec<&Ident> {
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

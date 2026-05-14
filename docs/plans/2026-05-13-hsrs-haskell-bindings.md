# hsrs — Rust-to-Haskell FFI Bindings Generator

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a proc-macro + codegen system that generates type-safe Haskell FFI bindings from annotated Rust code, using safer_ffi for the C FFI layer.

**Architecture:** `hsrs` proc macros transform annotated Rust types into safer_ffi-compatible FFI exports (adding `#[derive_ReprC]`, `#[repr(opaque)]`, generating `#[ffi_export]` wrappers). `hsrs-codegen` parses the original Rust source to generate idiomatic Haskell bindings with newtypes, ForeignPtr management, and pattern synonyms. The proc macros handle all Rust-side FFI generation; codegen only emits Haskell.

**Tech Stack:** Rust proc macros (syn 2, quote, proc-macro2), safer-ffi 0.2.0-rc1 (derive_ReprC, ffi_export, repr_c::Box), Haskell FFI (ccall, ForeignPtr, Storable, PatternSynonyms)

---

## Context

The user wants to annotate Rust code with `#[hsrs::enumeration]`, `#[hsrs::module]`, `#[hsrs::data_type]`, and `#[hsrs::function]` attributes, then:
1. Compile the Rust crate as a C-callable library (via safer_ffi)
2. Run `hsrs-codegen` to generate Haskell FFI bindings that preserve Rust type safety

Key design insight: `#[hsrs::module]` wraps a `mod` block, giving the proc macro full context to see the struct, impl block, and all methods — solving the "proc macro on a method doesn't know Self" problem.

## Design Decisions

### Why proc macros + separate codegen?
- Proc macros can add `#[derive_ReprC]`, `#[repr(opaque)]` to type definitions (required for safer_ffi)
- `#[hsrs::module]` on a mod block has full context to generate `#[ffi_export]` wrapper functions
- `hsrs-codegen` reads the *original* source (pre-expansion) to extract rich type info for Haskell

### Type Safety Preservation
The C layer loses type information (Register → uint8_t). The Haskell codegen restores it:
- Rust `#[hsrs::enumeration]` → Haskell `newtype Register = Register Word8` + pattern synonyms
- Rust `#[hsrs::module]` → Haskell `newtype QuectoVm = QuectoVm (ForeignPtr QuectoVmRaw)`
- High-level Haskell API uses rich types; unwrapping happens inside function bodies

### Type Mapping

| Rust | C (safer_ffi) | Haskell FFI | Haskell API |
|------|---------------|-------------|-------------|
| i64 | int64_t | Int64 | Int64 |
| u8 | uint8_t | Word8 | Word8 |
| bool | bool | CBool | Bool |
| `#[enumeration]` enum | uint8_t | Word8 | Register (newtype) |
| `#[module]` struct | opaque ptr | Ptr Raw | Module (ForeignPtr) |
| () | void | IO () | IO () |

### FFI Function Naming
- `{mod_name}_{method_name}` (e.g., `quecto_vm_add`)
- Auto-generated destructor: `{mod_name}_free`

---

## Prerequisite

Fix `self.clock++` → `self.clock += 1` in hsrs-examples/src/lib.rs (6 occurrences). Rust has no `++` operator.

---

## Task 1: Configure Dependencies and Lint Overrides

**Files:**
- Modify: `/Users/marko/Desktop/hsrs/hsrs/Cargo.toml`
- Modify: `/Users/marko/Desktop/hsrs/hsrs-codegen/Cargo.toml`
- Modify: `/Users/marko/Desktop/hsrs/hsrs-examples/Cargo.toml`

**Step 1: Update hsrs/Cargo.toml**

```toml
[package]
name = "hsrs"
version = "0.1.0"
edition = "2024"

[lib]
proc-macro = true

[dependencies]
syn = { version = "2", features = ["full"] }
quote = "1"
proc-macro2 = "1"

[lints]
workspace = true

[lints.rust]
unsafe_code = "allow"

[lints.clippy]
missing_docs_in_private_items = "allow"
arithmetic_side_effects = "allow"
integer_arithmetic = "allow"
indexing_slicing = "allow"
shadow_reuse = "allow"
shadow_same = "allow"
shadow_unrelated = "allow"
wildcard_imports = "allow"
exhaustive_enums = "allow"
exhaustive_structs = "allow"
```

**Step 2: Update hsrs-codegen/Cargo.toml**

```toml
[package]
name = "hsrs-codegen"
version = "0.1.0"
edition = "2024"

[dependencies]
syn = { version = "2", features = ["full"] }
quote = "1"
proc-macro2 = "1"

[lints]
workspace = true

[lints.clippy]
missing_docs_in_private_items = "allow"
print_stdout = "allow"
print_stderr = "allow"
arithmetic_side_effects = "allow"
integer_arithmetic = "allow"
indexing_slicing = "allow"
shadow_reuse = "allow"
shadow_same = "allow"
shadow_unrelated = "allow"
wildcard_imports = "allow"
exhaustive_enums = "allow"
exhaustive_structs = "allow"
```

**Step 3: Update hsrs-examples/Cargo.toml**

```toml
[package]
name = "hsrs-examples"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["lib", "staticlib"]

[dependencies]
hsrs = { path = "../hsrs" }
safer-ffi = { version = "0.2.0-rc1", features = ["alloc"] }

[lints]
workspace = true

[lints.rust]
unsafe_code = "allow"

[lints.clippy]
missing_docs_in_private_items = "allow"
arithmetic_side_effects = "allow"
integer_arithmetic = "allow"
float_arithmetic = "allow"
indexing_slicing = "allow"
shadow_reuse = "allow"
shadow_same = "allow"
shadow_unrelated = "allow"
wildcard_imports = "allow"
exhaustive_enums = "allow"
exhaustive_structs = "allow"
```

Note: `unsafe_code = "allow"` needed in hsrs-examples because edition 2024 makes `extern "C"` implicitly unsafe, and `#[derive_ReprC]` generates `unsafe impl` blocks.

**Step 4: Fix example code**

Replace all `self.clock++` with `self.clock += 1` in `hsrs-examples/src/lib.rs`.

**Step 5: Verify dependencies resolve**

Run: `cargo check --workspace 2>&1 | head -10`
Expected: Dependencies download, but compilation fails (macro bodies not yet implemented).

---

## Task 2: Implement hsrs Proc Macros — Entry Points

**Files:**
- Replace: `/Users/marko/Desktop/hsrs/hsrs/src/lib.rs`
- Create: `/Users/marko/Desktop/hsrs/hsrs/src/enumeration.rs`
- Create: `/Users/marko/Desktop/hsrs/hsrs/src/module.rs`

**Step 1: Write hsrs/src/lib.rs**

```rust
#![allow(
    clippy::needless_pass_by_value,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions
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
```

**Step 2: Write hsrs/src/enumeration.rs**

```rust
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
        #[::safer_ffi::derive_ReprC]
        #[repr(u8)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #vis enum #ident {
            #variants
        }
    })
}
```

**Step 3: Write hsrs/src/module.rs (stub)**

```rust
use proc_macro2::TokenStream;

pub(crate) fn expand(item: TokenStream) -> syn::Result<TokenStream> {
    // Stub — implemented in next task
    Ok(item)
}
```

**Step 4: Verify proc-macro crate compiles**

Run: `cargo check -p hsrs`
Expected: Clean compilation.

---

## Task 3: Implement `#[hsrs::module]` Proc Macro

**Files:**
- Replace: `/Users/marko/Desktop/hsrs/hsrs/src/module.rs`

This is the core macro. It processes the entire mod block.

**Step 1: Write the full module.rs**

```rust
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{
    FnArg, ImplItem, ImplItemFn, Item, ItemMod,
    Pat, ReturnType, Type,
};

pub(crate) fn expand(item: TokenStream) -> syn::Result<TokenStream> {
    let mut input: ItemMod = syn::parse2(item)?;
    let mod_name = input.ident.clone();

    let content = input.content.as_mut().ok_or_else(|| {
        syn::Error::new_spanned(&input, "hsrs::module requires an inline module")
    })?;
    let items = &mut content.1;

    // Find the #[hsrs::data_type] struct
    let struct_ident = find_data_type_struct(items)?;

    // Process struct: strip marker, add safer_ffi attrs
    process_struct(items, &struct_ident)?;

    // Find impl block, extract function info, generate wrappers
    let ffi_wrappers = generate_ffi_from_impl(items, &mod_name, &struct_ident)?;

    // Generate destructor
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

    // Inject use statements at top
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

    // Append FFI wrappers and destructor
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
                // Strip #[hsrs::data_type]
                s.attrs.retain(|a| !is_hsrs_path(a, "data_type"));
                // Add safer_ffi attributes
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
                        // Process methods
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
        // Constructor — no self param, returns Self
        Ok(syn::parse_quote! {
            #[allow(clippy::missing_docs_in_private_items)]
            #[::safer_ffi::ffi_export]
            fn #ffi_name(#(#params),*) -> repr_c::Box<#struct_ident> {
                Box::new(#struct_ident::#method_name(#(#param_names),*)).into()
            }
        })
    } else if is_mut {
        let output = &method.sig.output;
        match output {
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
        // &self
        let output = &method.sig.output;
        match output {
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
    let segs: Vec<_> = attr.path().segments.iter().map(|s| s.ident.to_string()).collect();
    segs == vec!["hsrs", name]
}

fn non_self_params(
    inputs: &syn::punctuated::Punctuated<FnArg, syn::Token![,]>,
) -> Vec<&FnArg> {
    inputs.iter().filter(|a| !matches!(a, FnArg::Receiver(_))).collect()
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
```

**Step 2: Verify proc-macro compiles**

Run: `cargo check -p hsrs`

**Step 3: Verify examples compile**

Run: `cargo check -p hsrs-examples`

**Step 4: Commit**

```bash
git add hsrs/ hsrs-examples/
git commit -m "feat: implement hsrs proc macros (enumeration, module, data_type, function)"
```

---

## Task 4: Implement hsrs-codegen IR and Parser

**Files:**
- Create: `/Users/marko/Desktop/hsrs/hsrs-codegen/src/ir.rs`
- Create: `/Users/marko/Desktop/hsrs/hsrs-codegen/src/parser.rs`

**Step 1: Write ir.rs**

```rust
pub struct ParsedFile {
    pub enums: Vec<FfiEnum>,
    pub modules: Vec<FfiModule>,
}

pub struct FfiEnum {
    pub name: String,
    pub variants: Vec<String>,
}

pub struct FfiModule {
    pub name: String,
    pub struct_name: String,
    pub functions: Vec<FfiFunction>,
}

pub struct FfiFunction {
    pub rust_name: String,
    pub c_name: String,
    pub kind: FfiFunctionKind,
    pub params: Vec<FfiParam>,
    pub return_type: Option<FfiType>,
}

pub enum FfiFunctionKind {
    Constructor,
    MutMethod,
    RefMethod,
    Destructor,
}

pub struct FfiParam {
    pub name: String,
    pub ty: FfiType,
}

pub enum FfiType {
    Int(u8),
    Uint(u8),
    Bool,
    Usize,
    Isize,
    Enum(String),
    Unit,
}
```

**Step 2: Write parser.rs**

```rust
use crate::ir::{
    FfiEnum, FfiFunction, FfiFunctionKind, FfiModule, FfiParam, FfiType, ParsedFile,
};
use std::path::Path;
use syn::{Fields, FnArg, ImplItem, Item, Pat, ReturnType, Type};

pub fn parse_file(path: &Path) -> Result<ParsedFile, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    let file = syn::parse_file(&source)
        .map_err(|e| format!("failed to parse {}: {e}", path.display()))?;

    let mut enums = Vec::new();
    let mut modules = Vec::new();

    for item in &file.items {
        match item {
            Item::Enum(e) if has_hsrs_attr(&e.attrs, "enumeration") => {
                enums.push(parse_enum(e)?);
            }
            Item::Mod(m) if has_hsrs_attr(&m.attrs, "module") => {
                modules.push(parse_module(m, &enums)?);
            }
            _ => {}
        }
    }

    Ok(ParsedFile { enums, modules })
}

fn has_hsrs_attr(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| {
        let segs: Vec<_> = attr.path().segments.iter().map(|s| s.ident.to_string()).collect();
        segs == vec!["hsrs", name]
    })
}

fn parse_enum(e: &syn::ItemEnum) -> Result<FfiEnum, String> {
    let mut variants = Vec::new();
    for v in &e.variants {
        if !matches!(v.fields, Fields::Unit) {
            return Err(format!("non-unit variant {} in {}", v.ident, e.ident));
        }
        variants.push(v.ident.to_string());
    }
    Ok(FfiEnum { name: e.ident.to_string(), variants })
}

fn parse_module(m: &syn::ItemMod, known_enums: &[FfiEnum]) -> Result<FfiModule, String> {
    let mod_name = m.ident.to_string();
    let content = m.content.as_ref()
        .ok_or_else(|| format!("module {mod_name} must be inline"))?;

    let data_struct = content.1.iter().find_map(|item| {
        if let Item::Struct(s) = item {
            if has_hsrs_attr(&s.attrs, "data_type") { return Some(s); }
        }
        None
    }).ok_or_else(|| format!("no #[hsrs::data_type] in {mod_name}"))?;

    let struct_name = data_struct.ident.to_string();

    let impl_block = content.1.iter().find_map(|item| {
        if let Item::Impl(imp) = item {
            if let Type::Path(tp) = &*imp.self_ty {
                if tp.path.is_ident(&data_struct.ident) { return Some(imp); }
            }
        }
        None
    }).ok_or_else(|| format!("no impl for {struct_name}"))?;

    let mut functions = Vec::new();
    for item in &impl_block.items {
        if let ImplItem::Fn(method) = item {
            if has_hsrs_attr(&method.attrs, "function") {
                functions.push(parse_function(method, &mod_name, known_enums)?);
            }
        }
    }

    // Auto-add destructor
    functions.push(FfiFunction {
        rust_name: "free".to_owned(),
        c_name: format!("{mod_name}_free"),
        kind: FfiFunctionKind::Destructor,
        params: vec![],
        return_type: None,
    });

    Ok(FfiModule { name: mod_name, struct_name, functions })
}

fn parse_function(
    method: &syn::ImplItemFn,
    mod_name: &str,
    known_enums: &[FfiEnum],
) -> Result<FfiFunction, String> {
    let name = method.sig.ident.to_string();
    let c_name = format!("{mod_name}_{name}");

    let kind = match method.sig.inputs.first() {
        Some(FnArg::Receiver(r)) if r.mutability.is_some() => FfiFunctionKind::MutMethod,
        Some(FnArg::Receiver(_)) => FfiFunctionKind::RefMethod,
        _ => FfiFunctionKind::Constructor,
    };

    let mut params = Vec::new();
    for arg in &method.sig.inputs {
        if let FnArg::Typed(pt) = arg {
            let param_name = if let Pat::Ident(pi) = &*pt.pat {
                pi.ident.to_string()
            } else {
                return Err(format!("unsupported param pattern in {name}"));
            };
            params.push(FfiParam {
                name: param_name,
                ty: resolve_type(&pt.ty, known_enums)?,
            });
        }
    }

    let return_type = match &method.sig.output {
        ReturnType::Default => None,
        ReturnType::Type(_, ty) => {
            let resolved = resolve_type(ty, known_enums)?;
            if matches!(resolved, FfiType::Unit) { None } else { Some(resolved) }
        }
    };

    Ok(FfiFunction { rust_name: name, c_name, kind, params, return_type })
}

fn resolve_type(ty: &Type, known_enums: &[FfiEnum]) -> Result<FfiType, String> {
    match ty {
        Type::Path(tp) => {
            let ident = tp.path.get_ident()
                .ok_or_else(|| "qualified types not supported".to_owned())?
                .to_string();
            match ident.as_str() {
                "i8" => Ok(FfiType::Int(8)),
                "i16" => Ok(FfiType::Int(16)),
                "i32" => Ok(FfiType::Int(32)),
                "i64" => Ok(FfiType::Int(64)),
                "u8" => Ok(FfiType::Uint(8)),
                "u16" => Ok(FfiType::Uint(16)),
                "u32" => Ok(FfiType::Uint(32)),
                "u64" => Ok(FfiType::Uint(64)),
                "bool" => Ok(FfiType::Bool),
                "usize" => Ok(FfiType::Usize),
                "isize" => Ok(FfiType::Isize),
                "Self" => Ok(FfiType::Unit),
                other => {
                    if known_enums.iter().any(|e| e.name == other) {
                        Ok(FfiType::Enum(other.to_owned()))
                    } else {
                        Err(format!("unknown type: {other}"))
                    }
                }
            }
        }
        Type::Tuple(tt) if tt.elems.is_empty() => Ok(FfiType::Unit),
        _ => Err("unsupported type syntax".to_owned()),
    }
}
```

**Step 3: Verify codegen compiles**

Run: `cargo check -p hsrs-codegen`

---

## Task 5: Implement Haskell Code Generator

**Files:**
- Create: `/Users/marko/Desktop/hsrs/hsrs-codegen/src/haskell.rs`
- Replace: `/Users/marko/Desktop/hsrs/hsrs-codegen/src/main.rs`

**Step 1: Write haskell.rs**

```rust
use crate::ir::{FfiEnum, FfiFunction, FfiFunctionKind, FfiModule, FfiParam, FfiType, ParsedFile};

pub fn generate(parsed: &ParsedFile) -> String {
    let mut out = String::new();

    out.push_str("{-# LANGUAGE PatternSynonyms #-}\n");
    out.push_str("{-# LANGUAGE GeneralizedNewtypeDeriving #-}\n\n");
    out.push_str("module Bindings where\n\n");
    out.push_str("import Foreign\n");
    out.push_str("import Foreign.C.Types\n");
    out.push_str("import Data.Int\n");
    out.push_str("import Data.Word\n\n");

    for e in &parsed.enums {
        generate_enum(&mut out, e);
    }
    for m in &parsed.modules {
        generate_module(&mut out, m);
    }

    out
}

fn generate_enum(out: &mut String, e: &FfiEnum) {
    out.push_str(&format!(
        "newtype {} = {} Word8\n  deriving (Eq, Show, Storable)\n\n",
        e.name, e.name
    ));
    for (i, variant) in e.variants.iter().enumerate() {
        out.push_str(&format!("pattern {} :: {}\n", variant, e.name));
        out.push_str(&format!("pattern {} = {} {}\n\n", variant, e.name, i));
    }
}

fn generate_module(out: &mut String, m: &FfiModule) {
    let raw = format!("{}Raw", m.struct_name);

    out.push_str(&format!("data {raw}\n\n"));
    out.push_str(&format!(
        "newtype {} = {} (ForeignPtr {raw})\n\n",
        m.struct_name, m.struct_name
    ));

    // Foreign imports
    for f in &m.functions {
        generate_foreign_import(out, f, &raw);
    }
    out.push('\n');

    // High-level API
    for f in &m.functions {
        if !matches!(f.kind, FfiFunctionKind::Destructor) {
            generate_high_level(out, f, &m.struct_name, &m.name);
        }
    }
}

fn generate_foreign_import(out: &mut String, f: &FfiFunction, raw: &str) {
    let hs = to_camel(&f.c_name);

    match f.kind {
        FfiFunctionKind::Destructor => {
            out.push_str(&format!(
                "foreign import ccall \"&{}\" c_{} :: FinalizerPtr {}\n",
                f.c_name, hs, raw
            ));
        }
        FfiFunctionKind::Constructor => {
            let params = f.params.iter()
                .map(|p| format!("{} -> ", ffi_type(&p.ty)))
                .collect::<String>();
            out.push_str(&format!(
                "foreign import ccall \"{}\" c_{} :: {}IO (Ptr {})\n",
                f.c_name, hs, params, raw
            ));
        }
        FfiFunctionKind::MutMethod | FfiFunctionKind::RefMethod => {
            let params = f.params.iter()
                .map(|p| format!("{} -> ", ffi_type(&p.ty)))
                .collect::<String>();
            let ret = match &f.return_type {
                Some(ty) => format!("IO {}", ffi_type(ty)),
                None => "IO ()".to_owned(),
            };
            out.push_str(&format!(
                "foreign import ccall \"{}\" c_{} :: Ptr {} -> {}{}\n",
                f.c_name, hs, raw, params, ret
            ));
        }
    }
}

fn generate_high_level(out: &mut String, f: &FfiFunction, struct_name: &str, mod_name: &str) {
    let hs_c = to_camel(&f.c_name);
    let free_hs = to_camel(&format!("{mod_name}_free"));

    match f.kind {
        FfiFunctionKind::Constructor => {
            let sig_params = f.params.iter()
                .map(|p| format!("{} -> ", hl_type(&p.ty)))
                .collect::<String>();
            out.push_str(&format!("\n{} :: {}IO {}\n", f.rust_name, sig_params, struct_name));

            let pnames: Vec<_> = f.params.iter().map(|p| p.name.clone()).collect();
            let unwrapped = f.params.iter()
                .map(|p| unwrap_param(&p.name, &p.ty))
                .collect::<Vec<_>>()
                .join(" ");

            if pnames.is_empty() {
                out.push_str(&format!(
                    "{} = do\n  ptr <- c_{}\n  fp <- newForeignPtr c_{} ptr\n  pure ({} fp)\n",
                    f.rust_name, hs_c, free_hs, struct_name
                ));
            } else {
                out.push_str(&format!(
                    "{} {} = do\n  ptr <- c_{} {}\n  fp <- newForeignPtr c_{} ptr\n  pure ({} fp)\n",
                    f.rust_name, pnames.join(" "), hs_c, unwrapped, free_hs, struct_name
                ));
            }
        }
        FfiFunctionKind::MutMethod | FfiFunctionKind::RefMethod => {
            let sig_params = f.params.iter()
                .map(|p| format!("{} -> ", hl_type(&p.ty)))
                .collect::<String>();
            let ret = match &f.return_type {
                Some(ty) => format!("IO {}", hl_type(ty)),
                None => "IO ()".to_owned(),
            };
            out.push_str(&format!(
                "\n{} :: {} -> {}{}\n",
                f.rust_name, struct_name, sig_params, ret
            ));

            let pnames: Vec<_> = f.params.iter().map(|p| p.name.clone()).collect();
            let plist = if pnames.is_empty() { String::new() } else { format!(" {}", pnames.join(" ")) };
            let unwrapped = if f.params.is_empty() {
                String::new()
            } else {
                format!(" {}", f.params.iter()
                    .map(|p| unwrap_param(&p.name, &p.ty))
                    .collect::<Vec<_>>()
                    .join(" "))
            };

            out.push_str(&format!(
                "{} ({} fp){} = withForeignPtr fp $ \\ptr -> c_{} ptr{}\n",
                f.rust_name, struct_name, plist, hs_c, unwrapped
            ));
        }
        FfiFunctionKind::Destructor => {}
    }
}

fn ffi_type(ty: &FfiType) -> &'static str {
    match ty {
        FfiType::Int(8) => "Int8",
        FfiType::Int(16) => "Int16",
        FfiType::Int(32) => "Int32",
        FfiType::Int(64) => "Int64",
        FfiType::Uint(8) => "Word8",
        FfiType::Uint(16) => "Word16",
        FfiType::Uint(32) => "Word32",
        FfiType::Uint(64) => "Word64",
        FfiType::Bool => "CBool",
        FfiType::Usize => "Word64",
        FfiType::Isize => "Int64",
        FfiType::Enum(_) => "Word8",
        FfiType::Unit => "()",
        _ => "()",
    }
}

fn hl_type(ty: &FfiType) -> String {
    match ty {
        FfiType::Enum(name) => name.clone(),
        other => ffi_type(other).to_owned(),
    }
}

fn to_camel(snake: &str) -> String {
    let mut result = String::new();
    let mut cap = false;
    for (i, ch) in snake.chars().enumerate() {
        if ch == '_' {
            cap = true;
        } else if cap {
            result.push(ch.to_ascii_uppercase());
            cap = false;
        } else if i == 0 {
            result.push(ch);
        } else {
            result.push(ch);
        }
    }
    result
}

fn unwrap_param(name: &str, ty: &FfiType) -> String {
    match ty {
        FfiType::Enum(enum_name) => {
            format!("(let ({} {name}') = {name} in {name}')", enum_name)
        }
        _ => name.to_owned(),
    }
}
```

**Step 2: Write main.rs**

```rust
mod haskell;
mod ir;
mod parser;

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: hsrs-codegen <source.rs> [output.hs]");
        std::process::exit(1);
    }

    let input = PathBuf::from(&args[1]);
    let parsed = match parser::parse_file(&input) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    let output = haskell::generate(&parsed);

    if args.len() >= 3 {
        std::fs::write(&args[2], &output).unwrap_or_else(|e| {
            eprintln!("Failed to write: {e}");
            std::process::exit(1);
        });
    } else {
        print!("{output}");
    }
}
```

**Step 3: Build and run**

Run: `cargo run -p hsrs-codegen -- hsrs-examples/src/lib.rs`
Expected: Haskell binding output to stdout.

**Step 4: Commit**

```bash
git add hsrs-codegen/
git commit -m "feat: implement hsrs-codegen Haskell bindings generator"
```

---

## Task 6: Integration Test

**Step 1: Full workspace check**

Run: `cargo check --workspace`

**Step 2: Clippy**

Run: `cargo clippy --workspace`

**Step 3: Generate Haskell and inspect**

Run: `cargo run -p hsrs-codegen -- hsrs-examples/src/lib.rs`

Expected output should contain:
- `newtype Register = Register Word8` with pattern synonyms
- `data QuectoVmRaw` + `newtype QuectoVm = QuectoVm (ForeignPtr QuectoVmRaw)`
- Foreign imports for all functions
- High-level wrappers preserving type safety

---

## Risks

1. **safer-ffi 0.2.0-rc1 + edition 2024**: Using the RC version for better edition 2024 support. If it breaks, fall back to 0.1.13 + edition 2021 for hsrs-examples.
2. **`#[repr(opaque)]` is safer-ffi-specific**: It's processed by `#[derive_ReprC]` before rustc sees it. Order matters — `#[derive_ReprC]` must come before `#[repr(opaque)]`.
3. **Constructor heuristic**: Methods without `self` are assumed constructors. A static method returning non-Self would be misclassified. Acceptable for v1.

---

## File Summary

| File | Action |
|------|--------|
| `hsrs/Cargo.toml` | Modify (deps + lint overrides) |
| `hsrs-codegen/Cargo.toml` | Modify (deps + lint overrides) |
| `hsrs-examples/Cargo.toml` | Modify (deps + lint overrides + crate-type) |
| `hsrs-examples/src/lib.rs` | Modify (fix clock++) |
| `hsrs/src/lib.rs` | Replace (proc macro entry points) |
| `hsrs/src/enumeration.rs` | Create |
| `hsrs/src/module.rs` | Create |
| `hsrs-codegen/src/main.rs` | Replace |
| `hsrs-codegen/src/ir.rs` | Create |
| `hsrs-codegen/src/parser.rs` | Create |
| `hsrs-codegen/src/haskell.rs` | Create |

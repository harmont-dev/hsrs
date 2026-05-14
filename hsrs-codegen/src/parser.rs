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
        let segs: Vec<_> = attr
            .path()
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect();
        segs == vec!["hsrs", name]
    })
}

fn extract_derives(attrs: &[syn::Attribute]) -> (bool, bool, bool) {
    let mut has_eq = false;
    let mut has_show = false;
    let mut has_ord = false;

    for attr in attrs {
        if attr.path().is_ident("derive") {
            if let Ok(nested) = attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
            ) {
                for path in &nested {
                    if let Some(ident) = path.get_ident() {
                        match ident.to_string().as_str() {
                            "PartialEq" | "Eq" => has_eq = true,
                            "Debug" | "Display" => has_show = true,
                            "PartialOrd" | "Ord" => has_ord = true,
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    (has_eq, has_show, has_ord)
}

fn parse_enum(e: &syn::ItemEnum) -> Result<FfiEnum, String> {
    let mut variants = Vec::new();
    for v in &e.variants {
        if !matches!(v.fields, Fields::Unit) {
            return Err(format!("non-unit variant {} in {}", v.ident, e.ident));
        }
        variants.push(v.ident.to_string());
    }
    let (has_eq, has_show, has_ord) = extract_derives(&e.attrs);
    Ok(FfiEnum {
        name: e.ident.to_string(),
        variants,
        has_eq,
        has_show,
        has_ord,
    })
}

fn parse_module(m: &syn::ItemMod, known_enums: &[FfiEnum]) -> Result<FfiModule, String> {
    let mod_name = m.ident.to_string();
    let content = m
        .content
        .as_ref()
        .ok_or_else(|| format!("module {mod_name} must be inline"))?;

    let data_struct = content
        .1
        .iter()
        .find_map(|item| {
            if let Item::Struct(s) = item {
                if has_hsrs_attr(&s.attrs, "data_type") {
                    return Some(s);
                }
            }
            None
        })
        .ok_or_else(|| format!("no #[hsrs::data_type] in {mod_name}"))?;

    let struct_name = data_struct.ident.to_string();

    let impl_block = content
        .1
        .iter()
        .find_map(|item| {
            if let Item::Impl(imp) = item {
                if let Type::Path(tp) = &*imp.self_ty {
                    if tp.path.is_ident(&data_struct.ident) {
                        return Some(imp);
                    }
                }
            }
            None
        })
        .ok_or_else(|| format!("no impl for {struct_name}"))?;

    let mut functions = Vec::new();
    for item in &impl_block.items {
        if let ImplItem::Fn(method) = item {
            if has_hsrs_attr(&method.attrs, "function") {
                functions.push(parse_function(method, &mod_name, known_enums)?);
            }
        }
    }

    functions.push(FfiFunction {
        rust_name: "free".to_owned(),
        c_name: format!("{mod_name}_free"),
        kind: FfiFunctionKind::Destructor,
        params: vec![],
        return_type: None,
    });

    Ok(FfiModule {
        name: mod_name,
        struct_name,
        functions,
    })
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
            if matches!(resolved, FfiType::Unit) {
                None
            } else {
                Some(resolved)
            }
        }
    };

    Ok(FfiFunction {
        rust_name: name,
        c_name,
        kind,
        params,
        return_type,
    })
}

fn resolve_type(ty: &Type, known_enums: &[FfiEnum]) -> Result<FfiType, String> {
    match ty {
        Type::Path(tp) => {
            let ident = tp
                .path
                .get_ident()
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

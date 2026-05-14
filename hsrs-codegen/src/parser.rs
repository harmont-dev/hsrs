use crate::ir::{
    FfiEnum, FfiField, FfiFunction, FfiFunctionKind, FfiModule, FfiParam, FfiType, FfiValueType,
    ParsedFile,
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
    let mut value_types = Vec::new();

    for item in &file.items {
        match item {
            Item::Enum(e) if has_hsrs_attr(&e.attrs, "enumeration") => {
                enums.push(parse_enum(e)?);
            }
            Item::Struct(s) if has_hsrs_attr(&s.attrs, "value_type") => {
                value_types.push(parse_value_type(s, &enums, &value_types)?);
            }
            Item::Mod(m) if has_hsrs_attr(&m.attrs, "module") => {
                modules.push(parse_module(m, &enums, &value_types)?);
            }
            _ => {}
        }
    }

    Ok(ParsedFile {
        enums,
        modules,
        value_types,
    })
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

fn extract_docs(attrs: &[syn::Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter_map(|attr| {
            if attr.path().is_ident("doc") {
                if let syn::Meta::NameValue(nv) = &attr.meta {
                    if let syn::Expr::Lit(lit) = &nv.value {
                        if let syn::Lit::Str(s) = &lit.lit {
                            return Some(s.value());
                        }
                    }
                }
            }
            None
        })
        .collect()
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
        docs: extract_docs(&e.attrs),
    })
}

fn parse_value_type(
    s: &syn::ItemStruct,
    known_enums: &[FfiEnum],
    known_value_types: &[FfiValueType],
) -> Result<FfiValueType, String> {
    let name = s.ident.to_string();
    let (has_eq, has_show, has_ord) = extract_derives(&s.attrs);

    let mut fields = Vec::new();
    if let Fields::Named(named) = &s.fields {
        for f in &named.named {
            let field_name = f
                .ident
                .as_ref()
                .ok_or_else(|| format!("unnamed field in {name}"))?
                .to_string();
            let ty = resolve_type(&f.ty, known_enums, known_value_types)?;
            fields.push(FfiField {
                name: field_name,
                ty,
            });
        }
    } else {
        return Err(format!("value_type {name} must have named fields"));
    }

    Ok(FfiValueType {
        name,
        fields,
        has_eq,
        has_show,
        has_ord,
        docs: extract_docs(&s.attrs),
    })
}

fn parse_module(
    m: &syn::ItemMod,
    known_enums: &[FfiEnum],
    known_value_types: &[FfiValueType],
) -> Result<FfiModule, String> {
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
                functions.push(parse_function(method, &mod_name, known_enums, known_value_types)?);
            }
        }
    }

    functions.push(FfiFunction {
        rust_name: "free".to_owned(),
        c_name: format!("{mod_name}_free"),
        kind: FfiFunctionKind::Destructor,
        params: vec![],
        return_type: None,
        docs: vec![],
        borsh_return: false,
        borsh_params: vec![],
    });

    Ok(FfiModule {
        name: mod_name,
        struct_name,
        functions,
        docs: extract_docs(&m.attrs),
    })
}

fn is_borsh_type(ty: &FfiType) -> bool {
    matches!(
        ty,
        FfiType::ValueType(_) | FfiType::Result(_, _) | FfiType::Option(_)
    )
}

fn parse_function(
    method: &syn::ImplItemFn,
    mod_name: &str,
    known_enums: &[FfiEnum],
    known_value_types: &[FfiValueType],
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
                ty: resolve_type(&pt.ty, known_enums, known_value_types)?,
            });
        }
    }

    let return_type = match &method.sig.output {
        ReturnType::Default => None,
        ReturnType::Type(_, ty) => {
            let resolved = resolve_type(ty, known_enums, known_value_types)?;
            if matches!(resolved, FfiType::Unit) {
                None
            } else {
                Some(resolved)
            }
        }
    };

    let borsh_return = return_type.as_ref().is_some_and(is_borsh_type);
    let borsh_params: Vec<String> = params
        .iter()
        .filter(|p| is_borsh_type(&p.ty))
        .map(|p| p.name.clone())
        .collect();

    Ok(FfiFunction {
        rust_name: name,
        c_name,
        kind,
        params,
        return_type,
        docs: extract_docs(&method.attrs),
        borsh_return,
        borsh_params,
    })
}

fn resolve_type(
    ty: &Type,
    known_enums: &[FfiEnum],
    known_value_types: &[FfiValueType],
) -> Result<FfiType, String> {
    match ty {
        Type::Path(tp) => {
            if let Some(ident) = tp.path.get_ident() {
                let s = ident.to_string();
                return match s.as_str() {
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
                        } else if known_value_types.iter().any(|v| v.name == other) {
                            Ok(FfiType::ValueType(other.to_owned()))
                        } else {
                            Err(format!("unknown type: {other}"))
                        }
                    }
                };
            }
            if let Some(seg) = tp.path.segments.last() {
                let name = seg.ident.to_string();
                if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                    let type_args: Vec<_> = args
                        .args
                        .iter()
                        .filter_map(|a| {
                            if let syn::GenericArgument::Type(ty) = a {
                                Some(ty)
                            } else {
                                None
                            }
                        })
                        .collect();
                    match name.as_str() {
                        "Result" => {
                            if type_args.len() != 2 {
                                return Err("Result requires exactly 2 type arguments".to_owned());
                            }
                            let ok_ty =
                                resolve_type(type_args[0], known_enums, known_value_types)?;
                            let err_ty =
                                resolve_type(type_args[1], known_enums, known_value_types)?;
                            return Ok(FfiType::Result(Box::new(ok_ty), Box::new(err_ty)));
                        }
                        "Option" => {
                            if type_args.len() != 1 {
                                return Err(
                                    "Option requires exactly 1 type argument".to_owned()
                                );
                            }
                            let inner =
                                resolve_type(type_args[0], known_enums, known_value_types)?;
                            return Ok(FfiType::Option(Box::new(inner)));
                        }
                        _ => return Err(format!("unsupported generic type: {name}")),
                    }
                }
            }
            Err("qualified types not supported".to_owned())
        }
        Type::Tuple(tt) if tt.elems.is_empty() => Ok(FfiType::Unit),
        _ => Err("unsupported type syntax".to_owned()),
    }
}

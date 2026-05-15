use crate::ir::{
    FfiEnum, FfiField, FfiFunction, FfiFunctionKind, FfiModule, FfiParam, FfiSafety, FfiType,
    FfiValueType, ParsedFile,
};
use std::path::Path;
use syn::{Fields, FnArg, ImplItem, Item, Pat, ReturnType, Type};

pub fn parse_file(path: &Path) -> Result<ParsedFile, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    parse_str(&source)
}

pub fn parse_sources(sources: &[&str]) -> Result<ParsedFile, String> {
    let files: Vec<syn::File> = sources
        .iter()
        .map(|s| syn::parse_file(s).map_err(|e| format!("failed to parse source: {e}")))
        .collect::<Result<_, _>>()?;

    let mut all_enums = Vec::new();
    let mut all_value_types = Vec::new();
    let mut all_modules = Vec::new();

    for file in &files {
        for item in &file.items {
            match item {
                Item::Enum(e) if has_hsrs_attr(&e.attrs, "enumeration") => {
                    all_enums.push(parse_enum(e)?);
                }
                Item::Struct(s) if has_hsrs_attr(&s.attrs, "value_type") => {
                    all_value_types.push(parse_value_type(s, &all_enums, &all_value_types)?);
                }
                _ => {}
            }
        }
    }

    for file in &files {
        for item in &file.items {
            if let Item::Mod(m) = item {
                if has_hsrs_attr(&m.attrs, "module") {
                    all_modules.push(parse_module(m, &all_enums, &all_value_types)?);
                }
            }
        }
    }

    Ok(ParsedFile {
        enums: all_enums,
        modules: all_modules,
        value_types: all_value_types,
    })
}

pub fn parse_files(paths: &[&Path]) -> Result<ParsedFile, String> {
    let sources: Vec<String> = paths
        .iter()
        .map(|p| {
            std::fs::read_to_string(p)
                .map_err(|e| format!("failed to read {}: {e}", p.display()))
        })
        .collect::<Result<_, _>>()?;
    let refs: Vec<&str> = sources.iter().map(String::as_str).collect();
    parse_sources(&refs)
}

pub fn parse_str(source: &str) -> Result<ParsedFile, String> {
    parse_sources(&[source])
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

fn extract_function_safety(attrs: &[syn::Attribute]) -> Option<FfiSafety> {
    for attr in attrs {
        let segs: Vec<_> = attr
            .path()
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect();
        if segs == vec!["hsrs", "function"] {
            if let syn::Meta::List(list) = &attr.meta {
                let token_str = list.tokens.to_string();
                let trimmed = token_str.trim();
                return match trimmed {
                    "safe" => Some(FfiSafety::Safe),
                    "unsafe" => Some(FfiSafety::Unsafe),
                    "interruptible" => Some(FfiSafety::Interruptible),
                    _ => None,
                };
            }
            // Bare #[hsrs::function] — no safety specified
            return None;
        }
    }
    None
}

fn extract_module_safety(attrs: &[syn::Attribute]) -> FfiSafety {
    for attr in attrs {
        let segs: Vec<_> = attr
            .path()
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect();
        if segs == vec!["hsrs", "module"] {
            if let syn::Meta::List(list) = &attr.meta {
                let token_str = list.tokens.to_string();
                // Look for `safety = <ident>` pattern in the token string.
                // We need to be careful because the string may also contain
                // things like `value_types(Foo, Bar)`.
                let mut tokens_iter = token_str.split_whitespace().peekable();
                while let Some(tok) = tokens_iter.next() {
                    // Handle tokens that may be glued together like "safety="
                    // or separated by whitespace like "safety = unsafe"
                    if tok == "safety" {
                        // Next should be "="
                        if let Some(&eq) = tokens_iter.peek() {
                            if eq.starts_with('=') {
                                tokens_iter.next(); // consume "="
                                // The value might be glued to "=" like "=unsafe"
                                let val = if eq.len() > 1 {
                                    eq[1..].trim_end_matches(',').to_string()
                                } else if let Some(v) = tokens_iter.next() {
                                    v.trim_end_matches(',').to_string()
                                } else {
                                    continue;
                                };
                                return match val.as_str() {
                                    "safe" => FfiSafety::Safe,
                                    "unsafe" => FfiSafety::Unsafe,
                                    "interruptible" => FfiSafety::Interruptible,
                                    _ => FfiSafety::Safe,
                                };
                            }
                        }
                    } else if tok.starts_with("safety") && tok.contains('=') {
                        // Handle "safety=unsafe" or "safety=unsafe,"
                        let after_eq = tok.split('=').nth(1).unwrap_or("");
                        let val = after_eq.trim_end_matches(',');
                        return match val {
                            "safe" => FfiSafety::Safe,
                            "unsafe" => FfiSafety::Unsafe,
                            "interruptible" => FfiSafety::Interruptible,
                            _ => FfiSafety::Safe,
                        };
                    }
                }
            }
        }
    }
    FfiSafety::Safe
}

fn parse_module(
    m: &syn::ItemMod,
    known_enums: &[FfiEnum],
    known_value_types: &[FfiValueType],
) -> Result<FfiModule, String> {
    let mod_name = m.ident.to_string();
    let default_safety = extract_module_safety(&m.attrs);
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
                functions.push(parse_function(method, &mod_name, &default_safety, known_enums, known_value_types)?);
            }
        }
    }

    functions.push(FfiFunction {
        rust_name: "free".to_owned(),
        c_name: format!("{mod_name}_free"),
        kind: FfiFunctionKind::Destructor,
        safety: FfiSafety::Safe,
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
        FfiType::ValueType(_) | FfiType::Result(_, _) | FfiType::Option(_) | FfiType::String | FfiType::Vec(_)
    )
}

fn parse_function(
    method: &syn::ImplItemFn,
    mod_name: &str,
    default_safety: &FfiSafety,
    known_enums: &[FfiEnum],
    known_value_types: &[FfiValueType],
) -> Result<FfiFunction, String> {
    let name = method.sig.ident.to_string();
    let c_name = format!("{mod_name}_{name}");
    let safety = extract_function_safety(&method.attrs).unwrap_or_else(|| default_safety.clone());

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
        safety,
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
                    "String" => Ok(FfiType::String),
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
                        "Vec" => {
                            if type_args.len() != 1 {
                                return Err(
                                    "Vec requires exactly 1 type argument".to_owned()
                                );
                            }
                            let inner =
                                resolve_type(type_args[0], known_enums, known_value_types)?;
                            return Ok(FfiType::Vec(Box::new(inner)));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::FfiSafety;

    fn parse_source(src: &str) -> ParsedFile {
        parse_str(src).unwrap()
    }

    #[test]
    fn function_default_safety_is_safe() {
        let src = r#"
            #[hsrs::module]
            mod example {
                #[hsrs::data_type]
                pub struct Example { val: i32 }
                impl Example {
                    #[hsrs::function]
                    pub fn get(&self) -> i32 { self.val }
                }
            }
        "#;
        let parsed = parse_source(src);
        let func = &parsed.modules[0].functions[0];
        assert_eq!(func.safety, FfiSafety::Safe);
    }

    #[test]
    fn function_explicit_unsafe() {
        let src = r#"
            #[hsrs::module]
            mod example {
                #[hsrs::data_type]
                pub struct Example { val: i32 }
                impl Example {
                    #[hsrs::function(unsafe)]
                    pub fn get(&self) -> i32 { self.val }
                }
            }
        "#;
        let parsed = parse_source(src);
        let func = &parsed.modules[0].functions[0];
        assert_eq!(func.safety, FfiSafety::Unsafe);
    }

    #[test]
    fn function_explicit_interruptible() {
        let src = r#"
            #[hsrs::module]
            mod example {
                #[hsrs::data_type]
                pub struct Example { val: i32 }
                impl Example {
                    #[hsrs::function(interruptible)]
                    pub fn get(&self) -> i32 { self.val }
                }
            }
        "#;
        let parsed = parse_source(src);
        let func = &parsed.modules[0].functions[0];
        assert_eq!(func.safety, FfiSafety::Interruptible);
    }

    #[test]
    fn module_level_safety_default() {
        let src = r#"
            #[hsrs::module(safety = unsafe)]
            mod example {
                #[hsrs::data_type]
                pub struct Example { val: i32 }
                impl Example {
                    #[hsrs::function]
                    pub fn get(&self) -> i32 { self.val }

                    #[hsrs::function(safe)]
                    pub fn get_safe(&self) -> i32 { self.val }
                }
            }
        "#;
        let parsed = parse_source(src);
        let funcs = &parsed.modules[0].functions;
        // Bare #[hsrs::function] inherits module-level unsafe
        assert_eq!(funcs[0].safety, FfiSafety::Unsafe);
        // #[hsrs::function(safe)] overrides to Safe
        assert_eq!(funcs[1].safety, FfiSafety::Safe);
    }

    #[test]
    fn module_level_safety_with_value_types() {
        let src = r#"
            #[hsrs::value_type]
            pub struct Foo { pub x: i32 }

            #[hsrs::module(value_types(Foo), safety = unsafe)]
            mod example {
                #[hsrs::data_type]
                pub struct Example { val: i32 }
                impl Example {
                    #[hsrs::function]
                    pub fn get(&self) -> i32 { self.val }
                }
            }
        "#;
        let parsed = parse_source(src);
        let func = &parsed.modules[0].functions[0];
        assert_eq!(func.safety, FfiSafety::Unsafe);
    }

    #[test]
    fn parses_enum_name_and_variants() {
        let src = r#"
            #[hsrs::enumeration]
            pub enum Color {
                Red,
                Green,
                Blue,
            }
        "#;
        let parsed = parse_source(src);
        assert_eq!(parsed.enums.len(), 1);
        let e = &parsed.enums[0];
        assert_eq!(e.name, "Color");
        assert_eq!(e.variants, vec!["Red", "Green", "Blue"]);
    }

    #[test]
    fn parses_enum_derives() {
        let src = r#"
            #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
            #[hsrs::enumeration]
            pub enum Priority {
                Low,
                High,
            }
        "#;
        let parsed = parse_source(src);
        let e = &parsed.enums[0];
        assert!(e.has_eq, "PartialEq should set has_eq");
        assert!(e.has_show, "Debug should set has_show");
        assert!(e.has_ord, "Ord should set has_ord");
    }

    #[test]
    fn parses_enum_without_derives() {
        let src = r#"
            #[hsrs::enumeration]
            pub enum Bare {
                A,
                B,
            }
        "#;
        let parsed = parse_source(src);
        let e = &parsed.enums[0];
        assert!(!e.has_eq);
        assert!(!e.has_show);
        assert!(!e.has_ord);
    }

    #[test]
    fn parses_enum_docs() {
        let src = r#"
            /// First line.
            /// Second line.
            #[hsrs::enumeration]
            pub enum Documented {
                A,
            }
        "#;
        let parsed = parse_source(src);
        let e = &parsed.enums[0];
        assert_eq!(e.docs.len(), 2);
        assert!(e.docs[0].contains("First line"));
        assert!(e.docs[1].contains("Second line"));
    }

    #[test]
    fn parses_value_type_fields() {
        let src = r#"
            #[hsrs::value_type]
            pub struct Rect {
                pub width: u32,
                pub height: u32,
            }
        "#;
        let parsed = parse_source(src);
        assert_eq!(parsed.value_types.len(), 1);
        let vt = &parsed.value_types[0];
        assert_eq!(vt.name, "Rect");
        assert_eq!(vt.fields.len(), 2);
        assert_eq!(vt.fields[0].name, "width");
        assert_eq!(vt.fields[1].name, "height");
        assert!(matches!(vt.fields[0].ty, FfiType::Uint(32)));
        assert!(matches!(vt.fields[1].ty, FfiType::Uint(32)));
    }

    #[test]
    fn parses_value_type_derives() {
        let src = r#"
            #[derive(Debug, PartialEq, Eq)]
            #[hsrs::value_type]
            pub struct Tagged {
                pub id: i64,
            }
        "#;
        let parsed = parse_source(src);
        let vt = &parsed.value_types[0];
        assert!(vt.has_eq);
        assert!(vt.has_show);
        assert!(!vt.has_ord);
    }

    #[test]
    fn parses_value_type_docs() {
        let src = r#"
            /// A tagged value.
            #[hsrs::value_type]
            pub struct Tagged {
                pub id: i64,
            }
        "#;
        let parsed = parse_source(src);
        assert_eq!(parsed.value_types[0].docs.len(), 1);
        assert!(parsed.value_types[0].docs[0].contains("tagged value"));
    }

    #[test]
    fn parses_all_primitive_param_types() {
        let src = r#"
            #[hsrs::module]
            mod prims {
                #[hsrs::data_type]
                pub struct Prims { x: i32 }
                impl Prims {
                    #[hsrs::function]
                    pub fn f(
                        &self,
                        a: i8, b: i16, c: i32, d: i64,
                        e: u8, f: u16, g: u32, h: u64,
                        i: bool, j: usize, k: isize,
                    ) -> i32 { 0 }
                }
            }
        "#;
        let parsed = parse_source(src);
        let params = &parsed.modules[0].functions[0].params;
        assert_eq!(params.len(), 11);
        assert!(matches!(params[0].ty, FfiType::Int(8)));
        assert!(matches!(params[1].ty, FfiType::Int(16)));
        assert!(matches!(params[2].ty, FfiType::Int(32)));
        assert!(matches!(params[3].ty, FfiType::Int(64)));
        assert!(matches!(params[4].ty, FfiType::Uint(8)));
        assert!(matches!(params[5].ty, FfiType::Uint(16)));
        assert!(matches!(params[6].ty, FfiType::Uint(32)));
        assert!(matches!(params[7].ty, FfiType::Uint(64)));
        assert!(matches!(params[8].ty, FfiType::Bool));
        assert!(matches!(params[9].ty, FfiType::Usize));
        assert!(matches!(params[10].ty, FfiType::Isize));
    }

    #[test]
    fn resolves_enum_param_type() {
        let src = r#"
            #[hsrs::enumeration]
            pub enum Dir { Up, Down }

            #[hsrs::module]
            mod nav {
                #[hsrs::data_type]
                pub struct Nav { x: i32 }
                impl Nav {
                    #[hsrs::function]
                    pub fn go(&mut self, d: Dir) { }
                }
            }
        "#;
        let parsed = parse_source(src);
        let param = &parsed.modules[0].functions[0].params[0];
        assert_eq!(param.name, "d");
        assert!(matches!(&param.ty, FfiType::Enum(n) if n == "Dir"));
    }

    #[test]
    fn resolves_value_type_param() {
        let src = r#"
            #[hsrs::value_type]
            pub struct Coord { pub x: i32, pub y: i32 }

            #[hsrs::module(value_types(Coord))]
            mod geo {
                #[hsrs::data_type]
                pub struct Geo { x: i32 }
                impl Geo {
                    #[hsrs::function]
                    pub fn set(&mut self, c: Coord) { }
                }
            }
        "#;
        let parsed = parse_source(src);
        let param = &parsed.modules[0].functions[0].params[0];
        assert!(matches!(&param.ty, FfiType::ValueType(n) if n == "Coord"));
    }

    #[test]
    fn resolves_result_return_type() {
        let src = r#"
            #[hsrs::value_type]
            pub struct Err { pub code: u32 }

            #[hsrs::module(value_types(Err))]
            mod op {
                #[hsrs::data_type]
                pub struct Op { x: i32 }
                impl Op {
                    #[hsrs::function]
                    pub fn try_it(&self) -> Result<i64, Err> { Ok(0) }
                }
            }
        "#;
        let parsed = parse_source(src);
        let ret = parsed.modules[0].functions[0].return_type.as_ref().unwrap();
        assert!(matches!(ret, FfiType::Result(ok, err)
            if matches!(**ok, FfiType::Int(64))
            && matches!(**err, FfiType::ValueType(ref n) if n == "Err")
        ));
    }

    #[test]
    fn resolves_option_return_type() {
        let src = r#"
            #[hsrs::module]
            mod op {
                #[hsrs::data_type]
                pub struct Op { x: i32 }
                impl Op {
                    #[hsrs::function]
                    pub fn maybe(&self) -> Option<i64> { Some(0) }
                }
            }
        "#;
        let parsed = parse_source(src);
        let ret = parsed.modules[0].functions[0].return_type.as_ref().unwrap();
        assert!(matches!(ret, FfiType::Option(inner) if matches!(**inner, FfiType::Int(64))));
    }

    #[test]
    fn parses_multiple_enums() {
        let src = r#"
            #[hsrs::enumeration]
            pub enum A { X }
            #[hsrs::enumeration]
            pub enum B { Y, Z }
        "#;
        let parsed = parse_source(src);
        assert_eq!(parsed.enums.len(), 2);
        assert_eq!(parsed.enums[0].name, "A");
        assert_eq!(parsed.enums[1].name, "B");
    }

    #[test]
    fn parses_multiple_value_types() {
        let src = r#"
            #[hsrs::value_type]
            pub struct A { pub x: i32 }
            #[hsrs::value_type]
            pub struct B { pub y: u64, pub z: bool }
        "#;
        let parsed = parse_source(src);
        assert_eq!(parsed.value_types.len(), 2);
        assert_eq!(parsed.value_types[0].fields.len(), 1);
        assert_eq!(parsed.value_types[1].fields.len(), 2);
    }

    #[test]
    fn detects_constructor_kind() {
        let src = r#"
            #[hsrs::module]
            mod m {
                #[hsrs::data_type]
                pub struct T { x: i32 }
                impl T {
                    #[hsrs::function]
                    pub fn new() -> Self { Self { x: 0 } }
                }
            }
        "#;
        let parsed = parse_source(src);
        let f = &parsed.modules[0].functions[0];
        assert_eq!(f.rust_name, "new");
        assert!(matches!(f.kind, FfiFunctionKind::Constructor));
    }

    #[test]
    fn detects_mut_method_kind() {
        let src = r#"
            #[hsrs::module]
            mod m {
                #[hsrs::data_type]
                pub struct T { x: i32 }
                impl T {
                    #[hsrs::function]
                    pub fn set(&mut self, v: i32) { self.x = v; }
                }
            }
        "#;
        let parsed = parse_source(src);
        assert!(matches!(parsed.modules[0].functions[0].kind, FfiFunctionKind::MutMethod));
    }

    #[test]
    fn detects_ref_method_kind() {
        let src = r#"
            #[hsrs::module]
            mod m {
                #[hsrs::data_type]
                pub struct T { x: i32 }
                impl T {
                    #[hsrs::function]
                    pub fn get(&self) -> i32 { self.x }
                }
            }
        "#;
        let parsed = parse_source(src);
        assert!(matches!(parsed.modules[0].functions[0].kind, FfiFunctionKind::RefMethod));
    }

    #[test]
    fn generates_correct_c_names() {
        let src = r#"
            #[hsrs::module]
            mod my_engine {
                #[hsrs::data_type]
                pub struct MyEngine { x: i32 }
                impl MyEngine {
                    #[hsrs::function]
                    pub fn new() -> Self { Self { x: 0 } }
                    #[hsrs::function]
                    pub fn get_value(&self) -> i32 { 0 }
                }
            }
        "#;
        let parsed = parse_source(src);
        let fns = &parsed.modules[0].functions;
        assert_eq!(fns[0].c_name, "my_engine_new");
        assert_eq!(fns[1].c_name, "my_engine_get_value");
    }

    #[test]
    fn auto_generates_destructor() {
        let src = r#"
            #[hsrs::module]
            mod m {
                #[hsrs::data_type]
                pub struct T { x: i32 }
                impl T {
                    #[hsrs::function]
                    pub fn new() -> Self { Self { x: 0 } }
                }
            }
        "#;
        let parsed = parse_source(src);
        let fns = &parsed.modules[0].functions;
        let destructor = fns.iter().find(|f| matches!(f.kind, FfiFunctionKind::Destructor));
        assert!(destructor.is_some(), "module should auto-generate destructor");
        let d = destructor.unwrap();
        assert_eq!(d.c_name, "m_free");
        assert_eq!(d.rust_name, "free");
        assert_eq!(d.safety, FfiSafety::Safe);
    }

    #[test]
    fn detects_borsh_return_for_value_type() {
        let src = r#"
            #[hsrs::value_type]
            pub struct Pos { pub x: i32 }

            #[hsrs::module(value_types(Pos))]
            mod m {
                #[hsrs::data_type]
                pub struct T { x: i32 }
                impl T {
                    #[hsrs::function]
                    pub fn pos(&self) -> Pos { Pos { x: 0 } }
                }
            }
        "#;
        let parsed = parse_source(src);
        assert!(parsed.modules[0].functions[0].borsh_return);
    }

    #[test]
    fn detects_borsh_return_for_result() {
        let src = r#"
            #[hsrs::value_type]
            pub struct E { pub code: u32 }

            #[hsrs::module(value_types(E))]
            mod m {
                #[hsrs::data_type]
                pub struct T { x: i32 }
                impl T {
                    #[hsrs::function]
                    pub fn try_it(&self) -> Result<i64, E> { Ok(0) }
                }
            }
        "#;
        let parsed = parse_source(src);
        assert!(parsed.modules[0].functions[0].borsh_return);
    }

    #[test]
    fn detects_borsh_return_for_option() {
        let src = r#"
            #[hsrs::module]
            mod m {
                #[hsrs::data_type]
                pub struct T { x: i32 }
                impl T {
                    #[hsrs::function]
                    pub fn maybe(&self) -> Option<i64> { None }
                }
            }
        "#;
        let parsed = parse_source(src);
        assert!(parsed.modules[0].functions[0].borsh_return);
    }

    #[test]
    fn primitive_return_is_not_borsh() {
        let src = r#"
            #[hsrs::module]
            mod m {
                #[hsrs::data_type]
                pub struct T { x: i32 }
                impl T {
                    #[hsrs::function]
                    pub fn get(&self) -> i64 { 0 }
                }
            }
        "#;
        let parsed = parse_source(src);
        assert!(!parsed.modules[0].functions[0].borsh_return);
    }

    #[test]
    fn detects_borsh_params() {
        let src = r#"
            #[hsrs::value_type]
            pub struct Cfg { pub level: u32 }

            #[hsrs::module(value_types(Cfg))]
            mod m {
                #[hsrs::data_type]
                pub struct T { x: i32 }
                impl T {
                    #[hsrs::function]
                    pub fn configure(&mut self, c: Cfg) { }
                }
            }
        "#;
        let parsed = parse_source(src);
        let f = &parsed.modules[0].functions[0];
        assert_eq!(f.borsh_params, vec!["c"]);
    }

    #[test]
    fn primitive_params_are_not_borsh() {
        let src = r#"
            #[hsrs::module]
            mod m {
                #[hsrs::data_type]
                pub struct T { x: i32 }
                impl T {
                    #[hsrs::function]
                    pub fn set(&mut self, a: i32, b: u64) { }
                }
            }
        "#;
        let parsed = parse_source(src);
        assert!(parsed.modules[0].functions[0].borsh_params.is_empty());
    }

    #[test]
    fn parses_function_docs() {
        let src = r#"
            #[hsrs::module]
            mod m {
                #[hsrs::data_type]
                pub struct T { x: i32 }
                impl T {
                    /// Does the thing.
                    #[hsrs::function]
                    pub fn do_it(&self) -> i32 { 0 }
                }
            }
        "#;
        let parsed = parse_source(src);
        let docs = &parsed.modules[0].functions[0].docs;
        assert_eq!(docs.len(), 1);
        assert!(docs[0].contains("Does the thing"));
    }

    #[test]
    fn parses_module_docs() {
        let src = r#"
            /// Engine module.
            #[hsrs::module]
            mod m {
                #[hsrs::data_type]
                pub struct T { x: i32 }
                impl T {
                    #[hsrs::function]
                    pub fn new() -> Self { Self { x: 0 } }
                }
            }
        "#;
        let parsed = parse_source(src);
        assert!(parsed.modules[0].docs[0].contains("Engine module"));
    }

    #[test]
    fn skips_non_annotated_functions() {
        let src = r#"
            #[hsrs::module]
            mod m {
                #[hsrs::data_type]
                pub struct T { x: i32 }
                impl T {
                    #[hsrs::function]
                    pub fn public_fn(&self) -> i32 { 0 }

                    fn private_helper(&self) -> i32 { 42 }
                }
            }
        "#;
        let parsed = parse_source(src);
        let fns = &parsed.modules[0].functions;
        let non_destructor: Vec<_> = fns.iter()
            .filter(|f| !matches!(f.kind, FfiFunctionKind::Destructor))
            .collect();
        assert_eq!(non_destructor.len(), 1);
        assert_eq!(non_destructor[0].rust_name, "public_fn");
    }

    #[test]
    fn void_method_has_no_return_type() {
        let src = r#"
            #[hsrs::module]
            mod m {
                #[hsrs::data_type]
                pub struct T { x: i32 }
                impl T {
                    #[hsrs::function]
                    pub fn reset(&mut self) { self.x = 0; }
                }
            }
        "#;
        let parsed = parse_source(src);
        assert!(parsed.modules[0].functions[0].return_type.is_none());
    }

    #[test]
    fn parses_module_struct_name() {
        let src = r#"
            #[hsrs::module]
            mod my_engine {
                #[hsrs::data_type]
                pub struct GameEngine { x: i32 }
                impl GameEngine {
                    #[hsrs::function]
                    pub fn new() -> Self { Self { x: 0 } }
                }
            }
        "#;
        let parsed = parse_source(src);
        assert_eq!(parsed.modules[0].name, "my_engine");
        assert_eq!(parsed.modules[0].struct_name, "GameEngine");
    }

    // ---- Error-path tests ----

    /// Helper: assert parse_str returns Err containing `needle`.
    fn assert_parse_err(src: &str, needle: &str) {
        let result = parse_str(src);
        match result {
            Ok(_) => panic!("expected parse error containing {needle:?}, but got Ok"),
            Err(e) => assert!(
                e.contains(needle),
                "expected error containing {needle:?}, got: {e:?}"
            ),
        }
    }

    #[test]
    fn err_invalid_rust_syntax() {
        assert_parse_err("fn foo( {", "failed to parse source");
    }

    #[test]
    fn err_enum_non_unit_variant() {
        let src = r#"
            #[hsrs::enumeration]
            pub enum Bad {
                Ok,
                WithData(i32),
            }
        "#;
        assert_parse_err(src, "non-unit variant WithData in Bad");
    }

    #[test]
    fn err_value_type_must_have_named_fields_tuple_struct() {
        let src = r#"
            #[hsrs::value_type]
            pub struct Bad(i32, u64);
        "#;
        assert_parse_err(src, "must have named fields");
    }

    #[test]
    fn err_value_type_must_have_named_fields_unit_struct() {
        let src = r#"
            #[hsrs::value_type]
            pub struct Bad;
        "#;
        assert_parse_err(src, "must have named fields");
    }

    #[test]
    fn err_value_type_unknown_field_type() {
        let src = r#"
            #[hsrs::value_type]
            pub struct Bad {
                pub x: HashMap,
            }
        "#;
        assert_parse_err(src, "unknown type: HashMap");
    }

    #[test]
    fn err_module_must_be_inline() {
        let src = r#"
            #[hsrs::module]
            mod foo;
        "#;
        assert_parse_err(src, "must be inline");
    }

    #[test]
    fn err_module_no_data_type() {
        let src = r#"
            #[hsrs::module]
            mod bad {
                pub struct NotAnnotated { x: i32 }
                impl NotAnnotated {
                    pub fn new() -> Self { Self { x: 0 } }
                }
            }
        "#;
        assert_parse_err(src, "no #[hsrs::data_type] in bad");
    }

    #[test]
    fn err_module_no_impl_block() {
        let src = r#"
            #[hsrs::module]
            mod bad {
                #[hsrs::data_type]
                pub struct MyType { x: i32 }
            }
        "#;
        assert_parse_err(src, "no impl for MyType");
    }

    #[test]
    fn err_unsupported_param_pattern() {
        let src = r#"
            #[hsrs::module]
            mod m {
                #[hsrs::data_type]
                pub struct T { x: i32 }
                impl T {
                    #[hsrs::function]
                    pub fn bad((a, b): (i32, i32)) -> Self { Self { x: a } }
                }
            }
        "#;
        assert_parse_err(src, "unsupported param pattern in bad");
    }

    #[test]
    fn err_unknown_type() {
        let src = r#"
            #[hsrs::module]
            mod m {
                #[hsrs::data_type]
                pub struct T { x: i32 }
                impl T {
                    #[hsrs::function]
                    pub fn get(&self) -> HashMap { todo!() }
                }
            }
        "#;
        assert_parse_err(src, "unknown type: HashMap");
    }

    #[test]
    fn err_result_wrong_arity() {
        let src = r#"
            #[hsrs::module]
            mod m {
                #[hsrs::data_type]
                pub struct T { x: i32 }
                impl T {
                    #[hsrs::function]
                    pub fn bad(&self) -> Result<i32> { Ok(0) }
                }
            }
        "#;
        assert_parse_err(src, "Result requires exactly 2 type arguments");
    }

    #[test]
    fn err_option_wrong_arity() {
        let src = r#"
            #[hsrs::module]
            mod m {
                #[hsrs::data_type]
                pub struct T { x: i32 }
                impl T {
                    #[hsrs::function]
                    pub fn bad(&self) -> Option<i32, u32> { None }
                }
            }
        "#;
        assert_parse_err(src, "Option requires exactly 1 type argument");
    }

    #[test]
    fn err_unsupported_generic_type() {
        let src = r#"
            #[hsrs::module]
            mod m {
                #[hsrs::data_type]
                pub struct T { x: i32 }
                impl T {
                    #[hsrs::function]
                    pub fn bad(&self) -> HashMap<i32, i32> { todo!() }
                }
            }
        "#;
        assert_parse_err(src, "unsupported generic type: HashMap");
    }

    #[test]
    fn err_qualified_types_not_supported() {
        let src = r#"
            #[hsrs::module]
            mod m {
                #[hsrs::data_type]
                pub struct T { x: i32 }
                impl T {
                    #[hsrs::function]
                    pub fn bad(&self) -> std::io::Error { todo!() }
                }
            }
        "#;
        assert_parse_err(src, "qualified types not supported");
    }

    #[test]
    fn err_unsupported_type_syntax_reference() {
        let src = r#"
            #[hsrs::module]
            mod m {
                #[hsrs::data_type]
                pub struct T { x: i32 }
                impl T {
                    #[hsrs::function]
                    pub fn bad(&self, x: &i32) -> i32 { 0 }
                }
            }
        "#;
        assert_parse_err(src, "unsupported type syntax");
    }

    #[test]
    fn err_unsupported_type_syntax_in_value_type_field() {
        let src = r#"
            #[hsrs::value_type]
            pub struct Bad {
                pub x: &'static i32,
            }
        "#;
        assert_parse_err(src, "unsupported type syntax");
    }

    #[test]
    fn resolves_string_type() {
        let parsed = parse_source(r#"
            #[hsrs::module]
            mod m {
                #[hsrs::data_type]
                pub struct S { x: i32 }
                impl S {
                    #[hsrs::function]
                    pub fn name(&self) -> String {}
                }
            }
        "#);
        let f = &parsed.modules[0].functions[0];
        assert!(matches!(f.return_type, Some(FfiType::String)));
        assert!(f.borsh_return);
    }

    #[test]
    fn string_param_is_borsh() {
        let parsed = parse_source(r#"
            #[hsrs::module]
            mod m {
                #[hsrs::data_type]
                pub struct S { x: i32 }
                impl S {
                    #[hsrs::function]
                    pub fn set_name(&mut self, name: String) {}
                }
            }
        "#);
        let f = &parsed.modules[0].functions[0];
        assert_eq!(f.borsh_params, vec!["name"]);
    }

    #[test]
    fn resolves_vec_type() {
        let parsed = parse_source(r#"
            #[hsrs::module]
            mod m {
                #[hsrs::data_type]
                pub struct S { x: i32 }
                impl S {
                    #[hsrs::function]
                    pub fn items(&self) -> Vec<i32> {}
                }
            }
        "#);
        let f = &parsed.modules[0].functions[0];
        assert!(matches!(f.return_type, Some(FfiType::Vec(_))));
        assert!(f.borsh_return);
    }

    #[test]
    fn vec_param_is_borsh() {
        let parsed = parse_source(r#"
            #[hsrs::module]
            mod m {
                #[hsrs::data_type]
                pub struct S { x: i32 }
                impl S {
                    #[hsrs::function]
                    pub fn add_items(&mut self, items: Vec<i32>) {}
                }
            }
        "#);
        let f = &parsed.modules[0].functions[0];
        assert_eq!(f.borsh_params, vec!["items"]);
    }

    #[test]
    fn vec_of_value_type() {
        let parsed = parse_source(r#"
            #[hsrs::value_type]
            pub struct Point { pub x: i32, pub y: i32 }

            #[hsrs::module(value_types(Point))]
            mod m {
                #[hsrs::data_type]
                pub struct S { x: i32 }
                impl S {
                    #[hsrs::function]
                    pub fn points(&self) -> Vec<Point> {}
                }
            }
        "#);
        let f = &parsed.modules[0].functions[0];
        match &f.return_type {
            Some(FfiType::Vec(inner)) => assert!(matches!(**inner, FfiType::ValueType(ref n) if n == "Point")),
            _ => panic!("expected Vec(ValueType(Point))"),
        }
    }

    #[test]
    fn error_on_vec_wrong_arity() {
        assert_parse_err(r#"
            #[hsrs::module]
            mod m {
                #[hsrs::data_type]
                pub struct S { x: i32 }
                impl S {
                    #[hsrs::function]
                    pub fn f(&self) -> Vec<i32, i32> {}
                }
            }
        "#, "Vec requires exactly 1 type argument");
    }

    #[test]
    fn parse_multi_merges_files() {
        let src_a = r#"
            #[hsrs::enumeration]
            pub enum Dir { Up, Down }
        "#;
        let src_b = r#"
            #[hsrs::value_type]
            pub struct Point { pub x: i32, pub y: i32 }
        "#;
        let parsed = parse_sources(&[src_a, src_b]).unwrap();
        assert_eq!(parsed.enums.len(), 1);
        assert_eq!(parsed.value_types.len(), 1);
    }

    #[test]
    fn parse_multi_module_sees_types_from_other_file() {
        let types_src = r#"
            #[hsrs::enumeration]
            pub enum Dir { Up, Down }

            #[hsrs::value_type]
            pub struct Point { pub x: i32, pub y: i32 }
        "#;
        let module_src = r#"
            #[hsrs::module(value_types(Point))]
            mod canvas {
                #[hsrs::data_type]
                pub struct Canvas { x: i32 }
                impl Canvas {
                    #[hsrs::function]
                    pub fn new() -> Self { Self { x: 0 } }
                    #[hsrs::function]
                    pub fn dir(&self) -> Dir {}
                    #[hsrs::function]
                    pub fn origin(&self) -> Point {}
                }
            }
        "#;
        let parsed = parse_sources(&[types_src, module_src]).unwrap();
        assert_eq!(parsed.enums.len(), 1);
        assert_eq!(parsed.value_types.len(), 1);
        assert_eq!(parsed.modules.len(), 1);
        let funcs = &parsed.modules[0].functions;
        // dir() returns Dir enum
        assert!(matches!(funcs[1].return_type, Some(FfiType::Enum(ref n)) if n == "Dir"));
        // origin() returns Point value type
        assert!(matches!(funcs[2].return_type, Some(FfiType::ValueType(ref n)) if n == "Point"));
    }
}

use crate::ir::{FfiEnum, FfiFunction, FfiFunctionKind, FfiModule, FfiType, ParsedFile};

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
    let mut derives = Vec::new();
    if e.has_eq {
        derives.push("Eq");
    }
    if e.has_show {
        derives.push("Show");
    }
    if e.has_ord {
        derives.push("Ord");
    }
    derives.push("Storable");

    out.push_str(&format!(
        "newtype {} = {} Word8\n  deriving ({})\n\n",
        e.name,
        e.name,
        derives.join(", ")
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

    for f in &m.functions {
        generate_foreign_import(out, f, &raw);
    }
    out.push('\n');

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
            let params = f
                .params
                .iter()
                .map(|p| format!("{} -> ", ffi_type(&p.ty)))
                .collect::<String>();
            out.push_str(&format!(
                "foreign import ccall \"{}\" c_{} :: {}IO (Ptr {})\n",
                f.c_name, hs, params, raw
            ));
        }
        FfiFunctionKind::MutMethod | FfiFunctionKind::RefMethod => {
            let params = f
                .params
                .iter()
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
            let sig_params = f
                .params
                .iter()
                .map(|p| format!("{} -> ", hl_type(&p.ty)))
                .collect::<String>();
            out.push_str(&format!(
                "\n{} :: {}IO {}\n",
                f.rust_name, sig_params, struct_name
            ));

            let pnames: Vec<_> = f.params.iter().map(|p| p.name.clone()).collect();
            let unwrapped = f
                .params
                .iter()
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
                    f.rust_name,
                    pnames.join(" "),
                    hs_c,
                    unwrapped,
                    free_hs,
                    struct_name
                ));
            }
        }
        FfiFunctionKind::MutMethod | FfiFunctionKind::RefMethod => {
            let sig_params = f
                .params
                .iter()
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
            let plist = if pnames.is_empty() {
                String::new()
            } else {
                format!(" {}", pnames.join(" "))
            };
            let unwrapped = if f.params.is_empty() {
                String::new()
            } else {
                format!(
                    " {}",
                    f.params
                        .iter()
                        .map(|p| unwrap_param(&p.name, &p.ty))
                        .collect::<Vec<_>>()
                        .join(" ")
                )
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
            format!("(let ({enum_name} {name}') = {name} in {name}')")
        }
        _ => name.to_owned(),
    }
}

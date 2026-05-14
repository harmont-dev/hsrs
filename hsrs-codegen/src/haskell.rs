use crate::ir::{
    FfiEnum, FfiFunction, FfiFunctionKind, FfiModule, FfiType, FfiValueType, ParsedFile,
};
use heck::{ToLowerCamelCase, ToUpperCamelCase};

pub fn generate(parsed: &ParsedFile) -> String {
    let mut out = String::new();

    out.push_str("{-# LANGUAGE PatternSynonyms #-}\n");
    out.push_str("{-# LANGUAGE GeneralizedNewtypeDeriving #-}\n");

    let has_value_types = !parsed.value_types.is_empty();
    let has_borsh_functions = parsed.modules.iter().any(|m| {
        m.functions
            .iter()
            .any(|f| f.borsh_return || !f.borsh_params.is_empty())
    });

    if has_value_types {
        out.push_str("{-# LANGUAGE DeriveGeneric #-}\n");
        out.push_str("{-# LANGUAGE DerivingVia #-}\n");
    }

    out.push_str("\nmodule Bindings where\n\n");
    out.push_str("import Foreign\n");
    out.push_str("import Foreign.C.Types\n");
    out.push_str("import Data.Int\n");
    out.push_str("import Data.Word\n");

    if has_value_types || has_borsh_functions {
        out.push_str("import GHC.Generics (Generic)\n");
        out.push_str("import Codec.Borsh\n");
    }
    if has_borsh_functions {
        out.push_str("import qualified Data.ByteString as BS\n");
        out.push_str("import Data.ByteString (useAsCStringLen)\n");
        out.push_str("import Data.ByteString.Unsafe (unsafePackCStringLen)\n");
    }

    out.push('\n');

    if has_borsh_functions {
        generate_borsh_utilities(&mut out);
    }

    for e in &parsed.enums {
        generate_enum(&mut out, e, has_borsh_functions || has_value_types);
    }
    for vt in &parsed.value_types {
        generate_value_type(&mut out, vt);
    }
    for m in &parsed.modules {
        generate_module(&mut out, m);
    }

    out
}

fn emit_haddock(out: &mut String, docs: &[String]) {
    for (i, line) in docs.iter().enumerate() {
        if i == 0 {
            out.push_str(&format!("-- |{}\n", line));
        } else {
            out.push_str(&format!("--{}\n", line));
        }
    }
}

fn generate_borsh_utilities(out: &mut String) {
    out.push_str("data BorshBufferRaw\n\n");
    out.push_str("foreign import ccall \"hsrs_borsh_len\" c_hsrsBorshLen :: Ptr BorshBufferRaw -> IO Word64\n");
    out.push_str("foreign import ccall \"hsrs_borsh_ptr\" c_hsrsBorshPtr :: Ptr BorshBufferRaw -> IO (Ptr Word8)\n");
    out.push_str("foreign import ccall \"&hsrs_borsh_free\" c_hsrsBorshFree :: FinalizerPtr BorshBufferRaw\n\n");
    out.push_str("fromBorshBuffer :: FromBorsh a => Ptr BorshBufferRaw -> IO a\n");
    out.push_str("fromBorshBuffer bufPtr = do\n");
    out.push_str("  fp <- newForeignPtr c_hsrsBorshFree bufPtr\n");
    out.push_str("  withForeignPtr fp $ \\p -> do\n");
    out.push_str("    len <- c_hsrsBorshLen p\n");
    out.push_str("    dataPtr <- c_hsrsBorshPtr p\n");
    out.push_str("    bs <- unsafePackCStringLen (castPtr dataPtr, fromIntegral len)\n");
    out.push_str("    case deserialiseBorsh bs of\n");
    out.push_str("      Left err -> error (show err)\n");
    out.push_str("      Right val -> pure val\n\n");
}

fn generate_enum(out: &mut String, e: &FfiEnum, with_borsh: bool) {
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

    emit_haddock(out, &e.docs);
    out.push_str(&format!(
        "newtype {} = {} Word8\n  deriving ({})\n",
        e.name,
        e.name,
        derives.join(", ")
    ));
    if with_borsh {
        out.push_str(&format!(
            "  deriving (BorshSize, ToBorsh, FromBorsh) via Word8\n"
        ));
    }
    out.push('\n');
    for (i, variant) in e.variants.iter().enumerate() {
        out.push_str(&format!("pattern {} :: {}\n", variant, e.name));
        out.push_str(&format!("pattern {} = {} {}\n\n", variant, e.name, i));
    }
}

fn generate_value_type(out: &mut String, vt: &FfiValueType) {
    emit_haddock(out, &vt.docs);

    let prefix = vt.name.to_lower_camel_case();

    out.push_str(&format!("data {} = {}\n", vt.name, vt.name));
    out.push_str("  { ");
    for (i, field) in vt.fields.iter().enumerate() {
        let hs_name = format!("{}{}", prefix, field.name.to_upper_camel_case());
        let hs_ty = hl_type(&field.ty);
        if i > 0 {
            out.push_str("  , ");
        }
        out.push_str(&format!("{} :: {}\n", hs_name, hs_ty));
    }
    out.push_str("  }");

    let mut derives = vec!["Generic"];
    if vt.has_eq {
        derives.push("Eq");
    }
    if vt.has_show {
        derives.push("Show");
    }
    if vt.has_ord {
        derives.push("Ord");
    }
    out.push_str(&format!(" deriving ({})\n", derives.join(", ")));
    out.push_str(&format!(
        "  deriving (BorshSize, ToBorsh, FromBorsh) via AsStruct {}\n\n",
        vt.name
    ));
}

fn generate_module(out: &mut String, m: &FfiModule) {
    let raw = format!("{}Raw", m.struct_name);

    emit_haddock(out, &m.docs);
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
    let hs = f.c_name.to_lower_camel_case();

    match f.kind {
        FfiFunctionKind::Destructor => {
            out.push_str(&format!(
                "foreign import ccall \"&{}\" c_{} :: FinalizerPtr {}\n",
                f.c_name, hs, raw
            ));
        }
        FfiFunctionKind::Constructor if f.borsh_return => {
            let params = ffi_param_types(f);
            out.push_str(&format!(
                "foreign import ccall \"{}\" c_{} :: {}IO (Ptr BorshBufferRaw)\n",
                f.c_name, hs, params
            ));
        }
        FfiFunctionKind::Constructor => {
            let params = ffi_param_types(f);
            out.push_str(&format!(
                "foreign import ccall \"{}\" c_{} :: {}IO (Ptr {})\n",
                f.c_name, hs, params, raw
            ));
        }
        FfiFunctionKind::MutMethod | FfiFunctionKind::RefMethod if f.borsh_return => {
            let params = ffi_param_types(f);
            out.push_str(&format!(
                "foreign import ccall \"{}\" c_{} :: Ptr {} -> {}IO (Ptr BorshBufferRaw)\n",
                f.c_name, hs, raw, params
            ));
        }
        FfiFunctionKind::MutMethod | FfiFunctionKind::RefMethod => {
            let params = ffi_param_types(f);
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

fn ffi_param_types(f: &FfiFunction) -> String {
    f.params
        .iter()
        .map(|p| {
            if f.borsh_params.contains(&p.name) {
                "Ptr Word8 -> Word64 -> ".to_owned()
            } else {
                format!("{} -> ", ffi_type(&p.ty))
            }
        })
        .collect::<String>()
}

fn generate_high_level(out: &mut String, f: &FfiFunction, struct_name: &str, mod_name: &str) {
    let hs_c = f.c_name.to_lower_camel_case();
    let free_hs = format!("{mod_name}_free").to_lower_camel_case();
    let hs_fn = f.rust_name.to_lower_camel_case();

    match f.kind {
        FfiFunctionKind::Constructor if f.borsh_return => {
            let sig_params = f
                .params
                .iter()
                .map(|p| format!("{} -> ", hl_type(&p.ty)))
                .collect::<String>();
            let ret = hl_type(f.return_type.as_ref().expect("borsh_return implies return type"));

            out.push('\n');
            emit_haddock(out, &f.docs);
            out.push_str(&format!("{} :: {}IO {}\n", hs_fn, sig_params, ret));

            let pnames: Vec<_> = f.params.iter().map(|p| p.name.to_lower_camel_case()).collect();
            let plist = if pnames.is_empty() {
                String::new()
            } else {
                format!(" {}", pnames.join(" "))
            };

            out.push_str(&format!("{}{} =\n", hs_fn, plist));
            let call = format!("c_{}", hs_c);
            let full_call =
                build_borsh_call(f, &call, None);
            out.push_str(&format!("  fromBorshBuffer =<< {}\n", full_call));
        }
        FfiFunctionKind::Constructor => {
            let sig_params = f
                .params
                .iter()
                .map(|p| format!("{} -> ", hl_type(&p.ty)))
                .collect::<String>();
            out.push('\n');
            emit_haddock(out, &f.docs);
            out.push_str(&format!(
                "{} :: {}IO {}\n",
                hs_fn, sig_params, struct_name
            ));

            let pnames: Vec<_> = f.params.iter().map(|p| p.name.to_lower_camel_case()).collect();
            let unwrapped = f
                .params
                .iter()
                .map(|p| unwrap_param(&p.name.to_lower_camel_case(), &p.ty))
                .collect::<Vec<_>>()
                .join(" ");

            if pnames.is_empty() {
                out.push_str(&format!(
                    "{} = do\n  ptr <- c_{}\n  fp <- newForeignPtr c_{} ptr\n  pure ({} fp)\n",
                    hs_fn, hs_c, free_hs, struct_name
                ));
            } else {
                out.push_str(&format!(
                    "{} {} = do\n  ptr <- c_{} {}\n  fp <- newForeignPtr c_{} ptr\n  pure ({} fp)\n",
                    hs_fn,
                    pnames.join(" "),
                    hs_c,
                    unwrapped,
                    free_hs,
                    struct_name
                ));
            }
        }
        FfiFunctionKind::MutMethod | FfiFunctionKind::RefMethod if f.borsh_return => {
            let sig_params = f
                .params
                .iter()
                .map(|p| format!("{} -> ", hl_type(&p.ty)))
                .collect::<String>();
            let ret = hl_type(f.return_type.as_ref().expect("borsh_return implies return type"));

            out.push('\n');
            emit_haddock(out, &f.docs);
            out.push_str(&format!(
                "{} :: {} -> {}IO {}\n",
                hs_fn, struct_name, sig_params, ret
            ));

            let pnames: Vec<_> = f.params.iter().map(|p| p.name.to_lower_camel_case()).collect();
            let plist = if pnames.is_empty() {
                String::new()
            } else {
                format!(" {}", pnames.join(" "))
            };

            out.push_str(&format!(
                "{} ({} fp){} = withForeignPtr fp $ \\ptr ->\n",
                hs_fn, struct_name, plist
            ));
            let call = format!("c_{}", hs_c);
            let full_call = build_borsh_call(f, &call, Some("ptr"));
            out.push_str(&format!("  fromBorshBuffer =<< {}\n", full_call));
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
            out.push('\n');
            emit_haddock(out, &f.docs);
            out.push_str(&format!(
                "{} :: {} -> {}{}\n",
                hs_fn, struct_name, sig_params, ret
            ));

            let pnames: Vec<_> = f.params.iter().map(|p| p.name.to_lower_camel_case()).collect();
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
                        .map(|p| unwrap_param(&p.name.to_lower_camel_case(), &p.ty))
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            };

            out.push_str(&format!(
                "{} ({} fp){} = withForeignPtr fp $ \\ptr -> c_{} ptr{}\n",
                hs_fn, struct_name, plist, hs_c, unwrapped
            ));
        }
        FfiFunctionKind::Destructor => {}
    }
}

fn build_borsh_call(f: &FfiFunction, c_func: &str, self_arg: Option<&str>) -> String {
    let mut args = Vec::new();
    if let Some(s) = self_arg {
        args.push(s.to_owned());
    }

    let mut borsh_wraps: Vec<(String, String, String)> = Vec::new();

    for p in &f.params {
        let hs = p.name.to_lower_camel_case();
        if f.borsh_params.contains(&p.name) {
            let ptr_var = format!("{}Ptr", hs);
            let len_var = format!("{}Len", hs);
            args.push(format!("(castPtr {})", ptr_var));
            args.push(format!("(fromIntegral {})", len_var));
            borsh_wraps.push((
                hs,
                ptr_var,
                len_var,
            ));
        } else {
            args.push(unwrap_param(&hs, &p.ty));
        }
    }

    let call = format!("{} {}", c_func, args.join(" "));

    let mut result = call;
    for (name, ptr_var, len_var) in borsh_wraps.iter().rev() {
        result = format!(
            "useAsCStringLen (serialiseBorsh {}) $ \\({}, {}) -> {}",
            name, ptr_var, len_var, result
        );
    }
    result
}

fn ffi_type(ty: &FfiType) -> String {
    match ty {
        FfiType::Int(8) => "Int8".to_owned(),
        FfiType::Int(16) => "Int16".to_owned(),
        FfiType::Int(32) => "Int32".to_owned(),
        FfiType::Int(64) => "Int64".to_owned(),
        FfiType::Uint(8) => "Word8".to_owned(),
        FfiType::Uint(16) => "Word16".to_owned(),
        FfiType::Uint(32) => "Word32".to_owned(),
        FfiType::Uint(64) => "Word64".to_owned(),
        FfiType::Bool => "CBool".to_owned(),
        FfiType::Usize => "Word64".to_owned(),
        FfiType::Isize => "Int64".to_owned(),
        FfiType::Enum(_) => "Word8".to_owned(),
        FfiType::Unit => "()".to_owned(),
        FfiType::ValueType(name) => name.clone(),
        FfiType::Result(_, _) | FfiType::Option(_) => "()".to_owned(),
        _ => "()".to_owned(),
    }
}

fn hl_type(ty: &FfiType) -> String {
    match ty {
        FfiType::Enum(name) | FfiType::ValueType(name) => name.clone(),
        FfiType::Result(ok, err) => format!("(Either {} {})", hl_type(err), hl_type(ok)),
        FfiType::Option(inner) => format!("(Maybe {})", hl_type(inner)),
        other => ffi_type(other),
    }
}

fn unwrap_param(name: &str, ty: &FfiType) -> String {
    match ty {
        FfiType::Enum(enum_name) => {
            format!("(let ({enum_name} {name}') = {name} in {name}')")
        }
        _ => name.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        FfiEnum, FfiField, FfiFunction, FfiFunctionKind, FfiModule, FfiParam, FfiType,
        FfiValueType, ParsedFile,
    };

    fn make_module_with_snake_case_fn() -> ParsedFile {
        ParsedFile {
            enums: vec![FfiEnum {
                name: "Direction".to_owned(),
                variants: vec!["North".to_owned(), "South".to_owned()],
                has_eq: true,
                has_show: true,
                has_ord: false,
                docs: vec![],
            }],
            modules: vec![FfiModule {
                name: "my_engine".to_owned(),
                struct_name: "MyEngine".to_owned(),
                functions: vec![
                    FfiFunction {
                        rust_name: "new".to_owned(),
                        c_name: "my_engine_new".to_owned(),
                        kind: FfiFunctionKind::Constructor,
                        params: vec![],
                        return_type: None,
                        docs: vec![],
                        borsh_return: false,
                        borsh_params: vec![],
                    },
                    FfiFunction {
                        rust_name: "get_state".to_owned(),
                        c_name: "my_engine_get_state".to_owned(),
                        kind: FfiFunctionKind::RefMethod,
                        params: vec![],
                        return_type: Some(FfiType::ValueType("GameState".to_owned())),
                        docs: vec![],
                        borsh_return: true,
                        borsh_params: vec![],
                    },
                    FfiFunction {
                        rust_name: "set_state".to_owned(),
                        c_name: "my_engine_set_state".to_owned(),
                        kind: FfiFunctionKind::MutMethod,
                        params: vec![FfiParam {
                            name: "new_state".to_owned(),
                            ty: FfiType::ValueType("GameState".to_owned()),
                        }],
                        return_type: Some(FfiType::ValueType("GameState".to_owned())),
                        docs: vec![],
                        borsh_return: true,
                        borsh_params: vec!["new_state".to_owned()],
                    },
                    FfiFunction {
                        rust_name: "free".to_owned(),
                        c_name: "my_engine_free".to_owned(),
                        kind: FfiFunctionKind::Destructor,
                        params: vec![],
                        return_type: None,
                        docs: vec![],
                        borsh_return: false,
                        borsh_params: vec![],
                    },
                    FfiFunction {
                        rust_name: "get_value".to_owned(),
                        c_name: "my_engine_get_value".to_owned(),
                        kind: FfiFunctionKind::RefMethod,
                        params: vec![],
                        return_type: Some(FfiType::Int(64)),
                        docs: vec![],
                        borsh_return: false,
                        borsh_params: vec![],
                    },
                    FfiFunction {
                        rust_name: "set_direction".to_owned(),
                        c_name: "my_engine_set_direction".to_owned(),
                        kind: FfiFunctionKind::MutMethod,
                        params: vec![FfiParam {
                            name: "new_dir".to_owned(),
                            ty: FfiType::Enum("Direction".to_owned()),
                        }],
                        return_type: None,
                        docs: vec![],
                        borsh_return: false,
                        borsh_params: vec![],
                    },
                ],
                docs: vec![],
            }],
            value_types: vec![FfiValueType {
                name: "GameState".to_owned(),
                fields: vec![FfiField {
                    name: "score".to_owned(),
                    ty: FfiType::Uint(32),
                }],
                has_eq: false,
                has_show: false,
                has_ord: false,
                docs: vec![],
            }],
        }
    }

    #[test]
    fn function_names_are_camel_case() {
        let output = generate(&make_module_with_snake_case_fn());
        assert!(
            !output.contains("\nget_value "),
            "output should not contain snake_case function name 'get_value'"
        );
        assert!(
            output.contains("\ngetValue "),
            "output should contain camelCase function name 'getValue'"
        );
        assert!(
            !output.contains("\nset_direction "),
            "output should not contain snake_case function name 'set_direction'"
        );
        assert!(
            output.contains("\nsetDirection "),
            "output should contain camelCase function name 'setDirection'"
        );
    }

    #[test]
    fn param_names_are_camel_case() {
        let output = generate(&make_module_with_snake_case_fn());
        assert!(
            !output.contains("new_dir"),
            "output should not contain snake_case param name 'new_dir'"
        );
        assert!(
            output.contains("newDir"),
            "output should contain camelCase param name 'newDir'"
        );
    }

    #[test]
    fn borsh_param_names_are_camel_case() {
        let output = generate(&make_module_with_snake_case_fn());
        assert!(output.contains("newStatePtr"), "borsh ptr var should be camelCase");
        assert!(output.contains("newStateLen"), "borsh len var should be camelCase");
        assert!(!output.contains("new_state_ptr"), "borsh ptr var should not be snake_case");
        assert!(output.contains("\ngetState "), "borsh function name should be camelCase");
    }

    #[test]
    fn foreign_import_c_symbols_unchanged() {
        let output = generate(&make_module_with_snake_case_fn());
        assert!(
            output.contains("\"my_engine_get_value\""),
            "C symbol 'my_engine_get_value' should be present in foreign import"
        );
        assert!(
            output.contains("\"my_engine_set_direction\""),
            "C symbol 'my_engine_set_direction' should be present in foreign import"
        );
    }
}

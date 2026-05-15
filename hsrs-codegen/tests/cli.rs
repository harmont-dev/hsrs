#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_docs_in_private_items,
    clippy::exhaustive_enums,
    clippy::exhaustive_structs,
    clippy::indexing_slicing,
    clippy::shadow_reuse,
    clippy::shadow_same,
    clippy::shadow_unrelated
)]

use std::process::Command;

fn hsrs_codegen() -> Command {
    Command::new(env!("CARGO_BIN_EXE_hsrs-codegen"))
}

#[test]
fn no_args_prints_usage_and_exits_1() {
    let output = hsrs_codegen().output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Usage:"));
}

#[test]
fn missing_file_prints_error() {
    let output = hsrs_codegen().arg("nonexistent.rs").output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Error:"));
}

#[test]
fn stdout_output_generates_haskell() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("test.rs");
    std::fs::write(
        &src,
        r#"
        #[hsrs::enumeration]
        pub enum Dir { Up, Down }
    "#,
    )
    .unwrap();

    let output = hsrs_codegen().arg(src.to_str().unwrap()).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("newtype Dir"));
}

#[test]
fn file_output_writes_haskell() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("test.rs");
    let out = dir.path().join("Out.hs");
    std::fs::write(
        &src,
        r#"
        #[hsrs::enumeration]
        pub enum Dir { Up, Down }
    "#,
    )
    .unwrap();

    let status = hsrs_codegen()
        .arg(src.to_str().unwrap())
        .arg("-o")
        .arg(out.to_str().unwrap())
        .output()
        .unwrap();
    assert!(status.status.success());
    let contents = std::fs::read_to_string(&out).unwrap();
    assert!(contents.contains("newtype Dir"));
}

#[test]
fn module_flag_sets_module_name() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("test.rs");
    std::fs::write(
        &src,
        r#"
        #[hsrs::enumeration]
        pub enum Dir { Up, Down }
    "#,
    )
    .unwrap();

    let output = hsrs_codegen()
        .arg(src.to_str().unwrap())
        .arg("--module")
        .arg("MyApp.FFI.Gen")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("module MyApp.FFI.Gen where"), "got: {stdout}");
}

#[test]
fn multi_file_merges_types() {
    let dir = tempfile::tempdir().unwrap();
    let types = dir.path().join("types.rs");
    std::fs::write(
        &types,
        r#"
        #[hsrs::enumeration]
        pub enum Dir { Up, Down }
    "#,
    )
    .unwrap();
    let module = dir.path().join("module.rs");
    std::fs::write(
        &module,
        r#"
        #[hsrs::module]
        mod canvas {
            #[hsrs::data_type]
            pub struct Canvas { x: i32 }
            impl Canvas {
                #[hsrs::function]
                pub fn dir(&self) -> Dir {}
            }
        }
    "#,
    )
    .unwrap();

    let output =
        hsrs_codegen().arg(types.to_str().unwrap()).arg(module.to_str().unwrap()).output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("newtype Dir"));
    assert!(stdout.contains("newtype Canvas"));
}

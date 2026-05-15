#![allow(
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::dbg_macro,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::wildcard_imports,
    clippy::implicit_clone,
    clippy::str_to_string,
    clippy::clone_on_ref_ptr,
    clippy::exhaustive_enums,
    clippy::exhaustive_structs,
    clippy::missing_docs_in_private_items,
    clippy::unwrap_in_result,
    clippy::indexing_slicing,
    clippy::float_arithmetic,
    clippy::arithmetic_side_effects,
    clippy::shadow_reuse,
    clippy::shadow_same,
    clippy::shadow_unrelated
)]

use std::path::{Path, PathBuf};

use hsrs_codegen::{haskell, parser};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut source_files: Vec<String> = Vec::new();
    let mut output_file = None;
    let mut module_name = "Bindings".to_owned();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--module" | "-m" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("--module requires a value");
                    std::process::exit(1);
                }
                module_name = args[i].clone();
                if module_name.is_empty() {
                    eprintln!("--module value must not be empty");
                    std::process::exit(1);
                }
            },
            "-o" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("-o requires a value");
                    std::process::exit(1);
                }
                output_file = Some(args[i].clone());
            },
            arg if !arg.starts_with('-') => {
                source_files.push(arg.to_owned());
            },
            other => {
                eprintln!("unknown flag: {other}");
                std::process::exit(1);
            },
        }
        i += 1;
    }

    if source_files.is_empty() {
        eprintln!("Usage: hsrs-codegen <source.rs>... [-o output.hs] [--module Name]");
        std::process::exit(1);
    }

    let paths: Vec<PathBuf> = source_files.iter().map(PathBuf::from).collect();
    let path_refs: Vec<&Path> = paths.iter().map(PathBuf::as_path).collect();
    let parsed = match parser::parse_files(&path_refs) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        },
    };

    let output = haskell::generate(&parsed, &module_name);

    if let Some(out_path) = output_file {
        std::fs::write(&out_path, &output).unwrap_or_else(|e| {
            eprintln!("Failed to write: {e}");
            std::process::exit(1);
        });
    } else {
        print!("{output}");
    }
}

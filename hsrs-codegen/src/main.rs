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
    clippy::shadow_unrelated,
)]

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

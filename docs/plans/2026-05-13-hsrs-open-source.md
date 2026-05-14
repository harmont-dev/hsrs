# hsrs Open-Source Release

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Prepare hsrs for open-source release with proper git setup, licensing, documentation, tests, and CI.

**Architecture:** Single root git repo (replacing per-subcrate `.git` dirs), dual MIT/Apache-2.0 licensing, insta snapshot tests for codegen, trybuild compile tests for proc macros, GitHub Actions CI.

**Tech Stack:** trybuild (proc-macro compile tests), insta (snapshot testing), GitHub Actions (CI)

---

## Context

The hsrs workspace has three crates (`hsrs`, `hsrs-codegen`, `hsrs-examples`) with working code but no git repo at the root, no license, no README, no tests, and no CI. Each subcrate has a stale `.git` from `cargo init` that must be removed. The workspace uses edition 2024 (MSRV: Rust 1.85.0) and strict clippy lints.

---

## Task 1: Git Setup and .gitignore

**Files:**
- Delete: `hsrs/.git/`, `hsrs-codegen/.git/`, `hsrs-examples/.git/` (stale init dirs)
- Delete: `hsrs/.gitignore`, `hsrs-codegen/.gitignore`, `hsrs-examples/.gitignore` (per-subcrate)
- Create: `/Users/marko/Desktop/hsrs/.gitignore`

**Step 1: Remove stale per-subcrate git dirs and gitignore files**

```bash
rm -rf hsrs/.git hsrs-codegen/.git hsrs-examples/.git
rm hsrs/.gitignore hsrs-codegen/.gitignore hsrs-examples/.gitignore
```

**Step 2: Create root .gitignore**

```gitignore
# Build
/target/

# IDE
.idea/
.vscode/
*.iml
*.swp
*.swo
*~

# OS
.DS_Store
Thumbs.db

# Environment
.env
```

Note: Track `Cargo.lock` (we ship a binary). Do NOT gitignore `*.hs` — generated Haskell in examples is useful as reference.

**Step 3: Initialize root git repo**

```bash
git init
```

**Step 4: Verify**

Run: `git status`
Expected: All files show as untracked, no nested repos.

---

## Task 2: Licenses and Cargo Metadata

**Files:**
- Create: `/Users/marko/Desktop/hsrs/LICENSE-MIT`
- Create: `/Users/marko/Desktop/hsrs/LICENSE-APACHE`
- Modify: `/Users/marko/Desktop/hsrs/Cargo.toml`
- Modify: `/Users/marko/Desktop/hsrs/hsrs/Cargo.toml`
- Modify: `/Users/marko/Desktop/hsrs/hsrs-codegen/Cargo.toml`
- Modify: `/Users/marko/Desktop/hsrs/hsrs-examples/Cargo.toml`

**Step 1: Create LICENSE-MIT**

Standard MIT license text with `Copyright (c) 2026 hsrs contributors`.

**Step 2: Create LICENSE-APACHE**

Standard Apache 2.0 license text.

**Step 3: Add workspace-level package metadata to root Cargo.toml**

Add to `[workspace]` section:
```toml
[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.85.0"
license = "MIT OR Apache-2.0"
repository = "https://github.com/USER/hsrs"
```

**Step 4: Update each crate's Cargo.toml to inherit workspace metadata**

Each crate gets:
```toml
[package]
name = "crate-name"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
description = "..."
```

Descriptions:
- `hsrs`: "Proc macros for generating Haskell FFI bindings from Rust"
- `hsrs-codegen`: "Haskell code generator for hsrs FFI bindings"
- `hsrs-examples`: "Example usage of the hsrs bindings generator"

Also add `publish = false` to `hsrs-examples` (not meant for crates.io).

**Step 5: Verify**

Run: `cargo check --workspace`

---

## Task 3: README.md

**Files:**
- Create: `/Users/marko/Desktop/hsrs/README.md`

**Step 1: Write README.md**

Structure:
1. **Title + one-liner** — "Generate type-safe Haskell FFI bindings from annotated Rust code."
2. **Overview** — 2-3 sentences: what it does, how (safer-ffi + codegen), what you get
3. **Example** — Two code blocks side by side:
   - Rust input (the annotated `hsrs-examples/src/lib.rs`)
   - Generated Haskell output (the `Bindings.hs`)
4. **Usage** — Steps: add deps, annotate code, compile, run codegen
5. **Attributes** — Table of `#[hsrs::enumeration]`, `#[hsrs::module]`, `#[hsrs::data_type]`, `#[hsrs::function]` with descriptions
6. **Type Mapping** — Table: Rust → C (safer-ffi) → Haskell FFI → Haskell API
7. **How It Works** — Brief: proc macros add safer-ffi annotations for C FFI, codegen reads original source and emits Haskell
8. **Limitations** — Edition 2024, safer-ffi RC, constructor heuristic, unit-variant enums only
9. **License** — MIT OR Apache-2.0

Use the actual example code and generated Haskell from `hsrs-examples/`.

**Step 2: Verify**

Eyeball the README. Ensure code blocks are correct.

---

## Task 4: hsrs-codegen Tests (insta snapshots)

**Files:**
- Modify: `/Users/marko/Desktop/hsrs/hsrs-codegen/Cargo.toml` (add insta dev-dep)
- Modify: `/Users/marko/Desktop/hsrs/hsrs-codegen/src/parser.rs` (extract `parse_source`)
- Modify: `/Users/marko/Desktop/hsrs/hsrs-codegen/src/haskell.rs` (add tests)
- Modify: `/Users/marko/Desktop/hsrs/hsrs-codegen/src/parser.rs` (add tests)

**Step 1: Add insta dev-dependency**

In `hsrs-codegen/Cargo.toml`:
```toml
[dev-dependencies]
insta = "1"
```

**Step 2: Refactor parser — extract `parse_source`**

Split `parse_file` into `parse_file` (reads file + calls `parse_source`) and `parse_source` (parses string):

```rust
pub fn parse_file(path: &Path) -> Result<ParsedFile, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    parse_source(&source)
}

pub fn parse_source(source: &str) -> Result<ParsedFile, String> {
    let file = syn::parse_file(source)
        .map_err(|e| format!("failed to parse: {e}"))?;
    // ... rest of existing parse logic (loop over file.items, etc.)
}
```

**Step 3: Add parser unit tests**

In `parser.rs`, add `#[cfg(test)] mod tests`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_enum() {
        let source = r#"
            #[hsrs::enumeration]
            pub enum Color { Red, Green, Blue }
        "#;
        let parsed = parse_source(source).unwrap();
        assert_eq!(parsed.enums.len(), 1);
        assert_eq!(parsed.enums[0].name, "Color");
        assert_eq!(parsed.enums[0].variants, vec!["Red", "Green", "Blue"]);
    }

    #[test]
    fn parse_full_example() {
        let source = include_str!("../../hsrs-examples/src/lib.rs");
        let parsed = parse_source(source).unwrap();
        assert_eq!(parsed.enums.len(), 1);
        assert_eq!(parsed.modules.len(), 1);
        assert_eq!(parsed.modules[0].struct_name, "QuectoVm");
        // 7 annotated functions + 1 auto-generated destructor
        assert_eq!(parsed.modules[0].functions.len(), 8);
    }

    #[test]
    fn parse_enum_non_unit_fails() {
        let source = r#"
            #[hsrs::enumeration]
            pub enum Bad { A(u8) }
        "#;
        assert!(parse_source(source).is_err());
    }
}
```

**Step 4: Run parser tests**

Run: `cargo test -p hsrs-codegen -- parser`
Expected: All pass.

**Step 5: Add haskell generator snapshot tests**

In `haskell.rs`, add `#[cfg(test)] mod tests`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_source;

    #[test]
    fn snapshot_full_example() {
        let source = include_str!("../../hsrs-examples/src/lib.rs");
        let parsed = parse_source(source).unwrap();
        let output = generate(&parsed);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn snapshot_enum_only() {
        let source = r#"
            #[hsrs::enumeration]
            pub enum Direction { North, South, East, West }
        "#;
        let parsed = parse_source(source).unwrap();
        let output = generate(&parsed);
        insta::assert_snapshot!(output);
    }

    #[test]
    fn snapshot_constructor_only() {
        let source = r#"
            #[hsrs::enumeration]
            pub enum Unit { A }

            #[hsrs::module]
            mod counter {
                #[hsrs::data_type]
                pub struct Counter { value: i64 }

                impl Counter {
                    #[hsrs::function]
                    pub fn new() -> Self {
                        Self { value: 0 }
                    }
                }
            }
        "#;
        let parsed = parse_source(source).unwrap();
        let output = generate(&parsed);
        insta::assert_snapshot!(output);
    }
}
```

**Step 6: Run tests and accept snapshots**

```bash
cargo test -p hsrs-codegen
cargo insta review
```

Expected: Tests fail on first run (no snapshots yet), `insta review` shows generated snapshots, accept them.

**Step 7: Run again to verify**

Run: `cargo test -p hsrs-codegen`
Expected: All pass.

**Step 8: Commit**

```bash
git add hsrs-codegen/
git commit -m "test: add insta snapshot tests for hsrs-codegen"
```

---

## Task 5: hsrs Proc-Macro Tests (trybuild)

**Files:**
- Modify: `/Users/marko/Desktop/hsrs/hsrs-examples/Cargo.toml` (add trybuild dev-dep)
- Create: `/Users/marko/Desktop/hsrs/hsrs-examples/tests/ui.rs`
- Create: `/Users/marko/Desktop/hsrs/hsrs-examples/tests/ui/pass/enum_basic.rs`
- Create: `/Users/marko/Desktop/hsrs/hsrs-examples/tests/ui/pass/module_basic.rs`
- Create: `/Users/marko/Desktop/hsrs/hsrs-examples/tests/ui/fail/data_type_standalone.rs`
- Create: `/Users/marko/Desktop/hsrs/hsrs-examples/tests/ui/fail/function_standalone.rs`
- Create: `/Users/marko/Desktop/hsrs/hsrs-examples/tests/ui/fail/enum_non_unit.rs`

**Step 1: Add trybuild dev-dependency**

In `hsrs-examples/Cargo.toml`:
```toml
[dev-dependencies]
trybuild = "1"
```

**Step 2: Create test runner**

`hsrs-examples/tests/ui.rs`:
```rust
#[test]
fn compile_tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/pass/*.rs");
    t.compile_fail("tests/ui/fail/*.rs");
}
```

**Step 3: Create pass test — basic enum**

`tests/ui/pass/enum_basic.rs`:
```rust
use hsrs::enumeration;

#[enumeration]
pub enum Color {
    Red,
    Green,
    Blue,
}

fn main() {
    let _c = Color::Red;
}
```

Note: This test uses `hsrs::enumeration` which generates `#[derive_ReprC]`, requiring safer-ffi. The `hsrs-examples` crate already depends on both.

**Step 4: Create pass test — full module**

`tests/ui/pass/module_basic.rs`:
```rust
use hsrs::{enumeration, module};

#[enumeration]
pub enum Reg {
    A,
    B,
}

#[module]
mod my_machine {
    #[hsrs::data_type]
    pub struct MyMachine {
        val: i64,
    }

    impl MyMachine {
        #[hsrs::function]
        pub fn new() -> Self {
            Self { val: 0 }
        }

        #[hsrs::function]
        pub fn get(&self) -> i64 {
            self.val
        }

        #[hsrs::function]
        pub fn set(&mut self, v: i64) {
            self.val = v;
        }
    }
}

fn main() {}
```

**Step 5: Create fail test — data_type outside module**

`tests/ui/fail/data_type_standalone.rs`:
```rust
#[hsrs::data_type]
pub struct Foo {
    x: i32,
}

fn main() {}
```

**Step 6: Create fail test — function outside module**

`tests/ui/fail/function_standalone.rs`:
```rust
struct Bar;

impl Bar {
    #[hsrs::function]
    pub fn do_thing(&self) {}
}

fn main() {}
```

**Step 7: Create fail test — enum with data variant**

`tests/ui/fail/enum_non_unit.rs`:
```rust
#[hsrs::enumeration]
pub enum Bad {
    Ok,
    WithData(u8),
}

fn main() {}
```

**Step 8: Run tests and capture stderr**

```bash
TRYBUILD=overwrite cargo test -p hsrs-examples -- compile_tests
```

This creates `.stderr` files for each fail test. Inspect them to ensure error messages are correct.

**Step 9: Run again to verify**

Run: `cargo test -p hsrs-examples -- compile_tests`
Expected: All pass (stderr files match).

**Step 10: Commit**

```bash
git add hsrs-examples/
git commit -m "test: add trybuild compile tests for hsrs proc macros"
```

---

## Task 6: GitHub Actions CI

**Files:**
- Create: `/Users/marko/Desktop/hsrs/.github/workflows/ci.yml`

**Step 1: Write CI workflow**

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

env:
  CARGO_TERM_COLOR: always

jobs:
  fmt:
    name: Format
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
        with:
          components: rustfmt
      - run: cargo fmt --all -- --check

  clippy:
    name: Clippy
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo clippy --workspace --all-targets

  test:
    name: Test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --workspace

  msrv:
    name: MSRV (1.85.0)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@1.85.0
      - uses: Swatinem/rust-cache@v2
      - run: cargo check --workspace
```

Notes:
- `rustfmt` uses nightly (needed for unstable format options in `rustfmt.toml`)
- `clippy` does NOT add `-- -D warnings` — workspace Cargo.toml already denies everything
- MSRV 1.85.0 = first stable Rust with edition 2024

**Step 2: Verify yaml syntax**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"`
(If python3/yaml available, otherwise just eyeball it.)

---

## Task 7: Final Cleanup and Initial Commit

**Files:**
- Delete: `/Users/marko/Desktop/hsrs/docs/plans/` (internal planning docs, not for open-source)
- Create: `/Users/marko/Desktop/hsrs/CONTRIBUTING.md`

**Step 1: Create CONTRIBUTING.md**

Brief contributing guide:
- How to build (`cargo build --workspace`)
- How to run tests (`cargo test --workspace`)
- How to run codegen example (`cargo run -p hsrs-codegen -- hsrs-examples/src/lib.rs`)
- Link to issues page
- Note about dual license

**Step 2: Remove internal planning docs**

```bash
rm -rf docs/
```

**Step 3: Verify everything**

```bash
cargo fmt --all -- --check
cargo clippy --workspace
cargo test --workspace
cargo run -p hsrs-codegen -- hsrs-examples/src/lib.rs
```

All must pass clean.

**Step 4: Initial commit**

```bash
git add -A
git commit -m "feat: initial release of hsrs — Rust-to-Haskell FFI bindings generator"
```

---

## Risks

1. **trybuild + safer-ffi interaction**: Pass tests generate `#[derive_ReprC]` and `extern "C"` code. Trybuild compiles these as separate crates outside the workspace, so workspace `forbid(unsafe_code)` shouldn't apply. If it does, add `#![allow(unsafe_code)]` to pass test files.

2. **insta version compatibility**: Use `insta = "1"` (stable). Snapshot format is stable across minor versions.

3. **Nightly rustfmt in CI**: If `rustfmt.toml` has unstable options, CI needs nightly for the fmt job. If no unstable options are used, stable suffices.

4. **MSRV 1.85.0**: Edition 2024 requires Rust 1.85.0+. This is relatively recent — some users on older toolchains won't be able to use hsrs.

---

## File Summary

| File | Action |
|------|--------|
| `hsrs/.git/`, `hsrs-codegen/.git/`, `hsrs-examples/.git/` | Delete |
| `hsrs/.gitignore`, `hsrs-codegen/.gitignore`, `hsrs-examples/.gitignore` | Delete |
| `.gitignore` | Create (root) |
| `LICENSE-MIT` | Create |
| `LICENSE-APACHE` | Create |
| `README.md` | Create |
| `CONTRIBUTING.md` | Create |
| `.github/workflows/ci.yml` | Create |
| `Cargo.toml` | Modify (workspace metadata) |
| `hsrs/Cargo.toml` | Modify (inherit metadata) |
| `hsrs-codegen/Cargo.toml` | Modify (inherit metadata, add insta) |
| `hsrs-examples/Cargo.toml` | Modify (inherit metadata, add trybuild) |
| `hsrs-codegen/src/parser.rs` | Modify (extract `parse_source`) |
| `hsrs-codegen/src/haskell.rs` | Modify (add snapshot tests) |
| `hsrs-codegen/src/parser.rs` | Modify (add parser tests) |
| `hsrs-examples/tests/ui.rs` | Create |
| `hsrs-examples/tests/ui/pass/enum_basic.rs` | Create |
| `hsrs-examples/tests/ui/pass/module_basic.rs` | Create |
| `hsrs-examples/tests/ui/fail/data_type_standalone.rs` | Create |
| `hsrs-examples/tests/ui/fail/function_standalone.rs` | Create |
| `hsrs-examples/tests/ui/fail/enum_non_unit.rs` | Create |
| `docs/plans/` | Delete |

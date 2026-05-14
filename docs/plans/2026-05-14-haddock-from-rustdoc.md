# Haddock Docs from Rust Doc Comments

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extract `///` doc comments from Rust source and emit Haddock comments (`-- |`) in the generated Haskell bindings.

**Architecture:** Three-layer change: parser extracts doc attributes from syn, IR carries doc strings, Haskell codegen emits Haddock before declarations.

**Tech Stack:** syn 2 (`#[doc = "..."]` attribute extraction), Haddock comment syntax

---

## Context

In syn, `/// This is a doc` is stored as `#[doc = " This is a doc"]`. The parser already has access to all attributes on enums, structs, functions, and modules. We extract the `doc` attributes, store them in the IR, and emit them as Haddock comments.

### Haddock Format

```haskell
-- | First line of documentation.
-- Continuation lines.
add :: QuectoVm -> Register -> Register -> IO ()
```

Multi-line docs use `-- |` for the first line, `--` for continuations.

### What Gets Documented

| Rust | Haskell |
|------|---------|
| Doc on `#[hsrs::enumeration] enum` | Before `newtype` declaration |
| Doc on `#[hsrs::module] mod` | Before `data Raw` / `newtype` declarations |
| Doc on `#[hsrs::function] fn` | Before the high-level function signature |

---

## Task 1: Update IR — Add Doc Strings

**Files:**
- Modify: `/Users/marko/Desktop/hsrs/hsrs-codegen/src/ir.rs`

Add `pub docs: Vec<String>` to `FfiEnum`, `FfiModule`, and `FfiFunction`:

```rust
pub struct FfiEnum {
    pub name: String,
    pub variants: Vec<String>,
    pub has_eq: bool,
    pub has_show: bool,
    pub has_ord: bool,
    pub docs: Vec<String>,
}

pub struct FfiModule {
    pub name: String,
    pub struct_name: String,
    pub functions: Vec<FfiFunction>,
    pub docs: Vec<String>,
}

pub struct FfiFunction {
    pub rust_name: String,
    pub c_name: String,
    pub kind: FfiFunctionKind,
    pub params: Vec<FfiParam>,
    pub return_type: Option<FfiType>,
    pub docs: Vec<String>,
}
```

Each `docs` entry is one line of documentation (without the `///` prefix, with leading space trimmed).

---

## Task 2: Parser — Extract Doc Comments

**Files:**
- Modify: `/Users/marko/Desktop/hsrs/hsrs-codegen/src/parser.rs`

**Step 1: Add `extract_docs` function**

```rust
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
```

Note: `syn` stores `/// foo` as `#[doc = " foo"]` — the leading space is part of the string value.

**Step 2: Update `parse_enum`**

Add `docs: extract_docs(&e.attrs)` to the `FfiEnum` constructor.

**Step 3: Update `parse_module`**

Add `docs: extract_docs(&m.attrs)` to the `FfiModule` constructor.

**Step 4: Update `parse_function`**

Add `docs: extract_docs(&method.attrs)` to the `FfiFunction` constructor.

**Step 5: Update the auto-generated destructor**

The destructor `FfiFunction` gets `docs: vec![]`.

**Step 6: Verify**

Run: `cargo check -p hsrs-codegen`

---

## Task 3: Haskell Codegen — Emit Haddock

**Files:**
- Modify: `/Users/marko/Desktop/hsrs/hsrs-codegen/src/haskell.rs`

**Step 1: Add `emit_haddock` helper**

```rust
fn emit_haddock(out: &mut String, docs: &[String]) {
    for (i, line) in docs.iter().enumerate() {
        if i == 0 {
            out.push_str(&format!("-- |{}\n", line));
        } else {
            out.push_str(&format!("--{}\n", line));
        }
    }
}
```

The doc strings already contain leading spaces from syn (e.g., `" foo"`), so `-- | foo` and `-- foo` format correctly without extra spacing.

**Step 2: Update `generate_enum`**

Call `emit_haddock(out, &e.docs)` before the `newtype` line.

**Step 3: Update `generate_module`**

Call `emit_haddock(out, &m.docs)` before the `data Raw` line.

**Step 4: Update `generate_high_level`**

Call `emit_haddock(out, &f.docs)` before the function type signature (inside the constructor and method branches, before the `\n{} :: ...` line).

**Step 5: Verify**

Run: `cargo check -p hsrs-codegen`

---

## Task 4: Add Doc Comments to Example and Verify

**Files:**
- Modify: `/Users/marko/Desktop/hsrs/hsrs-examples/src/lib.rs`

**Step 1: Add doc comments to the example**

```rust
/// CPU register identifiers.
#[derive(Debug, PartialEq, Eq)]
#[hsrs::enumeration]
pub enum Register {
    Reg0,
    Reg1,
    Count,
}

/// A minimal virtual machine with two registers.
#[hsrs::module]
mod quecto_vm {
    #[hsrs::data_type]
    pub struct QuectoVm {
        registers: [i64; Register::Count as usize],
        clock: usize,
    }

    impl QuectoVm {
        /// Creates a new VM with zeroed registers.
        #[hsrs::function]
        pub fn new() -> Self { ... }

        /// Adds register `b` into register `a`.
        #[hsrs::function]
        pub fn add(&mut self, a: Register, b: Register) { ... }

        /// Subtracts register `b` from register `a`.
        #[hsrs::function]
        pub fn sub(&mut self, a: Register, b: Register) { ... }

        /// Multiplies register `a` by register `b`.
        #[hsrs::function]
        pub fn mul(&mut self, a: Register, b: Register) { ... }

        /// Divides register `a` by register `b`.
        #[hsrs::function]
        pub fn div(&mut self, a: Register, b: Register) { ... }

        /// Reads the value in register `r`.
        #[hsrs::function]
        pub fn load(&mut self, r: Register) -> i64 { ... }

        /// Writes `v` into register `r`.
        #[hsrs::function]
        pub fn store(&mut self, r: Register, v: i64) { ... }
    }
}
```

**Step 2: Run codegen and verify Haddock output**

Run: `cargo run -p hsrs-codegen -- hsrs-examples/src/lib.rs`

Expected output should include:

```haskell
-- | CPU register identifiers.
newtype Register = Register Word8
  deriving (Eq, Show, Storable)

-- | A minimal virtual machine with two registers.
data QuectoVmRaw

-- | Creates a new VM with zeroed registers.
new :: IO QuectoVm

-- | Adds register `b` into register `a`.
add :: QuectoVm -> Register -> Register -> IO ()
```

**Step 3: Full workspace check**

Run: `cargo clippy --workspace`

---

## File Summary

| File | Action |
|------|--------|
| `hsrs-codegen/src/ir.rs` | Modify (add `docs: Vec<String>` to 3 structs) |
| `hsrs-codegen/src/parser.rs` | Modify (add `extract_docs`, update 3 parse functions + destructor) |
| `hsrs-codegen/src/haskell.rs` | Modify (add `emit_haddock`, call from 3 generate functions) |
| `hsrs-examples/src/lib.rs` | Modify (add doc comments to example) |

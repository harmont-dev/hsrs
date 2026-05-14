# Conditional Haskell Derives

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make Haskell `deriving` clauses conditional on which traits the Rust enum actually derives, instead of unconditionally emitting `(Eq, Show, Storable)`.

**Architecture:** Four-layer change: proc macro stops force-adding derives, codegen parser extracts `#[derive(...)]` info from Rust source, IR carries it, Haskell codegen conditionally emits Haskell derives based on the mapping.

**Tech Stack:** syn 2 (attribute parsing), existing IR/codegen pipeline

---

## Context

Currently the pipeline has a lie baked in at two levels:

1. **Proc macro** (`enumeration.rs`): Forces `#[derive(Debug, Clone, Copy, PartialEq, Eq)]` regardless of what the user wrote
2. **Haskell codegen** (`haskell.rs`): Always emits `deriving (Eq, Show, Storable)` regardless of what the Rust type supports

The fix: let the user declare their own derives. The proc macro only adds what's mechanically required for FFI (`Clone, Copy`). The codegen reads the user's derives and maps them to Haskell.

### Derive Mapping

| Rust derive | Haskell derive | Notes |
|-------------|----------------|-------|
| `PartialEq` or `Eq` | `Eq` | Either suffices — `#[repr(u8)]` enums are always total |
| `Debug` | `Show` | Haskell `Show` ≈ Rust `Debug` |
| `PartialOrd` or `Ord` | `Ord` | Optional |
| (always) | `Storable` | Required — Haskell FFI needs it to marshal the `Word8` representation |

`Storable` is always emitted because it's a mechanical FFI requirement, not a semantic trait. Without it, the newtype can't be passed across the FFI boundary.

---

## Task 1: Proc Macro — Stop Force-Adding Derives

**Files:**
- Modify: `/Users/marko/Desktop/hsrs/hsrs/src/enumeration.rs`

**Step 1: Change derive line**

Replace:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
```
With:
```rust
#[derive(Clone, Copy)]
```

`Clone` and `Copy` are required for safer-ffi's `#[derive_ReprC]` on `#[repr(u8)]` enums. User's own `#[derive(...)]` attributes are preserved via `#(#attrs)*` which already passes through all original attributes.

**Step 2: Verify proc macro compiles**

Run: `cargo check -p hsrs`

---

## Task 2: Update IR — Add Derive Tracking to FfiEnum

**Files:**
- Modify: `/Users/marko/Desktop/hsrs/hsrs-codegen/src/ir.rs`

**Step 1: Add derive fields to FfiEnum**

```rust
pub struct FfiEnum {
    pub name: String,
    pub variants: Vec<String>,
    pub has_eq: bool,
    pub has_show: bool,
    pub has_ord: bool,
}
```

**Step 2: Verify codegen compiles**

Run: `cargo check -p hsrs-codegen`
Expected: Fails — parser and haskell don't populate/use the new fields yet. That's fine.

---

## Task 3: Codegen Parser — Extract Derives

**Files:**
- Modify: `/Users/marko/Desktop/hsrs/hsrs-codegen/src/parser.rs`

**Step 1: Add derive extraction function**

```rust
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
```

**Step 2: Update `parse_enum` to use it**

```rust
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
```

**Step 3: Verify**

Run: `cargo check -p hsrs-codegen`

---

## Task 4: Haskell Codegen — Conditional Derives

**Files:**
- Modify: `/Users/marko/Desktop/hsrs/hsrs-codegen/src/haskell.rs`

**Step 1: Update `generate_enum`**

Replace the hardcoded `deriving (Eq, Show, Storable)` with conditional logic:

```rust
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
```

**Step 2: Verify codegen compiles**

Run: `cargo check -p hsrs-codegen`

---

## Task 5: Update Example and Verify Output

**Files:**
- Modify: `/Users/marko/Desktop/hsrs/hsrs-examples/src/lib.rs`

**Step 1: Add explicit derives to the example enum**

```rust
#[derive(Debug, PartialEq, Eq)]
#[hsrs::enumeration]
pub enum Register {
    Reg0,
    Reg1,
    Count,
}
```

This makes the example explicit about which Haskell derives it wants.

**Step 2: Verify examples compile**

Run: `cargo check -p hsrs-examples`

**Step 3: Run codegen and verify output**

Run: `cargo run -p hsrs-codegen -- hsrs-examples/src/lib.rs`

Expected: The Register newtype now shows `deriving (Eq, Show, Storable)` — same as before, but now because the Rust code asked for it.

**Step 4: Test without derives**

Create a quick test — temporarily remove `#[derive(Debug, PartialEq, Eq)]` from the example, re-run codegen.

Expected: Output shows `deriving (Storable)` only.

Restore the derives after testing.

**Step 5: Regenerate Bindings.hs**

Run: `cargo run -p hsrs-codegen -- hsrs-examples/src/lib.rs hsrs-examples/Bindings.hs`

**Step 6: Full workspace check**

Run: `cargo clippy --workspace`

---

## Risks

1. **User forgets derives**: If a user writes `#[hsrs::enumeration]` without `#[derive(PartialEq, Eq)]`, their Haskell type won't have `Eq`. This is intentional — explicit is better than implicit. The README should document this.

2. **`Display` is not derivable**: In standard Rust, `Display` requires a manual `impl`. The parser checks `#[derive(...)]` only, so manual `impl Display` won't be detected. This is acceptable for v1 — `Debug` is the standard derivable trait and maps to Haskell `Show`.

3. **Derive ordering**: The proc macro preserves user attrs (`#(#attrs)*`) then adds its own. If the user writes `#[derive(Debug)]` and the macro adds `#[derive(Clone, Copy)]`, the enum gets two derive attributes. Rust handles this fine — it merges them.

---

## File Summary

| File | Action |
|------|--------|
| `hsrs/src/enumeration.rs` | Modify (remove forced PartialEq, Eq, Debug) |
| `hsrs-codegen/src/ir.rs` | Modify (add has_eq, has_show, has_ord to FfiEnum) |
| `hsrs-codegen/src/parser.rs` | Modify (add extract_derives, update parse_enum) |
| `hsrs-codegen/src/haskell.rs` | Modify (conditional derive emission) |
| `hsrs-examples/src/lib.rs` | Modify (add explicit derives to Register) |
| `hsrs-examples/Bindings.hs` | Regenerate |

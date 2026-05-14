# Facade Crate Refactor

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rename `hsrs` → `hsrs-macros`, create `hsrs` facade crate that re-exports macros + safer-ffi so users need a single dependency.

**Architecture:** Standard Rust facade pattern (serde/serde_derive, tokio/tokio-macros). `hsrs-macros` is the proc-macro crate. `hsrs` is a thin facade that re-exports the macros and `safer-ffi`. Proc macro generated code uses `::hsrs::safer_ffi::*` paths instead of `::safer_ffi::*`.

**Tech Stack:** Cargo workspace, `pub use` re-exports

---

## Context

Currently users need two dependencies:
```toml
hsrs = { git = "..." }
safer-ffi = { version = "0.2.0-rc1", features = ["alloc"] }
```

After this refactor, one dependency:
```toml
hsrs = { git = "..." }
```

### Directory Layout Change

Before:
```
hsrs/            → proc-macro crate (name: hsrs)
hsrs-codegen/    → binary
hsrs-examples/   → examples
```

After:
```
hsrs-macros/     → proc-macro crate (renamed)
hsrs/            → facade crate (NEW)
hsrs-codegen/    → binary (unchanged)
hsrs-examples/   → examples (simplified deps)
```

---

## Task 1: Rename hsrs → hsrs-macros

**Step 1: Rename directory**

```bash
git mv hsrs hsrs-macros
```

**Step 2: Update hsrs-macros/Cargo.toml**

Change `name = "hsrs"` to `name = "hsrs-macros"`.

**Step 3: Update workspace members in root Cargo.toml**

Change `"hsrs"` to `"hsrs-macros"` in the members list.

**Step 4: Verify**

Run: `cargo check -p hsrs-macros`

---

## Task 2: Create hsrs Facade Crate

**Files:**
- Create: `/Users/marko/Desktop/hsrs/hsrs/Cargo.toml`
- Create: `/Users/marko/Desktop/hsrs/hsrs/src/lib.rs`
- Modify: `/Users/marko/Desktop/hsrs/Cargo.toml` (add `"hsrs"` back to members)

**Step 1: Create hsrs/Cargo.toml**

```toml
[package]
name = "hsrs"
version = "0.1.0"
edition = "2024"

[dependencies]
hsrs-macros = { path = "../hsrs-macros" }
safer-ffi = { version = "0.2.0-rc1", features = ["alloc"] }

[lints]
workspace = true
```

**Step 2: Create hsrs/src/lib.rs**

```rust
#![allow(
    clippy::missing_docs_in_private_items,
    clippy::cargo,
)]

pub use hsrs_macros::{data_type, enumeration, function, module};

pub use safer_ffi;
```

**Step 3: Add "hsrs" to workspace members**

The members list should be: `["hsrs", "hsrs-macros", "hsrs-codegen", "hsrs-examples"]`

**Step 4: Verify**

Run: `cargo check -p hsrs`

---

## Task 3: Update Proc Macro Paths

**Files:**
- Modify: `/Users/marko/Desktop/hsrs/hsrs-macros/src/enumeration.rs`
- Modify: `/Users/marko/Desktop/hsrs/hsrs-macros/src/module.rs`

All `::safer_ffi::` paths in generated code become `::hsrs::safer_ffi::`.

**Step 1: enumeration.rs**

Change:
```rust
#[::safer_ffi::derive_ReprC]
```
To:
```rust
#[::hsrs::safer_ffi::derive_ReprC]
```

**Step 2: module.rs — all occurrences**

Replace every `::safer_ffi::` with `::hsrs::safer_ffi::` in generated token output. There are these occurrences:

- `#[::safer_ffi::ffi_export]` (destructor, line 28)
- `use ::safer_ffi::prelude::*;` (line 40)
- `#[::safer_ffi::derive_ReprC]` (process_struct, line 81)
- `#[::safer_ffi::ffi_export]` (generate_wrapper, lines 146, 155, 162, 172, 179)

Also update `repr_c::Box` references — these resolve through the `use ::safer_ffi::prelude::*` import, which is being updated, so they remain unchanged.

**Step 3: Verify proc-macro compiles**

Run: `cargo check -p hsrs-macros`

---

## Task 4: Update hsrs-examples Dependencies

**Files:**
- Modify: `/Users/marko/Desktop/hsrs/hsrs-examples/Cargo.toml`
- Modify: `/Users/marko/Desktop/hsrs/hsrs-examples/src/lib.rs` (if needed — `#[hsrs::*]` paths should still work via facade re-export)

**Step 1: Simplify Cargo.toml dependencies**

Replace:
```toml
[dependencies]
hsrs = { path = "../hsrs" }
safer-ffi = { version = "0.2.0-rc1", features = ["alloc"] }
```

With:
```toml
[dependencies]
hsrs = { path = "../hsrs" }
```

Users no longer need `safer-ffi` directly — it's re-exported through `hsrs`.

**Step 2: Verify examples compile**

Run: `cargo check -p hsrs-examples`

If the `safer_ffi` paths from the expanded macro can't resolve, the module macro's `use ::hsrs::safer_ffi::prelude::*;` should fix this since `hsrs` re-exports `safer_ffi`.

**Step 3: Full workspace check**

Run: `cargo clippy --workspace`

---

## Task 5: Update README and Verify

**Files:**
- Modify: `/Users/marko/Desktop/hsrs/README.md`

**Step 1: Update Usage section**

Remove `safer-ffi` from the dependency example:

```toml
[lib]
crate-type = ["lib", "staticlib"]

[dependencies]
hsrs = { git = "https://github.com/harmont-dev/hsrs" }
```

**Step 2: Update Workspace table**

```markdown
| Crate | Role |
|-------|------|
| `hsrs` | Facade — re-exports proc macros and safer-ffi |
| `hsrs-macros` | Proc-macro — attribute macros for FFI generation |
| `hsrs-codegen` | Binary — parses Rust source, emits Haskell |
| `hsrs-examples` | Example — demo crate with generated bindings |
```

**Step 3: Update Requirements**

Remove `safer-ffi 0.2.0-rc1` from requirements (it's now an internal dependency).

**Step 4: Run codegen to verify it still works**

Run: `cargo run -p hsrs-codegen -- hsrs-examples/src/lib.rs`

Expected: Same Haskell output as before.

---

## Risks

1. **Proc macro paths through facade**: `#[::hsrs::safer_ffi::derive_ReprC]` must resolve through the facade's `pub use safer_ffi`. This works in modern Rust — `pub use` re-exports make proc macros accessible through the re-export path.

2. **`unsafe_code = "forbid"` in workspace**: The facade itself has no unsafe code. `pub use safer_ffi;` is just a re-export, not a definition. Should not trigger the lint.

3. **Circular dependency risk**: None — `hsrs` depends on `hsrs-macros`, `hsrs-macros` has no dependency on `hsrs`. The proc macro emits paths referencing `::hsrs::*` but doesn't depend on the facade at build time.

---

## File Summary

| File | Action |
|------|--------|
| `hsrs/` → `hsrs-macros/` | Rename (git mv) |
| `hsrs-macros/Cargo.toml` | Modify (name → hsrs-macros) |
| `hsrs/Cargo.toml` | Create (facade) |
| `hsrs/src/lib.rs` | Create (re-exports) |
| `hsrs-macros/src/enumeration.rs` | Modify (paths) |
| `hsrs-macros/src/module.rs` | Modify (paths) |
| `hsrs-examples/Cargo.toml` | Modify (remove safer-ffi dep) |
| `Cargo.toml` | Modify (workspace members) |
| `README.md` | Modify (usage, workspace table) |

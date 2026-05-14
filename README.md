# hsrs

Type-safe Haskell FFI bindings from annotated Rust.

`hsrs` proc macros generate C FFI exports via [safer-ffi](https://github.com/getditto/safer_ffi).
`hsrs-codegen` reads the annotated Rust source and emits idiomatic Haskell with newtypes,
`ForeignPtr` management, and pattern synonyms — preserving the type safety that the C layer erases.

## Example

### Rust

```rust
#[derive(Debug, PartialEq, Eq)]
#[hsrs::enumeration]
pub enum Register {
    Reg0,
    Reg1,
    Count,
}

#[hsrs::module]
mod quecto_vm {
    #[hsrs::data_type]
    pub struct QuectoVm {
        registers: [i64; Register::Count as usize],
        clock: usize,
    }

    impl QuectoVm {
        #[hsrs::function]
        pub fn new() -> Self { /* ... */ }

        #[hsrs::function]
        pub fn add(&mut self, a: Register, b: Register) { /* ... */ }

        #[hsrs::function]
        pub fn load(&mut self, r: Register) -> i64 { /* ... */ }

        #[hsrs::function]
        pub fn store(&mut self, r: Register, v: i64) { /* ... */ }
    }
}
```

### Generated Haskell

```haskell
newtype Register = Register Word8
  deriving (Eq, Show, Storable)

pattern Reg0 :: Register
pattern Reg0 = Register 0

data QuectoVmRaw

newtype QuectoVm = QuectoVm (ForeignPtr QuectoVmRaw)

foreign import ccall "quecto_vm_new"   c_quectoVmNew   :: IO (Ptr QuectoVmRaw)
foreign import ccall "quecto_vm_add"   c_quectoVmAdd   :: Ptr QuectoVmRaw -> Word8 -> Word8 -> IO ()
foreign import ccall "quecto_vm_store" c_quectoVmStore :: Ptr QuectoVmRaw -> Word8 -> Int64 -> IO ()
foreign import ccall "&quecto_vm_free" c_quectoVmFree  :: FinalizerPtr QuectoVmRaw

new :: IO QuectoVm
new = do
  ptr <- c_quectoVmNew
  fp  <- newForeignPtr c_quectoVmFree ptr
  pure (QuectoVm fp)

add :: QuectoVm -> Register -> Register -> IO ()
add (QuectoVm fp) a b =
  withForeignPtr fp $ \ptr ->
    c_quectoVmAdd ptr (let (Register a') = a in a') (let (Register b') = b in b')
```

Full output: [`hsrs-examples/Bindings.hs`](hsrs-examples/Bindings.hs)

## Usage

Add `hsrs` and `safer-ffi` to your crate:

```toml
[lib]
crate-type = ["lib", "staticlib"]

[dependencies]
hsrs = { git = "..." }
safer-ffi = { version = "0.2.0-rc1", features = ["alloc"] }
```

Annotate your types, then generate bindings:

```sh
cargo run -p hsrs-codegen -- path/to/lib.rs -o Bindings.hs
```

## Attributes

| Attribute | Target | Effect |
|-----------|--------|--------|
| `#[hsrs::enumeration]` | `enum` (unit variants only) | Adds `#[derive_ReprC]`, `#[repr(u8)]` |
| `#[hsrs::module]` | `mod` block | Processes the module; generates FFI wrappers + destructor |
| `#[hsrs::data_type]` | `struct` (inside module) | Adds `#[derive_ReprC]`, `#[repr(opaque)]` |
| `#[hsrs::function]` | `fn` (inside module impl) | Generates `#[ffi_export]` wrapper |

## Type Mapping

| Rust | C (safer-ffi) | Haskell FFI | Haskell API |
|------|---------------|-------------|-------------|
| `i8`..`i64` | `int8_t`..`int64_t` | `Int8`..`Int64` | `Int8`..`Int64` |
| `u8`..`u64` | `uint8_t`..`uint64_t` | `Word8`..`Word64` | `Word8`..`Word64` |
| `bool` | `bool` | `CBool` | `CBool` |
| `#[enumeration]` enum | `uint8_t` | `Word8` | newtype (e.g. `Register`) |
| `#[data_type]` struct | opaque ptr | `Ptr Raw` | newtype over `ForeignPtr` |

## Haskell Derives

Haskell `deriving` clauses mirror the Rust source:

| Rust `#[derive(...)]` | Haskell `deriving (...)` |
|------------------------|--------------------------|
| `PartialEq` / `Eq` | `Eq` |
| `Debug` | `Show` |
| `PartialOrd` / `Ord` | `Ord` |
| *(always)* | `Storable` |

## Workspace

| Crate | Role |
|-------|------|
| `hsrs` | Proc-macro — attribute macros for FFI generation |
| `hsrs-codegen` | Binary — parses Rust source, emits Haskell |
| `hsrs-examples` | Example — demo crate with generated bindings |

## Requirements

- Rust 1.85+ (edition 2024)
- `safer-ffi` 0.2.0-rc1

## License

MIT OR Apache-2.0

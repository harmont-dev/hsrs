# hsrs

Type-safe Haskell FFI bindings from annotated Rust.

`hsrs` generates C FFI exports via [safer-ffi](https://github.com/getditto/safer_ffi).
`hsrs-codegen` reads the annotated Rust source and emits idiomatic Haskell with newtypes,
`ForeignPtr` management, and pattern synonyms — preserving the type safety that the C layer erases.

## Supported types

| Rust | Haskell | Transfer |
|------|---------|----------|
| `#[hsrs::data_type]` struct | `ForeignPtr` newtype | Opaque pointer |
| `#[hsrs::enumeration]` enum | `Word8` newtype + pattern synonyms | `repr(u8)` |
| `#[hsrs::value_type]` struct | `data` record + borsh deriving | Borsh-serialized bytes |
| `Result<T, E>` | `Either E T` | Borsh-serialized bytes |
| `Option<T>` | `Maybe T` | Borsh-serialized bytes |
| Primitives (`i32`, `u64`, `bool`, ...) | `Int32`, `Word64`, `CBool`, ... | Direct FFI |

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

#[derive(Debug, PartialEq, Eq)]
#[hsrs::value_type]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, PartialEq, Eq)]
#[hsrs::value_type]
pub struct VmError {
    pub code: u32,
}

#[hsrs::module(value_types(Point, VmError))]
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

        #[hsrs::function]
        pub fn snapshot(&self) -> Point { /* ... */ }

        #[hsrs::function]
        pub fn safe_div(&mut self, a: Register, b: Register) -> Result<i64, VmError> { /* ... */ }

        #[hsrs::function]
        pub fn nonzero(&self, r: Register) -> Option<i64> { /* ... */ }
    }
}
```

### Generated Haskell

```haskell
newtype Register = Register Word8
  deriving (Eq, Show, Storable)
  deriving (BorshSize, ToBorsh, FromBorsh) via Word8

pattern Reg0 :: Register
pattern Reg0 = Register 0

data Point = Point
  { pointX :: Int32
  , pointY :: Int32
  } deriving (Generic, Eq, Show)
  deriving (BorshSize, ToBorsh, FromBorsh) via AsStruct Point

data VmError = VmError
  { vmErrorCode :: Word32
  } deriving (Generic, Eq, Show)
  deriving (BorshSize, ToBorsh, FromBorsh) via AsStruct VmError

data QuectoVmRaw

newtype QuectoVm = QuectoVm (ForeignPtr QuectoVmRaw)

new :: IO QuectoVm
new = do
  ptr <- c_quectoVmNew
  fp  <- newForeignPtr c_quectoVmFree ptr
  pure (QuectoVm fp)

add :: QuectoVm -> Register -> Register -> IO ()
add (QuectoVm fp) a b =
  withForeignPtr fp $ \ptr ->
    c_quectoVmAdd ptr (let (Register a') = a in a') (let (Register b') = b in b')

snapshot :: QuectoVm -> IO Point
snapshot (QuectoVm fp) = withForeignPtr fp $ \ptr ->
  fromBorshBuffer =<< c_quectoVmSnapshot ptr

safeDiv :: QuectoVm -> Register -> Register -> IO (Either VmError Int64)
safeDiv (QuectoVm fp) a b = withForeignPtr fp $ \ptr ->
  fromBorshBuffer =<< c_quectoVmSafeDiv ptr
    (let (Register a') = a in a') (let (Register b') = b in b')

nonzero :: QuectoVm -> Register -> IO (Maybe Int64)
nonzero (QuectoVm fp) r = withForeignPtr fp $ \ptr ->
  fromBorshBuffer =<< c_quectoVmNonzero ptr (let (Register r') = r in r')
```

## Usage

Add `hsrs` to your crate:

```toml
[lib]
crate-type = ["lib", "staticlib"]

[dependencies]
hsrs = { git = "https://github.com/harmont-dev/hsrs" }
```

Annotate your types, then generate bindings:

```sh
cargo run -p hsrs-codegen -- path/to/lib.rs -o Bindings.hs
```

On the Haskell side, add [`borsh`](https://hackage.haskell.org/package/borsh) to your
build-depends for value type deserialization.

## License

MIT OR Apache-2.0

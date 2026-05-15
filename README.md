# hsrs

Call Rust from Haskell with type-safe, automatically generated FFI bindings.

Annotate your Rust types and functions, run the code generator, and get idiomatic Haskell that handles memory management, serialization, and type conversions for you.

## Quick start

### 1. Annotate your Rust code

```rust
#[hsrs::value_type]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[hsrs::module(value_types(Point))]
mod canvas {
    #[hsrs::data_type]
    pub struct Canvas {
        points: Vec<Point>,
    }

    impl Canvas {
        #[hsrs::function]
        pub fn new() -> Self { Self { points: vec![] } }

        #[hsrs::function]
        pub fn add_point(&mut self, p: Point) { self.points.push(p); }

        #[hsrs::function]
        pub fn count(&self) -> u64 { self.points.len() as u64 }
    }
}
```

### 2. Generate Haskell bindings

```sh
cargo run -p hsrs-codegen -- src/lib.rs -o Bindings.hs
```

### 3. Use from Haskell

```haskell
import Bindings

main :: IO ()
main = do
  c <- new
  addPoint c (Point 10 20)
  n <- count c
  print n  -- 1
```

That's it. Memory is managed automatically via `ForeignPtr`, and complex types like `Point` are serialized across the boundary with [Borsh](https://borsh.io).

## Setup

**Rust side** — add `hsrs` to your crate:

```toml
[lib]
crate-type = ["lib", "staticlib"]

[dependencies]
hsrs = { git = "https://github.com/harmont-dev/hsrs" }
```

**Haskell side** — add the `hsrs` runtime package:

```cabal
build-depends:
    hsrs >= 0.1 && < 0.2
```

This pulls in Borsh serialization automatically — no extra dependencies needed.

> Until published on Hackage, add as a local package in your `cabal.project`:
> ```
> packages: .
>           /path/to/hsrs/hsrs-haskell
> ```

## What you can annotate

| Annotation | What it does | Haskell result |
|---|---|---|
| `#[hsrs::data_type]` | Opaque struct passed by pointer | `ForeignPtr` newtype with automatic cleanup |
| `#[hsrs::enumeration]` | C-compatible enum (`repr(u8)`) | `Word8` newtype with pattern synonyms |
| `#[hsrs::value_type]` | Struct passed by value via Borsh | `data` record with Borsh deriving |
| `#[hsrs::function]` | Method exported over FFI | Type-safe Haskell wrapper |
| `#[hsrs::module]` | Groups a data type with its methods | Generates all FFI glue for the type |

`Result<T, E>` becomes `Either E T` and `Option<T>` becomes `Maybe T`, both serialized transparently.

## Platform notes

`usize` and `isize` are mapped to `Word64` and `Int64` respectively. This matches 64-bit platforms (x86_64, aarch64). If you target 32-bit platforms, be aware that values may be truncated.

## Full example

<details>
<summary>A small VM with enums, value types, Result, and Option</summary>

### Rust

```rust
#[derive(Debug, PartialEq, Eq)]
#[hsrs::enumeration]
pub enum Register { Reg0, Reg1, Count }

#[derive(Debug, PartialEq, Eq)]
#[hsrs::value_type]
pub struct Point { pub x: i32, pub y: i32 }

#[derive(Debug, PartialEq, Eq)]
#[hsrs::value_type]
pub struct VmError { pub code: u32 }

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

new       :: IO QuectoVm
store     :: QuectoVm -> Register -> Int64 -> IO ()
snapshot  :: QuectoVm -> IO Point
safeDiv   :: QuectoVm -> Register -> Register -> IO (Either VmError Int64)
nonzero   :: QuectoVm -> Register -> IO (Maybe Int64)
```

</details>

## License

MIT OR Apache-2.0

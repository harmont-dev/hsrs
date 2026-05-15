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
    clippy::shadow_unrelated,
)]

use hsrs_codegen::{haskell, parser};

fn source_to_haskell(src: &str) -> String {
    let parsed = parser::parse_str(src).expect("parse failed");
    haskell::generate(&parsed, "Bindings")
}

fn source_to_haskell_with_module(src: &str, module_name: &str) -> String {
    let parsed = parser::parse_str(src).expect("parse failed");
    haskell::generate(&parsed, module_name)
}

#[test]
fn minimal_module_round_trip() {
    let src = r#"
        #[hsrs::module]
        mod counter {
            #[hsrs::data_type]
            pub struct Counter {
                val: i64,
            }

            impl Counter {
                #[hsrs::function]
                pub fn new() -> Self {
                    Self { val: 0 }
                }

                #[hsrs::function]
                pub fn get(&self) -> i64 {
                    self.val
                }

                #[hsrs::function]
                pub fn increment(&mut self) {
                    self.val += 1;
                }
            }
        }
    "#;
    let hs = source_to_haskell(src);

    // Module structure
    assert!(hs.contains("module Bindings where"));
    assert!(hs.contains("data CounterRaw"));
    assert!(hs.contains("newtype Counter = Counter (ForeignPtr CounterRaw)"));

    // Foreign imports
    assert!(hs.contains("foreign import ccall safe \"counter_new\" c_counterNew :: IO (Ptr CounterRaw)"));
    assert!(hs.contains("foreign import ccall safe \"counter_get\" c_counterGet :: Ptr CounterRaw -> IO Int64"));
    assert!(hs.contains("foreign import ccall safe \"counter_increment\" c_counterIncrement :: Ptr CounterRaw -> IO ()"));
    assert!(hs.contains("foreign import ccall \"&counter_free\" c_counterFree :: FinalizerPtr CounterRaw"));

    // High-level wrappers
    assert!(hs.contains("new :: IO Counter"));
    assert!(hs.contains("get :: Counter -> IO Int64"));
    assert!(hs.contains("increment :: Counter -> IO ()"));

    // No borsh infrastructure for primitive-only module
    assert!(!hs.contains("BorshBufferRaw"));
    assert!(!hs.contains("Codec.Borsh"));
}

#[test]
fn full_featured_round_trip() {
    let src = r#"
        /// Direction of movement.
        #[derive(Debug, PartialEq, Eq)]
        #[hsrs::enumeration]
        pub enum Direction {
            North,
            South,
            East,
            West,
        }

        /// A 2D position.
        #[derive(Debug, PartialEq, Eq)]
        #[hsrs::value_type]
        pub struct Position {
            pub x: i32,
            pub y: i32,
        }

        /// Error from the navigator.
        #[hsrs::value_type]
        pub struct NavError {
            pub code: u32,
        }

        /// Navigation engine.
        #[hsrs::module(value_types(Position, NavError))]
        mod navigator {
            #[hsrs::data_type]
            pub struct Navigator {
                x: i32,
                y: i32,
            }

            impl Navigator {
                /// Create a navigator at origin.
                #[hsrs::function]
                pub fn new() -> Self {
                    Self { x: 0, y: 0 }
                }

                /// Move in a direction.
                #[hsrs::function]
                pub fn go(&mut self, dir: Direction) { }

                /// Get current position.
                #[hsrs::function]
                pub fn position(&self) -> Position {
                    Position { x: self.x, y: self.y }
                }

                /// Try to teleport, may fail.
                #[hsrs::function]
                pub fn teleport(&mut self, target: Position) -> Result<Position, NavError> {
                    Ok(target)
                }

                /// Find something at current position.
                #[hsrs::function]
                pub fn find(&self) -> Option<i64> {
                    None
                }
            }
        }
    "#;
    let hs = source_to_haskell(src);

    // Language pragmas
    assert!(hs.contains("{-# LANGUAGE DeriveGeneric #-}"));
    assert!(hs.contains("{-# LANGUAGE DerivingVia #-}"));
    assert!(hs.contains("{-# LANGUAGE PatternSynonyms #-}"));

    // Borsh infrastructure via Hsrs.Runtime
    assert!(hs.contains("import Hsrs.Runtime"));
    assert!(!hs.contains("import Codec.Borsh"));
    assert!(!hs.contains("data BorshBufferRaw"));
    assert!(hs.contains("fromBorshBuffer"));

    // Enum
    assert!(hs.contains("newtype Direction = Direction Word8"));
    assert!(hs.contains("pattern North = Direction 0"));
    assert!(hs.contains("pattern South = Direction 1"));
    assert!(hs.contains("pattern East = Direction 2"));
    assert!(hs.contains("pattern West = Direction 3"));
    assert!(hs.contains("-- | Direction of movement."));

    // Value types
    assert!(hs.contains("data Position = Position"));
    assert!(hs.contains("positionX :: Int32"));
    assert!(hs.contains("positionY :: Int32"));
    assert!(hs.contains("deriving (BorshSize, ToBorsh, FromBorsh) via AsStruct Position"));

    // Module structure
    assert!(hs.contains("data NavigatorRaw"));
    assert!(hs.contains("newtype Navigator = Navigator (ForeignPtr NavigatorRaw)"));

    // Enum param unwrapping
    assert!(hs.contains("Word8 ->"), "enum param should be Word8 in FFI");

    // Borsh return (value type)
    assert!(hs.contains("position :: Navigator -> IO Position"));
    assert!(hs.contains("fromBorshBuffer =<< c_navigatorPosition"));

    // Result return → Either
    assert!(hs.contains("IO (Either NavError Position)"));

    // Option return → Maybe
    assert!(hs.contains("IO (Maybe Int64)"));

    // Borsh param (value type param)
    assert!(hs.contains("withBorshArg target"));

    // Haddock
    assert!(hs.contains("-- | Create a navigator at origin."));
    assert!(hs.contains("-- | Navigation engine."));
}

#[test]
fn safety_annotations_round_trip() {
    let src = r#"
        #[hsrs::module(safety = unsafe)]
        mod fast {
            #[hsrs::data_type]
            pub struct Fast { x: i32 }
            impl Fast {
                #[hsrs::function]
                pub fn new() -> Self { Self { x: 0 } }

                #[hsrs::function]
                pub fn get(&self) -> i32 { self.x }

                #[hsrs::function(safe)]
                pub fn slow_get(&self) -> i32 { self.x }

                #[hsrs::function(interruptible)]
                pub fn blocking(&mut self) -> i32 { 0 }
            }
        }
    "#;
    let hs = source_to_haskell(src);

    assert!(hs.contains("ccall unsafe \"fast_new\""), "constructor inherits unsafe");
    assert!(hs.contains("ccall unsafe \"fast_get\""), "get inherits unsafe");
    assert!(hs.contains("ccall safe \"fast_slow_get\""), "slow_get overridden to safe");
    assert!(hs.contains("ccall interruptible \"fast_blocking\""), "blocking is interruptible");
    assert!(hs.contains("ccall \"&fast_free\""), "destructor has no safety");
}

#[test]
fn enum_only_output() {
    let src = r#"
        #[derive(PartialEq)]
        #[hsrs::enumeration]
        pub enum Flag {
            Off,
            On,
        }
    "#;
    let hs = source_to_haskell(src);
    assert!(hs.contains("newtype Flag = Flag Word8"));
    assert!(hs.contains("deriving (Eq, Storable)"));
    assert!(hs.contains("pattern Off = Flag 0"));
    assert!(hs.contains("pattern On = Flag 1"));
    assert!(!hs.contains("Codec.Borsh"));
    assert!(!hs.contains("ForeignPtr"));
}

#[test]
fn value_type_only_output() {
    let src = r#"
        #[derive(Debug, PartialEq, PartialOrd, Ord)]
        #[hsrs::value_type]
        pub struct Color {
            pub r: u8,
            pub g: u8,
            pub b: u8,
        }
    "#;
    let hs = source_to_haskell(src);
    assert!(hs.contains("data Color = Color"));
    assert!(hs.contains("colorR :: Word8"));
    assert!(hs.contains("colorG :: Word8"));
    assert!(hs.contains("colorB :: Word8"));
    assert!(hs.contains("deriving (Generic, Eq, Show, Ord)"));
    assert!(hs.contains("{-# LANGUAGE DeriveGeneric #-}"));
}

#[test]
fn multiple_modules_round_trip() {
    let src = r#"
        #[hsrs::module]
        mod alpha {
            #[hsrs::data_type]
            pub struct Alpha { x: i32 }
            impl Alpha {
                #[hsrs::function]
                pub fn new() -> Self { Self { x: 0 } }
            }
        }

        #[hsrs::module]
        mod beta {
            #[hsrs::data_type]
            pub struct Beta { y: u64 }
            impl Beta {
                #[hsrs::function]
                pub fn new() -> Self { Self { y: 0 } }
                #[hsrs::function]
                pub fn get(&self) -> u64 { self.y }
            }
        }
    "#;
    let hs = source_to_haskell(src);
    assert!(hs.contains("data AlphaRaw"));
    assert!(hs.contains("data BetaRaw"));
    assert!(hs.contains("newtype Alpha = Alpha (ForeignPtr AlphaRaw)"));
    assert!(hs.contains("newtype Beta = Beta (ForeignPtr BetaRaw)"));
    assert!(hs.contains("ccall safe \"alpha_new\""));
    assert!(hs.contains("ccall safe \"beta_new\""));
    assert!(hs.contains("ccall safe \"beta_get\""));
}

#[test]
fn borsh_module_imports_hsrs_runtime() {
    let src = r#"
        #[hsrs::value_type]
        pub struct State {
            pub x: i32,
        }

        #[hsrs::module(value_types(State))]
        mod engine {
            #[hsrs::data_type]
            pub struct Engine { x: i32 }
            impl Engine {
                #[hsrs::function]
                pub fn new() -> Self { Self { x: 0 } }

                #[hsrs::function]
                pub fn get(&self) -> State {
                    State { x: self.x }
                }
            }
        }
    "#;
    let hs = source_to_haskell(src);
    assert!(hs.contains("import Hsrs.Runtime"), "should import Hsrs.Runtime");
    assert!(!hs.contains("import Codec.Borsh"), "should not import Codec.Borsh");
    assert!(!hs.contains("data BorshBufferRaw"), "should not define BorshBufferRaw inline");
}

#[test]
fn borsh_param_uses_with_borsh_arg() {
    let src = r#"
        #[hsrs::value_type]
        pub struct Config {
            pub level: u32,
        }

        #[hsrs::module(value_types(Config))]
        mod engine {
            #[hsrs::data_type]
            pub struct Engine { x: i32 }
            impl Engine {
                #[hsrs::function]
                pub fn new() -> Self { Self { x: 0 } }

                #[hsrs::function]
                pub fn apply(&mut self, config: Config) {}
            }
        }
    "#;
    let hs = source_to_haskell(src);
    assert!(hs.contains("withBorshArg"), "should use withBorshArg");
    assert!(!hs.contains("useAsCStringLen"), "should not use useAsCStringLen");
    assert!(!hs.contains("castPtr"), "should not use castPtr");
}

#[test]
fn string_type_round_trip() {
    let src = r#"
        #[hsrs::module]
        mod db {
            #[hsrs::data_type]
            pub struct Db { x: i32 }
            impl Db {
                #[hsrs::function]
                pub fn new() -> Self { Self { x: 0 } }
                #[hsrs::function]
                pub fn name(&self) -> String { String::new() }
                #[hsrs::function]
                pub fn set_name(&mut self, name: String) {}
            }
        }
    "#;
    let output = source_to_haskell(src);
    assert!(output.contains(":: Db -> IO Text"), "String return maps to Text: {output}");
    assert!(output.contains(":: Db -> Text -> IO ()"), "String param maps to Text: {output}");
    assert!(output.contains("fromBorshBuffer"), "String return uses fromBorshBuffer: {output}");
    assert!(output.contains("withBorshArg name"), "String param uses withBorshArg: {output}");
}

#[test]
fn vec_type_round_trip() {
    let src = r#"
        #[hsrs::value_type]
        pub struct Point { pub x: i32, pub y: i32 }

        #[hsrs::module(value_types(Point))]
        mod canvas {
            #[hsrs::data_type]
            pub struct Canvas { x: i32 }
            impl Canvas {
                #[hsrs::function]
                pub fn new() -> Self { Self { x: 0 } }
                #[hsrs::function]
                pub fn points(&self) -> Vec<Point> {}
                #[hsrs::function]
                pub fn add_points(&mut self, pts: Vec<Point>) {}
                #[hsrs::function]
                pub fn counts(&self) -> Vec<u64> {}
            }
        }
    "#;
    let output = source_to_haskell(src);
    assert!(output.contains(":: Canvas -> IO [Point]"), "Vec<Point> return: {output}");
    assert!(output.contains(":: Canvas -> [Point] -> IO ()"), "Vec<Point> param: {output}");
    assert!(output.contains(":: Canvas -> IO [Word64]"), "Vec<u64> return: {output}");
    assert!(output.contains("withBorshArg pts"), "Vec param uses withBorshArg: {output}");
}

#[test]
fn kitchen_sink_all_types() {
    let src = r#"
        #[hsrs::enumeration]
        pub enum Status { Active, Inactive, Pending }

        #[hsrs::value_type]
        pub struct Config {
            pub max_size: u64,
        }

        #[hsrs::value_type]
        pub struct Error {
            pub code: u32,
        }

        #[hsrs::module(value_types(Config, Error))]
        mod engine {
            #[hsrs::data_type]
            pub struct Engine { x: i32 }

            impl Engine {
                #[hsrs::function]
                pub fn new(config: Config) -> Self { Self { x: 0 } }

                #[hsrs::function]
                pub fn status(&self) -> Status {}

                #[hsrs::function]
                pub fn items(&self) -> Vec<u64> {}

                #[hsrs::function]
                pub fn name(&self) -> String {}

                #[hsrs::function]
                pub fn set_name(&mut self, name: String) {}

                #[hsrs::function]
                pub fn process(&mut self, items: Vec<Config>) -> Result<u64, Error> {}

                #[hsrs::function]
                pub fn find(&self, id: u64) -> Option<Config> {}
            }
        }
    "#;
    let output = source_to_haskell(src);

    // Enum
    assert!(output.contains("newtype Status = Status Word8"), "enum newtype: {output}");
    assert!(output.contains("pattern Active"), "pattern Active: {output}");
    assert!(output.contains("pattern Inactive"), "pattern Inactive: {output}");
    assert!(output.contains("pattern Pending"), "pattern Pending: {output}");

    // Value types
    assert!(output.contains("data Config = Config"), "Config value type: {output}");
    assert!(output.contains("data Error = Error"), "Error value type: {output}");

    // Constructor with borsh param
    assert!(output.contains("new :: Config -> IO Engine"), "constructor sig: {output}");
    assert!(output.contains("withBorshArg config"), "constructor uses withBorshArg: {output}");

    // Enum return
    assert!(output.contains("status :: Engine -> IO Status"), "enum return sig: {output}");

    // Vec return
    assert!(output.contains("items :: Engine -> IO [Word64]"), "vec return sig: {output}");

    // String return and param
    assert!(output.contains("name :: Engine -> IO Text"), "string return sig: {output}");
    assert!(output.contains("setName :: Engine -> Text -> IO ()"), "string param sig: {output}");

    // Result return with borsh param
    assert!(output.contains("process :: Engine -> [Config] -> IO (Either Error Word64)"), "result return sig: {output}");

    // Option return
    assert!(output.contains("find :: Engine -> Word64 -> IO (Maybe Config)"), "option return sig: {output}");
}

#[test]
fn multiple_modules_in_one_file() {
    let src = r#"
        #[hsrs::module]
        mod alpha {
            #[hsrs::data_type]
            pub struct Alpha { x: i32 }
            impl Alpha {
                #[hsrs::function]
                pub fn new() -> Self { Self { x: 0 } }
            }
        }

        #[hsrs::module]
        mod beta {
            #[hsrs::data_type]
            pub struct Beta { y: i32 }
            impl Beta {
                #[hsrs::function]
                pub fn new() -> Self { Self { y: 0 } }
            }
        }
    "#;
    let output = source_to_haskell(src);
    assert!(output.contains("newtype Alpha"), "Alpha newtype: {output}");
    assert!(output.contains("newtype Beta"), "Beta newtype: {output}");
    assert!(output.contains("c_alphaNew"), "Alpha FFI: {output}");
    assert!(output.contains("c_betaNew"), "Beta FFI: {output}");
}

#[test]
fn string_in_value_type_field() {
    let src = r#"
        #[hsrs::value_type]
        pub struct Person {
            pub name: String,
            pub age: u32,
        }

        #[hsrs::module(value_types(Person))]
        mod db {
            #[hsrs::data_type]
            pub struct Db { x: i32 }
            impl Db {
                #[hsrs::function]
                pub fn new() -> Self { Self { x: 0 } }
                #[hsrs::function]
                pub fn get_person(&self) -> Person {}
            }
        }
    "#;
    let output = source_to_haskell(src);
    assert!(output.contains("data Person = Person"), "Person value type: {output}");
    assert!(output.contains("personName :: Text"), "String field maps to Text: {output}");
    assert!(output.contains("personAge :: Word32"), "u32 field maps to Word32: {output}");
}

#[test]
fn vec_in_value_type_field() {
    let src = r#"
        #[hsrs::value_type]
        pub struct Batch {
            pub items: Vec<u32>,
            pub label: String,
        }

        #[hsrs::module(value_types(Batch))]
        mod proc {
            #[hsrs::data_type]
            pub struct Proc { x: i32 }
            impl Proc {
                #[hsrs::function]
                pub fn new() -> Self { Self { x: 0 } }
                #[hsrs::function]
                pub fn run(&mut self, b: Batch) {}
            }
        }
    "#;
    let output = source_to_haskell(src);
    assert!(output.contains("data Batch = Batch"), "Batch value type: {output}");
    assert!(output.contains("batchItems :: [Word32]"), "Vec<u32> field maps to [Word32]: {output}");
    assert!(output.contains("batchLabel :: Text"), "String field maps to Text: {output}");
}

#[test]
fn custom_module_name_in_output() {
    let src = r#"
        #[hsrs::enumeration]
        pub enum Dir { Up, Down }
    "#;
    let output = source_to_haskell_with_module(src, "MyApp.FFI.Bindings");
    assert!(output.contains("module MyApp.FFI.Bindings where"), "custom module name: {output}");
    assert!(!output.contains("module Bindings where"), "should not contain default module name: {output}");
}

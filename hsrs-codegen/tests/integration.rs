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
    haskell::generate(&parsed)
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

    // Borsh infrastructure present
    assert!(hs.contains("import Codec.Borsh"));
    assert!(hs.contains("import qualified Data.ByteString"));
    assert!(hs.contains("data BorshBufferRaw"));
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
    assert!(hs.contains("serialiseBorsh target"));

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

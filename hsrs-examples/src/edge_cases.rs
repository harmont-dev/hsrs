// Edge case exercises for macro expansion.
// If this compiles, all macro paths work.

#![allow(
    dead_code,
    unused_variables,
    clippy::unnecessary_wraps,
    clippy::unused_self,
    clippy::needless_pass_by_value,
    clippy::no_effect_underscore_binding,
    clippy::new_without_default,
    clippy::must_use_candidate,
)]

// --- Enum edge cases ---

#[hsrs::enumeration]
pub enum SingleVariant {
    Only,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
#[hsrs::enumeration]
pub enum ManyVariants {
    V0,
    V1,
    V2,
    V3,
    V4,
    V5,
    V6,
    V7,
}

#[hsrs::enumeration]
pub enum BareEnum {
    A,
    B,
}

// --- Value type edge cases ---

#[hsrs::value_type]
pub struct OneField {
    pub val: i64,
}

#[hsrs::value_type]
pub struct AllPrimitives {
    pub a: i8,
    pub b: i16,
    pub c: i32,
    pub d: i64,
    pub e: u8,
    pub f: u16,
    pub g: u32,
    pub h: u64,
    pub i: bool,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
#[hsrs::value_type]
pub struct DerivedValueType {
    pub x: i32,
}

// --- Module edge cases ---

#[hsrs::module]
mod bare_module {
    #[hsrs::data_type]
    pub struct BareModule {
        val: i32,
    }

    impl BareModule {
        #[hsrs::function]
        pub fn new() -> Self {
            Self { val: 0 }
        }
    }
}

#[hsrs::module]
mod constructor_only {
    #[hsrs::data_type]
    pub struct ConstructorOnly {
        x: i32,
    }

    impl ConstructorOnly {
        #[hsrs::function]
        pub fn new() -> Self {
            Self { x: 0 }
        }
    }
}

#[hsrs::module]
mod all_method_kinds {
    #[hsrs::data_type]
    pub struct AllKinds {
        val: i64,
    }

    impl AllKinds {
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

        #[hsrs::function]
        pub fn void_ref(&self) {}

        #[hsrs::function]
        pub fn void_mut(&mut self) {}
    }
}

#[hsrs::module]
mod many_params {
    #[hsrs::data_type]
    pub struct ManyParams {
        val: i32,
    }

    impl ManyParams {
        #[hsrs::function]
        pub fn new() -> Self {
            Self { val: 0 }
        }

        #[hsrs::function]
        pub fn all_ints(&mut self, a: i8, b: i16, c: i32, d: i64) {}

        #[hsrs::function]
        pub fn all_uints(&mut self, a: u8, b: u16, c: u32, d: u64) {}

        #[hsrs::function]
        pub fn bool_and_sizes(&mut self, flag: bool, us: usize, is: isize) {}
    }
}

#[hsrs::module]
mod enum_user {
    #[hsrs::data_type]
    pub struct EnumUser {
        val: i32,
    }

    impl EnumUser {
        #[hsrs::function]
        pub fn new() -> Self {
            Self { val: 0 }
        }

        #[hsrs::function]
        pub fn set_single(&mut self, s: SingleVariant) {
            let _ = s;
        }

        #[hsrs::function]
        pub fn set_many(&mut self, m: ManyVariants) {
            let _ = m;
        }
    }
}

#[hsrs::module(value_types(OneField, AllPrimitives))]
mod borsh_user {
    #[hsrs::data_type]
    pub struct BorshUser {
        val: i32,
    }

    impl BorshUser {
        #[hsrs::function]
        pub fn new() -> Self {
            Self { val: 0 }
        }

        #[hsrs::function]
        pub fn get_field(&self) -> OneField {
            OneField { val: 0 }
        }

        #[hsrs::function]
        pub fn set_field(&mut self, f: OneField) {
            let _ = f;
        }

        #[hsrs::function]
        pub fn roundtrip(&mut self, p: AllPrimitives) -> OneField {
            let _ = p;
            OneField { val: 0 }
        }
    }
}

#[hsrs::module(value_types(OneField))]
mod fallible {
    #[hsrs::data_type]
    pub struct Fallible {
        val: i32,
    }

    impl Fallible {
        #[hsrs::function]
        pub fn new() -> Self {
            Self { val: 0 }
        }

        #[hsrs::function]
        pub fn try_get(&self) -> Result<i64, OneField> {
            Ok(0)
        }

        #[hsrs::function]
        pub fn maybe_get(&self) -> Option<i64> {
            None
        }
    }
}

#[hsrs::module(safety = unsafe)]
mod fast_module {
    #[hsrs::data_type]
    pub struct FastModule {
        val: i32,
    }

    impl FastModule {
        #[hsrs::function]
        pub fn new() -> Self {
            Self { val: 0 }
        }

        #[hsrs::function]
        pub fn fast_get(&self) -> i32 {
            self.val
        }

        #[hsrs::function(safe)]
        pub fn safe_get(&self) -> i32 {
            self.val
        }

        #[hsrs::function(interruptible)]
        pub fn blocking_get(&self) -> i32 {
            self.val
        }
    }
}

#[hsrs::module(value_types(OneField), safety = unsafe)]
mod combined_attrs {
    #[hsrs::data_type]
    pub struct Combined {
        val: i32,
    }

    impl Combined {
        #[hsrs::function]
        pub fn new() -> Self {
            Self { val: 0 }
        }

        #[hsrs::function]
        pub fn get(&self) -> OneField {
            OneField { val: 0 }
        }

        #[hsrs::function(safe)]
        pub fn slow_get(&self) -> OneField {
            OneField { val: 0 }
        }
    }
}

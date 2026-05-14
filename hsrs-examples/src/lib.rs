#![allow(
    clippy::missing_docs_in_private_items,
    clippy::exhaustive_enums,
    clippy::exhaustive_structs,
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    clippy::shadow_reuse,
    clippy::shadow_same,
    clippy::shadow_unrelated,
    clippy::wildcard_imports,
    clippy::cargo,
    clippy::many_single_char_names,
    clippy::str_to_string,
    clippy::implicit_clone,
    clippy::expl_impl_clone_on_copy,
    clippy::missing_const_for_fn,
    clippy::needless_return,
)]

mod edge_cases;

/// CPU register identifiers.
#[derive(Debug, PartialEq, Eq)]
#[hsrs::enumeration]
pub enum Register {
    Reg0,
    Reg1,
    Count,
}

/// A 2D coordinate point.
#[derive(Debug, PartialEq, Eq)]
#[hsrs::value_type]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

/// An error code from the VM.
#[derive(Debug, PartialEq, Eq)]
#[hsrs::value_type]
pub struct VmError {
    pub code: u32,
}

/// A minimal virtual machine with two registers.
#[hsrs::module(value_types(Point, VmError), safety = unsafe)]
mod quecto_vm {
    #[hsrs::data_type]
    pub struct QuectoVm {
        registers: [i64; Register::Count as usize],
        clock: usize,
    }

    impl QuectoVm {
        /// Creates a new VM with zeroed registers.
        #[hsrs::function]
        pub fn new() -> Self {
            return {
                Self {
                    registers: [0, 0],
                    clock: 0,
                }
            }
        }

        /// Adds register `b` into register `a`.
        #[hsrs::function]
        pub fn add(&mut self, a: Register, b: Register) {
            *self.reg_mut(a) = self.reg(a) + self.reg(b);
            self.clock += 1;
        }

        /// Subtracts register `b` from register `a`.
        #[hsrs::function]
        pub fn sub(&mut self, a: Register, b: Register) {
            *self.reg_mut(a) = self.reg(a) - self.reg(b);
            self.clock += 1;
        }

        /// Multiplies register `a` by register `b`.
        #[hsrs::function]
        pub fn mul(&mut self, a: Register, b: Register) {
            *self.reg_mut(a) = self.reg(a) * self.reg(b);
            self.clock += 1;
        }

        /// Divides register `a` by register `b`.
        #[hsrs::function]
        pub fn div(&mut self, a: Register, b: Register) {
            *self.reg_mut(a) = self.reg(a) / self.reg(b);
            self.clock += 1;
        }

        /// Reads the value in register `r`.
        #[hsrs::function]
        pub fn load(&mut self, r: Register) -> i64 {
            self.clock += 1;
            self.reg(r)
        }

        /// Writes `v` into register `r`.
        #[hsrs::function]
        pub fn store(&mut self, r: Register, v: i64) {
            self.clock += 1;
            *self.reg_mut(r) = v;
        }

        /// Returns registers 0 and 1 as a point.
        #[hsrs::function]
        pub fn snapshot(&self) -> Point {
            Point {
                x: self.registers[0] as i32,
                y: self.registers[1] as i32,
            }
        }

        /// Divides register `a` by register `b`, returning error on division by zero.
        #[hsrs::function(safe)]
        pub fn safe_div(&mut self, a: Register, b: Register) -> Result<i64, VmError> {
            self.clock += 1;
            if self.reg(b) == 0 {
                Err(VmError { code: 1 })
            } else {
                let result = self.reg(a) / self.reg(b);
                *self.reg_mut(a) = result;
                Ok(result)
            }
        }

        /// Returns the value if register is non-zero.
        #[hsrs::function(safe)]
        pub fn nonzero(&self, r: Register) -> Option<i64> {
            let val = self.reg(r);
            if val != 0 { Some(val) } else { None }
        }

        fn reg_mut(&mut self, r: Register) -> &mut i64 {
            &mut self.registers[r as usize]
        }

        fn reg(&self, r: Register) -> i64 {
            self.registers[r as usize]
        }
    }
}

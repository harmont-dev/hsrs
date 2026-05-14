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
        pub fn new() -> Self {
            return {
                Self {
                    registers: [0, 0],
                    clock: 0,
                }
            }
        }

        #[hsrs::function]
        pub fn add(&mut self, a: Register, b: Register) {
            *self.reg_mut(a) = self.reg(a) + self.reg(b);
            self.clock += 1;
        }

        #[hsrs::function]
        pub fn sub(&mut self, a: Register, b: Register) {
            *self.reg_mut(a) = self.reg(a) - self.reg(b);
            self.clock += 1;
        }

        #[hsrs::function]
        pub fn mul(&mut self, a: Register, b: Register) {
            *self.reg_mut(a) = self.reg(a) * self.reg(b);
            self.clock += 1;
        }

        #[hsrs::function]
        pub fn div(&mut self, a: Register, b: Register) {
            *self.reg_mut(a) = self.reg(a) / self.reg(b);
            self.clock += 1;
        }

        #[hsrs::function]
        pub fn load(&mut self, r: Register) -> i64 {
            self.clock += 1;
            self.reg(r)
        }

        #[hsrs::function]
        pub fn store(&mut self, r: Register, v: i64) {
            self.clock += 1;
            *self.reg_mut(r) = v;
        }

        fn reg_mut(&mut self, r: Register) -> &mut i64 {
            &mut self.registers[r as usize]
        }

        fn reg(&self, r: Register) -> i64 {
            self.registers[r as usize]
        }
    }
}

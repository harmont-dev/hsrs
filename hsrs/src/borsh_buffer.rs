use safer_ffi::prelude::*;

/// Opaque byte buffer holding borsh-serialized data for FFI transfer.
#[derive_ReprC]
#[repr(opaque)]
pub struct BorshBuffer {
    /// The borsh-serialized bytes.
    bytes: Vec<u8>,
}

impl BorshBuffer {
    /// Serialize a value into a new buffer.
    pub fn from_borsh<T: borsh::BorshSerialize>(val: &T) -> Self {
        borsh::to_vec(val).map_or_else(|_| Self { bytes: Vec::new() }, |bytes| Self { bytes })
    }
}

/// Returns the byte length of the serialized data.
#[allow(clippy::missing_docs_in_private_items)]
#[ffi_export]
fn hsrs_borsh_len(buf: &BorshBuffer) -> u64 {
    buf.bytes.len() as u64
}

/// Returns a pointer to the serialized data bytes.
#[allow(clippy::missing_docs_in_private_items)]
#[ffi_export]
fn hsrs_borsh_ptr(buf: &BorshBuffer) -> *const u8 {
    buf.bytes.as_ptr()
}

/// Frees the `BorshBuffer` and its backing memory.
#[allow(clippy::needless_pass_by_value, clippy::missing_docs_in_private_items)]
#[ffi_export]
fn hsrs_borsh_free(buf: repr_c::Box<BorshBuffer>) {
    drop(buf);
}

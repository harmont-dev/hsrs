/// Deserialize a borsh value from a raw byte pointer and length.
///
/// # Safety
/// `ptr` must point to `len` readable bytes.
#[allow(unsafe_code, clippy::expect_used)]
pub fn borsh_deserialize<T: borsh::BorshDeserialize>(ptr: *const u8, len: u64) -> T {
    let bytes = unsafe { core::slice::from_raw_parts(ptr, len as usize) };
    borsh::from_slice(bytes).expect("hsrs: borsh deserialization failed")
}

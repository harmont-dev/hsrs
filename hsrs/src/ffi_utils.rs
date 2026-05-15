/// Deserialize a borsh value from a raw byte pointer and length.
///
/// # Safety
/// `ptr` must point to `len` readable bytes.
///
/// # Panics
/// Panics if borsh deserialization fails.
#[must_use]
#[allow(unsafe_code, clippy::expect_used, clippy::cast_possible_truncation)]
pub unsafe fn borsh_deserialize<T: borsh::BorshDeserialize>(ptr: *const u8, len: u64) -> T {
    let bytes = unsafe { core::slice::from_raw_parts(ptr, len as usize) };
    borsh::from_slice(bytes).expect("hsrs: borsh deserialization failed")
}

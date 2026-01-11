pub fn cast_slice(s: &[u32]) -> &[u8] {
    // SAFETY: it is always valid to cast a `u32` slice to a slice of `u8`s as
    // they have lower alignment requirements and there is no padding
    unsafe { core::slice::from_raw_parts(s.as_ptr().cast(), core::mem::size_of_val(s)) }
}

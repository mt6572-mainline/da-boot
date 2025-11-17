pub mod fuzzy;

/// Search in the `slice` for the `pattern`
///
/// Returns `None` if not found
#[inline]
#[must_use]
pub fn search(slice: &[u8], pattern: &[u8]) -> Option<usize> {
    (0..slice.len()).find(|&i| slice[i..].starts_with(pattern))
}

/// Replace in `slice` starting with `at` position with `replacement`
#[inline]
pub fn replace(slice: &mut [u8], at: usize, replacement: &[u8]) {
    slice[at..at + replacement.len()].clone_from_slice(replacement);
}

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

/// Compares fixed-size digests so the first differing secret byte does not
/// control comparison time. Hashing itself still scales with input length.
pub fn constant_time_secret_eq(expected: &str, supplied: &str) -> bool {
    let expected = Sha256::digest(expected.as_bytes());
    let supplied = Sha256::digest(supplied.as_bytes());
    bool::from(expected.ct_eq(&supplied))
}

/// Constant-time comparison for already decoded, fixed-width signature bytes.
pub fn constant_time_bytes_eq(expected: &[u8], supplied: &[u8]) -> bool {
    expected.len() == supplied.len() && bool::from(expected.ct_eq(supplied))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_comparison_covers_equal_different_and_different_length_values() {
        assert!(constant_time_secret_eq("secret", "secret"));
        assert!(!constant_time_secret_eq("secret", "secreu"));
        assert!(!constant_time_secret_eq("secret", "short"));
        assert!(constant_time_bytes_eq(&[1, 2, 3], &[1, 2, 3]));
        assert!(!constant_time_bytes_eq(&[1, 2, 3], &[1, 2, 4]));
        assert!(!constant_time_bytes_eq(&[1, 2, 3], &[1, 2]));
    }
}

pub(crate) fn timestamp_before(timestamp: i64, seconds: i64) -> i64 {
    timestamp.saturating_sub(seconds)
}

pub(crate) fn timestamp_after(timestamp: i64, seconds: i64) -> i64 {
    timestamp.saturating_add(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_arithmetic_saturates_at_i64_bounds() {
        assert_eq!(timestamp_before(1_000, 300), 700);
        assert_eq!(timestamp_after(1_000, 300), 1_300);
        assert_eq!(timestamp_before(i64::MIN, 1), i64::MIN);
        assert_eq!(timestamp_after(i64::MAX, 1), i64::MAX);
    }
}

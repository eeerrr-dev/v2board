pub(crate) fn finish_item_batch(
    kind: &str,
    total: usize,
    failed: usize,
    first_error: Option<String>,
) -> anyhow::Result<()> {
    if failed == 0 {
        return Ok(());
    }
    anyhow::bail!(
        "{failed} of {total} {kind} failed; first error: {}",
        first_error.as_deref().unwrap_or("unknown error")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_item_batches_fail_the_outer_job_metric() {
        assert!(finish_item_batch("orders", 3, 0, None).is_ok());
        assert!(
            finish_item_batch("orders", 3, 1, Some("broken".to_string()))
                .unwrap_err()
                .to_string()
                .contains("1 of 3 orders failed")
        );
    }
}

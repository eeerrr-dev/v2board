use super::*;
use serde::Deserialize;

// === W11 modern commerce family (docs/api-dialect.md §6.2/§6.4) ===
//
// Plans, payments, orders, and payment reconciliations on dialect-v2
// semantics: JSON bodies, §4.4 double-Option partial updates, §4.5 RFC 3339
// timestamps, §1 201 `{id}`/`{trade_no}` creates, §7 DSL order filtering,
// §8 pagination, and typed §3.4 problem codes. Since W14 the §6.9 staff
// mirror consumes the same modern `plans_list`.

mod orders;
mod payments;
mod plans;
mod reconciliations;

pub use orders::{OrderAssign, OrderPatch};
pub use payments::{AdminPaymentItem, PaymentCreate, PaymentPatch};
pub use plans::{AdminPlanItem, PlanCreate, PlanPatch};
pub use reconciliations::ReconciliationResolveRequest;

#[cfg(test)]
pub(super) use payments::{parse_payment_config, resolve_redacted_payment_config};
#[cfg(test)]
pub(super) use reconciliations::{reconciliation_resolution, reconciliation_resolved_filter};

fn payment_reconciliation_identity_hash(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}

fn nonnegative_i32(field: &str, value: i64) -> Result<i64, ApiError> {
    if !(0..=i64::from(i32::MAX)).contains(&value) {
        return Err(ApiError::from(Problem::validation_field(
            field,
            "Value must be a non-negative 32-bit integer",
        )));
    }
    Ok(value)
}

pub(super) fn optional_nonnegative_i32(
    field: &str,
    value: Option<i64>,
) -> Result<Option<i64>, ApiError> {
    value.map(|value| nonnegative_i32(field, value)).transpose()
}

fn optional_smallint(field: &str, value: Option<i64>) -> Result<Option<i64>, ApiError> {
    if value.is_some_and(|value| i16::try_from(value).is_err()) {
        return Err(ApiError::from(Problem::validation_field(
            field,
            "Value must be a 16-bit integer",
        )));
    }
    Ok(value)
}

/// POST `plans/sort` / POST `payments/sort` (§6.2): JSON `{ids}` full
/// resequencing (the legacy `plan_ids` key becomes `ids` per §4.1).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SortIdsRequest {
    pub ids: Vec<i64>,
}

#[cfg(test)]
mod tests;

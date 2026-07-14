use serde::Serialize;
use sqlx::{FromRow, PgPool};

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct PaymentMethodRow {
    pub id: i32,
    pub name: String,
    pub payment: String,
    pub icon: Option<String>,
    pub handling_fee_fixed: Option<i32>,
    // Eloquent returns the `decimal(5,2)` column verbatim as its string form
    // (e.g. "0.50"), never as a JSON number. PostgreSQL's `::text` preserves that
    // scale so the emitted value matches Laravel's PaymentController::getPaymentMethod.
    pub handling_fee_percent: Option<String>,
}

pub async fn fetch_enabled_payment_methods(
    pool: &PgPool,
) -> Result<Vec<PaymentMethodRow>, sqlx::Error> {
    sqlx::query_as::<_, PaymentMethodRow>(
        r#"
        SELECT
            id,
            name,
            payment,
            icon,
            handling_fee_fixed,
            handling_fee_percent::text AS handling_fee_percent
        FROM payment_method
        WHERE enable = 1 AND archived_at IS NULL
        ORDER BY sort ASC NULLS FIRST
        "#,
    )
    .fetch_all(pool)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handling_fee_percent_serializes_as_decimal_string() {
        let row = PaymentMethodRow {
            id: 1,
            name: "Alipay".into(),
            payment: "AlipayF2F".into(),
            icon: None,
            handling_fee_fixed: None,
            handling_fee_percent: Some("0.50".into()),
        };
        let value = serde_json::to_value(&row).unwrap();
        assert_eq!(value["handling_fee_percent"], serde_json::json!("0.50"));
    }

    #[test]
    fn handling_fee_percent_null_serializes_as_null() {
        let row = PaymentMethodRow {
            id: 1,
            name: "Alipay".into(),
            payment: "AlipayF2F".into(),
            icon: None,
            handling_fee_fixed: None,
            handling_fee_percent: None,
        };
        let value = serde_json::to_value(&row).unwrap();
        assert!(value["handling_fee_percent"].is_null());
    }

    #[test]
    fn webhook_routing_key_is_unique_per_payment_driver() {
        let finalize = include_str!("../../../migrations-postgres/0002_import_finalize.sql");
        assert!(
            finalize.contains("CONSTRAINT uniq_payment_method_driver_uuid UNIQUE (payment, uuid)")
        );
    }
}

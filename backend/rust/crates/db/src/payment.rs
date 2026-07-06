use serde::Serialize;
use sqlx::{FromRow, MySqlPool};

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct PaymentMethodRow {
    pub id: i32,
    pub name: String,
    pub payment: String,
    pub icon: Option<String>,
    pub handling_fee_fixed: Option<i32>,
    // Eloquent returns the `decimal(5,2)` column verbatim as its string form
    // (e.g. "0.50"), never as a JSON number. `CAST(... AS CHAR)` preserves that
    // scale so the emitted value matches Laravel's PaymentController::getPaymentMethod.
    pub handling_fee_percent: Option<String>,
}

#[derive(Debug, FromRow)]
struct PaymentConfigRow {
    config: String,
}

pub async fn fetch_enabled_payment_methods(
    pool: &MySqlPool,
) -> Result<Vec<PaymentMethodRow>, sqlx::Error> {
    sqlx::query_as::<_, PaymentMethodRow>(
        r#"
        SELECT
            id,
            name,
            payment,
            icon,
            handling_fee_fixed,
            CAST(handling_fee_percent AS CHAR) AS handling_fee_percent
        FROM v2_payment
        WHERE enable = 1
        ORDER BY sort ASC
        "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn find_stripe_public_key(
    pool: &MySqlPool,
    id: i32,
) -> Result<Option<String>, sqlx::Error> {
    let row = sqlx::query_as::<_, PaymentConfigRow>(
        r#"
        SELECT config
        FROM v2_payment
        WHERE id = ? AND payment = 'StripeCredit'
        LIMIT 1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.and_then(|row| {
        serde_json::from_str::<serde_json::Value>(&row.config)
            .ok()
            .and_then(|config| {
                config
                    .get("stripe_pk_live")
                    .and_then(|value| value.as_str())
                    .map(ToOwned::to_owned)
            })
    }))
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
}

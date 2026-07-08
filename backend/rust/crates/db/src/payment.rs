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

/// The `stripe_pk_live` value inside a StripeCredit payment's config JSON.
/// Mirrors PHP's `$payment->config['stripe_pk_live']`, which yields `null`
/// (not an error) when the key is absent, the value is null, or the config is
/// unusable — so a present row with no key resolves to `None`, never a failure.
fn stripe_pk_from_config(config: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(config)
        .ok()
        .and_then(|config| {
            config
                .get("stripe_pk_live")
                .and_then(|value| value.as_str())
                .map(ToOwned::to_owned)
        })
}

/// Returns `None` when no StripeCredit gate has this id (Laravel aborts 500
/// 'payment is not found' there); `Some(inner)` when the gate exists, where
/// `inner` is the `stripe_pk_live` value or `None` if the key is missing/null
/// (Laravel returns `{"data": null}` at HTTP 200 for that case).
pub async fn find_stripe_public_key(
    pool: &MySqlPool,
    id: i32,
) -> Result<Option<Option<String>>, sqlx::Error> {
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

    Ok(row.map(|row| stripe_pk_from_config(&row.config)))
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
    fn stripe_pk_from_config_reads_key_or_yields_none() {
        // Present key -> the string value.
        assert_eq!(
            stripe_pk_from_config(r#"{"stripe_pk_live":"pk_live_abc"}"#),
            Some("pk_live_abc".to_string())
        );
        // Missing key, explicit null, and unparseable config all yield None, so the
        // handler serves `{"data": null}` at 200 rather than treating them as errors.
        assert_eq!(
            stripe_pk_from_config(r#"{"stripe_sk_live":"secret"}"#),
            None
        );
        assert_eq!(stripe_pk_from_config(r#"{"stripe_pk_live":null}"#), None);
        assert_eq!(stripe_pk_from_config("not json"), None);
    }
}

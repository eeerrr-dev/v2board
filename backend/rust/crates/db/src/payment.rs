use serde::Serialize;
use sqlx::{FromRow, MySqlPool};

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct PaymentMethodRow {
    pub id: i32,
    pub name: String,
    pub payment: String,
    pub icon: Option<String>,
    pub handling_fee_fixed: Option<i32>,
    pub handling_fee_percent: Option<f64>,
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
            CAST(handling_fee_percent AS DOUBLE) AS handling_fee_percent
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

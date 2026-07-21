//! Admin/staff TOTP MFA rows (`admin_mfa`): sealed secret storage plus the
//! compare-and-set step consumption that makes every accepted code
//! one-time-use even under concurrent logins.

use sqlx::{FromRow, PgPool};

#[derive(FromRow)]
pub struct AdminMfaRow {
    pub secret_nonce: Vec<u8>,
    pub secret_ciphertext: Vec<u8>,
    pub secret_tag: Vec<u8>,
    pub enabled_at: Option<i64>,
    pub last_step: i64,
}

pub async fn find(pool: &PgPool, user_id: i64) -> Result<Option<AdminMfaRow>, sqlx::Error> {
    sqlx::query_as::<_, AdminMfaRow>(
        r#"
        SELECT secret_nonce, secret_ciphertext, secret_tag, enabled_at, last_step
        FROM admin_mfa
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Store (or replace) a pending enrollment secret. An already-enabled row is
/// never overwritten — re-enrollment requires an explicit disable first — so
/// the upsert returns 0 rows in that case.
pub async fn upsert_pending(
    pool: &PgPool,
    user_id: i64,
    nonce: &[u8],
    ciphertext: &[u8],
    tag: &[u8],
    now: i64,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        INSERT INTO admin_mfa
            (user_id, secret_nonce, secret_ciphertext, secret_tag, enabled_at, last_step, created_at, updated_at)
        VALUES ($1, $2, $3, $4, NULL, 0, $5, $5)
        ON CONFLICT (user_id) DO UPDATE SET
            secret_nonce = EXCLUDED.secret_nonce,
            secret_ciphertext = EXCLUDED.secret_ciphertext,
            secret_tag = EXCLUDED.secret_tag,
            enabled_at = NULL,
            last_step = 0,
            updated_at = EXCLUDED.updated_at
        WHERE admin_mfa.enabled_at IS NULL
        "#,
    )
    .bind(user_id)
    .bind(nonce)
    .bind(ciphertext)
    .bind(tag)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// Flip a pending enrollment to enabled, consuming the confirming code's
/// time-step. Returns 0 rows if the enrollment is missing or already enabled.
pub async fn enable(
    pool: &PgPool,
    user_id: i64,
    accepted_step: i64,
    now: i64,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE admin_mfa
        SET enabled_at = $3, last_step = $2, updated_at = $3
        WHERE user_id = $1 AND enabled_at IS NULL AND last_step < $2
        "#,
    )
    .bind(user_id)
    .bind(accepted_step)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// Consume an accepted login code's time-step. The `last_step < $2` guard is
/// the replay protection: a concurrent login that already consumed this step
/// makes the second attempt fail with 0 rows.
pub async fn consume_step(
    pool: &PgPool,
    user_id: i64,
    accepted_step: i64,
    now: i64,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE admin_mfa
        SET last_step = $2, updated_at = $3
        WHERE user_id = $1 AND enabled_at IS NOT NULL AND last_step < $2
        "#,
    )
    .bind(user_id)
    .bind(accepted_step)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// Disable an enabled enrollment, consuming the authorizing code's time-step
/// in the same compare-and-set so the code cannot be replayed elsewhere.
pub async fn disable(pool: &PgPool, user_id: i64, accepted_step: i64) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        DELETE FROM admin_mfa
        WHERE user_id = $1 AND enabled_at IS NOT NULL AND last_step < $2
        "#,
    )
    .bind(user_id)
    .bind(accepted_step)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

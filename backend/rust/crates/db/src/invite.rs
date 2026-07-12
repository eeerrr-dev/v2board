use serde::Serialize;
use sqlx::{FromRow, MySql, MySqlPool, Transaction};

const VALID_COMMISSION_SUM_SQL: &str = "SELECT CAST(COALESCE(SUM(get_amount), 0) AS CHAR) FROM v2_commission_log WHERE invite_user_id = ?";
const PENDING_COMMISSION_SUM_SQL: &str = r#"
    SELECT CAST(COALESCE(SUM(commission_balance), 0) AS CHAR)
    FROM v2_order
    WHERE status = 3 AND commission_status = 0 AND invite_user_id = ?
"#;

fn exact_i64_aggregate(value: &str, metric: &str) -> Result<i64, sqlx::Error> {
    let invalid = |reason: &str| {
        sqlx::Error::Decode(
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("{metric} aggregate {reason}"),
            )
            .into(),
        )
    };
    let exact = value
        .parse::<i128>()
        .map_err(|_| invalid("is not a valid integer"))?;
    i64::try_from(exact).map_err(|_| invalid("exceeds the supported range"))
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct InviteCodeRow {
    pub id: i32,
    pub user_id: i64,
    pub code: String,
    pub status: i8,
    pub pv: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct CommissionDetailRow {
    pub id: i32,
    pub trade_no: String,
    pub order_amount: i32,
    pub get_amount: i32,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct InviteFetchRow {
    pub codes: Vec<InviteCodeRow>,
    pub stat: [i64; 5],
}

#[derive(Debug, Clone, FromRow)]
pub struct InviteUserRow {
    pub commission_rate: Option<i32>,
    pub commission_balance: i32,
}

pub async fn create_invite_code(
    pool: &MySqlPool,
    user_id: i64,
    limit: i64,
    now: i64,
) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM v2_invite_code WHERE user_id = ? AND status = 0 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;
    if count >= limit {
        tx.rollback().await?;
        return Ok(false);
    }
    insert_invite_code(&mut tx, user_id, now).await?;
    tx.commit().await?;
    Ok(true)
}

pub async fn fetch_invite(pool: &MySqlPool, user_id: i64) -> Result<InviteFetchRow, sqlx::Error> {
    let codes = sqlx::query_as::<_, InviteCodeRow>(
        r#"
        SELECT id, user_id, code, status, pv, created_at, updated_at
        FROM v2_invite_code
        WHERE user_id = ? AND status = 0
        ORDER BY id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    let user = sqlx::query_as::<_, InviteUserRow>(
        "SELECT commission_rate, commission_balance FROM v2_user WHERE id = ? LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    let registered: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM v2_user WHERE invite_user_id = ?")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    let valid_commission = exact_i64_aggregate(
        &sqlx::query_scalar::<_, String>(VALID_COMMISSION_SUM_SQL)
            .bind(user_id)
            .fetch_one(pool)
            .await?,
        "valid commission",
    )?;
    let pending_commission = exact_i64_aggregate(
        &sqlx::query_scalar::<_, String>(PENDING_COMMISSION_SUM_SQL)
            .bind(user_id)
            .fetch_one(pool)
            .await?,
        "pending commission",
    )?;
    let commission_rate = user
        .as_ref()
        .and_then(|user| user.commission_rate)
        .map(i64::from)
        .unwrap_or(10);
    let available_commission = user
        .map(|user| i64::from(user.commission_balance))
        .unwrap_or_default();

    Ok(InviteFetchRow {
        codes,
        stat: [
            registered,
            valid_commission,
            pending_commission,
            commission_rate,
            available_commission,
        ],
    })
}

pub async fn fetch_commission_details(
    pool: &MySqlPool,
    user_id: i64,
    page_size: i64,
    offset: i64,
) -> Result<(Vec<CommissionDetailRow>, i64), sqlx::Error> {
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM v2_commission_log WHERE invite_user_id = ? AND get_amount > 0",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    let rows = sqlx::query_as::<_, CommissionDetailRow>(
        r#"
        SELECT id, trade_no, order_amount, get_amount, created_at
        FROM v2_commission_log
        WHERE invite_user_id = ? AND get_amount > 0
        ORDER BY created_at DESC
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(user_id)
    .bind(page_size)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok((rows, total))
}

async fn insert_invite_code(
    tx: &mut Transaction<'_, MySql>,
    user_id: i64,
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO v2_invite_code (user_id, code, status, pv, created_at, updated_at)
        VALUES (?, SUBSTRING(MD5(CONCAT(UUID(), RAND())), 1, 8), 0, 0, ?, ?)
        "#,
    )
    .bind(user_id)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commission_aggregates_preserve_exact_values_and_reject_i64_overflow() {
        assert!(VALID_COMMISSION_SUM_SQL.contains("AS CHAR"));
        assert!(PENDING_COMMISSION_SUM_SQL.contains("AS CHAR"));
        assert!(!VALID_COMMISSION_SUM_SQL.contains("AS SIGNED"));
        assert!(!PENDING_COMMISSION_SUM_SQL.contains("AS SIGNED"));

        assert_eq!(exact_i64_aggregate("0", "test").unwrap(), 0);
        assert_eq!(
            exact_i64_aggregate("9223372036854775807", "test").unwrap(),
            i64::MAX
        );
        assert_eq!(
            exact_i64_aggregate("-9223372036854775808", "test").unwrap(),
            i64::MIN
        );
        assert!(exact_i64_aggregate("9223372036854775808", "test").is_err());
        assert!(exact_i64_aggregate("-9223372036854775809", "test").is_err());
        assert!(exact_i64_aggregate("not-a-number", "test").is_err());
    }
}

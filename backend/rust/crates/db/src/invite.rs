use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

const VALID_COMMISSION_SUM_SQL: &str =
    "SELECT COALESCE(SUM(get_amount), 0)::text FROM commission_log WHERE invite_user_id = $1";
const PENDING_COMMISSION_SUM_SQL: &str = r#"
    SELECT COALESCE(SUM(commission_balance), 0)::text
    FROM orders
    WHERE status = 3 AND commission_status = 0 AND invite_user_id = $1
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

#[derive(Debug, Clone, FromRow)]
pub struct InviteCodeRow {
    pub id: i32,
    pub user_id: i64,
    pub code: String,
    pub status: i16,
    pub pv: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct CommissionDetailRow {
    pub id: i64,
    pub trade_no: String,
    pub order_amount: i32,
    pub get_amount: i32,
    pub created_at: i64,
}

/// The invite overview stat, named per docs/api-dialect.md §9.2 (was the
/// legacy 5-tuple `[registered, valid_commission, pending_commission,
/// commission_rate, available_commission]`). Commissions are integer cents;
/// `commission_rate` is an integer percent (default 10 when unset).
#[derive(Debug, Clone, Copy)]
pub struct InviteStat {
    pub registered_count: i64,
    pub valid_commission: i64,
    pub pending_commission: i64,
    pub commission_rate: i64,
    pub available_commission: i64,
}

#[derive(Debug, Clone)]
pub struct InviteFetchRow {
    pub codes: Vec<InviteCodeRow>,
    pub stat: InviteStat,
}

#[derive(Debug, Clone, FromRow)]
pub struct InviteUserRow {
    pub commission_rate: Option<i32>,
    pub commission_balance: i32,
}

pub async fn create_invite_code(
    pool: &PgPool,
    user_id: i64,
    limit: i64,
    now: i64,
) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let user_exists =
        sqlx::query_scalar::<_, i64>("SELECT id FROM users WHERE id = $1 LIMIT 1 FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
    if user_exists.is_none() {
        tx.rollback().await?;
        return Ok(false);
    }
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM invite_code WHERE user_id = $1 AND status = 0")
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

pub async fn fetch_invite(pool: &PgPool, user_id: i64) -> Result<InviteFetchRow, sqlx::Error> {
    let codes = sqlx::query_as::<_, InviteCodeRow>(
        r#"
        SELECT id, user_id, code, status, pv, created_at, updated_at
        FROM invite_code
        WHERE user_id = $1 AND status = 0
        ORDER BY id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    let user = sqlx::query_as::<_, InviteUserRow>(
        "SELECT commission_rate, commission_balance FROM users WHERE id = $1 LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    let registered: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE invite_user_id = $1")
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
        stat: InviteStat {
            registered_count: registered,
            valid_commission,
            pending_commission,
            commission_rate,
            available_commission,
        },
    })
}

pub async fn fetch_commission_details(
    pool: &PgPool,
    user_id: i64,
    page_size: i64,
    offset: i64,
) -> Result<(Vec<CommissionDetailRow>, i64), sqlx::Error> {
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM commission_log WHERE invite_user_id = $1 AND get_amount > 0",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    let rows = sqlx::query_as::<_, CommissionDetailRow>(
        r#"
        SELECT id, trade_no, order_amount, get_amount, created_at
        FROM commission_log
        WHERE invite_user_id = $1 AND get_amount > 0
        ORDER BY created_at DESC
        LIMIT $2 OFFSET $3
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
    tx: &mut Transaction<'_, Postgres>,
    user_id: i64,
    now: i64,
) -> Result<(), sqlx::Error> {
    for _ in 0..8 {
        let code = random_invite_code();
        let result = sqlx::query(
            r#"
            INSERT INTO invite_code (user_id, code, status, pv, created_at, updated_at)
            VALUES ($1, $2, 0, 0, $3, $4)
            "#,
        )
        .bind(user_id)
        .bind(code)
        .bind(now)
        .bind(now)
        .execute(&mut **tx)
        .await;
        match result {
            Ok(_) => return Ok(()),
            Err(error)
                if error
                    .as_database_error()
                    .is_some_and(|error| error.is_unique_violation()) => {}
            Err(error) => return Err(error),
        }
    }
    Err(sqlx::Error::Protocol(
        "could not allocate a unique invitation code after 8 attempts".to_string(),
    ))
}

fn random_invite_code() -> String {
    Uuid::new_v4().simple().to_string()[..8].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commission_aggregates_preserve_exact_values_and_reject_i64_overflow() {
        assert!(VALID_COMMISSION_SUM_SQL.contains("::text"));
        assert!(PENDING_COMMISSION_SUM_SQL.contains("::text"));

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

    #[test]
    fn generated_invite_codes_keep_the_eight_character_contract() {
        let code = random_invite_code();
        assert_eq!(code.len(), 8);
        assert!(code.bytes().all(|byte| byte.is_ascii_hexdigit()));

        let finalize = include_str!("../../../migrations-postgres/0002_import_finalize.sql");
        assert!(finalize.contains("uniq_invite_code_canonical"));
    }
}

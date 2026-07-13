use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgConnection, PgPool, Postgres, Transaction};
use uuid::Uuid;

const POLICY_DOMAIN: &[u8] = b"v2board.analytics-admission-policy.v1\0";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AnalyticsAdmissionPolicy {
    pub recovery_pending_rows: u64,
    pub soft_pending_rows: u64,
    pub hard_pending_rows: u64,
    pub recovery_relation_bytes: u64,
    pub soft_relation_bytes: u64,
    pub hard_relation_bytes: u64,
    pub recovery_oldest_age_seconds: u64,
    pub soft_oldest_age_seconds: u64,
    pub hard_oldest_age_seconds: u64,
    pub database_capacity_bytes: u64,
    pub hard_min_headroom_bytes: u64,
    pub soft_min_headroom_bytes: u64,
    pub recovery_min_headroom_bytes: u64,
    pub event_reservation_bytes: u64,
    pub soft_max_new_rows_per_second: u64,
    pub sample_interval_seconds: u64,
    pub stale_after_seconds: u64,
    pub capacity_evidence: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsPressureState {
    Normal,
    SoftPressure,
    HardStop,
}

impl AnalyticsPressureState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::SoftPressure => "soft_pressure",
            Self::HardStop => "hard_stop",
        }
    }

    fn parse(value: &str) -> Result<Self, AnalyticsAdmissionError> {
        match value {
            "normal" => Ok(Self::Normal),
            "soft_pressure" => Ok(Self::SoftPressure),
            "hard_stop" => Ok(Self::HardStop),
            _ => Err(AnalyticsAdmissionError::InvalidState),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnalyticsAdmissionSnapshot {
    pub installation_id: Uuid,
    pub policy_sha256: String,
    pub pressure_state: AnalyticsPressureState,
    pub generation: u64,
    pub sampled_at: i64,
    pub sample_age_seconds: u64,
    pub sample_fresh: bool,
    pub sample_interval_seconds: u64,
    pub stale_after_seconds: u64,
    pub recovery_pending_rows: u64,
    pub soft_pending_rows: u64,
    pub hard_pending_rows: u64,
    pub recovery_relation_bytes: u64,
    pub soft_relation_bytes: u64,
    pub hard_relation_bytes: u64,
    pub recovery_oldest_age_seconds: u64,
    pub soft_oldest_age_seconds: u64,
    pub hard_oldest_age_seconds: u64,
    pub database_capacity_bytes: u64,
    pub hard_min_headroom_bytes: u64,
    pub soft_min_headroom_bytes: u64,
    pub recovery_min_headroom_bytes: u64,
    pub event_reservation_bytes: u64,
    pub soft_max_new_rows_per_second: u64,
    pub pending_rows: u64,
    pub accounted_pending_rows: u64,
    pub oldest_pending_age_seconds: Option<u64>,
    pub relation_heap_bytes: u64,
    pub relation_index_bytes: u64,
    pub relation_toast_bytes: u64,
    pub relation_total_bytes: u64,
    pub accounted_relation_bytes: u64,
    pub database_bytes: u64,
    pub capacity_headroom_bytes: i64,
    pub state_changed_at: i64,
    pub last_transition_reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnalyticsAdmissionRefresh {
    pub previous_state: AnalyticsPressureState,
    pub snapshot: AnalyticsAdmissionSnapshot,
}

#[derive(Debug, thiserror::Error)]
pub enum AnalyticsAdmissionError {
    #[error("analytics admission database operation failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("analytics admission policy is invalid")]
    InvalidPolicy,
    #[error("analytics admission policy is missing or bound to another installation")]
    MissingOrMismatchedPolicy,
    #[error("analytics admission state is malformed")]
    InvalidState,
    #[error("analytics admission is hard-stopped")]
    HardStop,
    #[error("analytics admission is soft-pressure rate limited")]
    SoftRateLimited,
    #[error("analytics admission arithmetic overflowed")]
    Overflow,
}

#[derive(Debug, FromRow)]
struct PolicyRow {
    installation_id: Uuid,
    policy_sha256: String,
    recovery_pending_rows: i64,
    soft_pending_rows: i64,
    hard_pending_rows: i64,
    recovery_relation_bytes: i64,
    soft_relation_bytes: i64,
    hard_relation_bytes: i64,
    recovery_oldest_age_seconds: i64,
    soft_oldest_age_seconds: i64,
    hard_oldest_age_seconds: i64,
    database_capacity_bytes: i64,
    hard_min_headroom_bytes: i64,
    soft_min_headroom_bytes: i64,
    recovery_min_headroom_bytes: i64,
    event_reservation_bytes: i64,
    soft_max_new_rows_per_second: i64,
    sample_interval_seconds: i64,
    stale_after_seconds: i64,
    capacity_evidence: String,
}

#[derive(Debug, FromRow)]
struct StateRow {
    installation_id: Uuid,
    pressure_state: String,
    generation: i64,
    sampled_at: i64,
    state_changed_at: i64,
    pending_rows: i64,
    oldest_pending_created_at: Option<i64>,
    relation_heap_bytes: i64,
    relation_index_bytes: i64,
    relation_toast_bytes: i64,
    relation_total_bytes: i64,
    database_bytes: i64,
    capacity_headroom_bytes: i64,
    accounted_pending_rows: i64,
    accounted_relation_bytes: i64,
    soft_window_started_at: i64,
    soft_window_admitted_rows: i64,
    last_transition_reason: String,
}

#[derive(Clone, Copy, Debug)]
struct Metrics {
    pending_rows: u64,
    oldest_pending_created_at: Option<i64>,
    relation_heap_bytes: u64,
    relation_index_bytes: u64,
    relation_toast_bytes: u64,
    relation_total_bytes: u64,
    database_bytes: u64,
    capacity_headroom_bytes: i64,
}

pub fn analytics_admission_policy_sha256(
    policy: &AnalyticsAdmissionPolicy,
) -> Result<String, AnalyticsAdmissionError> {
    validate_policy(policy)?;
    let encoded = serde_json::to_vec(policy).map_err(|_| AnalyticsAdmissionError::InvalidPolicy)?;
    let mut digest = Sha256::new();
    digest.update(POLICY_DOMAIN);
    digest.update((encoded.len() as u64).to_be_bytes());
    digest.update(encoded);
    Ok(hex::encode(digest.finalize()))
}

pub async fn install_analytics_admission_policy(
    pool: &PgPool,
    installation_id: Uuid,
    policy: &AnalyticsAdmissionPolicy,
    installed_at: i64,
) -> Result<String, AnalyticsAdmissionError> {
    validate_policy(policy)?;
    if installed_at <= 0 {
        return Err(AnalyticsAdmissionError::InvalidPolicy);
    }
    let policy_sha256 = analytics_admission_policy_sha256(policy)?;
    let mut tx = pool.begin().await?;
    let installations = sqlx::query_scalar::<_, Uuid>(
        "SELECT installation_id FROM system_installation \
         WHERE singleton = 1 FOR SHARE",
    )
    .fetch_all(&mut *tx)
    .await?;
    let [observed_installation_id] = installations.as_slice() else {
        return Err(AnalyticsAdmissionError::MissingOrMismatchedPolicy);
    };
    if *observed_installation_id != installation_id {
        return Err(AnalyticsAdmissionError::MissingOrMismatchedPolicy);
    }
    let inserted = sqlx::query(
        "INSERT INTO analytics_admission_policy (\
             singleton, installation_id, policy_sha256, recovery_pending_rows, \
             soft_pending_rows, hard_pending_rows, recovery_relation_bytes, \
             soft_relation_bytes, hard_relation_bytes, recovery_oldest_age_seconds, \
             soft_oldest_age_seconds, hard_oldest_age_seconds, database_capacity_bytes, \
             hard_min_headroom_bytes, soft_min_headroom_bytes, recovery_min_headroom_bytes, \
             event_reservation_bytes, soft_max_new_rows_per_second, sample_interval_seconds, \
             stale_after_seconds, capacity_evidence, installed_at\
         ) VALUES (\
             1, $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, \
             $15, $16, $17, $18, $19, $20, $21\
         ) ON CONFLICT (singleton) DO NOTHING",
    )
    .bind(installation_id)
    .bind(&policy_sha256)
    .bind(to_i64(policy.recovery_pending_rows)?)
    .bind(to_i64(policy.soft_pending_rows)?)
    .bind(to_i64(policy.hard_pending_rows)?)
    .bind(to_i64(policy.recovery_relation_bytes)?)
    .bind(to_i64(policy.soft_relation_bytes)?)
    .bind(to_i64(policy.hard_relation_bytes)?)
    .bind(to_i64(policy.recovery_oldest_age_seconds)?)
    .bind(to_i64(policy.soft_oldest_age_seconds)?)
    .bind(to_i64(policy.hard_oldest_age_seconds)?)
    .bind(to_i64(policy.database_capacity_bytes)?)
    .bind(to_i64(policy.hard_min_headroom_bytes)?)
    .bind(to_i64(policy.soft_min_headroom_bytes)?)
    .bind(to_i64(policy.recovery_min_headroom_bytes)?)
    .bind(to_i64(policy.event_reservation_bytes)?)
    .bind(to_i64(policy.soft_max_new_rows_per_second)?)
    .bind(to_i64(policy.sample_interval_seconds)?)
    .bind(to_i64(policy.stale_after_seconds)?)
    .bind(&policy.capacity_evidence)
    .bind(installed_at)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    let stored_policy = load_policy(&mut tx, false).await?;
    if stored_policy.installation_id != installation_id
        || stored_policy.policy_sha256 != policy_sha256
        || stored_policy.to_policy()? != *policy
    {
        return Err(AnalyticsAdmissionError::MissingOrMismatchedPolicy);
    }
    if inserted == 1 {
        let metrics = exact_metrics(&mut tx, policy.database_capacity_bytes).await?;
        let pressure = classify_pressure(
            AnalyticsPressureState::Normal,
            policy,
            metrics,
            installed_at,
        );
        sqlx::query(
            "INSERT INTO analytics_admission_state (\
                 singleton, installation_id, pressure_state, generation, sampled_at, \
                 state_changed_at, pending_rows, oldest_pending_created_at, relation_heap_bytes, \
                 relation_index_bytes, relation_toast_bytes, relation_total_bytes, database_bytes, \
                 capacity_headroom_bytes, accounted_pending_rows, accounted_relation_bytes, \
                 soft_window_started_at, soft_window_admitted_rows, last_transition_reason\
             ) VALUES (1, $1, $2, 0, $3, $3, $4, $5, $6, $7, $8, $9, $10, $11, \
                       $4, $9, $3, 0, 'policy_installed')",
        )
        .bind(installation_id)
        .bind(pressure.as_str())
        .bind(installed_at)
        .bind(to_i64(metrics.pending_rows)?)
        .bind(metrics.oldest_pending_created_at)
        .bind(to_i64(metrics.relation_heap_bytes)?)
        .bind(to_i64(metrics.relation_index_bytes)?)
        .bind(to_i64(metrics.relation_toast_bytes)?)
        .bind(to_i64(metrics.relation_total_bytes)?)
        .bind(to_i64(metrics.database_bytes)?)
        .bind(metrics.capacity_headroom_bytes)
        .execute(&mut *tx)
        .await?;
    } else {
        let state = load_state(&mut tx, false).await?;
        if state.installation_id != installation_id {
            return Err(AnalyticsAdmissionError::MissingOrMismatchedPolicy);
        }
    }
    tx.commit().await?;
    Ok(policy_sha256)
}

pub async fn refresh_analytics_admission(
    pool: &PgPool,
) -> Result<AnalyticsAdmissionRefresh, AnalyticsAdmissionError> {
    let mut tx = pool.begin().await?;
    let now = database_now(&mut tx).await?;
    let policy_row = load_policy(&mut tx, false).await?;
    let policy = policy_row.to_policy()?;
    let state = load_state(&mut tx, true).await?;
    if state.installation_id != policy_row.installation_id {
        return Err(AnalyticsAdmissionError::MissingOrMismatchedPolicy);
    }
    validate_state_timestamps(&state, now)?;
    let previous = AnalyticsPressureState::parse(&state.pressure_state)?;
    let metrics = exact_metrics(&mut tx, policy.database_capacity_bytes).await?;
    let next = classify_pressure(previous, &policy, metrics, now);
    let reason = transition_reason(previous, next, metrics, &policy, now);
    let window_expired = now.saturating_sub(state.soft_window_started_at) >= 1;
    let window_started_at = if window_expired {
        now
    } else {
        state.soft_window_started_at
    };
    let window_rows = if window_expired {
        0
    } else {
        state.soft_window_admitted_rows
    };
    sqlx::query(
        "UPDATE analytics_admission_state SET \
             pressure_state = $1, generation = generation + 1, sampled_at = $2, \
             state_changed_at = CASE WHEN pressure_state IS DISTINCT FROM $1 THEN $2 ELSE state_changed_at END, \
             pending_rows = $3, oldest_pending_created_at = $4, relation_heap_bytes = $5, \
             relation_index_bytes = $6, relation_toast_bytes = $7, relation_total_bytes = $8, \
             database_bytes = $9, capacity_headroom_bytes = $10, accounted_pending_rows = $3, \
             accounted_relation_bytes = $8, soft_window_started_at = $11, \
             soft_window_admitted_rows = $12, last_transition_reason = $13 \
         WHERE singleton = 1",
    )
    .bind(next.as_str())
    .bind(now)
    .bind(to_i64(metrics.pending_rows)?)
    .bind(metrics.oldest_pending_created_at)
    .bind(to_i64(metrics.relation_heap_bytes)?)
    .bind(to_i64(metrics.relation_index_bytes)?)
    .bind(to_i64(metrics.relation_toast_bytes)?)
    .bind(to_i64(metrics.relation_total_bytes)?)
    .bind(to_i64(metrics.database_bytes)?)
    .bind(metrics.capacity_headroom_bytes)
    .bind(window_started_at)
    .bind(window_rows)
    .bind(reason)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let snapshot = analytics_admission_snapshot(pool).await?;
    Ok(AnalyticsAdmissionRefresh {
        previous_state: previous,
        snapshot,
    })
}

pub(crate) async fn admit_analytics_rows(
    tx: &mut Transaction<'_, Postgres>,
    requested_rows: usize,
    oldest_new_created_at: i64,
) -> Result<AnalyticsPressureState, AnalyticsAdmissionError> {
    if requested_rows == 0 {
        return Ok(AnalyticsPressureState::Normal);
    }
    let now = database_now(tx).await?;
    let policy_row = load_policy(tx, false).await?;
    let policy = policy_row.to_policy()?;
    let state = load_state(tx, true).await?;
    if state.installation_id != policy_row.installation_id {
        return Err(AnalyticsAdmissionError::MissingOrMismatchedPolicy);
    }
    validate_state_timestamps(&state, now)?;
    let current = AnalyticsPressureState::parse(&state.pressure_state)?;
    let sample_age = now.saturating_sub(state.sampled_at);
    if sample_age > to_i64(policy.stale_after_seconds)?
        || current == AnalyticsPressureState::HardStop
    {
        return Err(AnalyticsAdmissionError::HardStop);
    }
    let requested = u64::try_from(requested_rows).map_err(|_| AnalyticsAdmissionError::Overflow)?;
    let reserved_bytes = requested
        .checked_mul(policy.event_reservation_bytes)
        .ok_or(AnalyticsAdmissionError::Overflow)?;
    let accounted_rows = to_u64(state.accounted_pending_rows)?;
    let accounted_relation = to_u64(state.accounted_relation_bytes)?;
    let projected_rows = accounted_rows
        .checked_add(requested)
        .ok_or(AnalyticsAdmissionError::Overflow)?;
    let projected_relation = accounted_relation
        .checked_add(reserved_bytes)
        .ok_or(AnalyticsAdmissionError::Overflow)?;
    let projected_database = to_u64(state.database_bytes)?
        .checked_add(reserved_bytes)
        .ok_or(AnalyticsAdmissionError::Overflow)?;
    let projected_headroom = signed_headroom(policy.database_capacity_bytes, projected_database);
    let projected_oldest = state
        .oldest_pending_created_at
        .map_or(oldest_new_created_at, |oldest| {
            oldest.min(oldest_new_created_at)
        });
    let projected = Metrics {
        pending_rows: projected_rows,
        oldest_pending_created_at: Some(projected_oldest),
        relation_heap_bytes: to_u64(state.relation_heap_bytes)?,
        relation_index_bytes: to_u64(state.relation_index_bytes)?,
        relation_toast_bytes: to_u64(state.relation_toast_bytes)?,
        relation_total_bytes: projected_relation,
        database_bytes: projected_database,
        capacity_headroom_bytes: projected_headroom,
    };
    let next = classify_pressure(current, &policy, projected, now);
    if next == AnalyticsPressureState::HardStop {
        return Err(AnalyticsAdmissionError::HardStop);
    }
    let window_expired = now.saturating_sub(state.soft_window_started_at) >= 1;
    let window_started_at = if window_expired {
        now
    } else {
        state.soft_window_started_at
    };
    let window_rows = if window_expired {
        0_u64
    } else {
        to_u64(state.soft_window_admitted_rows)?
    };
    let admitted_rows = if next == AnalyticsPressureState::SoftPressure {
        let admitted = window_rows
            .checked_add(requested)
            .ok_or(AnalyticsAdmissionError::Overflow)?;
        if admitted > policy.soft_max_new_rows_per_second {
            return Err(AnalyticsAdmissionError::SoftRateLimited);
        }
        admitted
    } else {
        window_rows
    };
    sqlx::query(
        "UPDATE analytics_admission_state SET \
             pressure_state = $1, generation = generation + 1, \
             state_changed_at = CASE WHEN pressure_state IS DISTINCT FROM $1 THEN $2 ELSE state_changed_at END, \
             accounted_pending_rows = $3, accounted_relation_bytes = $4, \
             oldest_pending_created_at = $5, soft_window_started_at = $6, \
             soft_window_admitted_rows = $7, last_transition_reason = $8 \
         WHERE singleton = 1",
    )
    .bind(next.as_str())
    .bind(now)
    .bind(to_i64(projected_rows)?)
    .bind(to_i64(projected_relation)?)
    .bind(projected_oldest)
    .bind(window_started_at)
    .bind(to_i64(admitted_rows)?)
    .bind(if next == AnalyticsPressureState::SoftPressure {
        "producer_admitted_soft_pressure"
    } else {
        "producer_admitted_normal"
    })
    .execute(&mut **tx)
    .await?;
    Ok(next)
}

pub(crate) async fn release_terminal_rows(
    tx: &mut Transaction<'_, Postgres>,
    terminal_rows: usize,
) -> Result<(), AnalyticsAdmissionError> {
    if terminal_rows == 0 {
        return Ok(());
    }
    let now = database_now(tx).await?;
    let rows =
        to_i64(u64::try_from(terminal_rows).map_err(|_| AnalyticsAdmissionError::Overflow)?)?;
    let updated = sqlx::query(
        "UPDATE analytics_admission_state SET \
             generation = generation + 1, \
             accounted_pending_rows = greatest(0, accounted_pending_rows - $1), \
             last_transition_reason = 'relay_terminal_rows', \
             state_changed_at = state_changed_at \
         WHERE singleton = 1 AND sampled_at <= $2",
    )
    .bind(rows)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(AnalyticsAdmissionError::MissingOrMismatchedPolicy);
    }
    Ok(())
}

pub async fn analytics_admission_snapshot(
    pool: &PgPool,
) -> Result<AnalyticsAdmissionSnapshot, AnalyticsAdmissionError> {
    let mut connection = pool.acquire().await?;
    let now = database_now_connection(&mut connection).await?;
    let policy = load_policy_connection(&mut connection).await?;
    let state = load_state_connection(&mut connection).await?;
    snapshot_from_rows(&policy, &state, now)
}

/// Re-measure the PostgreSQL backlog and physical storage without mutating the
/// admission singleton. Lifecycle uses this after all legacy writers are
/// fenced and immediately before authority commit.
pub async fn inspect_analytics_admission_exact(
    pool: &PgPool,
) -> Result<AnalyticsAdmissionSnapshot, AnalyticsAdmissionError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let now = database_now(&mut tx).await?;
    let policy_row = load_policy(&mut tx, false).await?;
    let policy = policy_row.to_policy()?;
    validate_policy(&policy)?;
    let state = load_state(&mut tx, false).await?;
    if state.installation_id != policy_row.installation_id {
        return Err(AnalyticsAdmissionError::MissingOrMismatchedPolicy);
    }
    validate_state_timestamps(&state, now)?;
    let metrics = exact_metrics(&mut tx, policy.database_capacity_bytes).await?;
    let pressure = classify_pressure(
        AnalyticsPressureState::parse(&state.pressure_state)?,
        &policy,
        metrics,
        now,
    );
    let measured = StateRow {
        installation_id: state.installation_id,
        pressure_state: pressure.as_str().to_owned(),
        generation: state.generation,
        sampled_at: now,
        state_changed_at: state.state_changed_at,
        pending_rows: to_i64(metrics.pending_rows)?,
        oldest_pending_created_at: metrics.oldest_pending_created_at,
        relation_heap_bytes: to_i64(metrics.relation_heap_bytes)?,
        relation_index_bytes: to_i64(metrics.relation_index_bytes)?,
        relation_toast_bytes: to_i64(metrics.relation_toast_bytes)?,
        relation_total_bytes: to_i64(metrics.relation_total_bytes)?,
        database_bytes: to_i64(metrics.database_bytes)?,
        capacity_headroom_bytes: metrics.capacity_headroom_bytes,
        accounted_pending_rows: to_i64(metrics.pending_rows)?,
        accounted_relation_bytes: to_i64(metrics.relation_total_bytes)?,
        soft_window_started_at: state.soft_window_started_at,
        soft_window_admitted_rows: state.soft_window_admitted_rows,
        last_transition_reason: "read_only_exact_inspection".to_owned(),
    };
    let snapshot = snapshot_from_rows(&policy_row, &measured, now)?;
    tx.rollback().await?;
    Ok(snapshot)
}

async fn exact_metrics(
    tx: &mut Transaction<'_, Postgres>,
    database_capacity_bytes: u64,
) -> Result<Metrics, AnalyticsAdmissionError> {
    let row = sqlx::query_as::<_, (i64, Option<i64>, i64, i64, i64, i64, i64)>(
        "WITH backlog AS ( \
             SELECT count(*)::bigint AS pending_rows, \
                    min(created_at) AS oldest_pending_created_at \
             FROM analytics_outbox \
             WHERE published_at IS NULL AND quarantined_at IS NULL \
         ), target AS ( \
             SELECT oid, reltoastrelid FROM pg_class \
             WHERE oid = 'public.analytics_outbox'::regclass \
         ), measured AS ( \
             SELECT oid, \
                    CASE WHEN reltoastrelid = 0 THEN 0 \
                         ELSE pg_total_relation_size(reltoastrelid) END::bigint AS toast_bytes \
             FROM target \
         ) \
         SELECT \
             backlog.pending_rows, backlog.oldest_pending_created_at, \
             (pg_table_size(oid) - toast_bytes)::bigint, \
             pg_indexes_size(oid)::bigint, \
             toast_bytes, \
             pg_total_relation_size(oid)::bigint, \
             pg_database_size(current_database())::bigint \
         FROM measured CROSS JOIN backlog",
    )
    .fetch_one(&mut **tx)
    .await?;
    let heap = to_u64(row.2)?;
    let indexes = to_u64(row.3)?;
    let toast = to_u64(row.4)?;
    let total = to_u64(row.5)?;
    if heap
        .checked_add(indexes)
        .and_then(|value| value.checked_add(toast))
        != Some(total)
    {
        return Err(AnalyticsAdmissionError::InvalidState);
    }
    let database = to_u64(row.6)?;
    Ok(Metrics {
        pending_rows: to_u64(row.0)?,
        oldest_pending_created_at: row.1,
        relation_heap_bytes: heap,
        relation_index_bytes: indexes,
        relation_toast_bytes: toast,
        relation_total_bytes: total,
        database_bytes: database,
        capacity_headroom_bytes: signed_headroom(database_capacity_bytes, database),
    })
}

fn classify_pressure(
    current: AnalyticsPressureState,
    policy: &AnalyticsAdmissionPolicy,
    metrics: Metrics,
    now: i64,
) -> AnalyticsPressureState {
    let age = oldest_age(metrics.oldest_pending_created_at, now).unwrap_or(0);
    let hard = metrics.pending_rows >= policy.hard_pending_rows
        || metrics.relation_total_bytes >= policy.hard_relation_bytes
        || age >= policy.hard_oldest_age_seconds
        || metrics.capacity_headroom_bytes <= signed(policy.hard_min_headroom_bytes);
    let soft = metrics.pending_rows >= policy.soft_pending_rows
        || metrics.relation_total_bytes >= policy.soft_relation_bytes
        || age >= policy.soft_oldest_age_seconds
        || metrics.capacity_headroom_bytes <= signed(policy.soft_min_headroom_bytes);
    let recovered = metrics.pending_rows <= policy.recovery_pending_rows
        && metrics.relation_total_bytes <= policy.recovery_relation_bytes
        && age <= policy.recovery_oldest_age_seconds
        && metrics.capacity_headroom_bytes >= signed(policy.recovery_min_headroom_bytes);
    match current {
        AnalyticsPressureState::HardStop if recovered => AnalyticsPressureState::Normal,
        AnalyticsPressureState::HardStop => AnalyticsPressureState::HardStop,
        AnalyticsPressureState::SoftPressure if hard => AnalyticsPressureState::HardStop,
        AnalyticsPressureState::SoftPressure if recovered => AnalyticsPressureState::Normal,
        AnalyticsPressureState::SoftPressure => AnalyticsPressureState::SoftPressure,
        AnalyticsPressureState::Normal if hard => AnalyticsPressureState::HardStop,
        AnalyticsPressureState::Normal if soft => AnalyticsPressureState::SoftPressure,
        AnalyticsPressureState::Normal => AnalyticsPressureState::Normal,
    }
}

fn transition_reason(
    previous: AnalyticsPressureState,
    next: AnalyticsPressureState,
    metrics: Metrics,
    policy: &AnalyticsAdmissionPolicy,
    now: i64,
) -> &'static str {
    if previous != next {
        return match next {
            AnalyticsPressureState::Normal => "hysteresis_recovered",
            AnalyticsPressureState::SoftPressure => "soft_threshold_crossed",
            AnalyticsPressureState::HardStop => "hard_threshold_crossed",
        };
    }
    let age = oldest_age(metrics.oldest_pending_created_at, now).unwrap_or(0);
    if metrics.pending_rows >= policy.hard_pending_rows {
        "hard_pending_rows"
    } else if metrics.relation_total_bytes >= policy.hard_relation_bytes {
        "hard_relation_bytes"
    } else if age >= policy.hard_oldest_age_seconds {
        "hard_oldest_age"
    } else if metrics.capacity_headroom_bytes <= signed(policy.hard_min_headroom_bytes) {
        "hard_capacity_headroom"
    } else {
        "exact_sample"
    }
}

fn snapshot_from_rows(
    policy: &PolicyRow,
    state: &StateRow,
    now: i64,
) -> Result<AnalyticsAdmissionSnapshot, AnalyticsAdmissionError> {
    if policy.installation_id != state.installation_id {
        return Err(AnalyticsAdmissionError::MissingOrMismatchedPolicy);
    }
    validate_state_timestamps(state, now)?;
    let sample_age = oldest_age(Some(state.sampled_at), now).unwrap_or(0);
    let stale_after = to_u64(policy.stale_after_seconds)?;
    let fresh = sample_age <= stale_after;
    let stored = AnalyticsPressureState::parse(&state.pressure_state)?;
    Ok(AnalyticsAdmissionSnapshot {
        installation_id: state.installation_id,
        policy_sha256: policy.policy_sha256.clone(),
        pressure_state: if fresh {
            stored
        } else {
            AnalyticsPressureState::HardStop
        },
        generation: to_u64(state.generation)?,
        sampled_at: state.sampled_at,
        sample_age_seconds: sample_age,
        sample_fresh: fresh,
        sample_interval_seconds: to_u64(policy.sample_interval_seconds)?,
        stale_after_seconds: stale_after,
        recovery_pending_rows: to_u64(policy.recovery_pending_rows)?,
        soft_pending_rows: to_u64(policy.soft_pending_rows)?,
        hard_pending_rows: to_u64(policy.hard_pending_rows)?,
        recovery_relation_bytes: to_u64(policy.recovery_relation_bytes)?,
        soft_relation_bytes: to_u64(policy.soft_relation_bytes)?,
        hard_relation_bytes: to_u64(policy.hard_relation_bytes)?,
        recovery_oldest_age_seconds: to_u64(policy.recovery_oldest_age_seconds)?,
        soft_oldest_age_seconds: to_u64(policy.soft_oldest_age_seconds)?,
        hard_oldest_age_seconds: to_u64(policy.hard_oldest_age_seconds)?,
        database_capacity_bytes: to_u64(policy.database_capacity_bytes)?,
        hard_min_headroom_bytes: to_u64(policy.hard_min_headroom_bytes)?,
        soft_min_headroom_bytes: to_u64(policy.soft_min_headroom_bytes)?,
        recovery_min_headroom_bytes: to_u64(policy.recovery_min_headroom_bytes)?,
        event_reservation_bytes: to_u64(policy.event_reservation_bytes)?,
        soft_max_new_rows_per_second: to_u64(policy.soft_max_new_rows_per_second)?,
        pending_rows: to_u64(state.pending_rows)?,
        accounted_pending_rows: to_u64(state.accounted_pending_rows)?,
        oldest_pending_age_seconds: oldest_age(state.oldest_pending_created_at, now),
        relation_heap_bytes: to_u64(state.relation_heap_bytes)?,
        relation_index_bytes: to_u64(state.relation_index_bytes)?,
        relation_toast_bytes: to_u64(state.relation_toast_bytes)?,
        relation_total_bytes: to_u64(state.relation_total_bytes)?,
        accounted_relation_bytes: to_u64(state.accounted_relation_bytes)?,
        database_bytes: to_u64(state.database_bytes)?,
        capacity_headroom_bytes: state.capacity_headroom_bytes,
        state_changed_at: state.state_changed_at,
        last_transition_reason: state.last_transition_reason.clone(),
    })
}

fn validate_state_timestamps(state: &StateRow, now: i64) -> Result<(), AnalyticsAdmissionError> {
    if state.sampled_at > now || state.state_changed_at > now || state.soft_window_started_at > now
    {
        Err(AnalyticsAdmissionError::InvalidState)
    } else {
        Ok(())
    }
}

async fn load_policy(
    tx: &mut Transaction<'_, Postgres>,
    lock: bool,
) -> Result<PolicyRow, AnalyticsAdmissionError> {
    let query = if lock {
        POLICY_SELECT_FOR_SHARE
    } else {
        POLICY_SELECT
    };
    sqlx::query_as::<_, PolicyRow>(query)
        .fetch_one(&mut **tx)
        .await
        .map_err(map_required_row_error)
}

async fn load_state(
    tx: &mut Transaction<'_, Postgres>,
    lock: bool,
) -> Result<StateRow, AnalyticsAdmissionError> {
    let query = if lock {
        STATE_SELECT_FOR_UPDATE
    } else {
        STATE_SELECT
    };
    sqlx::query_as::<_, StateRow>(query)
        .fetch_one(&mut **tx)
        .await
        .map_err(map_required_row_error)
}

async fn load_policy_connection(
    connection: &mut PgConnection,
) -> Result<PolicyRow, AnalyticsAdmissionError> {
    sqlx::query_as::<_, PolicyRow>(POLICY_SELECT)
        .fetch_one(connection)
        .await
        .map_err(map_required_row_error)
}

async fn load_state_connection(
    connection: &mut PgConnection,
) -> Result<StateRow, AnalyticsAdmissionError> {
    sqlx::query_as::<_, StateRow>(STATE_SELECT)
        .fetch_one(connection)
        .await
        .map_err(map_required_row_error)
}

fn map_required_row_error(error: sqlx::Error) -> AnalyticsAdmissionError {
    if matches!(error, sqlx::Error::RowNotFound) {
        AnalyticsAdmissionError::MissingOrMismatchedPolicy
    } else {
        AnalyticsAdmissionError::Database(error)
    }
}

async fn database_now(tx: &mut Transaction<'_, Postgres>) -> Result<i64, AnalyticsAdmissionError> {
    Ok(
        sqlx::query_scalar("SELECT floor(extract(epoch FROM clock_timestamp()))::bigint")
            .fetch_one(&mut **tx)
            .await?,
    )
}

async fn database_now_connection(
    connection: &mut PgConnection,
) -> Result<i64, AnalyticsAdmissionError> {
    Ok(
        sqlx::query_scalar("SELECT floor(extract(epoch FROM clock_timestamp()))::bigint")
            .fetch_one(connection)
            .await?,
    )
}

impl PolicyRow {
    fn to_policy(&self) -> Result<AnalyticsAdmissionPolicy, AnalyticsAdmissionError> {
        Ok(AnalyticsAdmissionPolicy {
            recovery_pending_rows: to_u64(self.recovery_pending_rows)?,
            soft_pending_rows: to_u64(self.soft_pending_rows)?,
            hard_pending_rows: to_u64(self.hard_pending_rows)?,
            recovery_relation_bytes: to_u64(self.recovery_relation_bytes)?,
            soft_relation_bytes: to_u64(self.soft_relation_bytes)?,
            hard_relation_bytes: to_u64(self.hard_relation_bytes)?,
            recovery_oldest_age_seconds: to_u64(self.recovery_oldest_age_seconds)?,
            soft_oldest_age_seconds: to_u64(self.soft_oldest_age_seconds)?,
            hard_oldest_age_seconds: to_u64(self.hard_oldest_age_seconds)?,
            database_capacity_bytes: to_u64(self.database_capacity_bytes)?,
            hard_min_headroom_bytes: to_u64(self.hard_min_headroom_bytes)?,
            soft_min_headroom_bytes: to_u64(self.soft_min_headroom_bytes)?,
            recovery_min_headroom_bytes: to_u64(self.recovery_min_headroom_bytes)?,
            event_reservation_bytes: to_u64(self.event_reservation_bytes)?,
            soft_max_new_rows_per_second: to_u64(self.soft_max_new_rows_per_second)?,
            sample_interval_seconds: to_u64(self.sample_interval_seconds)?,
            stale_after_seconds: to_u64(self.stale_after_seconds)?,
            capacity_evidence: self.capacity_evidence.clone(),
        })
    }
}

fn validate_policy(policy: &AnalyticsAdmissionPolicy) -> Result<(), AnalyticsAdmissionError> {
    let values = [
        policy.recovery_pending_rows,
        policy.soft_pending_rows,
        policy.hard_pending_rows,
        policy.recovery_relation_bytes,
        policy.soft_relation_bytes,
        policy.hard_relation_bytes,
        policy.recovery_oldest_age_seconds,
        policy.soft_oldest_age_seconds,
        policy.hard_oldest_age_seconds,
        policy.database_capacity_bytes,
        policy.hard_min_headroom_bytes,
        policy.soft_min_headroom_bytes,
        policy.recovery_min_headroom_bytes,
        policy.event_reservation_bytes,
        policy.soft_max_new_rows_per_second,
        policy.sample_interval_seconds,
        policy.stale_after_seconds,
    ];
    if values.iter().any(|value| i64::try_from(*value).is_err())
        || !(policy.recovery_pending_rows < policy.soft_pending_rows
            && policy.soft_pending_rows < policy.hard_pending_rows
            && policy.recovery_relation_bytes < policy.soft_relation_bytes
            && policy.soft_relation_bytes < policy.hard_relation_bytes
            && policy.recovery_oldest_age_seconds < policy.soft_oldest_age_seconds
            && policy.soft_oldest_age_seconds < policy.hard_oldest_age_seconds
            && policy.hard_min_headroom_bytes < policy.soft_min_headroom_bytes
            && policy.soft_min_headroom_bytes < policy.recovery_min_headroom_bytes)
        || policy.database_capacity_bytes <= policy.recovery_min_headroom_bytes
        || policy.event_reservation_bytes == 0
        || policy.event_reservation_bytes > policy.hard_relation_bytes
        || !(100_000..=10_000_000).contains(&policy.soft_max_new_rows_per_second)
        || !(1..=60).contains(&policy.sample_interval_seconds)
        || !(policy.sample_interval_seconds.saturating_mul(2)..=600)
            .contains(&policy.stale_after_seconds)
        || !(8..=1024).contains(&policy.capacity_evidence.trim().len())
    {
        return Err(AnalyticsAdmissionError::InvalidPolicy);
    }
    Ok(())
}

fn oldest_age(timestamp: Option<i64>, now: i64) -> Option<u64> {
    timestamp.map(|value| u64::try_from(now.saturating_sub(value)).unwrap_or(0))
}

fn signed(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn signed_headroom(capacity: u64, used: u64) -> i64 {
    let value = i128::from(capacity) - i128::from(used);
    i64::try_from(value).unwrap_or(if value.is_negative() {
        i64::MIN
    } else {
        i64::MAX
    })
}

fn to_i64(value: u64) -> Result<i64, AnalyticsAdmissionError> {
    i64::try_from(value).map_err(|_| AnalyticsAdmissionError::Overflow)
}

fn to_u64(value: i64) -> Result<u64, AnalyticsAdmissionError> {
    u64::try_from(value).map_err(|_| AnalyticsAdmissionError::InvalidState)
}

const POLICY_SELECT: &str = "SELECT installation_id, policy_sha256, recovery_pending_rows, \
    soft_pending_rows, hard_pending_rows, recovery_relation_bytes, soft_relation_bytes, \
    hard_relation_bytes, recovery_oldest_age_seconds, soft_oldest_age_seconds, \
    hard_oldest_age_seconds, database_capacity_bytes, hard_min_headroom_bytes, \
    soft_min_headroom_bytes, recovery_min_headroom_bytes, event_reservation_bytes, \
    soft_max_new_rows_per_second, sample_interval_seconds, stale_after_seconds, capacity_evidence \
    FROM analytics_admission_policy WHERE singleton = 1";

const POLICY_SELECT_FOR_SHARE: &str = "SELECT installation_id, policy_sha256, recovery_pending_rows, \
    soft_pending_rows, hard_pending_rows, recovery_relation_bytes, soft_relation_bytes, \
    hard_relation_bytes, recovery_oldest_age_seconds, soft_oldest_age_seconds, \
    hard_oldest_age_seconds, database_capacity_bytes, hard_min_headroom_bytes, \
    soft_min_headroom_bytes, recovery_min_headroom_bytes, event_reservation_bytes, \
    soft_max_new_rows_per_second, sample_interval_seconds, stale_after_seconds, capacity_evidence \
    FROM analytics_admission_policy WHERE singleton = 1 FOR SHARE";

const STATE_SELECT: &str = "SELECT installation_id, pressure_state, generation, sampled_at, \
    state_changed_at, pending_rows, oldest_pending_created_at, relation_heap_bytes, \
    relation_index_bytes, relation_toast_bytes, relation_total_bytes, database_bytes, \
    capacity_headroom_bytes, accounted_pending_rows, accounted_relation_bytes, \
    soft_window_started_at, soft_window_admitted_rows, last_transition_reason \
    FROM analytics_admission_state WHERE singleton = 1";

const STATE_SELECT_FOR_UPDATE: &str = "SELECT installation_id, pressure_state, generation, sampled_at, \
    state_changed_at, pending_rows, oldest_pending_created_at, relation_heap_bytes, \
    relation_index_bytes, relation_toast_bytes, relation_total_bytes, database_bytes, \
    capacity_headroom_bytes, accounted_pending_rows, accounted_relation_bytes, \
    soft_window_started_at, soft_window_admitted_rows, last_transition_reason \
    FROM analytics_admission_state WHERE singleton = 1 FOR UPDATE";

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> AnalyticsAdmissionPolicy {
        AnalyticsAdmissionPolicy {
            recovery_pending_rows: 10,
            soft_pending_rows: 20,
            hard_pending_rows: 30,
            recovery_relation_bytes: 1_000,
            soft_relation_bytes: 2_000,
            hard_relation_bytes: 3_000,
            recovery_oldest_age_seconds: 10,
            soft_oldest_age_seconds: 20,
            hard_oldest_age_seconds: 30,
            database_capacity_bytes: 100_000,
            hard_min_headroom_bytes: 10_000,
            soft_min_headroom_bytes: 20_000,
            recovery_min_headroom_bytes: 30_000,
            event_reservation_bytes: 100,
            soft_max_new_rows_per_second: 100_000,
            sample_interval_seconds: 1,
            stale_after_seconds: 3,
            capacity_evidence: "capacity-ticket-1".to_string(),
        }
    }

    fn metrics(rows: u64) -> Metrics {
        let relation_bytes = rows.checked_mul(100).unwrap();
        Metrics {
            pending_rows: rows,
            oldest_pending_created_at: None,
            relation_heap_bytes: 0,
            relation_index_bytes: 0,
            relation_toast_bytes: 0,
            relation_total_bytes: relation_bytes,
            database_bytes: relation_bytes,
            capacity_headroom_bytes: 100_000 - i64::try_from(relation_bytes).unwrap(),
        }
    }

    #[test]
    fn pressure_transitions_have_hysteresis() {
        let policy = policy();
        assert_eq!(
            classify_pressure(AnalyticsPressureState::Normal, &policy, metrics(20), 100),
            AnalyticsPressureState::SoftPressure
        );
        assert_eq!(
            classify_pressure(
                AnalyticsPressureState::SoftPressure,
                &policy,
                metrics(15),
                100
            ),
            AnalyticsPressureState::SoftPressure
        );
        assert_eq!(
            classify_pressure(
                AnalyticsPressureState::SoftPressure,
                &policy,
                metrics(10),
                100
            ),
            AnalyticsPressureState::Normal
        );
        assert_eq!(
            classify_pressure(AnalyticsPressureState::Normal, &policy, metrics(30), 100),
            AnalyticsPressureState::HardStop
        );
        assert_eq!(
            classify_pressure(AnalyticsPressureState::HardStop, &policy, metrics(15), 100),
            AnalyticsPressureState::HardStop
        );
        assert_eq!(
            classify_pressure(AnalyticsPressureState::HardStop, &policy, metrics(10), 100),
            AnalyticsPressureState::Normal
        );
    }

    #[test]
    fn policy_hash_is_deterministic_and_validated() {
        let policy = policy();
        assert_eq!(
            analytics_admission_policy_sha256(&policy).unwrap(),
            analytics_admission_policy_sha256(&policy).unwrap()
        );
        assert_eq!(
            analytics_admission_policy_sha256(&policy).unwrap().len(),
            64
        );
    }
}

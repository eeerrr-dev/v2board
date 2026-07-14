use super::*;
use rust_decimal::prelude::ToPrimitive;

const ORDER_INCOME_SUM_SQL: &str = "SELECT CAST(COALESCE(SUM(total_amount), 0) AS TEXT) FROM orders \
     WHERE created_at >= $1 AND created_at < $2 AND status NOT IN (0, 2)";
const COMMISSION_PAYOUT_SUM_SQL: &str = "SELECT CAST(COALESCE(SUM(get_amount), 0) AS TEXT) FROM commission_log \
     WHERE created_at >= $1 AND created_at < $2";

#[derive(Debug, Default)]
struct WorkerSnapshot {
    schedule_last_seen_at: Option<i64>,
    totals: BTreeMap<String, i64>,
    failed: BTreeMap<String, i64>,
    last_run_at: BTreeMap<String, i64>,
    last_success_at: BTreeMap<String, i64>,
    last_failure_at: BTreeMap<String, i64>,
}

impl WorkerSnapshot {
    fn total_jobs(&self) -> i64 {
        self.totals
            .values()
            .copied()
            .fold(0_i64, i64::saturating_add)
    }

    fn failed_jobs(&self) -> i64 {
        self.failed
            .values()
            .copied()
            .fold(0_i64, i64::saturating_add)
    }

    fn last_seen_at(&self) -> Option<i64> {
        self.schedule_last_seen_at
            .into_iter()
            .chain(self.last_run_at.values().copied())
            .max()
    }

    fn worker_running(&self, now: i64, seconds: i64) -> bool {
        self.last_seen_at()
            .map(|last_seen| timestamp_is_recent(now, last_seen, seconds))
            .unwrap_or(false)
    }

    fn max_counter_key(&self) -> Option<String> {
        self.totals
            .iter()
            .max_by_key(|(_, value)| *value)
            .map(|(key, _)| key.clone())
    }

    fn job_names(&self) -> Vec<String> {
        let mut names = self
            .totals
            .keys()
            .chain(self.failed.keys())
            .chain(self.last_run_at.keys())
            .cloned()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        names.sort();
        names
    }
}

fn timestamp_is_recent(now: i64, last_seen: i64, seconds: i64) -> bool {
    now.saturating_sub(last_seen) <= seconds
}

fn exact_stat_sum_i64(value: &str, metric: &str) -> Result<i64, ApiError> {
    let value = value.parse::<Decimal>().map_err(|_| {
        ApiError::internal(format!("{metric} aggregate is not a valid decimal integer"))
    })?;
    if value != value.trunc() {
        return Err(ApiError::internal(format!(
            "{metric} aggregate is not an integer"
        )));
    }
    value.to_i64().ok_or_else(|| {
        ApiError::internal(format!("{metric} aggregate exceeds the supported range"))
    })
}

impl AdminService {
    async fn order_income_between(&self, start: i64, end: i64) -> Result<i64, ApiError> {
        let value: String = sqlx::query_scalar(ORDER_INCOME_SUM_SQL)
            .bind(start)
            .bind(end)
            .fetch_one(&self.db)
            .await?;
        exact_stat_sum_i64(&value, "Order income")
    }

    async fn commission_payout_between(&self, start: i64, end: i64) -> Result<i64, ApiError> {
        let value: String = sqlx::query_scalar(COMMISSION_PAYOUT_SUM_SQL)
            .bind(start)
            .bind(end)
            .fetch_one(&self.db)
            .await?;
        exact_stat_sum_i64(&value, "Commission payout")
    }

    pub(super) async fn stat_summary(&self) -> Result<AdminOutput, ApiError> {
        // Ports StatController::getOverride (:26-66).
        let now = Utc::now().timestamp();
        let today = start_of_today();
        let month = first_day_of_month();
        let last_month = first_day_of_previous_month();

        let online_user: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE t >= $1")
            .bind(now.saturating_sub(600))
            .fetch_one(&self.db)
            .await?;
        let month_income = self.order_income_between(month, now).await?;
        let day_income = self.order_income_between(today, now).await?;
        let last_month_income = self.order_income_between(last_month, month).await?;
        let month_register_total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM users WHERE created_at >= $1 AND created_at < $2",
        )
        .bind(month)
        .bind(now)
        .fetch_one(&self.db)
        .await?;
        let day_register_total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM users WHERE created_at >= $1 AND created_at < $2",
        )
        .bind(today)
        .bind(now)
        .fetch_one(&self.db)
        .await?;
        let ticket_pending_total: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM ticket WHERE status = 0 AND reply_status = 0")
                .fetch_one(&self.db)
                .await?;
        let commission_pending_total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM orders WHERE commission_status = 0 AND invite_user_id IS NOT NULL \
             AND status NOT IN (0, 2) AND commission_balance > 0",
        )
        .fetch_one(&self.db)
        .await?;
        let payment_reconciliation_pending_total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM payment_reconciliation WHERE resolved_at IS NULL",
        )
        .fetch_one(&self.db)
        .await?;
        let payment_reconciliation_pending_amount: String = sqlx::query_scalar(
            "SELECT CAST(COALESCE(SUM(COALESCE(settled_amount, expected_amount)), 0) AS TEXT) \
             FROM payment_reconciliation WHERE resolved_at IS NULL",
        )
        .fetch_one(&self.db)
        .await?;
        let payment_reconciliation_pending_amount = exact_stat_sum_i64(
            &payment_reconciliation_pending_amount,
            "Payment reconciliation amount",
        )?;
        let commission_month_payout = self.commission_payout_between(month, now).await?;
        let commission_last_month_payout =
            self.commission_payout_between(last_month, month).await?;

        Ok(AdminOutput::Data(json!({
            "online_user": online_user,
            "month_income": month_income,
            "month_register_total": month_register_total,
            "day_register_total": day_register_total,
            "ticket_pending_total": ticket_pending_total,
            "commission_pending_total": commission_pending_total,
            "payment_reconciliation_pending_total": payment_reconciliation_pending_total,
            "payment_reconciliation_pending_amount": payment_reconciliation_pending_amount,
            "day_income": day_income,
            "last_month_income": last_month_income,
            "commission_month_payout": commission_month_payout,
            "commission_last_month_payout": commission_last_month_payout,
        })))
    }

    /// Resolves `(canonical_type, id) -> name` for every root (parent_id IS NULL)
    /// node, used to label the server rank rows.
    async fn server_name_map(&self) -> Result<HashMap<(String, i64), String>, ApiError> {
        let mut names = HashMap::new();
        for (kind, table) in SERVER_TABLES {
            let rows: Vec<(i64, String)> = QueryBuilder::<Postgres>::new(format!(
                "SELECT id::BIGINT, name FROM {table} WHERE parent_id IS NULL"
            ))
            .build_query_as()
            .fetch_all(&self.db)
            .await?;
            for (id, name) in rows {
                names.insert(((*kind).to_string(), id), name);
            }
        }
        Ok(names)
    }

    pub(super) async fn server_rank(&self, today: bool) -> Result<AdminOutput, ApiError> {
        // Ports StatController::getServerLastRank / getServerTodayRank.
        let (start, end) = if today {
            (start_of_today(), Utc::now().timestamp())
        } else {
            (start_of_yesterday(), start_of_today())
        };
        let rows: Vec<(i64, String, i64, i64)> = sqlx::query_as(
            "SELECT server_id::BIGINT, server_type, u, d FROM server_traffic \
             WHERE record_at >= $1 AND record_at < $2 AND record_type = 'd' \
             ORDER BY (CAST(u AS NUMERIC(30,0)) + CAST(d AS NUMERIC(30,0))) DESC LIMIT 15",
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.db)
        .await?;

        let names = self.server_name_map().await?;
        let mut result: Vec<Value> = rows
            .into_iter()
            .map(|(server_id, server_type, u, d)| {
                let total = (u as f64 + d as f64) / GIB as f64;
                let key = (normalize_stat_server_type(&server_type), server_id);
                let server_name = names.get(&key).cloned();
                json!({
                    "server_id": server_id,
                    "server_type": server_type,
                    "u": u,
                    "d": d,
                    "total": total,
                    "server_name": server_name,
                })
            })
            .collect();
        result.sort_by(|a, b| {
            let left = a["total"].as_f64().unwrap_or_default();
            let right = b["total"].as_f64().unwrap_or_default();
            right.total_cmp(&left)
        });
        Ok(AdminOutput::Data(json!(result)))
    }

    pub(super) async fn user_rank(&self, today: bool) -> Result<AdminOutput, ApiError> {
        // Ports StatController::getUserTodayRank / getUserLastRank: weight traffic
        // by server_rate, aggregate per user, then keep the top 15.
        let (start, end) = if today {
            (start_of_today(), Utc::now().timestamp())
        } else {
            (start_of_yesterday(), start_of_today())
        };
        let rows: Vec<(i64, f64, i64, i64, Option<String>)> = sqlx::query_as(
            "SELECT s.user_id, CAST(s.server_rate AS DOUBLE PRECISION), s.u, s.d, u.email \
             FROM user_traffic s LEFT JOIN users u ON u.id = s.user_id \
             WHERE s.record_at >= $1 AND s.record_at < $2 AND s.record_type = 'd' \
             ORDER BY (CAST(s.u AS NUMERIC(30,0)) + CAST(s.d AS NUMERIC(30,0))) DESC LIMIT 30",
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.db)
        .await?;

        // Keep the first row's raw u/d per user (Laravel only sums `total`, not the
        // displayed u/d columns) alongside the aggregated weighted total and email.
        let mut order: Vec<i64> = Vec::new();
        let mut totals: HashMap<i64, (String, f64, i64, i64)> = HashMap::new();
        for (user_id, server_rate, u, d, email) in rows {
            let total = (u as f64 + d as f64) * server_rate / GIB as f64;
            match totals.get_mut(&user_id) {
                Some(entry) => entry.1 += total,
                None => {
                    order.push(user_id);
                    totals.insert(
                        user_id,
                        (email.unwrap_or_else(|| "null".to_string()), total, u, d),
                    );
                }
            }
        }
        let mut result: Vec<Value> = order
            .into_iter()
            .filter_map(|user_id| {
                totals.get(&user_id).map(|(email, total, u, d)| {
                    json!({ "user_id": user_id, "email": email, "u": u, "d": d, "total": total })
                })
            })
            .collect();
        result.sort_by(|a, b| {
            let left = a["total"].as_f64().unwrap_or_default();
            let right = b["total"].as_f64().unwrap_or_default();
            right.total_cmp(&left)
        });
        result.truncate(15);
        Ok(AdminOutput::Data(json!(result)))
    }

    pub(super) async fn order_stat(&self) -> Result<AdminOutput, ApiError> {
        // Ports StatController::getOrder (:68-108): five series per recorded day,
        // newest 31 days, flattened then reversed to run oldest-first.
        let rows: Vec<(i64, i64, i64, i64, i64, i64)> = sqlx::query_as(
            "SELECT record_at, register_count::BIGINT, paid_total, paid_count::BIGINT, \
                    commission_total, commission_count::BIGINT \
             FROM stat WHERE record_type = 'd' ORDER BY record_at DESC LIMIT 31",
        )
        .fetch_all(&self.db)
        .await?;

        let mut result: Vec<Value> = Vec::with_capacity(rows.len() * 5);
        for (
            record_at,
            register_count,
            paid_total,
            paid_count,
            commission_total,
            commission_count,
        ) in rows
        {
            let date = local_month_day(record_at);
            result.push(json!({ "type": "注册人数", "date": date, "value": register_count }));
            result.push(
                json!({ "type": "收款金额", "date": date, "value": paid_total as f64 / 100.0 }),
            );
            result.push(json!({ "type": "收款笔数", "date": date, "value": paid_count }));
            result.push(json!({
                "type": "佣金金额(已发放)", "date": date, "value": commission_total as f64 / 100.0
            }));
            result.push(
                json!({ "type": "佣金笔数(已发放)", "date": date, "value": commission_count }),
            );
        }
        result.reverse();
        Ok(AdminOutput::Data(json!(result)))
    }

    pub(super) async fn stat_user(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let user_id = required_i64(params, "user_id")?;
        let pagination = page(params)?;
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_traffic WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&self.db)
            .await?;
        let data = fetch_json_list_page_bind(
            &self.db,
            r#"
            SELECT jsonb_build_object('record_at', record_at, 'u', u, 'd', d, 'server_rate', server_rate)
            FROM user_traffic
            WHERE user_id = $1
            ORDER BY record_at DESC
            LIMIT $2 OFFSET $3
            "#,
            user_id,
            pagination.limit,
            pagination.offset,
        )
        .await?;
        Ok(AdminOutput::Page { data, total })
    }

    pub(super) async fn stat_record(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let pagination = page(params)?;
        let record_type = params
            .get("record_type")
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let total: i64 = if let Some(record_type) = record_type {
            sqlx::query_scalar("SELECT COUNT(*) FROM stat WHERE record_type = $1")
                .bind(record_type)
                .fetch_one(&self.db)
                .await?
        } else {
            sqlx::query_scalar("SELECT COUNT(*) FROM stat")
                .fetch_one(&self.db)
                .await?
        };
        let data = if let Some(record_type) = record_type {
            fetch_json_list_page_bind_text(
                &self.db,
                r#"
                SELECT jsonb_build_object(
                    'id', id, 'record_at', record_at, 'record_type', record_type,
                    'order_count', order_count, 'order_total', order_total,
                    'commission_count', commission_count, 'commission_total', commission_total,
                    'paid_count', paid_count, 'paid_total', paid_total,
                    'register_count', register_count, 'invite_count', invite_count,
                    'transfer_used_total', transfer_used_total,
                    'created_at', created_at, 'updated_at', updated_at
                )
                FROM stat
                WHERE record_type = $1
                ORDER BY record_at DESC
                LIMIT $2 OFFSET $3
                "#,
                record_type,
                pagination.limit,
                pagination.offset,
            )
            .await?
        } else {
            fetch_json_list_page(
                &self.db,
                r#"
                SELECT jsonb_build_object(
                    'id', id, 'record_at', record_at, 'record_type', record_type,
                    'order_count', order_count, 'order_total', order_total,
                    'commission_count', commission_count, 'commission_total', commission_total,
                    'paid_count', paid_count, 'paid_total', paid_total,
                    'register_count', register_count, 'invite_count', invite_count,
                    'transfer_used_total', transfer_used_total,
                    'created_at', created_at, 'updated_at', updated_at
                )
                FROM stat
                ORDER BY record_at DESC
                LIMIT $1 OFFSET $2
                "#,
                pagination.limit,
                pagination.offset,
            )
            .await?
        };
        Ok(AdminOutput::Page { data, total })
    }

    pub(super) async fn system_status(&self) -> Result<AdminOutput, ApiError> {
        let snapshot = self.worker_snapshot().await?;
        let now = Utc::now().timestamp();
        let schedule_recent = snapshot
            .schedule_last_seen_at
            .map(|last_seen| timestamp_is_recent(now, last_seen, 180))
            .unwrap_or(false);
        let worker_running = snapshot.worker_running(now, 180);
        Ok(AdminOutput::Data(json!({
            "schedule": schedule_recent,
            "horizon": worker_running,
            "schedule_last_runtime": snapshot.schedule_last_seen_at.unwrap_or_default(),
            "logChannel": "rust",
            "logLevel": std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
            "cacheDriver": "redis",
            "backendVersion": env!("CARGO_PKG_VERSION"),
            "frontendVersion": env!("CARGO_PKG_VERSION"),
        })))
    }

    pub(super) async fn queue_stats(&self) -> Result<AdminOutput, ApiError> {
        let snapshot = self.worker_snapshot().await?;
        let now = Utc::now().timestamp();
        let worker_running = snapshot.worker_running(now, 180);
        let jobs_per_minute = snapshot
            .last_run_at
            .values()
            .filter(|last_run| timestamp_is_recent(now, **last_run, 60))
            .count();
        Ok(AdminOutput::Data(json!({
            "failedJobs": snapshot.failed_jobs(),
            "jobsPerMinute": jobs_per_minute,
            "pausedMasters": 0,
            "periods": {
                "failedJobs": snapshot.failed_jobs(),
                "recentJobs": snapshot.total_jobs(),
            },
            "processes": if worker_running { 1 } else { 0 },
            "queueWithMaxRuntime": null,
            "queueWithMaxThroughput": snapshot.max_counter_key(),
            "recentJobs": snapshot.total_jobs(),
            "status": worker_running,
            "wait": {},
            "lastRunAt": snapshot.last_run_at,
            "lastSuccessAt": snapshot.last_success_at,
            "lastFailureAt": snapshot.last_failure_at,
        })))
    }

    pub(super) async fn queue_workload(&self) -> Result<AdminOutput, ApiError> {
        let snapshot = self.worker_snapshot().await?;
        let now = Utc::now().timestamp();
        let rows = snapshot
            .job_names()
            .into_iter()
            .map(|name| {
                let total = snapshot.totals.get(&name).copied().unwrap_or_default();
                let failed = snapshot.failed.get(&name).copied().unwrap_or_default();
                let last_run_at = snapshot.last_run_at.get(&name).copied();
                json!({
                    "name": name,
                    "length": 0,
                    "wait": 0,
                    "processes": if last_run_at.map(|seen| timestamp_is_recent(now, seen, 180)).unwrap_or(false) { 1 } else { 0 },
                    "recent_jobs": total,
                    "failed_jobs": failed,
                    "last_run_at": last_run_at,
                    "last_success_at": snapshot.last_success_at.get(&name).copied(),
                    "last_failure_at": snapshot.last_failure_at.get(&name).copied(),
                })
            })
            .collect::<Vec<_>>();
        Ok(AdminOutput::Data(json!(rows)))
    }

    pub(super) async fn queue_masters(&self) -> Result<AdminOutput, ApiError> {
        let snapshot = self.worker_snapshot().await?;
        let now = Utc::now().timestamp();
        let worker_running = snapshot.worker_running(now, 180);
        Ok(AdminOutput::Data(json!([{
            "name": "rust-worker",
            "status": if worker_running { "running" } else { "stale" },
            "pid": null,
            "supervisors": snapshot.job_names(),
            "last_seen_at": snapshot.last_seen_at(),
            "schedule_last_seen_at": snapshot.schedule_last_seen_at,
        }])))
    }

    async fn worker_snapshot(&self) -> Result<WorkerSnapshot, ApiError> {
        let mut conn = self
            .redis
            .get_multiplexed_async_connection()
            .await
            .map_err(|_| ApiError::internal("failed to connect redis for worker metrics"))?;
        let schedule_last_seen_at = conn
            .get::<_, Option<i64>>(self.redis_key("SCHEDULE_LAST_CHECK_AT_"))
            .await
            .map_err(|_| ApiError::internal("failed to read scheduler heartbeat"))?;
        let totals = conn
            .hgetall::<_, BTreeMap<String, i64>>(self.redis_key("RUST_WORKER_JOBS_TOTAL"))
            .await
            .map_err(|_| ApiError::internal("failed to read worker totals"))?;
        let failed = conn
            .hgetall::<_, BTreeMap<String, i64>>(self.redis_key("RUST_WORKER_JOBS_FAILED"))
            .await
            .map_err(|_| ApiError::internal("failed to read worker failures"))?;
        let last_run_at = conn
            .hgetall::<_, BTreeMap<String, i64>>(self.redis_key("RUST_WORKER_LAST_RUN_AT"))
            .await
            .map_err(|_| ApiError::internal("failed to read worker last run"))?;
        let last_success_at = conn
            .hgetall::<_, BTreeMap<String, i64>>(self.redis_key("RUST_WORKER_LAST_SUCCESS_AT"))
            .await
            .map_err(|_| ApiError::internal("failed to read worker last success"))?;
        let last_failure_at = conn
            .hgetall::<_, BTreeMap<String, i64>>(self.redis_key("RUST_WORKER_LAST_FAILURE_AT"))
            .await
            .map_err(|_| ApiError::internal("failed to read worker last failure"))?;
        Ok(WorkerSnapshot {
            schedule_last_seen_at,
            totals,
            failed,
            last_run_at,
            last_success_at,
            last_failure_at,
        })
    }

    pub(super) async fn system_log(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let pagination = page(params)?;
        // SystemController::getSystemLog reads the ProTable filter[] scope via
        // setFilterAllowKeys('level'): each entry must be key=level, a supported
        // condition, and a non-empty value.
        let entries = collect_filter_entries(params);
        for entry in &entries {
            let key = entry.get("key").map(String::as_str).unwrap_or_default();
            let condition = entry
                .get("condition")
                .map(String::as_str)
                .unwrap_or_default();
            let value = entry.get("value").map(String::as_str).unwrap_or_default();
            if key != "level" {
                return Err(validation_error("filter.key", "选择的 filter.key 不存在"));
            }
            if !LOG_FILTER_CONDITIONS.contains(&condition) {
                return Err(validation_error(
                    "filter.condition",
                    "选择的 filter.condition 不存在",
                ));
            }
            if value.is_empty() {
                return Err(validation_error("filter.value", "filter.value 不能为空"));
            }
        }

        let mut count_builder =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM system_log WHERE 1 = 1");
        push_log_filters(&mut count_builder, &entries);
        let total: i64 = count_builder
            .build_query_scalar()
            .fetch_one(&self.db)
            .await?;

        let mut builder = QueryBuilder::<Postgres>::new(
            r#"
            SELECT jsonb_build_object(
                'id', id, 'title', title, 'level', level, 'host', host, 'uri', uri,
                'method', method, 'data', data, 'ip', ip, 'context', context,
                'created_at', created_at, 'updated_at', updated_at
            )
            FROM system_log
            WHERE 1 = 1
            "#,
        );
        push_log_filters(&mut builder, &entries);
        builder.push(" ORDER BY created_at DESC LIMIT ");
        builder.push_bind(pagination.limit);
        builder.push(" OFFSET ");
        builder.push_bind(pagination.offset);
        let data = builder
            .build_query_scalar::<Json<Value>>()
            .fetch_all(&self.db)
            .await?
            .into_iter()
            .map(|row| row.0)
            .collect();
        Ok(AdminOutput::Page { data, total })
    }
}

#[cfg(test)]
mod arithmetic_tests {
    use super::*;

    #[test]
    fn worker_counter_totals_saturate_at_i64_boundaries() {
        let snapshot = WorkerSnapshot {
            totals: BTreeMap::from([("a".to_string(), i64::MAX), ("b".to_string(), 1)]),
            failed: BTreeMap::from([("a".to_string(), i64::MIN), ("b".to_string(), -1)]),
            ..WorkerSnapshot::default()
        };
        assert_eq!(snapshot.total_jobs(), i64::MAX);
        assert_eq!(snapshot.failed_jobs(), i64::MIN);
    }

    #[test]
    fn worker_recency_handles_extreme_redis_timestamps_without_overflow() {
        assert!(timestamp_is_recent(100, 40, 60));
        assert!(!timestamp_is_recent(100, 39, 60));
        assert!(!timestamp_is_recent(i64::MAX, i64::MIN, 180));
        assert!(timestamp_is_recent(i64::MIN, i64::MAX, 180));

        let snapshot = WorkerSnapshot {
            schedule_last_seen_at: Some(i64::MIN),
            ..WorkerSnapshot::default()
        };
        assert!(!snapshot.worker_running(i64::MAX, 180));
    }

    #[test]
    fn statistic_sums_use_exact_decimal_text_and_reject_i64_overflow() {
        assert!(ORDER_INCOME_SUM_SQL.contains("AS TEXT"));
        assert!(COMMISSION_PAYOUT_SUM_SQL.contains("AS TEXT"));
        assert!(!ORDER_INCOME_SUM_SQL.contains("AS SIGNED"));
        assert!(!COMMISSION_PAYOUT_SUM_SQL.contains("AS SIGNED"));

        assert_eq!(exact_stat_sum_i64("0", "test").unwrap(), 0);
        assert_eq!(
            exact_stat_sum_i64("9223372036854775807", "test").unwrap(),
            i64::MAX
        );
        assert_eq!(
            exact_stat_sum_i64("-9223372036854775808", "test").unwrap(),
            i64::MIN
        );
        assert!(exact_stat_sum_i64("9223372036854775808", "test").is_err());
        assert!(exact_stat_sum_i64("-9223372036854775809", "test").is_err());
        assert!(exact_stat_sum_i64("1.5", "test").is_err());
        assert!(exact_stat_sum_i64("not-a-number", "test").is_err());
    }
}

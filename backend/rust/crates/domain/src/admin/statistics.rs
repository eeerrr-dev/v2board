use super::*;
use rust_decimal::prelude::ToPrimitive;
use v2board_compat::Pagination;

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

    /// GET `stats/summary` (docs/api-dialect.md §6.8, W14): the one modern
    /// route replacing the three legacy aliases (`getStat`, `getOverride`,
    /// `getRanking`). Bare object; every money field is integer cents.
    pub async fn stats_summary(&self) -> Result<Value, ApiError> {
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

        Ok(json!({
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
        }))
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

    /// GET `stats/server-rank` `?window=today|previous` (§6.8, W14): bare
    /// array replacing the legacy `getServerTodayRank`/`getServerLastRank`
    /// route pair. Row shape unchanged.
    pub async fn stats_server_rank(&self, today: bool) -> Result<Vec<Value>, ApiError> {
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
        Ok(result)
    }

    /// GET `stats/user-rank` `?window=today|previous` (§6.8, W14): bare array
    /// replacing the legacy `getUserTodayRank`/`getUserLastRank` pair.
    pub async fn stats_user_rank(&self, today: bool) -> Result<Vec<Value>, ApiError> {
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
        Ok(result)
    }

    /// GET `stats/orders` (§6.8, W14): bare array of `{series, date, value}`
    /// rows — the series re-spec replaces the legacy Chinese `type` literals
    /// with stable snake_case slugs and the yuan floats with integer cents.
    /// Window semantics unchanged from `getOrder`: five series per recorded
    /// day, newest 31 days, flattened oldest-first.
    pub async fn stats_orders(&self) -> Result<Vec<Value>, ApiError> {
        let rows: Vec<StatSeriesSourceRow> = sqlx::query_as(
            "SELECT record_at, register_count::BIGINT, paid_total, paid_count::BIGINT, \
                    commission_total, commission_count::BIGINT \
             FROM stat WHERE record_type = 'd' ORDER BY record_at DESC LIMIT 31",
        )
        .fetch_all(&self.db)
        .await?;
        Ok(stat_series_rows(rows))
    }

    /// GET `stats/user-traffic` `?user_id=&page=&per_page=` (§6.8, W14): §8
    /// `{items,total}` page. `server_rate` crosses as a JSON number and
    /// `record_at` as an RFC 3339 instant (§4.5).
    pub async fn stats_user_traffic(
        &self,
        user_id: i64,
        pagination: Pagination,
    ) -> Result<(Vec<Value>, i64), ApiError> {
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_traffic WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&self.db)
            .await?;
        let rows: Vec<(i64, i64, i64, f64)> = sqlx::query_as(
            "SELECT record_at, u, d, CAST(server_rate AS DOUBLE PRECISION) \
             FROM user_traffic WHERE user_id = $1 \
             ORDER BY record_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(user_id)
        .bind(pagination.limit())
        .bind(pagination.offset())
        .fetch_all(&self.db)
        .await?;
        let items = rows
            .into_iter()
            .map(|(record_at, u, d, server_rate)| {
                json!({
                    "record_at": rfc3339_epoch(record_at),
                    "u": u,
                    "d": d,
                    "server_rate": server_rate,
                })
            })
            .collect();
        Ok((items, total))
    }

    /// GET `stats/records` `?type=` (§6.8, W14): bare array of the same
    /// `{series, date, value}` rows as `stats/orders`, parameterized by the
    /// `stat.record_type` bucket (`d` daily — the default — or `m` monthly).
    /// The legacy paginated raw-row projection is retired with the series
    /// re-spec; the window mirrors `stats/orders` (newest 31 recorded
    /// periods, flattened oldest-first).
    pub async fn stats_records(&self, record_type: &str) -> Result<Vec<Value>, ApiError> {
        let rows: Vec<StatSeriesSourceRow> = sqlx::query_as(
            "SELECT record_at, register_count::BIGINT, paid_total, paid_count::BIGINT, \
                    commission_total, commission_count::BIGINT \
             FROM stat WHERE record_type = $1 ORDER BY record_at DESC LIMIT 31",
        )
        .bind(record_type)
        .fetch_all(&self.db)
        .await?;
        Ok(stat_series_rows(rows))
    }

    /// GET `system/status` (docs/api-dialect.md §6.1): scheduler/worker
    /// health as a bare object — snake_case keys and RFC 3339 timestamps per
    /// §4.5 (the legacy camelCase Horizon vocabulary is retired).
    pub async fn system_status_view(&self) -> Result<Value, ApiError> {
        let snapshot = self.worker_snapshot().await?;
        let now = Utc::now().timestamp();
        let schedule_recent = snapshot
            .schedule_last_seen_at
            .map(|last_seen| timestamp_is_recent(now, last_seen, 180))
            .unwrap_or(false);
        let worker_running = snapshot.worker_running(now, 180);
        Ok(json!({
            "schedule": schedule_recent,
            "horizon": worker_running,
            "schedule_last_runtime": rfc3339_epoch_option(snapshot.schedule_last_seen_at),
            "log_channel": "rust",
            "log_level": std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
            "cache_driver": "redis",
            "backend_version": env!("CARGO_PKG_VERSION"),
            "frontend_version": env!("CARGO_PKG_VERSION"),
        }))
    }

    /// GET `system/queue-stats` (docs/api-dialect.md §6.1): bare worker
    /// counters — snake_case keys, RFC 3339 timestamp maps.
    pub async fn queue_stats_view(&self) -> Result<Value, ApiError> {
        let snapshot = self.worker_snapshot().await?;
        let now = Utc::now().timestamp();
        let worker_running = snapshot.worker_running(now, 180);
        let jobs_per_minute = snapshot
            .last_run_at
            .values()
            .filter(|last_run| timestamp_is_recent(now, **last_run, 60))
            .count();
        Ok(json!({
            "failed_jobs": snapshot.failed_jobs(),
            "jobs_per_minute": jobs_per_minute,
            "paused_masters": 0,
            "periods": {
                "failed_jobs": snapshot.failed_jobs(),
                "recent_jobs": snapshot.total_jobs(),
            },
            "processes": if worker_running { 1 } else { 0 },
            "queue_with_max_runtime": null,
            "queue_with_max_throughput": snapshot.max_counter_key(),
            "recent_jobs": snapshot.total_jobs(),
            "status": worker_running,
            "wait": {},
            "last_run_at": rfc3339_epoch_map(&snapshot.last_run_at),
            "last_success_at": rfc3339_epoch_map(&snapshot.last_success_at),
            "last_failure_at": rfc3339_epoch_map(&snapshot.last_failure_at),
        }))
    }

    /// GET `system/queue-workload` (docs/api-dialect.md §6.1): a bare array of
    /// per-job counters with RFC 3339 timestamps.
    pub async fn queue_workload_view(&self) -> Result<Value, ApiError> {
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
                    "last_run_at": rfc3339_epoch_option(last_run_at),
                    "last_success_at": rfc3339_epoch_option(snapshot.last_success_at.get(&name).copied()),
                    "last_failure_at": rfc3339_epoch_option(snapshot.last_failure_at.get(&name).copied()),
                })
            })
            .collect::<Vec<_>>();
        Ok(json!(rows))
    }

    /// GET `system/queue-masters` (docs/api-dialect.md §6.1): a bare array —
    /// the single native worker process with RFC 3339 heartbeats.
    pub async fn queue_masters_view(&self) -> Result<Value, ApiError> {
        let snapshot = self.worker_snapshot().await?;
        let now = Utc::now().timestamp();
        let worker_running = snapshot.worker_running(now, 180);
        Ok(json!([{
            "name": "rust-worker",
            "status": if worker_running { "running" } else { "stale" },
            "pid": null,
            "supervisors": snapshot.job_names(),
            "last_seen_at": rfc3339_epoch_option(snapshot.last_seen_at()),
            "schedule_last_seen_at": rfc3339_epoch_option(snapshot.schedule_last_seen_at),
        }]))
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

    /// GET `system/logs` (docs/api-dialect.md §6.1): §8 pagination plus the
    /// §7 filter/sort DSL — this route is the DSL's first consumer, with the
    /// §7.1 whitelist pinned to `level` only. Rows keep the legacy key set
    /// with §4.5 RFC 3339 `created_at`/`updated_at`.
    pub async fn system_logs(
        &self,
        pagination: Pagination,
        filter: Option<&str>,
        sort_by: Option<&str>,
        sort_dir: Option<&str>,
    ) -> Result<(Vec<Value>, i64), ApiError> {
        let clauses = filter
            .map(filter_dsl::parse_filter_param)
            .transpose()?
            .unwrap_or_default();
        let filters = filter_dsl::resolve_filters(&clauses, SYSTEM_LOG_FILTER_COLUMNS)?;
        let sort = filter_dsl::resolve_sort(sort_by, sort_dir, SYSTEM_LOG_SORT_COLUMNS)?;

        let mut count_builder =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM system_log WHERE 1 = 1");
        filter_dsl::push_filter_where(&mut count_builder, &filters);
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
        filter_dsl::push_filter_where(&mut builder, &filters);
        builder.push(format!(" ORDER BY {} LIMIT ", sort.order_by()));
        builder.push_bind(pagination.limit());
        builder.push(" OFFSET ");
        builder.push_bind(pagination.offset());
        let items = builder
            .build_query_scalar::<Json<Value>>()
            .fetch_all(&self.db)
            .await?
            .into_iter()
            .map(|row| epoch_fields_to_rfc3339(row.0, &["created_at", "updated_at"]))
            .collect();
        Ok((items, total))
    }

    /// GET `system/audit-logs` (docs/api-dialect.md §6.11): the append-only
    /// operator audit trail, read through the same §8 pagination and §7
    /// filter/sort DSL as `system/logs`.
    pub async fn audit_logs(
        &self,
        pagination: Pagination,
        filter: Option<&str>,
        sort_by: Option<&str>,
        sort_dir: Option<&str>,
    ) -> Result<(Vec<Value>, i64), ApiError> {
        let clauses = filter
            .map(filter_dsl::parse_filter_param)
            .transpose()?
            .unwrap_or_default();
        let filters = filter_dsl::resolve_filters(&clauses, AUDIT_LOG_FILTER_COLUMNS)?;
        let sort = filter_dsl::resolve_sort(sort_by, sort_dir, AUDIT_LOG_SORT_COLUMNS)?;

        let mut count_builder =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM audit_log WHERE 1 = 1");
        filter_dsl::push_filter_where(&mut count_builder, &filters);
        let total: i64 = count_builder
            .build_query_scalar()
            .fetch_one(&self.db)
            .await?;

        let mut builder = QueryBuilder::<Postgres>::new(
            r#"
            SELECT jsonb_build_object(
                'id', id, 'actor_id', actor_id, 'actor_email', actor_email,
                'session_id', session_id, 'surface', surface, 'method', method,
                'path', path, 'status_code', status_code, 'client_ip', client_ip,
                'request_id', request_id, 'created_at', created_at
            )
            FROM audit_log
            WHERE 1 = 1
            "#,
        );
        filter_dsl::push_filter_where(&mut builder, &filters);
        builder.push(format!(" ORDER BY {} LIMIT ", sort.order_by()));
        builder.push_bind(pagination.limit());
        builder.push(" OFFSET ");
        builder.push_bind(pagination.offset());
        let items = builder
            .build_query_scalar::<Json<Value>>()
            .fetch_all(&self.db)
            .await?
            .into_iter()
            .map(|row| epoch_fields_to_rfc3339(row.0, &["created_at"]))
            .collect();
        Ok((items, total))
    }
}

/// §7.1 filter whitelist for `GET system/logs`: `level` only.
const SYSTEM_LOG_FILTER_COLUMNS: &[filter_dsl::FilterColumn] = &[filter_dsl::FilterColumn {
    field: "level",
    expr: "level",
    kind: filter_dsl::ColumnKind::Text,
}];

/// §7.2 sort whitelist: the filterable fields plus the `created_at` default.
const SYSTEM_LOG_SORT_COLUMNS: &[filter_dsl::SortColumn] = &[
    filter_dsl::SortColumn {
        field: "created_at",
        expr: "created_at",
    },
    filter_dsl::SortColumn {
        field: "level",
        expr: "level",
    },
];

/// §7.1 filter whitelist for `GET system/audit-logs`.
const AUDIT_LOG_FILTER_COLUMNS: &[filter_dsl::FilterColumn] = &[
    filter_dsl::FilterColumn {
        field: "surface",
        expr: "surface",
        kind: filter_dsl::ColumnKind::Text,
    },
    filter_dsl::FilterColumn {
        field: "actor_email",
        expr: "actor_email",
        kind: filter_dsl::ColumnKind::Text,
    },
    filter_dsl::FilterColumn {
        field: "method",
        expr: "method",
        kind: filter_dsl::ColumnKind::Text,
    },
];

/// §7.2 sort whitelist: the `created_at` default only.
const AUDIT_LOG_SORT_COLUMNS: &[filter_dsl::SortColumn] = &[filter_dsl::SortColumn {
    field: "created_at",
    expr: "created_at",
}];

/// §4.5: epoch seconds cross the boundary as RFC 3339 UTC strings (or null).
/// Matches `v2board_compat::json::rfc3339`'s `Z`-suffixed seconds form.
fn rfc3339_epoch(epoch_seconds: i64) -> Value {
    chrono::DateTime::from_timestamp(epoch_seconds, 0)
        .map(|instant| Value::String(instant.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)))
        .unwrap_or(Value::Null)
}

fn rfc3339_epoch_option(epoch_seconds: Option<i64>) -> Value {
    epoch_seconds.map(rfc3339_epoch).unwrap_or(Value::Null)
}

fn rfc3339_epoch_map(timestamps: &BTreeMap<String, i64>) -> Value {
    Value::Object(
        timestamps
            .iter()
            .map(|(name, epoch)| (name.clone(), rfc3339_epoch(*epoch)))
            .collect(),
    )
}

/// Converts named epoch-integer members of a database-built JSON row into
/// their §4.5 RFC 3339 form. Shared with the W11 commerce lists.
pub(super) fn epoch_fields_to_rfc3339(mut row: Value, fields: &[&str]) -> Value {
    if let Some(object) = row.as_object_mut() {
        for field in fields {
            if let Some(value) = object.get_mut(*field)
                && let Some(epoch) = value.as_i64()
            {
                *value = rfc3339_epoch(epoch);
            }
        }
    }
    row
}

/// One decoded `stat` source row feeding the §6.8 series builders:
/// `(record_at, register_count, paid_total, paid_count, commission_total,
/// commission_count)`.
type StatSeriesSourceRow = (i64, i64, i64, i64, i64, i64);

/// The W14 series re-spec (docs/api-dialect.md §6.8): the legacy rows embedded
/// Chinese literals as machine series keys and shipped money as yuan floats.
/// Modern rows are `{series, date, value}` with stable snake_case slugs and
/// integer-cent money; the client maps `series → i18n`. Rows flatten
/// newest-first and then reverse, preserving the legacy oldest-first chart
/// order exactly (including the within-day series order the reversal implies).
fn stat_series_rows(rows: Vec<StatSeriesSourceRow>) -> Vec<Value> {
    let mut result: Vec<Value> = Vec::with_capacity(rows.len() * 5);
    for (record_at, register_count, paid_total, paid_count, commission_total, commission_count) in
        rows
    {
        let date = local_month_day(record_at);
        result.push(json!({ "series": "register_count", "date": date, "value": register_count }));
        result.push(json!({ "series": "paid_total", "date": date, "value": paid_total }));
        result.push(json!({ "series": "paid_count", "date": date, "value": paid_count }));
        result.push(
            json!({ "series": "commission_paid_total", "date": date, "value": commission_total }),
        );
        result.push(
            json!({ "series": "commission_paid_count", "date": date, "value": commission_count }),
        );
    }
    result.reverse();
    result
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
    fn stat_series_rows_use_snake_case_slugs_and_integer_cents() {
        // §6.8 W14 series re-spec: five rows per recorded period, snake_case
        // `series` slugs (never the legacy Chinese literals), integer-cent
        // money (never `paid_total / 100.0` yuan floats), flattened
        // oldest-first via the legacy flatten-then-reverse order.
        let rows = stat_series_rows(vec![
            (86_400 * 2, 7, 12_345, 3, 999, 2), // newer period
            (86_400, 1, 100, 1, 50, 1),         // older period
        ]);
        assert_eq!(rows.len(), 10);

        // Reversal puts the older period first, its series order reversed.
        let expected_series = [
            "commission_paid_count",
            "commission_paid_total",
            "paid_count",
            "paid_total",
            "register_count",
        ];
        for (index, series) in expected_series.iter().enumerate() {
            assert_eq!(rows[index]["series"], json!(series));
            assert_eq!(rows[index + 5]["series"], json!(series));
        }

        // Money crosses as raw integer cents; every value is a JSON integer.
        let newer: Vec<&Value> = rows[5..].iter().collect();
        assert_eq!(newer[3]["value"], json!(12_345));
        assert_eq!(newer[1]["value"], json!(999));
        for row in &rows {
            assert!(row["value"].is_i64(), "series value must be an integer");
            assert!(row["date"].is_string());
            let series = row["series"].as_str().unwrap();
            assert_eq!(series.to_ascii_lowercase(), series);
            assert!(series.is_ascii());
        }
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

//! Loopback-only Prometheus exporter.
//!
//! `GET /metrics` shares the `/healthz`/`/readyz` origin gate: in production
//! it answers only the direct `127.0.0.1:8080` peer and is never reachable
//! through the Cloudflare Tunnel. It surfaces the alerting signals the
//! operations documentation mandates — worker/scheduler heartbeats and job
//! counters (read from the worker-owned Redis metrics keys the API principal
//! may read), the analytics outbox admission gauges, readiness component
//! states, and process-local HTTP counters — in the Prometheus text
//! exposition format. Collection failures degrade to absent families or
//! zeroed `_up` gauges; the endpoint itself always answers 200.

use std::{
    collections::BTreeMap,
    fmt::Write as _,
    sync::atomic::{AtomicI64, AtomicU64, Ordering},
    time::Duration,
};

use axum::{
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use v2board_analytics::{
    AnalyticsAdmissionSnapshot, AnalyticsPressureState, analytics_admission_snapshot,
};
use v2board_db::migrations_current;

use crate::runtime::AppState;

const COLLECT_DEADLINE: Duration = Duration::from_secs(3);
const PROMETHEUS_CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";
const STATUS_CLASSES: [&str; 5] = ["1xx", "2xx", "3xx", "4xx", "5xx"];
const THRESHOLD_LEVELS: [&str; 3] = ["recovery", "soft", "hard"];

/// Process-local HTTP counters incremented by [`http_metrics_middleware`].
#[derive(Debug, Default)]
pub(crate) struct HttpMetrics {
    in_flight: AtomicI64,
    classes: [AtomicU64; 5],
}

impl HttpMetrics {
    fn begin(&self) {
        self.in_flight.fetch_add(1, Ordering::Relaxed);
    }

    fn finish(&self, status: StatusCode) {
        self.in_flight.fetch_sub(1, Ordering::Relaxed);
        if let Some(class) = status_class_index(status) {
            self.classes[class].fetch_add(1, Ordering::Relaxed);
        }
    }

    fn snapshot(&self) -> HttpSnapshot {
        HttpSnapshot {
            in_flight: self.in_flight.load(Ordering::Relaxed),
            classes: [0, 1, 2, 3, 4].map(|class| self.classes[class].load(Ordering::Relaxed)),
        }
    }
}

fn status_class_index(status: StatusCode) -> Option<usize> {
    match status.as_u16() / 100 {
        class @ 1..=5 => Some(class as usize - 1),
        _ => None,
    }
}

pub(crate) async fn http_metrics_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    state.http_metrics.begin();
    let response = next.run(request).await;
    state.http_metrics.finish(response.status());
    response
}

#[derive(Debug)]
struct HttpSnapshot {
    in_flight: i64,
    classes: [u64; 5],
}

/// Worker heartbeat and job counters mirrored from the worker-owned Redis
/// metrics keys (the API Redis principal holds read-only access to them).
#[derive(Debug, Default)]
struct WorkerMetrics {
    scheduler_last_check_at: Option<i64>,
    loop_heartbeat_at: BTreeMap<String, i64>,
    jobs_total: BTreeMap<String, i64>,
    jobs_failed: BTreeMap<String, i64>,
    last_run_at: BTreeMap<String, i64>,
    last_success_at: BTreeMap<String, i64>,
    last_failure_at: BTreeMap<String, i64>,
}

async fn read_worker_metrics(state: &AppState) -> Result<WorkerMetrics, redis::RedisError> {
    let mut connection = state.auth_redis.clone();
    let scheduler_last_check_at = redis::cmd("GET")
        .arg(state.redis_key("SCHEDULE_LAST_CHECK_AT_"))
        .query_async::<Option<i64>>(&mut connection)
        .await?;
    let mut hashes = Vec::with_capacity(6);
    for logical_key in [
        "RUST_WORKER_LOOP_HEARTBEAT_AT",
        "RUST_WORKER_JOBS_TOTAL",
        "RUST_WORKER_JOBS_FAILED",
        "RUST_WORKER_LAST_RUN_AT",
        "RUST_WORKER_LAST_SUCCESS_AT",
        "RUST_WORKER_LAST_FAILURE_AT",
    ] {
        hashes.push(
            redis::cmd("HGETALL")
                .arg(state.redis_key(logical_key))
                .query_async::<BTreeMap<String, i64>>(&mut connection)
                .await?,
        );
    }
    let mut hashes = hashes.into_iter();
    Ok(WorkerMetrics {
        scheduler_last_check_at,
        loop_heartbeat_at: hashes.next().unwrap_or_default(),
        jobs_total: hashes.next().unwrap_or_default(),
        jobs_failed: hashes.next().unwrap_or_default(),
        last_run_at: hashes.next().unwrap_or_default(),
        last_success_at: hashes.next().unwrap_or_default(),
        last_failure_at: hashes.next().unwrap_or_default(),
    })
}

#[derive(Debug)]
struct MetricsSnapshot {
    postgres_up: bool,
    redis_up: bool,
    frontend_release_present: bool,
    operator_config_acknowledged: bool,
    operator_config_authority_healthy: bool,
    admission: Option<AnalyticsAdmissionSnapshot>,
    worker: Option<WorkerMetrics>,
    http: HttpSnapshot,
}

pub(crate) async fn metrics(State(state): State<AppState>) -> Response {
    let postgres = tokio::time::timeout(COLLECT_DEADLINE, async {
        sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&state.db)
            .await?;
        migrations_current(&state.db).await
    });
    let redis_ping = tokio::time::timeout(COLLECT_DEADLINE, async {
        let mut connection = state.auth_redis.clone();
        redis::cmd("PING")
            .query_async::<String>(&mut connection)
            .await
    });
    let admission = tokio::time::timeout(COLLECT_DEADLINE, analytics_admission_snapshot(&state.db));
    let worker = tokio::time::timeout(COLLECT_DEADLINE, read_worker_metrics(&state));
    let config = state.config_snapshot();
    let user_index = tokio::fs::metadata(
        config
            .runtime_paths
            .frontend
            .join("current/user/index.html"),
    );
    let admin_index = tokio::fs::metadata(
        config
            .runtime_paths
            .frontend
            .join("current/admin/index.html"),
    );
    let (postgres, redis_ping, admission, worker, user_index, admin_index) = tokio::join!(
        postgres,
        redis_ping,
        admission,
        worker,
        user_index,
        admin_index
    );
    let snapshot = MetricsSnapshot {
        postgres_up: postgres.is_ok_and(|result| result.is_ok_and(|current| current)),
        redis_up: redis_ping.is_ok_and(|result| result.as_deref() == Ok("PONG")),
        frontend_release_present: user_index.is_ok_and(|metadata| metadata.is_file())
            && admin_index.is_ok_and(|metadata| metadata.is_file()),
        operator_config_acknowledged: state.operator_config_acknowledged(),
        operator_config_authority_healthy: state.operator_config_authority_healthy(),
        admission: admission.ok().and_then(|result| match result {
            Ok(snapshot) => Some(snapshot),
            Err(error) => {
                tracing::warn!(?error, "analytics admission metrics observation failed");
                None
            }
        }),
        worker: worker.ok().and_then(|result| match result {
            Ok(worker) => Some(worker),
            Err(error) => {
                tracing::warn!(?error, "worker metrics observation failed");
                None
            }
        }),
        http: state.http_metrics.snapshot(),
    };
    let mut response = render(&snapshot).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(PROMETHEUS_CONTENT_TYPE),
    );
    response
}

fn render(snapshot: &MetricsSnapshot) -> String {
    let mut out = String::with_capacity(4096);
    gauge(
        &mut out,
        "v2board_build_info",
        "Constant 1 labeled with the running crate version.",
    );
    sample(
        &mut out,
        "v2board_build_info",
        &[("crate_version", env!("CARGO_PKG_VERSION"))],
        1,
    );
    for (name, help, value) in [
        (
            "v2board_postgres_up",
            "PostgreSQL answers and the migration ledger is current (1) or not (0).",
            snapshot.postgres_up,
        ),
        (
            "v2board_redis_up",
            "Redis answers PING (1) or not (0).",
            snapshot.redis_up,
        ),
        (
            "v2board_frontend_release_present",
            "Both frontend index documents exist in the current release (1) or not (0).",
            snapshot.frontend_release_present,
        ),
        (
            "v2board_operator_config_acknowledged",
            "No operator config revision is awaiting acknowledgement (1) or one is pending (0).",
            snapshot.operator_config_acknowledged,
        ),
        (
            "v2board_operator_config_authority_healthy",
            "The PostgreSQL operator-config authority is readable (1) or not (0).",
            snapshot.operator_config_authority_healthy,
        ),
    ] {
        gauge(&mut out, name, help);
        sample(&mut out, name, &[], i64::from(value));
    }
    render_admission(&mut out, snapshot.admission.as_ref());
    if let Some(worker) = &snapshot.worker {
        render_worker(&mut out, worker);
    }
    gauge(
        &mut out,
        "v2board_http_requests_in_flight",
        "HTTP requests currently being served.",
    );
    sample(
        &mut out,
        "v2board_http_requests_in_flight",
        &[],
        snapshot.http.in_flight,
    );
    counter(
        &mut out,
        "v2board_http_requests_total",
        "HTTP responses since process start, by status class.",
    );
    for (class, count) in STATUS_CLASSES.iter().zip(snapshot.http.classes) {
        sample(
            &mut out,
            "v2board_http_requests_total",
            &[("class", class)],
            count,
        );
    }
    out
}

fn render_admission(out: &mut String, admission: Option<&AnalyticsAdmissionSnapshot>) {
    gauge(
        out,
        "v2board_analytics_admission_observed",
        "The analytics admission snapshot was readable this scrape (1) or not (0).",
    );
    sample(
        out,
        "v2board_analytics_admission_observed",
        &[],
        i64::from(admission.is_some()),
    );
    let Some(admission) = admission else {
        return;
    };
    gauge(
        out,
        "v2board_analytics_pressure_state",
        "One-hot analytics admission pressure state.",
    );
    for state in [
        AnalyticsPressureState::Normal,
        AnalyticsPressureState::SoftPressure,
        AnalyticsPressureState::HardStop,
    ] {
        sample(
            out,
            "v2board_analytics_pressure_state",
            &[("state", state.as_str())],
            i64::from(admission.pressure_state == state),
        );
    }
    for (name, help, value) in [
        (
            "v2board_analytics_sample_fresh",
            "The admission sample is within its staleness budget (1) or stale (0).",
            u64::from(admission.sample_fresh),
        ),
        (
            "v2board_analytics_sample_age_seconds",
            "Age of the admission sample.",
            admission.sample_age_seconds,
        ),
        (
            "v2board_analytics_pending_rows",
            "Rows pending in the analytics outbox.",
            admission.pending_rows,
        ),
        (
            "v2board_analytics_accounted_pending_rows",
            "Outbox rows the admission policy currently accounts for.",
            admission.accounted_pending_rows,
        ),
        (
            "v2board_analytics_relation_total_bytes",
            "Total outbox relation size (heap, index, and toast).",
            admission.relation_total_bytes,
        ),
        (
            "v2board_analytics_accounted_relation_bytes",
            "Relation bytes the admission policy currently accounts for.",
            admission.accounted_relation_bytes,
        ),
        (
            "v2board_analytics_database_bytes",
            "Size of the PostgreSQL database that hosts the outbox.",
            admission.database_bytes,
        ),
        (
            "v2board_analytics_database_capacity_bytes",
            "Operator-declared PostgreSQL capacity budget.",
            admission.database_capacity_bytes,
        ),
    ] {
        gauge(out, name, help);
        sample(out, name, &[], value);
    }
    gauge(
        out,
        "v2board_analytics_capacity_headroom_bytes",
        "Declared capacity minus the current database size.",
    );
    sample(
        out,
        "v2board_analytics_capacity_headroom_bytes",
        &[],
        admission.capacity_headroom_bytes,
    );
    if let Some(oldest) = admission.oldest_pending_age_seconds {
        gauge(
            out,
            "v2board_analytics_oldest_pending_age_seconds",
            "Age of the oldest pending outbox row; absent when the outbox is empty.",
        );
        sample(
            out,
            "v2board_analytics_oldest_pending_age_seconds",
            &[],
            oldest,
        );
    }
    for (name, help, values) in [
        (
            "v2board_analytics_threshold_pending_rows",
            "Admission pending-row thresholds by level.",
            [
                admission.recovery_pending_rows,
                admission.soft_pending_rows,
                admission.hard_pending_rows,
            ],
        ),
        (
            "v2board_analytics_threshold_relation_bytes",
            "Admission relation-size thresholds by level.",
            [
                admission.recovery_relation_bytes,
                admission.soft_relation_bytes,
                admission.hard_relation_bytes,
            ],
        ),
        (
            "v2board_analytics_threshold_oldest_age_seconds",
            "Admission oldest-pending-age thresholds by level.",
            [
                admission.recovery_oldest_age_seconds,
                admission.soft_oldest_age_seconds,
                admission.hard_oldest_age_seconds,
            ],
        ),
        (
            "v2board_analytics_threshold_min_headroom_bytes",
            "Admission minimum-headroom thresholds by level.",
            [
                admission.recovery_min_headroom_bytes,
                admission.soft_min_headroom_bytes,
                admission.hard_min_headroom_bytes,
            ],
        ),
    ] {
        gauge(out, name, help);
        for (level, value) in THRESHOLD_LEVELS.iter().zip(values) {
            sample(out, name, &[("level", level)], value);
        }
    }
}

fn render_worker(out: &mut String, worker: &WorkerMetrics) {
    if let Some(at) = worker.scheduler_last_check_at {
        gauge(
            out,
            "v2board_worker_scheduler_last_check_timestamp_seconds",
            "Unix time of the scheduler's last liveness mark.",
        );
        sample(
            out,
            "v2board_worker_scheduler_last_check_timestamp_seconds",
            &[],
            at,
        );
    }
    labeled_family(
        out,
        "v2board_worker_loop_heartbeat_timestamp_seconds",
        "Unix time of each worker loop's last heartbeat.",
        MetricKind::Gauge,
        "loop",
        &worker.loop_heartbeat_at,
    );
    labeled_family(
        out,
        "v2board_worker_jobs_total",
        "Scheduled job executions since the metrics keys were created.",
        MetricKind::Counter,
        "job",
        &worker.jobs_total,
    );
    labeled_family(
        out,
        "v2board_worker_jobs_failed_total",
        "Failed scheduled job executions since the metrics keys were created.",
        MetricKind::Counter,
        "job",
        &worker.jobs_failed,
    );
    labeled_family(
        out,
        "v2board_worker_job_last_run_timestamp_seconds",
        "Unix time each job last ran.",
        MetricKind::Gauge,
        "job",
        &worker.last_run_at,
    );
    labeled_family(
        out,
        "v2board_worker_job_last_success_timestamp_seconds",
        "Unix time each job last succeeded.",
        MetricKind::Gauge,
        "job",
        &worker.last_success_at,
    );
    labeled_family(
        out,
        "v2board_worker_job_last_failure_timestamp_seconds",
        "Unix time each job last failed.",
        MetricKind::Gauge,
        "job",
        &worker.last_failure_at,
    );
}

#[derive(Clone, Copy)]
enum MetricKind {
    Gauge,
    Counter,
}

fn labeled_family(
    out: &mut String,
    name: &str,
    help: &str,
    kind: MetricKind,
    label: &str,
    values: &BTreeMap<String, i64>,
) {
    if values.is_empty() {
        return;
    }
    match kind {
        MetricKind::Gauge => gauge(out, name, help),
        MetricKind::Counter => counter(out, name, help),
    }
    for (key, value) in values {
        sample(out, name, &[(label, key)], *value);
    }
}

fn gauge(out: &mut String, name: &str, help: &str) {
    family_header(out, name, help, "gauge");
}

fn counter(out: &mut String, name: &str, help: &str) {
    family_header(out, name, help, "counter");
}

fn family_header(out: &mut String, name: &str, help: &str, kind: &str) {
    let help = help.replace('\\', "\\\\").replace('\n', "\\n");
    let _ = writeln!(out, "# HELP {name} {help}");
    let _ = writeln!(out, "# TYPE {name} {kind}");
}

fn sample<V: std::fmt::Display>(out: &mut String, name: &str, labels: &[(&str, &str)], value: V) {
    out.push_str(name);
    if !labels.is_empty() {
        out.push('{');
        for (index, (key, label_value)) in labels.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            let _ = write!(out, "{key}=\"{}\"", escape_label_value(label_value));
        }
        out.push('}');
    }
    let _ = writeln!(out, " {value}");
}

fn escape_label_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    fn admission_fixture() -> AnalyticsAdmissionSnapshot {
        AnalyticsAdmissionSnapshot {
            installation_id: Uuid::nil(),
            policy_sha256: "policy".into(),
            pressure_state: AnalyticsPressureState::SoftPressure,
            generation: 3,
            sampled_at: 1_700_000_000,
            sample_age_seconds: 12,
            sample_fresh: true,
            sample_interval_seconds: 30,
            stale_after_seconds: 120,
            recovery_pending_rows: 10,
            soft_pending_rows: 20,
            hard_pending_rows: 30,
            recovery_relation_bytes: 100,
            soft_relation_bytes: 200,
            hard_relation_bytes: 300,
            recovery_oldest_age_seconds: 60,
            soft_oldest_age_seconds: 120,
            hard_oldest_age_seconds: 240,
            database_capacity_bytes: 1_000_000,
            hard_min_headroom_bytes: 1_000,
            soft_min_headroom_bytes: 2_000,
            recovery_min_headroom_bytes: 3_000,
            event_reservation_bytes: 64,
            soft_max_new_rows_per_second: 500,
            pending_rows: 21,
            accounted_pending_rows: 19,
            oldest_pending_age_seconds: Some(90),
            relation_heap_bytes: 1,
            relation_index_bytes: 2,
            relation_toast_bytes: 3,
            relation_total_bytes: 6,
            accounted_relation_bytes: 5,
            database_bytes: 500_000,
            capacity_headroom_bytes: 500_000,
            state_changed_at: 1_699_999_000,
            last_transition_reason: "test".into(),
        }
    }

    #[test]
    fn label_values_escape_prometheus_metacharacters() {
        assert_eq!(escape_label_value(r#"a"b\c"#), r#"a\"b\\c"#);
        assert_eq!(escape_label_value("a\nb"), r"a\nb");
    }

    #[test]
    fn status_classes_map_to_their_counter_slot() {
        assert_eq!(status_class_index(StatusCode::CONTINUE), Some(0));
        assert_eq!(status_class_index(StatusCode::NO_CONTENT), Some(1));
        assert_eq!(status_class_index(StatusCode::PERMANENT_REDIRECT), Some(2));
        assert_eq!(status_class_index(StatusCode::NOT_FOUND), Some(3));
        assert_eq!(status_class_index(StatusCode::BAD_GATEWAY), Some(4));
    }

    #[test]
    fn render_emits_the_mandated_alerting_families() {
        let mut worker = WorkerMetrics {
            scheduler_last_check_at: Some(1_700_000_100),
            ..WorkerMetrics::default()
        };
        worker
            .loop_heartbeat_at
            .insert("stat_rollup".into(), 1_700_000_050);
        worker.jobs_total.insert("send_email".into(), 42);
        worker.jobs_failed.insert("send_email".into(), 2);
        let snapshot = MetricsSnapshot {
            postgres_up: true,
            redis_up: false,
            frontend_release_present: true,
            operator_config_acknowledged: true,
            operator_config_authority_healthy: true,
            admission: Some(admission_fixture()),
            worker: Some(worker),
            http: HttpSnapshot {
                in_flight: 1,
                classes: [0, 7, 0, 3, 1],
            },
        };
        let text = render(&snapshot);
        assert!(text.contains("v2board_postgres_up 1\n"));
        assert!(text.contains("v2board_redis_up 0\n"));
        assert!(text.contains("# TYPE v2board_worker_jobs_total counter\n"));
        assert!(text.contains("v2board_worker_jobs_total{job=\"send_email\"} 42\n"));
        assert!(text.contains("v2board_worker_jobs_failed_total{job=\"send_email\"} 2\n"));
        assert!(
            text.contains("v2board_worker_scheduler_last_check_timestamp_seconds 1700000100\n")
        );
        assert!(text.contains(
            "v2board_worker_loop_heartbeat_timestamp_seconds{loop=\"stat_rollup\"} 1700000050\n"
        ));
        assert!(text.contains("v2board_analytics_admission_observed 1\n"));
        assert!(text.contains("v2board_analytics_pressure_state{state=\"soft_pressure\"} 1\n"));
        assert!(text.contains("v2board_analytics_pressure_state{state=\"normal\"} 0\n"));
        assert!(text.contains("v2board_analytics_pending_rows 21\n"));
        assert!(text.contains("v2board_analytics_oldest_pending_age_seconds 90\n"));
        assert!(text.contains("v2board_analytics_threshold_pending_rows{level=\"hard\"} 30\n"));
        assert!(text.contains("v2board_http_requests_total{class=\"2xx\"} 7\n"));
        assert!(text.contains("v2board_http_requests_in_flight 1\n"));
    }

    #[test]
    fn empty_worker_and_absent_admission_render_without_empty_families() {
        let snapshot = MetricsSnapshot {
            postgres_up: false,
            redis_up: false,
            frontend_release_present: false,
            operator_config_acknowledged: false,
            operator_config_authority_healthy: false,
            admission: None,
            worker: Some(WorkerMetrics::default()),
            http: HttpSnapshot {
                in_flight: 0,
                classes: [0; 5],
            },
        };
        let text = render(&snapshot);
        assert!(text.contains("v2board_analytics_admission_observed 0\n"));
        assert!(!text.contains("v2board_analytics_pending_rows"));
        assert!(!text.contains("v2board_worker_jobs_total"));
        assert!(!text.contains("v2board_worker_scheduler_last_check_timestamp_seconds"));
        assert!(text.contains("v2board_http_requests_total{class=\"5xx\"} 0\n"));
    }

    #[tokio::test]
    async fn metrics_answer_even_when_every_backing_service_is_down() {
        let state = AppState::service_free_test(v2board_config::AppConfig::from_api_env());
        let response = metrics(State(state)).await;
        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        assert_eq!(content_type, PROMETHEUS_CONTENT_TYPE);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("metrics body");
        let text = String::from_utf8(body.to_vec()).expect("utf-8 metrics body");
        assert!(text.contains("v2board_postgres_up 0\n"));
        assert!(text.contains("v2board_redis_up 0\n"));
        assert!(text.contains("v2board_analytics_admission_observed 0\n"));
        assert!(text.contains("v2board_build_info{crate_version=\""));
    }
}

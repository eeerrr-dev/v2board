use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use redis::AsyncCommands;
use reqwest::{Client, Method, StatusCode};
use serde_json::{Map, Value, json};
use sqlx::MySqlPool;

#[tokio::main]
async fn main() -> Result<()> {
    match env::args().nth(1).as_deref().unwrap_or("contract") {
        "contract" => run_contract().await,
        "worker-reconcile" => run_worker_reconcile().await,
        command => bail!("unknown command `{command}`; expected `contract` or `worker-reconcile`"),
    }
}

async fn run_contract() -> Result<()> {
    let config = ContractConfig::from_env();
    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let laravel = Target::new("laravel", config.laravel_base_url.clone(), client.clone());
    let rust = Target::new("rust", config.rust_base_url.clone(), client);
    let selected = selected_scenarios();
    let mut results = Vec::new();

    let laravel_login = login(&laravel, &config).await.context("laravel login")?;
    let rust_login = login(&rust, &config).await.context("rust login")?;
    if scenario_selected(&selected, "auth.login") {
        results.push(compare_pair(
            "auth.login",
            &laravel_login.snapshot,
            &rust_login.snapshot,
            &Mode::Exact,
        ));
    }

    for scenario in scenarios() {
        if !scenario_selected(&selected, scenario.name) {
            continue;
        }
        let laravel_auth = scenario
            .auth_header
            .then_some(laravel_login.auth_data.as_str());
        let rust_auth = scenario
            .auth_header
            .then_some(rust_login.auth_data.as_str());
        let laravel_token = scenario.token_query.then_some(laravel_login.token.as_str());
        let rust_token = scenario.token_query.then_some(rust_login.token.as_str());
        let laravel_snapshot = laravel
            .request(&scenario, laravel_auth, laravel_token)
            .await
            .with_context(|| format!("laravel {}", scenario.name))?;
        let rust_snapshot = rust
            .request(&scenario, rust_auth, rust_token)
            .await
            .with_context(|| format!("rust {}", scenario.name))?;
        results.push(compare_pair(
            scenario.name,
            &laravel_snapshot,
            &rust_snapshot,
            &scenario.mode,
        ));
    }

    report_contract_results(&results)?;
    Ok(())
}

async fn run_worker_reconcile() -> Result<()> {
    let database_url = env_or("DATABASE_URL", "mysql://v2board:v2board@mysql:3306/v2board");
    let redis_url = env_or("REDIS_URL", "redis://redis:6379/1");
    let strict = env_bool("WORKER_RECONCILE_STRICT", true);
    let pool = MySqlPool::connect(&database_url).await?;
    let redis = redis::Client::open(redis_url.as_str())?;
    let mut conn = redis.get_multiplexed_async_connection().await?;
    let now = Utc::now().timestamp();
    let mut checks = Vec::new();

    let heartbeat = conn
        .get::<_, Option<i64>>("SCHEDULE_LAST_CHECK_AT_")
        .await?
        .unwrap_or_default();
    let heartbeat_age = (heartbeat > 0).then_some(now - heartbeat);
    checks.push(ReconcileCheck::new(
        "scheduler_heartbeat_recent",
        heartbeat > 0 && now - heartbeat <= 180,
        json!({ "last_seen": heartbeat, "age_seconds": heartbeat_age }),
        true,
    ));

    let last_runs = conn
        .hgetall::<_, BTreeMap<String, i64>>("RUST_WORKER_LAST_RUN_AT")
        .await
        .unwrap_or_default();
    for job in ["traffic_update", "check_order", "check_ticket"] {
        let recent = last_runs
            .get(job)
            .map(|last_run| now - *last_run <= 180)
            .unwrap_or(false);
        checks.push(ReconcileCheck::new(
            format!("worker_metric_{job}_recent"),
            recent,
            json!({ "last_seen": last_runs.get(job), "age_seconds": last_runs.get(job).map(|last_run| now - *last_run) }),
            true,
        ));
    }

    let scheduler_locks = conn
        .keys::<_, Vec<String>>("RUST_SCHEDULER_LOCK_*")
        .await
        .unwrap_or_default();
    checks.push(ReconcileCheck::new(
        "scheduler_locks_released",
        scheduler_locks.is_empty(),
        json!({ "locks": scheduler_locks }),
        true,
    ));

    let upload_traffic_len = conn
        .hlen::<_, usize>("v2board_upload_traffic")
        .await
        .unwrap_or_default();
    let download_traffic_len = conn
        .hlen::<_, usize>("v2board_download_traffic")
        .await
        .unwrap_or_default();
    checks.push(ReconcileCheck::new(
        "traffic_redis_buffers_drained",
        upload_traffic_len == 0 && download_traffic_len == 0,
        json!({ "upload_entries": upload_traffic_len, "download_entries": download_traffic_len }),
        true,
    ));

    let traffic_reset_lock_exists = conn
        .exists::<_, bool>("traffic_reset_lock")
        .await
        .unwrap_or_default();
    checks.push(ReconcileCheck::new(
        "traffic_reset_lock_absent",
        !traffic_reset_lock_exists,
        json!({ "exists": traffic_reset_lock_exists }),
        true,
    ));

    let pending_paid_orders =
        count(&pool, "SELECT COUNT(*) FROM v2_order WHERE status = 1").await?;
    checks.push(ReconcileCheck::new(
        "paid_orders_opened",
        pending_paid_orders == 0,
        json!({ "status_1_orders": pending_paid_orders }),
        true,
    ));

    let expired_unpaid_orders = count_with_i64(
        &pool,
        "SELECT COUNT(*) FROM v2_order WHERE status = 0 AND created_at <= ?",
        now - 7200,
    )
    .await?;
    checks.push(ReconcileCheck::new(
        "expired_unpaid_orders_cancelled",
        expired_unpaid_orders == 0,
        json!({ "expired_status_0_orders": expired_unpaid_orders }),
        true,
    ));

    let stale_tickets = count_with_i64(
        &pool,
        r#"
        SELECT COUNT(*)
        FROM v2_ticket t
        WHERE t.status = 0
          AND t.reply_status = 1
          AND t.updated_at <= ?
          AND (
            SELECT tm.user_id
            FROM v2_ticket_message tm
            WHERE tm.ticket_id = t.id
            ORDER BY tm.id DESC
            LIMIT 1
          ) <> t.user_id
        "#,
        now - 86_400,
    )
    .await?;
    checks.push(ReconcileCheck::new(
        "stale_answered_tickets_closed",
        stale_tickets == 0,
        json!({ "stale_open_tickets": stale_tickets }),
        true,
    ));

    let commission_ready = count(
        &pool,
        r#"
        SELECT COUNT(*)
        FROM v2_order
        WHERE commission_status = 1
          AND invite_user_id IS NOT NULL
        "#,
    )
    .await?;
    checks.push(ReconcileCheck::new(
        "commission_ready_queue_drained",
        commission_ready == 0,
        json!({ "commission_status_1_orders": commission_ready }),
        false,
    ));

    let yesterday = yesterday_start_timestamp();
    let stat_exists = count_with_i64(
        &pool,
        "SELECT COUNT(*) FROM v2_stat WHERE record_at = ?",
        yesterday,
    )
    .await?;
    checks.push(ReconcileCheck::new(
        "yesterday_statistics_present",
        stat_exists > 0,
        json!({ "record_at": yesterday, "rows": stat_exists }),
        false,
    ));

    report_reconcile_results(&checks, strict)?;
    Ok(())
}

#[derive(Clone)]
struct ContractConfig {
    laravel_base_url: String,
    rust_base_url: String,
    email: String,
    password: String,
}

impl ContractConfig {
    fn from_env() -> Self {
        Self {
            laravel_base_url: env_or("CONTRACT_LARAVEL_BASE_URL", "http://app:8000"),
            rust_base_url: env_or("CONTRACT_RUST_BASE_URL", "http://rust-api:8080"),
            email: env_or("CONTRACT_ADMIN_EMAIL", "admin@local"),
            password: env_or("CONTRACT_ADMIN_PASSWORD", "12345678"),
        }
    }
}

#[derive(Clone)]
struct Target {
    name: &'static str,
    base_url: String,
    client: Client,
}

impl Target {
    fn new(name: &'static str, base_url: String, client: Client) -> Self {
        Self {
            name,
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
        }
    }

    async fn request(
        &self,
        scenario: &Scenario,
        auth_data: Option<&str>,
        token: Option<&str>,
    ) -> Result<Snapshot> {
        let mut url = format!("{}{}", self.base_url, scenario.path);
        if let Some(token) = token {
            let separator = if scenario.path.contains('?') {
                '&'
            } else {
                '?'
            };
            url.push(separator);
            url.push_str("token=");
            url.push_str(token);
        }
        let mut request = self
            .client
            .request(scenario.method.clone(), url)
            .header("Accept", "application/json")
            .header("User-Agent", "v2board-rust-contract/0.1");
        if let Some(auth_data) = auth_data {
            request = request.header("authorization", auth_data);
        }
        if !scenario.form.is_empty() {
            let body = serde_urlencoded::to_string(&scenario.form)?;
            request = request
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(body);
        }
        Snapshot::from_response(self.name, scenario.name, request.send().await?).await
    }
}

struct LoginOutput {
    auth_data: String,
    token: String,
    snapshot: Snapshot,
}

async fn login(target: &Target, config: &ContractConfig) -> Result<LoginOutput> {
    let scenario = Scenario {
        name: "auth.login",
        method: Method::POST,
        path: "/api/v1/passport/auth/login",
        auth_header: false,
        token_query: false,
        form: vec![
            ("email", config.email.clone()),
            ("password", config.password.clone()),
        ],
        mode: Mode::Exact,
    };
    let snapshot = target.request(&scenario, None, None).await?;
    if snapshot.status != StatusCode::OK.as_u16() {
        bail!(
            "{} login returned HTTP {}: {}",
            target.name,
            snapshot.status,
            snapshot.body
        );
    }
    let auth_data = snapshot
        .json
        .as_ref()
        .and_then(|json| json_at(json, "data.auth_data"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| {
            anyhow!(
                "{} login response did not include data.auth_data",
                target.name
            )
        })?;
    let token = snapshot
        .json
        .as_ref()
        .and_then(|json| json_at(json, "data.token"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow!("{} login response did not include data.token", target.name))?;
    Ok(LoginOutput {
        auth_data,
        token,
        snapshot,
    })
}

#[derive(Clone)]
struct Scenario {
    name: &'static str,
    method: Method,
    path: &'static str,
    auth_header: bool,
    token_query: bool,
    form: Vec<(&'static str, String)>,
    mode: Mode,
}

#[derive(Clone)]
enum Mode {
    Exact,
    Selected(Vec<&'static str>),
    Shape(Vec<&'static str>),
    BodyNonEmpty(&'static str),
    StatusOnly,
    RustMayImproveLegacy5xx,
}

fn scenarios() -> Vec<Scenario> {
    vec![
        get(
            "guest.config",
            "/api/v1/guest/comm/config",
            false,
            Mode::Exact,
        ),
        client_get(
            "client.app.config",
            "/api/v1/client/app/getConfig",
            Mode::BodyNonEmpty("yaml"),
        ),
        client_get(
            "client.app.version",
            "/api/v1/client/app/getVersion",
            Mode::Exact,
        ),
        get(
            "user.info",
            "/api/v1/user/info",
            true,
            Mode::Selected(vec![
                "data.email",
                "data.is_admin",
                "data.is_staff",
                "data.token",
            ]),
        ),
        get(
            "user.check_login",
            "/api/v1/user/checkLogin",
            true,
            Mode::Selected(vec!["data.is_login", "data.is_admin"]),
        ),
        get(
            "user.stat",
            "/api/v1/user/getStat",
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "user.subscribe",
            "/api/v1/user/getSubscribe",
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "user.plan.fetch",
            "/api/v1/user/plan/fetch",
            true,
            Mode::Exact,
        ),
        get(
            "user.order.fetch",
            "/api/v1/user/order/fetch",
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "user.order.payment_methods",
            "/api/v1/user/order/getPaymentMethod",
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "user.invite.fetch",
            "/api/v1/user/invite/fetch",
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "user.invite.details",
            "/api/v1/user/invite/details",
            true,
            Mode::Shape(vec!["data", "total"]),
        ),
        get(
            "user.ticket.fetch",
            "/api/v1/user/ticket/fetch",
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "user.server.fetch",
            "/api/v1/user/server/fetch",
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "user.knowledge.fetch",
            "/api/v1/user/knowledge/fetch?language=zh-CN",
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "user.notice.fetch",
            "/api/v1/user/notice/fetch",
            true,
            Mode::Shape(vec!["data", "total"]),
        ),
        get(
            "user.telegram.bot_info",
            "/api/v1/user/telegram/getBotInfo",
            true,
            Mode::StatusOnly,
        ),
        get(
            "user.comm.config",
            "/api/v1/user/comm/config",
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "user.traffic.logs",
            "/api/v1/user/stat/getTrafficLog",
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.config.fetch",
            "/api/v1/admin/config/fetch",
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.plan.fetch",
            "/api/v1/admin/plan/fetch",
            true,
            Mode::Exact,
        ),
        get(
            "admin.user.fetch",
            "/api/v1/admin/user/fetch",
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.order.fetch",
            "/api/v1/admin/order/fetch",
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.notice.fetch",
            "/api/v1/admin/notice/fetch",
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.ticket.fetch",
            "/api/v1/admin/ticket/fetch",
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.coupon.fetch",
            "/api/v1/admin/coupon/fetch",
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.giftcard.fetch",
            "/api/v1/admin/giftcard/fetch",
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.knowledge.fetch",
            "/api/v1/admin/knowledge/fetch",
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.payment.fetch",
            "/api/v1/admin/payment/fetch",
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.payment.methods",
            "/api/v1/admin/payment/getPaymentMethods",
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.server.groups",
            "/api/v1/admin/server/group/fetch",
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.server.routes",
            "/api/v1/admin/server/route/fetch",
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.server.nodes",
            "/api/v1/admin/server/manage/getNodes",
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.stat.summary",
            "/api/v1/admin/stat/getStat",
            true,
            Mode::RustMayImproveLegacy5xx,
        ),
        get(
            "admin.stat.override",
            "/api/v1/admin/stat/getOverride",
            true,
            Mode::StatusOnly,
        ),
        get(
            "admin.stat.ranking",
            "/api/v1/admin/stat/getRanking",
            true,
            Mode::RustMayImproveLegacy5xx,
        ),
        get(
            "admin.system.status",
            "/api/v1/admin/system/getSystemStatus",
            true,
            Mode::Shape(vec!["data.schedule", "data.horizon"]),
        ),
        get(
            "admin.queue.stats",
            "/api/v1/admin/system/getQueueStats",
            true,
            Mode::Shape(vec!["data.status", "data.recentJobs"]),
        ),
        get(
            "admin.queue.workload",
            "/api/v1/admin/system/getQueueWorkload",
            true,
            Mode::Shape(vec!["data"]),
        ),
    ]
}

fn get(name: &'static str, path: &'static str, auth: bool, mode: Mode) -> Scenario {
    Scenario {
        name,
        method: Method::GET,
        path,
        auth_header: auth,
        token_query: false,
        form: Vec::new(),
        mode,
    }
}

fn client_get(name: &'static str, path: &'static str, mode: Mode) -> Scenario {
    Scenario {
        name,
        method: Method::GET,
        path,
        auth_header: false,
        token_query: true,
        form: Vec::new(),
        mode,
    }
}

#[derive(Debug)]
struct Snapshot {
    status: u16,
    content_type: Option<String>,
    body: String,
    json: Option<Value>,
}

impl Snapshot {
    async fn from_response(
        target: &str,
        scenario: &str,
        response: reqwest::Response,
    ) -> Result<Self> {
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let body = response.text().await?;
        let json = serde_json::from_str(&body).ok();
        let _ = (target, scenario);
        Ok(Self {
            status,
            content_type,
            body,
            json,
        })
    }
}

struct ContractResult {
    name: String,
    ok: bool,
    details: String,
}

fn compare_pair(name: &str, laravel: &Snapshot, rust: &Snapshot, mode: &Mode) -> ContractResult {
    if matches!(mode, Mode::RustMayImproveLegacy5xx) {
        return compare_rust_may_improve_legacy_5xx(name, laravel, rust);
    }
    if laravel.status != rust.status {
        return ContractResult {
            name: name.to_string(),
            ok: false,
            details: format!(
                "HTTP status mismatch: laravel={} rust={}",
                laravel.status, rust.status
            ),
        };
    }

    match mode {
        Mode::Exact => {
            let left = normalize_snapshot_json(laravel);
            let right = normalize_snapshot_json(rust);
            if left == right {
                pass(name)
            } else {
                ContractResult {
                    name: name.to_string(),
                    ok: false,
                    details: format!(
                        "JSON mismatch\nlaravel={}\nrust={}",
                        pretty(&left),
                        pretty(&right)
                    ),
                }
            }
        }
        Mode::Selected(paths) => compare_selected(name, laravel, rust, paths),
        Mode::Shape(paths) => compare_shape(name, laravel, rust, paths),
        Mode::BodyNonEmpty(content_type_hint) => {
            compare_body_non_empty(name, laravel, rust, content_type_hint)
        }
        Mode::StatusOnly => pass(name),
        Mode::RustMayImproveLegacy5xx => unreachable!("handled before status comparison"),
    }
}

fn compare_rust_may_improve_legacy_5xx(
    name: &str,
    laravel: &Snapshot,
    rust: &Snapshot,
) -> ContractResult {
    if laravel.status == rust.status {
        return pass(name);
    }
    if laravel.status >= 500 && rust.status == StatusCode::OK.as_u16() {
        return pass(name);
    }
    fail(
        name,
        format!(
            "HTTP status mismatch: laravel={} rust={}",
            laravel.status, rust.status
        ),
    )
}

fn compare_selected(
    name: &str,
    laravel: &Snapshot,
    rust: &Snapshot,
    paths: &[&'static str],
) -> ContractResult {
    let Some(laravel_json) = laravel.json.as_ref() else {
        return fail(name, "laravel response is not JSON");
    };
    let Some(rust_json) = rust.json.as_ref() else {
        return fail(name, "rust response is not JSON");
    };
    let laravel_json = normalize_json(laravel_json);
    let rust_json = normalize_json(rust_json);
    for path in paths {
        let left = json_at(&laravel_json, path);
        let right = json_at(&rust_json, path);
        if left != right {
            return ContractResult {
                name: name.to_string(),
                ok: false,
                details: format!(
                    "selected field `{path}` mismatch: laravel={} rust={}",
                    pretty_opt(left),
                    pretty_opt(right)
                ),
            };
        }
    }
    pass(name)
}

fn compare_shape(
    name: &str,
    laravel: &Snapshot,
    rust: &Snapshot,
    paths: &[&'static str],
) -> ContractResult {
    let Some(laravel_json) = laravel.json.as_ref() else {
        return fail(name, "laravel response is not JSON");
    };
    let Some(rust_json) = rust.json.as_ref() else {
        return fail(name, "rust response is not JSON");
    };
    for path in paths {
        let left = json_at(laravel_json, path);
        let right = json_at(rust_json, path);
        if left.is_none() || right.is_none() {
            return ContractResult {
                name: name.to_string(),
                ok: false,
                details: format!(
                    "required path `{path}` missing: laravel={} rust={}",
                    left.is_some(),
                    right.is_some()
                ),
            };
        }
        if left.map(value_kind) != right.map(value_kind) {
            return ContractResult {
                name: name.to_string(),
                ok: false,
                details: format!(
                    "required path `{path}` kind mismatch: laravel={} rust={}",
                    left.map(value_kind).unwrap_or("missing"),
                    right.map(value_kind).unwrap_or("missing")
                ),
            };
        }
    }
    pass(name)
}

fn compare_body_non_empty(
    name: &str,
    laravel: &Snapshot,
    rust: &Snapshot,
    content_type_hint: &str,
) -> ContractResult {
    if laravel.body.trim().is_empty() || rust.body.trim().is_empty() {
        return fail(
            name,
            format!(
                "empty body: laravel={} rust={}",
                laravel.body.trim().is_empty(),
                rust.body.trim().is_empty()
            ),
        );
    }
    for (target, content_type) in [
        ("laravel", laravel.content_type.as_deref()),
        ("rust", rust.content_type.as_deref()),
    ] {
        if !content_type
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains(content_type_hint)
        {
            return fail(
                name,
                format!(
                    "{target} content-type `{}` does not contain `{content_type_hint}`",
                    content_type.unwrap_or("<missing>")
                ),
            );
        }
    }
    pass(name)
}

fn normalize_snapshot_json(snapshot: &Snapshot) -> Value {
    snapshot
        .json
        .as_ref()
        .map(normalize_json)
        .unwrap_or_else(|| Value::String(snapshot.body.clone()))
}

fn normalize_json(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut normalized = Map::new();
            for (key, value) in object {
                if dynamic_json_key(key) {
                    normalized.insert(key.clone(), Value::String("<dynamic>".to_string()));
                } else {
                    normalized.insert(key.clone(), normalize_json(value));
                }
            }
            Value::Object(normalized)
        }
        Value::Array(items) => Value::Array(items.iter().map(normalize_json).collect()),
        _ => value.clone(),
    }
}

fn dynamic_json_key(key: &str) -> bool {
    matches!(
        key,
        "auth_data"
            | "session"
            | "created_at"
            | "updated_at"
            | "last_login_at"
            | "schedule_last_runtime"
    )
}

fn json_at<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.')
        .try_fold(value, |current, segment| match current {
            Value::Object(object) => object.get(segment),
            _ => None,
        })
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn pretty_opt(value: Option<&Value>) -> String {
    value.map(pretty).unwrap_or_else(|| "<missing>".to_string())
}

fn pass(name: &str) -> ContractResult {
    ContractResult {
        name: name.to_string(),
        ok: true,
        details: String::new(),
    }
}

fn fail(name: &str, details: impl Into<String>) -> ContractResult {
    ContractResult {
        name: name.to_string(),
        ok: false,
        details: details.into(),
    }
}

fn report_contract_results(results: &[ContractResult]) -> Result<()> {
    let mut failed = 0;
    for result in results {
        if result.ok {
            println!("PASS {}", result.name);
        } else {
            failed += 1;
            println!("FAIL {}\n{}", result.name, result.details);
        }
    }
    if failed > 0 {
        bail!(
            "contract parity failed: {failed}/{} scenarios failed",
            results.len()
        );
    }
    println!("Contract parity OK: {} scenarios passed.", results.len());
    Ok(())
}

fn selected_scenarios() -> Option<BTreeSet<String>> {
    env::var("CONTRACT_SCENARIOS").ok().map(|value| {
        value
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(ToOwned::to_owned)
            .collect()
    })
}

fn scenario_selected(selected: &Option<BTreeSet<String>>, name: &str) -> bool {
    selected
        .as_ref()
        .map(|selected| selected.contains(name))
        .unwrap_or(true)
}

struct ReconcileCheck {
    name: String,
    ok: bool,
    strict: bool,
    details: Value,
}

impl ReconcileCheck {
    fn new(name: impl Into<String>, ok: bool, details: Value, strict: bool) -> Self {
        Self {
            name: name.into(),
            ok,
            strict,
            details,
        }
    }
}

fn report_reconcile_results(checks: &[ReconcileCheck], strict_mode: bool) -> Result<()> {
    let mut failed = 0;
    let mut warnings = 0;
    for check in checks {
        if check.ok {
            println!("PASS {} {}", check.name, check.details);
        } else if check.strict && strict_mode {
            failed += 1;
            println!("FAIL {} {}", check.name, check.details);
        } else {
            warnings += 1;
            println!("WARN {} {}", check.name, check.details);
        }
    }
    if failed > 0 {
        bail!("worker reconciliation failed: {failed} strict checks failed");
    }
    println!(
        "Worker reconciliation OK: {} checks passed, {warnings} warnings.",
        checks.len() - failed - warnings
    );
    Ok(())
}

async fn count(pool: &MySqlPool, sql: &'static str) -> Result<i64> {
    sqlx::query_scalar::<_, i64>(sql)
        .fetch_one(pool)
        .await
        .map_err(Into::into)
}

async fn count_with_i64(pool: &MySqlPool, sql: &'static str, value: i64) -> Result<i64> {
    sqlx::query_scalar::<_, i64>(sql)
        .bind(value)
        .fetch_one(pool)
        .await
        .map_err(Into::into)
}

fn yesterday_start_timestamp() -> i64 {
    let now = Utc::now();
    let today = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .expect("valid midnight");
    today.and_utc().timestamp() - 86_400
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_bool(key: &str, default: bool) -> bool {
    env::var(key)
        .ok()
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(default)
}

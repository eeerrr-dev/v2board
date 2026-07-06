use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::Path,
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
        "route-audit" => run_route_audit(),
        "worker-reconcile" => run_worker_reconcile().await,
        command => bail!(
            "unknown command `{command}`; expected `contract`, `route-audit`, or `worker-reconcile`"
        ),
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

    for scenario in scenarios(&config) {
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

fn run_route_audit() -> Result<()> {
    let config = ContractConfig::from_env();
    let laravel_root = env_or("ROUTE_AUDIT_LARAVEL_ROOT", "/src/backend/laravel");
    let rust_root = env_or("ROUTE_AUDIT_RUST_ROOT", "/src/backend/rust");
    let laravel = collect_laravel_routes(Path::new(&laravel_root), &config.admin_path)?;
    let rust = collect_rust_routes(Path::new(&rust_root), &config.admin_path)?;
    let missing = laravel.difference(&rust).cloned().collect::<Vec<_>>();

    if !missing.is_empty() {
        println!("Laravel routes missing in Rust:");
        for route in &missing {
            println!("MISSING {} {}", route.method, route.path);
        }
        bail!(
            "route audit failed: {} Laravel routes are missing",
            missing.len()
        );
    }
    println!(
        "Route audit OK: {} Laravel routes are represented in Rust.",
        laravel.len()
    );
    Ok(())
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct RouteKey {
    method: String,
    path: String,
}

fn route_key(method: impl AsRef<str>, path: impl AsRef<str>) -> RouteKey {
    RouteKey {
        method: method.as_ref().to_ascii_uppercase(),
        path: normalize_route_path(path.as_ref()),
    }
}

fn collect_laravel_routes(root: &Path, admin_path: &str) -> Result<BTreeSet<RouteKey>> {
    let v1 = root.join("app/Http/Routes/V1");
    let v2 = root.join("app/Http/Routes/V2");
    let mut routes = BTreeSet::new();
    for (file, prefix, root_segment) in [
        ("GuestRoute.php", "/api/v1/guest", "guest"),
        ("ClientRoute.php", "/api/v1/client", "client"),
        ("PassportRoute.php", "/api/v1/passport", "passport"),
        ("UserRoute.php", "/api/v1/user", "user"),
        ("StaffRoute.php", "/api/v1/staff", "staff"),
        ("AdminRoute.php", "", ""),
        ("ServerRoute.php", "/api/v1/server", "server"),
    ] {
        let prefix = if file == "AdminRoute.php" {
            format!("/api/v1/{admin_path}")
        } else {
            prefix.to_string()
        };
        routes.extend(parse_laravel_route_file(
            &v1.join(file),
            &prefix,
            root_segment,
        )?);
    }
    routes.extend(parse_laravel_route_file(
        &v2.join("ServerRoute.php"),
        "/api/v2/server",
        "server",
    )?);
    Ok(routes)
}

fn parse_laravel_route_file(
    path: &Path,
    root_prefix: &str,
    root_segment: &str,
) -> Result<BTreeSet<RouteKey>> {
    let content =
        fs::read_to_string(path).with_context(|| format!("read Laravel route file {path:?}"))?;
    let mut routes = BTreeSet::new();
    let mut prefixes = vec![normalize_route_path(root_prefix)];
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("});") && prefixes.len() > 1 {
            prefixes.pop();
            continue;
        }
        if let Some(prefix) = extract_laravel_group_prefix(trimmed) {
            if prefixes.len() == 1 && prefix.trim_matches('/') == root_segment {
                continue;
            }
            prefixes.push(prefix);
            continue;
        }
        let Some((methods, route_path)) = extract_laravel_route(trimmed) else {
            continue;
        };
        let full_path = join_route_parts(
            prefixes
                .iter()
                .map(String::as_str)
                .chain([route_path.as_str()]),
        );
        for method in methods {
            routes.insert(route_key(method, &full_path));
        }
    }
    Ok(routes)
}

fn collect_rust_routes(root: &Path, admin_path: &str) -> Result<BTreeSet<RouteKey>> {
    let api_main = fs::read_to_string(root.join("crates/api/src/main.rs"))?;
    let admin = fs::read_to_string(root.join("crates/domain/src/admin.rs"))?;
    let mut routes = collect_rust_axum_routes(&api_main);
    routes.retain(|route| {
        !route.path.contains("{*admin_path}") && !route.path.contains("{*staff_path}")
    });

    for path in rust_admin_match_paths(&admin, "get") {
        routes.insert(route_key("GET", format!("/api/v1/{admin_path}/{path}")));
    }
    for path in rust_admin_match_paths(&admin, "post") {
        routes.insert(route_key("POST", format!("/api/v1/{admin_path}/{path}")));
    }
    for path in rust_admin_match_paths(&admin, "staff_get") {
        routes.insert(route_key("GET", format!("/api/v1/staff/{path}")));
    }
    for path in rust_admin_match_paths(&admin, "staff_post") {
        routes.insert(route_key("POST", format!("/api/v1/staff/{path}")));
    }
    for kind in [
        "shadowsocks",
        "vmess",
        "trojan",
        "tuic",
        "hysteria",
        "vless",
        "anytls",
        "v2node",
    ] {
        for action in ["save", "drop", "update", "copy"] {
            routes.insert(route_key(
                "POST",
                format!("/api/v1/{admin_path}/server/{kind}/{action}"),
            ));
        }
    }
    Ok(routes)
}

fn collect_rust_axum_routes(content: &str) -> BTreeSet<RouteKey> {
    let lines = content.lines().collect::<Vec<_>>();
    let mut routes = BTreeSet::new();
    let mut index = 0;
    while index < lines.len() {
        if !lines[index].contains(".route(") {
            index += 1;
            continue;
        }
        let (block, next_index) = rust_route_block(&lines, index);
        index = next_index;
        let Some(path) = quoted_strings(&block)
            .into_iter()
            .find(|value| value.starts_with('/'))
        else {
            continue;
        };
        if path.contains("{*") {
            continue;
        }
        if block.contains("get(") {
            routes.insert(route_key("GET", &path));
        }
        if block.contains("post(") || block.contains(".post(") {
            routes.insert(route_key("POST", &path));
        }
    }
    routes
}

fn rust_route_block(lines: &[&str], start: usize) -> (String, usize) {
    let mut block = String::new();
    let mut depth = 0_i32;
    for (index, line) in lines.iter().enumerate().skip(start) {
        let segment = if index == start {
            line.split_once(".route(")
                .map(|(_, right)| format!(".route({right}"))
                .unwrap_or_else(|| (*line).to_string())
        } else {
            (*line).to_string()
        };
        for ch in segment.chars() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
        }
        block.push_str(&segment);
        block.push(' ');
        if depth <= 0 {
            return (block, index + 1);
        }
    }
    (block, lines.len())
}

fn rust_admin_match_paths(content: &str, function_name: &str) -> Vec<String> {
    let start_marker = format!("pub async fn {function_name}");
    let mut in_function = false;
    let mut in_match = false;
    let mut paths = Vec::new();
    for line in content.lines() {
        if line.contains(&start_marker) {
            in_function = true;
            continue;
        }
        if !in_function {
            continue;
        }
        if line.contains("match path.as_str()") {
            in_match = true;
            continue;
        }
        if !in_match {
            continue;
        }
        if line.contains("_ =>") {
            break;
        }
        let Some((left, _)) = line.split_once("=>") else {
            continue;
        };
        if left.trim_start().starts_with('_') {
            continue;
        }
        paths.extend(quoted_strings(left));
    }
    paths
}

fn extract_laravel_group_prefix(line: &str) -> Option<String> {
    let (left, right) = line.split_once("=>")?;
    if !left.contains("'prefix'") && !left.contains("\"prefix\"") {
        return None;
    }
    let right = right.trim();
    if !(right.starts_with('\'') || right.starts_with('"')) {
        return None;
    }
    quoted_strings(right).into_iter().next()
}

fn extract_laravel_route(line: &str) -> Option<(Vec<&'static str>, String)> {
    let router = line.find("$router->")?;
    let rest = &line[router + "$router->".len()..];
    let method = rest.split_once('(')?.0.trim().to_ascii_lowercase();
    let methods = match method.as_str() {
        "get" => vec!["GET"],
        "post" => vec!["POST"],
        "any" => vec!["GET", "POST"],
        "match" => vec!["GET", "POST"],
        _ => return None,
    };
    let route_path = quoted_strings(rest).into_iter().find(|value| {
        !matches!(value.as_str(), "get" | "post" | "put" | "patch" | "delete")
            && !value.contains('\\')
    })?;
    Some((methods, route_path))
}

fn quoted_strings(input: &str) -> Vec<String> {
    let mut output = Vec::new();
    let mut chars = input.char_indices().peekable();
    while let Some((_, ch)) = chars.next() {
        if ch != '\'' && ch != '"' {
            continue;
        }
        let quote = ch;
        let mut value = String::new();
        let mut escaped = false;
        for (_, ch) in chars.by_ref() {
            if escaped {
                value.push(ch);
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == quote {
                break;
            }
            value.push(ch);
        }
        output.push(value);
    }
    output
}

fn join_route_parts<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    let body = parts
        .into_iter()
        .flat_map(|part| part.split('/'))
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    format!("/{body}")
}

fn normalize_route_path(path: &str) -> String {
    join_route_parts([path])
}

#[derive(Clone)]
struct ContractConfig {
    laravel_base_url: String,
    rust_base_url: String,
    email: String,
    password: String,
    admin_path: String,
}

impl ContractConfig {
    fn from_env() -> Self {
        let admin_path = env_or("CONTRACT_ADMIN_PATH", "admin")
            .trim_matches('/')
            .to_string();
        Self {
            laravel_base_url: env_or("CONTRACT_LARAVEL_BASE_URL", "http://app:8000"),
            rust_base_url: env_or("CONTRACT_RUST_BASE_URL", "http://rust-api:8080"),
            email: env_or("CONTRACT_ADMIN_EMAIL", "admin@local"),
            password: env_or("CONTRACT_ADMIN_PASSWORD", "12345678"),
            admin_path: if admin_path.is_empty() {
                "admin".to_string()
            } else {
                admin_path
            },
        }
    }

    fn admin_api_path(&self, path: &str) -> String {
        format!(
            "/api/v1/{}/{}",
            self.admin_path,
            path.trim_start_matches('/')
        )
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
        path: "/api/v1/passport/auth/login".to_string(),
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
    path: String,
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
    RustBodyContains {
        content_type_hint: &'static str,
        needles: Vec<&'static str>,
    },
    RustContentType(&'static str),
    RustShape(Vec<&'static str>),
    ErrorMessage,
    StatusOnly,
    RustMayImproveLegacy5xx,
}

fn scenarios(config: &ContractConfig) -> Vec<Scenario> {
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
        client_get(
            "client.subscribe.clash",
            "/api/v1/client/subscribe?flag=clash",
            Mode::RustBodyContains {
                content_type_hint: "yaml",
                needles: vec!["proxies:", "proxy-groups:"],
            },
        ),
        client_get(
            "client.subscribe.singbox",
            "/api/v1/client/subscribe?flag=sing-box",
            Mode::RustBodyContains {
                content_type_hint: "json",
                needles: vec!["\"outbounds\"", "节点选择", "domain_strategy"],
            },
        ),
        client_get(
            "client.subscribe.singbox.modern",
            "/api/v1/client/subscribe?flag=sing-box%201.12.0",
            Mode::RustBodyContains {
                content_type_hint: "json",
                needles: vec!["\"outbounds\"", "节点选择", "route_exclude_address_set"],
            },
        ),
        client_get(
            "client.subscribe.surge",
            "/api/v1/client/subscribe?flag=surge",
            Mode::RustBodyContains {
                content_type_hint: "text/plain",
                needles: vec!["[Proxy]", "[Proxy Group]"],
            },
        ),
        client_get(
            "client.subscribe.surfboard",
            "/api/v1/client/subscribe?flag=surfboard",
            Mode::RustBodyContains {
                content_type_hint: "text/plain",
                needles: vec!["[Proxy]", "[Proxy Group]"],
            },
        ),
        client_get(
            "client.subscribe.loon",
            "/api/v1/client/subscribe?flag=loon",
            Mode::RustContentType("text/plain"),
        ),
        client_get(
            "client.subscribe.shadowrocket",
            "/api/v1/client/subscribe?flag=shadowrocket",
            Mode::RustBodyContains {
                content_type_hint: "text/plain",
                needles: vec![],
            },
        ),
        client_get(
            "client.subscribe.shadowsocks",
            "/api/v1/client/subscribe?flag=shadowsocks",
            Mode::RustBodyContains {
                content_type_hint: "json",
                needles: vec!["\"version\"", "\"servers\""],
            },
        ),
        client_get(
            "client.subscribe.v2rayn",
            "/api/v1/client/subscribe?flag=v2rayn",
            Mode::RustContentType("text/plain"),
        ),
        client_get(
            "client.subscribe.v2rayng",
            "/api/v1/client/subscribe?flag=v2rayng",
            Mode::RustContentType("text/plain"),
        ),
        client_get(
            "client.subscribe.v2raytun",
            "/api/v1/client/subscribe?flag=v2raytun",
            Mode::RustContentType("text/plain"),
        ),
        client_get(
            "client.subscribe.passwall",
            "/api/v1/client/subscribe?flag=passwall",
            Mode::RustContentType("text/plain"),
        ),
        client_get(
            "client.subscribe.ssrplus",
            "/api/v1/client/subscribe?flag=ssrplus",
            Mode::RustContentType("text/plain"),
        ),
        client_get(
            "client.subscribe.sagernet",
            "/api/v1/client/subscribe?flag=sagernet",
            Mode::RustContentType("text/plain"),
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
            "user.order.check_missing",
            "/api/v1/user/order/check?trade_no=rust-contract-missing-order",
            true,
            Mode::ErrorMessage,
        ),
        post(
            "user.order.cancel_empty",
            "/api/v1/user/order/cancel",
            true,
            vec![("trade_no", "")],
            Mode::ErrorMessage,
        ),
        post(
            "user.coupon.empty",
            "/api/v1/user/coupon/check",
            true,
            vec![("code", ""), ("plan_id", "1")],
            Mode::ErrorMessage,
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
            config.admin_api_path("config/fetch"),
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.config.safe_path",
            config.admin_api_path("config/fetch?key=safe"),
            true,
            Mode::Selected(vec!["data.safe.secure_path"]),
        ),
        get(
            "admin.plan.fetch",
            config.admin_api_path("plan/fetch"),
            true,
            Mode::Exact,
        ),
        get(
            "admin.user.fetch",
            config.admin_api_path("user/fetch"),
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.order.fetch",
            config.admin_api_path("order/fetch"),
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.notice.fetch",
            config.admin_api_path("notice/fetch"),
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.ticket.fetch",
            config.admin_api_path("ticket/fetch"),
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.coupon.fetch",
            config.admin_api_path("coupon/fetch"),
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.giftcard.fetch",
            config.admin_api_path("giftcard/fetch"),
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.knowledge.fetch",
            config.admin_api_path("knowledge/fetch"),
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.payment.fetch",
            config.admin_api_path("payment/fetch"),
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.payment.methods",
            config.admin_api_path("payment/getPaymentMethods"),
            true,
            Mode::Shape(vec!["data"]),
        ),
        post(
            "admin.payment.form",
            config.admin_api_path("payment/getPaymentForm"),
            true,
            vec![("payment", "StripeCredit")],
            Mode::Shape(vec!["data"]),
        ),
        post(
            "admin.order.detail_missing",
            config.admin_api_path("order/detail"),
            true,
            vec![("id", "0")],
            Mode::ErrorMessage,
        ),
        get(
            "admin.server.groups",
            config.admin_api_path("server/group/fetch"),
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.server.routes",
            config.admin_api_path("server/route/fetch"),
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.server.nodes",
            config.admin_api_path("server/manage/getNodes"),
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.stat.summary",
            config.admin_api_path("stat/getStat"),
            true,
            Mode::RustMayImproveLegacy5xx,
        ),
        get(
            "admin.stat.override",
            config.admin_api_path("stat/getOverride"),
            true,
            Mode::StatusOnly,
        ),
        get(
            "admin.stat.ranking",
            config.admin_api_path("stat/getRanking"),
            true,
            Mode::RustMayImproveLegacy5xx,
        ),
        get(
            "admin.stat.records",
            config.admin_api_path("stat/getStatRecord"),
            true,
            Mode::RustShape(vec!["data", "total"]),
        ),
        get(
            "admin.system.status",
            config.admin_api_path("system/getSystemStatus"),
            true,
            Mode::Shape(vec!["data.schedule", "data.horizon"]),
        ),
        get(
            "admin.queue.stats",
            config.admin_api_path("system/getQueueStats"),
            true,
            Mode::Shape(vec!["data.status", "data.recentJobs"]),
        ),
        get(
            "admin.queue.workload",
            config.admin_api_path("system/getQueueWorkload"),
            true,
            Mode::Shape(vec!["data"]),
        ),
        get(
            "admin.queue.masters",
            config.admin_api_path("system/getQueueMasters"),
            true,
            Mode::RustShape(vec!["data"]),
        ),
        get(
            "admin.system.logs",
            config.admin_api_path("system/getSystemLog"),
            true,
            Mode::Shape(vec!["data", "total"]),
        ),
        get(
            "staff.plan.fetch",
            "/api/v1/staff/plan/fetch",
            true,
            Mode::StatusOnly,
        ),
        get(
            "staff.notice.fetch",
            "/api/v1/staff/notice/fetch",
            true,
            Mode::StatusOnly,
        ),
        get(
            "staff.admin_endpoint_blocked",
            "/api/v1/staff/config/fetch",
            true,
            Mode::StatusOnly,
        ),
    ]
}

fn get(name: &'static str, path: impl Into<String>, auth: bool, mode: Mode) -> Scenario {
    Scenario {
        name,
        method: Method::GET,
        path: path.into(),
        auth_header: auth,
        token_query: false,
        form: Vec::new(),
        mode,
    }
}

fn post(
    name: &'static str,
    path: impl Into<String>,
    auth: bool,
    form: Vec<(&'static str, &'static str)>,
    mode: Mode,
) -> Scenario {
    Scenario {
        name,
        method: Method::POST,
        path: path.into(),
        auth_header: auth,
        token_query: false,
        form: form
            .into_iter()
            .map(|(key, value)| (key, value.to_string()))
            .collect(),
        mode,
    }
}

fn client_get(name: &'static str, path: impl Into<String>, mode: Mode) -> Scenario {
    Scenario {
        name,
        method: Method::GET,
        path: path.into(),
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
    if let Mode::RustBodyContains {
        content_type_hint,
        needles,
    } = mode
    {
        return compare_rust_body_contains(name, rust, content_type_hint, needles);
    }
    if let Mode::RustShape(paths) = mode {
        return compare_rust_shape(name, rust, paths);
    }
    if let Mode::RustContentType(content_type_hint) = mode {
        return compare_rust_content_type(name, rust, content_type_hint);
    }
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
        Mode::RustBodyContains { .. } => unreachable!("handled before status comparison"),
        Mode::RustContentType(_) => unreachable!("handled before status comparison"),
        Mode::RustShape(_) => unreachable!("handled before status comparison"),
        Mode::ErrorMessage => compare_error_message(name, laravel, rust),
        Mode::StatusOnly => pass(name),
        Mode::RustMayImproveLegacy5xx => unreachable!("handled before status comparison"),
    }
}

fn compare_error_message(name: &str, laravel: &Snapshot, rust: &Snapshot) -> ContractResult {
    for (target, snapshot) in [("laravel", laravel), ("rust", rust)] {
        let has_message = snapshot
            .json
            .as_ref()
            .and_then(|json| json_at(json, "message"))
            .and_then(Value::as_str)
            .filter(|message| !message.trim().is_empty())
            .is_some();
        if !has_message {
            return fail(name, format!("{target} error response is missing message"));
        }
    }
    pass(name)
}

fn compare_rust_shape(name: &str, rust: &Snapshot, paths: &[&'static str]) -> ContractResult {
    if rust.status != StatusCode::OK.as_u16() {
        return fail(name, format!("rust status is {}", rust.status));
    }
    let Some(rust_json) = rust.json.as_ref() else {
        return fail(name, "rust response is not JSON");
    };
    for path in paths {
        if json_at(rust_json, path).is_none() {
            return fail(name, format!("required rust path `{path}` missing"));
        }
    }
    pass(name)
}

fn compare_rust_content_type(
    name: &str,
    rust: &Snapshot,
    content_type_hint: &str,
) -> ContractResult {
    if rust.status != StatusCode::OK.as_u16() {
        return fail(name, format!("rust status is {}", rust.status));
    }
    if !rust
        .content_type
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .contains(content_type_hint)
    {
        return fail(
            name,
            format!(
                "rust content-type `{}` does not contain `{content_type_hint}`",
                rust.content_type.as_deref().unwrap_or("<missing>")
            ),
        );
    }
    pass(name)
}

fn compare_rust_body_contains(
    name: &str,
    rust: &Snapshot,
    content_type_hint: &str,
    needles: &[&'static str],
) -> ContractResult {
    if rust.status != StatusCode::OK.as_u16() {
        return fail(name, format!("rust status is {}", rust.status));
    }
    if !rust
        .content_type
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .contains(content_type_hint)
    {
        return fail(
            name,
            format!(
                "rust content-type `{}` does not contain `{content_type_hint}`",
                rust.content_type.as_deref().unwrap_or("<missing>")
            ),
        );
    }
    if rust.body.trim().is_empty() {
        return fail(name, "rust response body is empty");
    }
    for needle in needles {
        if !rust.body.contains(needle) {
            return fail(
                name,
                format!("rust response body does not contain `{needle}`"),
            );
        }
    }
    pass(name)
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

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use sqlx::{AssertSqlSafe, Executor, PgPool, SqlSafeStr};
use syn::{
    Expr, ExprCall, ExprLit, ExprPath, ItemConst, ItemStatic, Lit,
    visit::{self, Visit},
};

const DEFAULT_RUST_ROOT: &str = "/src/backend/rust";

#[derive(Debug)]
struct StaticQuery {
    source: String,
    sql: String,
}

#[derive(Default)]
struct SourceInventory {
    static_queries: Vec<StaticQuery>,
    dynamic_sqlx: BTreeMap<String, usize>,
    query_builders: BTreeMap<String, usize>,
}

/// Prepare every statically recoverable runtime query against the freshly
/// migrated PostgreSQL schema. SQLx's dynamic APIs remain necessary for safe
/// table dispatch and variable-size batches, so those call sites are tracked
/// by an explicit per-file inventory: adding one without updating this audit
/// and its integration scenario fails CI.
pub async fn run(pool: &PgPool) -> Result<()> {
    let inventory = source_inventory()?;
    assert_dynamic_inventory(&inventory)?;
    assert_dynamic_tables(pool).await?;

    let mut connection = pool.acquire().await?;
    let mut prepared = 0_usize;
    let mut prepare_failures = Vec::new();
    for query in inventory.static_queries {
        let result = connection
            .prepare(AssertSqlSafe(query.sql.clone()).into_sql_str())
            .await;
        match result {
            Ok(_) => prepared += 1,
            Err(error) => prepare_failures.push(format!(
                "{}: {}: {error}",
                query.source,
                compact_sql(&query.sql)
            )),
        }
    }
    ensure!(
        prepare_failures.is_empty(),
        "PostgreSQL failed to prepare {} static native runtime queries:\n{}",
        prepare_failures.len(),
        prepare_failures.join("\n")
    );
    ensure!(
        prepared >= 250,
        "only {prepared} static runtime queries were discovered"
    );
    println!(
        "SQL schema prepare inventory: {prepared} static queries prepared; {} dynamic sqlx calls and {} QueryBuilder sites explicitly inventoried; native runtime has no MySQL SQL exclusions.",
        inventory.dynamic_sqlx.values().sum::<usize>(),
        inventory.query_builders.values().sum::<usize>(),
    );
    Ok(())
}

async fn assert_dynamic_tables(pool: &PgPool) -> Result<()> {
    const TABLES: &[&str] = &[
        "plan",
        "payment_method",
        "notice",
        "knowledge",
        "coupon",
        "gift_card",
        "server_group",
        "server_route",
        "users",
        "server_shadowsocks",
        "server_vmess",
        "server_trojan",
        "server_tuic",
        "server_vless",
        "server_hysteria",
        "server_anytls",
        "server_v2node",
        "mail_outbox",
        "mail_outbox_batch",
        "mail_log",
        "analytics_delivery_batch",
        "analytics_outbox",
        "server_traffic_report",
        "server_traffic_report_item",
    ];
    for table in TABLES {
        let id_columns: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM information_schema.columns
            WHERE table_schema = current_schema() AND table_name = $1
              AND column_name = CASE
                    WHEN table_name = 'mail_outbox_batch' THEN 'batch_key'
                    WHEN table_name IN (
                        'server_traffic_report', 'server_traffic_report_item'
                    ) THEN 'report_key'
                    WHEN table_name = 'analytics_delivery_batch' THEN 'batch_id'
                    WHEN table_name = 'analytics_outbox' THEN 'outbox_id'
                    ELSE 'id'
                  END
            "#,
        )
        .bind(table)
        .fetch_one(pool)
        .await?;
        ensure!(
            id_columns == 1,
            "dynamic SQL table {table} or its identity column is missing"
        );
    }
    Ok(())
}

pub fn audit_dynamic_inventory() -> Result<()> {
    let inventory = source_inventory()?;
    assert_dynamic_inventory(&inventory)
}

fn source_inventory() -> Result<SourceInventory> {
    let root = PathBuf::from(
        env::var("RUST_INTEGRATION_RUST_ROOT").unwrap_or_else(|_| DEFAULT_RUST_ROOT.to_string()),
    );
    inventory(&root)
}

fn inventory(root: &Path) -> Result<SourceInventory> {
    let mut files = Vec::new();
    // Contract code is the gate itself rather than a production runtime. Scan
    // every crate that can issue database traffic in API or worker processes.
    // `provision` is intentionally absent: its explicitly named legacy MySQL
    // source adapter is not native runtime SQL. Analytics is included because
    // API/worker processes call its PostgreSQL outbox directly.
    for crate_name in ["analytics", "api", "db", "domain", "workers"] {
        let source = root.join("crates").join(crate_name).join("src");
        ensure!(
            source.is_dir(),
            "Rust source root is unavailable: {}",
            source.display()
        );
        collect_rust_files(&source, &mut files)?;
    }
    files.sort();

    let mut inventory = SourceInventory::default();
    for path in files {
        let source = fs::read_to_string(&path)
            .with_context(|| format!("read Rust SQL source {}", path.display()))?;
        let syntax = syn::parse_file(&source)
            .with_context(|| format!("parse Rust SQL source {}", path.display()))?;
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        let mut constants = ConstantCollector::default();
        constants.visit_file(&syntax);
        let mut calls = SqlCallCollector {
            source: relative.clone(),
            constants: &constants.values,
            static_queries: Vec::new(),
            indirect_queries: Vec::new(),
            builder_seeds: HashSet::new(),
            dynamic_sqlx: 0,
            query_builders: 0,
        };
        calls.visit_file(&syntax);
        for (name, sql) in &constants.values {
            if name.ends_with("_SQL") && looks_like_sql(sql) && !calls.builder_seeds.contains(sql) {
                calls.indirect_queries.push(StaticQuery {
                    source: relative.clone(),
                    sql: sql.clone(),
                });
            }
        }
        inventory.static_queries.extend(calls.static_queries);
        inventory.static_queries.extend(calls.indirect_queries);
        if calls.dynamic_sqlx > 0 {
            inventory
                .dynamic_sqlx
                .insert(relative.clone(), calls.dynamic_sqlx);
        }
        if calls.query_builders > 0 {
            inventory
                .query_builders
                .insert(relative, calls.query_builders);
        }
    }
    Ok(inventory)
}

#[derive(Default)]
struct ConstantCollector {
    values: HashMap<String, String>,
}

impl<'ast> Visit<'ast> for ConstantCollector {
    fn visit_item_const(&mut self, node: &'ast ItemConst) {
        if let Some(value) = literal_string(&node.expr) {
            self.values.insert(node.ident.to_string(), value);
        }
        visit::visit_item_const(self, node);
    }

    fn visit_item_static(&mut self, node: &'ast ItemStatic) {
        if let Some(value) = literal_string(&node.expr) {
            self.values.insert(node.ident.to_string(), value);
        }
        visit::visit_item_static(self, node);
    }
}

struct SqlCallCollector<'a> {
    source: String,
    constants: &'a HashMap<String, String>,
    static_queries: Vec<StaticQuery>,
    indirect_queries: Vec<StaticQuery>,
    builder_seeds: HashSet<String>,
    dynamic_sqlx: usize,
    query_builders: usize,
}

impl<'ast> Visit<'ast> for SqlCallCollector<'_> {
    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if let Expr::Path(function) = &*node.func {
            let segments = function
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>();
            if segments.last().is_some_and(|name| name == "new")
                && segments.iter().any(|name| name == "QueryBuilder")
            {
                self.query_builders += 1;
                if let Some(seed) = node
                    .args
                    .first()
                    .and_then(|argument| self.resolve_sql(argument))
                {
                    self.builder_seeds.insert(seed);
                }
            }
            if segments.iter().any(|name| name == "sqlx")
                && segments.last().is_some_and(|name| {
                    matches!(name.as_str(), "query" | "query_as" | "query_scalar")
                })
            {
                match node
                    .args
                    .first()
                    .and_then(|argument| self.resolve_sql(argument))
                {
                    Some(sql) => self.static_queries.push(StaticQuery {
                        source: self.source.clone(),
                        sql,
                    }),
                    None => self.dynamic_sqlx += 1,
                }
            }
            let function_name = segments.last().map(String::as_str).unwrap_or_default();
            let indirect_argument = if function_name == "Some" {
                node.args.first()
            } else if function_name.starts_with("fetch_json_") {
                node.args.iter().nth(1)
            } else {
                None
            };
            if let Some(sql) = indirect_argument
                .and_then(|argument| self.resolve_sql(argument))
                .filter(|sql| looks_like_sql(sql))
            {
                self.indirect_queries.push(StaticQuery {
                    source: self.source.clone(),
                    sql,
                });
            }
        }
        visit::visit_expr_call(self, node);
    }
}

impl SqlCallCollector<'_> {
    fn resolve_sql(&self, expression: &Expr) -> Option<String> {
        literal_string(expression).or_else(|| {
            let Expr::Path(ExprPath { path, .. }) = expression else {
                return None;
            };
            path.get_ident()
                .and_then(|ident| self.constants.get(&ident.to_string()))
                .cloned()
        })
    }
}

fn literal_string(expression: &Expr) -> Option<String> {
    let Expr::Lit(ExprLit {
        lit: Lit::Str(value),
        ..
    }) = expression
    else {
        return None;
    };
    Some(value.value())
}

fn assert_dynamic_inventory(inventory: &SourceInventory) -> Result<()> {
    let expected_builders = site_counts(QUERY_BUILDER_SITES);
    let expected_dynamic_sqlx = site_counts(DYNAMIC_SQLX_SITES);
    if inventory.query_builders != expected_builders
        || inventory.dynamic_sqlx != expected_dynamic_sqlx
    {
        bail!(
            "dynamic SQL inventory changed; audit the new/removed construction and update the explicit inventory. observed sqlx={:?}; observed QueryBuilder={:?}",
            inventory.dynamic_sqlx,
            inventory.query_builders
        );
    }
    Ok(())
}

struct DynamicSite {
    source: &'static str,
    count: usize,
    coverage: &'static str,
}

const DYNAMIC_SQLX_SITES: &[DynamicSite] = &[
    DynamicSite {
        source: "crates/analytics/src/admission.rs",
        count: 2,
        coverage: "fixed policy/state SELECT variants plus PostgreSQL admission integration tests",
    },
    DynamicSite {
        source: "crates/api/src/server_api/repository.rs",
        count: 1,
        coverage: "all node variants are collected as indirect static SQL and server API contracts",
    },
    DynamicSite {
        source: "crates/db/src/pool.rs",
        count: 2,
        coverage: "connection-init statement/lock timeout SETs interpolate validated integer \
                   milliseconds (SET takes no binds); pinned by the timeout parsing unit tests",
    },
    DynamicSite {
        source: "crates/domain/src/admin/codes.rs",
        count: 2,
        coverage: "§7.2 whitelist-sorted coupon/gift-card list SELECTs, pinned by the golden \
                   responses and production-invariant projections",
    },
    DynamicSite {
        source: "crates/domain/src/admin/repository.rs",
        count: 3,
        coverage: "safe-table allowlist and admin interaction parity",
    },
    DynamicSite {
        source: "crates/domain/src/admin/servers.rs",
        count: 8,
        coverage: "server-table allowlist queries plus the fixed plan/user dependency-lock SQL \
                   variants, W13 node CRUD, and the wire-shape unit tests",
    },
    DynamicSite {
        source: "crates/domain/src/admin/support/common.rs",
        count: 3,
        coverage: "literal caller SQL is collected and prepared by this AST audit",
    },
    DynamicSite {
        source: "crates/domain/src/operator_config.rs",
        count: 1,
        coverage: "fixed API/worker acknowledgement variants, unit tests, and PostgreSQL 18 role-isolation tests",
    },
    DynamicSite {
        source: "crates/domain/src/order/lifecycle.rs",
        count: 1,
        coverage: "variable ID batch and payment lifecycle integration",
    },
    DynamicSite {
        source: "crates/workers/src/outbox.rs",
        count: 1,
        coverage: "constant retention SQL is prepared and an isolated live worker runs cleanup",
    },
    DynamicSite {
        source: "crates/workers/src/reset.rs",
        count: 1,
        coverage: "fixed retention SQL variants and bounded reset worker tests",
    },
];

const QUERY_BUILDER_SITES: &[DynamicSite] = &[
    DynamicSite {
        source: "crates/analytics/src/outbox.rs",
        count: 1,
        coverage: "2,001-row enqueue, conflict, and PostgreSQL-to-ClickHouse round-trip tests",
    },
    DynamicSite {
        source: "crates/api/src/server_api/config.rs",
        count: 1,
        coverage: "server API contracts",
    },
    DynamicSite {
        source: "crates/api/src/server_api/repository.rs",
        count: 1,
        coverage: "server-node enumeration and API contracts",
    },
    DynamicSite {
        source: "crates/api/src/server_api/traffic.rs",
        count: 3,
        coverage: "traffic epoch production invariant",
    },
    DynamicSite {
        source: "crates/db/src/order.rs",
        count: 1,
        coverage: "order lifecycle and late-payment invariant",
    },
    DynamicSite {
        source: "crates/db/src/plan.rs",
        count: 6,
        coverage: "fixed normalized-price projections and the bind-only plan-ID batch, pinned by \
                   commerce interaction contracts and plan repository integration tests",
    },
    DynamicSite {
        source: "crates/domain/src/admin/codes.rs",
        count: 3,
        coverage: "coupon/gift-card batch and single code INSERTs, pinned by the generated-code \
                   unit tests and admin content interaction parity",
    },
    DynamicSite {
        source: "crates/domain/src/admin/commerce/orders.rs",
        count: 3,
        coverage: "admin commerce interaction parity",
    },
    DynamicSite {
        source: "crates/domain/src/admin/commerce/payments.rs",
        count: 1,
        coverage: "admin commerce interaction parity",
    },
    DynamicSite {
        source: "crates/domain/src/admin/commerce/plans.rs",
        count: 1,
        coverage: "admin commerce interaction parity",
    },
    DynamicSite {
        source: "crates/domain/src/admin/repository.rs",
        count: 1,
        coverage: "safe-table allowlisted §4.4 PATCH executor and admin interaction parity",
    },
    DynamicSite {
        source: "crates/domain/src/admin/servers.rs",
        count: 4,
        coverage: "server-table schema probes and admin server parity",
    },
    DynamicSite {
        source: "crates/domain/src/admin/statistics.rs",
        count: 5,
        coverage: "admin dashboard/list interaction parity plus the audit-trail list production invariant",
    },
    DynamicSite {
        source: "crates/domain/src/admin/support/filter_dsl.rs",
        count: 1,
        coverage: "§7 DSL bind-only SQL-shape unit tests (whitelisted exprs, bound values) plus the system/logs production-invariant projection",
    },
    DynamicSite {
        source: "crates/domain/src/admin/support/filters.rs",
        count: 1,
        coverage: "§7 GET users whitelist bind-only SQL-shape unit tests (every user column resolves and binds its expression)",
    },
    DynamicSite {
        source: "crates/domain/src/admin/support/values.rs",
        count: 2,
        coverage: "exact PostgreSQL integer-cast builder unit tests and caller interaction parity",
    },
    DynamicSite {
        source: "crates/domain/src/admin/tickets.rs",
        count: 2,
        coverage: "admin ticket interaction parity",
    },
    DynamicSite {
        source: "crates/domain/src/admin/users.rs",
        count: 18,
        coverage: "admin user filter/bulk interaction parity",
    },
    DynamicSite {
        source: "crates/domain/src/mail/outbox.rs",
        count: 5,
        coverage: "mail outbox tests and isolated live worker",
    },
    DynamicSite {
        source: "crates/domain/src/order/lifecycle.rs",
        count: 1,
        coverage: "order lifecycle and late-payment invariant",
    },
    DynamicSite {
        source: "crates/workers/src/outbox.rs",
        count: 1,
        coverage: "isolated live worker and reconciliation",
    },
    DynamicSite {
        source: "crates/workers/src/reset.rs",
        count: 1,
        coverage: "traffic epoch/reset integration",
    },
    DynamicSite {
        source: "crates/workers/src/traffic.rs",
        count: 2,
        coverage: "delayed-report traffic integration",
    },
];

fn site_counts(sites: &[DynamicSite]) -> BTreeMap<String, usize> {
    debug_assert!(sites.iter().all(|site| !site.coverage.is_empty()));
    sites
        .iter()
        .map(|site| (site.source.to_string(), site.count))
        .collect()
}

fn looks_like_sql(value: &str) -> bool {
    let start = value.trim_start().to_ascii_uppercase();
    ["SELECT ", "INSERT ", "UPDATE ", "DELETE ", "WITH "]
        .iter()
        .any(|prefix| start.starts_with(prefix))
}

fn collect_rust_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(directory)
        .with_context(|| format!("read Rust source directory {}", directory.display()))?
    {
        let path = entry?.path();
        if path.is_dir() {
            collect_rust_files(&path, files)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    Ok(())
}

fn compact_sql(sql: &str) -> String {
    let compact = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.len() <= 240 {
        compact
    } else {
        format!("{}…", &compact[..240])
    }
}

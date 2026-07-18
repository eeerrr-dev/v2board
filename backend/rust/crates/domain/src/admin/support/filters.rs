use std::sync::LazyLock;

use super::*;

// ---------------------------------------------------------------------------
// Admin user filtering / sorting.
// Ports UserController::filter (laravel .../Admin/UserController.php:36-62) and
// the sort/sort_type parsing in fetch (:66-69). All dynamic SQL is guarded by
// column and operator whitelists to stay injection-safe.
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub(in super::super) enum UserFilterClause {
    Compare {
        column: &'static str,
        op: &'static str,
        value: FilterBind,
    },
    IsNull {
        column: &'static str,
    },
}

#[derive(Debug)]
pub(in super::super) enum FilterBind {
    Int(i64),
    Text(String),
}

/// Whitelisted users columns usable in a filter[] key or a sort. Guards the
/// dynamically-built WHERE/ORDER BY clauses against SQL injection.
pub(in super::super) fn user_column(key: &str) -> Option<&'static str> {
    const COLUMNS: &[&str] = &[
        "id",
        "email",
        "telegram_id",
        "balance",
        "discount",
        "commission_type",
        "commission_rate",
        "commission_balance",
        "t",
        "u",
        "d",
        "transfer_enable",
        "device_limit",
        "banned",
        "is_admin",
        "is_staff",
        "last_login_at",
        "uuid",
        "group_id",
        "plan_id",
        "speed_limit",
        "token",
        "expired_at",
        "remarks",
        "invite_user_id",
        "created_at",
        "updated_at",
    ];
    COLUMNS.iter().copied().find(|column| *column == key)
}

pub(in super::super) fn user_filter_operator(condition: &str) -> Option<&'static str> {
    match condition {
        "=" => Some("="),
        ">" => Some(">"),
        "<" => Some("<"),
        ">=" => Some(">="),
        "<=" => Some("<="),
        "<>" | "!=" => Some("<>"),
        "like" | "LIKE" => Some("like"),
        _ => None,
    }
}

/// Returns the validated `(ORDER BY expression, direction)`. Mirrors fetch():
/// sort defaults to created_at, sort_type is DESC unless exactly "ASC".
pub(in super::super) fn user_sort(params: &HashMap<String, String>) -> (String, &'static str) {
    let direction = match params.get("sort_type").map(String::as_str) {
        Some("ASC") => "ASC",
        _ => "DESC",
    };
    let sort_expr = match params
        .get("sort")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        Some("total_used") => {
            "(CAST(u.u AS NUMERIC(65,0)) + CAST(u.d AS NUMERIC(65,0)))".to_string()
        }
        Some(sort) => match user_column(sort) {
            Some(column) => format!("u.{column}"),
            None => "u.created_at".to_string(),
        },
        None => "u.created_at".to_string(),
    };
    (sort_expr, direction)
}

pub(in super::super) fn push_user_where(
    builder: &mut QueryBuilder<Postgres>,
    clauses: &[UserFilterClause],
) {
    for clause in clauses {
        builder.push(" AND ");
        match clause {
            UserFilterClause::Compare { column, op, value } => {
                if *op == "like" {
                    builder.push(format!("u.{column}::text ILIKE "));
                } else if *column == "email" && matches!(*op, "=" | "<>") {
                    builder.push(format!("lower(btrim(u.email)) {op} lower(btrim("));
                } else {
                    builder.push(format!("u.{column} {op} "));
                }
                match value {
                    FilterBind::Int(value) => {
                        builder.push_bind(*value);
                    }
                    FilterBind::Text(value) => {
                        builder.push_bind(value.clone());
                    }
                }
                if *column == "email" && *op != "like" && matches!(*op, "=" | "<>") {
                    builder.push("))");
                }
            }
            UserFilterClause::IsNull { column } => {
                builder.push(format!("u.{column} IS NULL"));
            }
        }
    }
}

/// §7.1 filter whitelist for `GET orders` (docs/api-dialect.md §7.1: "the
/// guarded `order_column` list") on the §7 DSL. The columns are the legacy
/// `order_column` set; expressions carry the `o.` alias the list projection
/// uses, with `"type"` quoted as before.
pub(in super::super) const ORDER_FILTER_COLUMNS: &[filter_dsl::FilterColumn] = &[
    column("id", "o.id", filter_dsl::ColumnKind::Integer),
    column(
        "invite_user_id",
        "o.invite_user_id",
        filter_dsl::ColumnKind::Integer,
    ),
    column("user_id", "o.user_id", filter_dsl::ColumnKind::Integer),
    column("plan_id", "o.plan_id", filter_dsl::ColumnKind::Integer),
    column("coupon_id", "o.coupon_id", filter_dsl::ColumnKind::Integer),
    column(
        "payment_id",
        "o.payment_id",
        filter_dsl::ColumnKind::Integer,
    ),
    column("type", "o.\"type\"", filter_dsl::ColumnKind::Integer),
    column("period", "o.period", filter_dsl::ColumnKind::Text),
    column("trade_no", "o.trade_no", filter_dsl::ColumnKind::Text),
    column("callback_no", "o.callback_no", filter_dsl::ColumnKind::Text),
    column(
        "total_amount",
        "o.total_amount",
        filter_dsl::ColumnKind::Integer,
    ),
    column(
        "handling_amount",
        "o.handling_amount",
        filter_dsl::ColumnKind::Integer,
    ),
    column(
        "discount_amount",
        "o.discount_amount",
        filter_dsl::ColumnKind::Integer,
    ),
    column(
        "surplus_amount",
        "o.surplus_amount",
        filter_dsl::ColumnKind::Integer,
    ),
    column(
        "refund_amount",
        "o.refund_amount",
        filter_dsl::ColumnKind::Integer,
    ),
    column(
        "balance_amount",
        "o.balance_amount",
        filter_dsl::ColumnKind::Integer,
    ),
    column("status", "o.status", filter_dsl::ColumnKind::Integer),
    column(
        "commission_status",
        "o.commission_status",
        filter_dsl::ColumnKind::Integer,
    ),
    column(
        "commission_balance",
        "o.commission_balance",
        filter_dsl::ColumnKind::Integer,
    ),
    column(
        "actual_commission_balance",
        "o.actual_commission_balance",
        filter_dsl::ColumnKind::Integer,
    ),
    column("paid_at", "o.paid_at", filter_dsl::ColumnKind::Timestamp),
    column(
        "created_at",
        "o.created_at",
        filter_dsl::ColumnKind::Timestamp,
    ),
    column(
        "updated_at",
        "o.updated_at",
        filter_dsl::ColumnKind::Timestamp,
    ),
];

const fn column(
    field: &'static str,
    expr: &'static str,
    kind: filter_dsl::ColumnKind,
) -> filter_dsl::FilterColumn {
    filter_dsl::FilterColumn { field, expr, kind }
}

/// §7.2 sort whitelist for `GET orders`: the same per-endpoint field list as
/// the filters (no computed additions), including the `created_at` default.
pub(in super::super) static ORDER_SORT_COLUMNS: LazyLock<Vec<filter_dsl::SortColumn>> =
    LazyLock::new(|| {
        ORDER_FILTER_COLUMNS
            .iter()
            .map(|column| filter_dsl::SortColumn {
                field: column.field,
                expr: column.expr,
            })
            .collect()
    });

pub(in super::super) fn user_column_is_numeric(column: &str) -> bool {
    !matches!(column, "email" | "uuid" | "token" | "remarks")
}

/// Reconstructs `filter[<i>][<field>]` request keys into per-index maps of raw
/// string values, ordered by index. Kept as raw strings (not `json_scalar`d) so
/// the literal `plan_id == 'null'` sentinel survives.
pub(in super::super) fn collect_filter_entries(
    params: &HashMap<String, String>,
) -> Vec<BTreeMap<String, String>> {
    let mut entries: BTreeMap<usize, BTreeMap<String, String>> = BTreeMap::new();
    for (key, value) in params {
        let Some(rest) = key.strip_prefix("filter[") else {
            continue;
        };
        let Some((index, rest)) = rest.split_once(']') else {
            continue;
        };
        let Ok(index) = index.parse::<usize>() else {
            continue;
        };
        let Some(field) = rest
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        else {
            continue;
        };
        entries
            .entry(index)
            .or_default()
            .insert(field.to_string(), value.clone());
    }
    entries.into_values().collect()
}

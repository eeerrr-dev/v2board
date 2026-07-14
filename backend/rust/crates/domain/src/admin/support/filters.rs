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

/// A single WHERE comparison for an order filter[]. Values are bound as strings,
/// matching Laravel's default PDO parameter binding.
#[derive(Debug)]
pub(in super::super) enum OrderFilterClause {
    Compare {
        column: &'static str,
        op: &'static str,
        value: FilterBind,
    },
}

/// Whitelisted orders columns usable in a filter[] key. Guards the dynamically
/// built WHERE clause (OrderController::filter trusts the raw request key).
pub(in super::super) fn order_column(key: &str) -> Option<&'static str> {
    const COLUMNS: &[&str] = &[
        "id",
        "invite_user_id",
        "user_id",
        "plan_id",
        "coupon_id",
        "payment_id",
        "type",
        "period",
        "trade_no",
        "callback_no",
        "total_amount",
        "handling_amount",
        "discount_amount",
        "surplus_amount",
        "refund_amount",
        "balance_amount",
        "status",
        "commission_status",
        "commission_balance",
        "actual_commission_balance",
        "paid_at",
        "created_at",
        "updated_at",
    ];
    COLUMNS.iter().copied().find(|column| *column == key)
}

/// Applies the is_commission scope and filter[] clauses to an order builder whose
/// order table is aliased `o`. Ports OrderController::fetch (:58-63) + filter().
pub(in super::super) fn push_order_where(
    builder: &mut QueryBuilder<Postgres>,
    is_commission: bool,
    clauses: &[OrderFilterClause],
) {
    if is_commission {
        builder.push(
            " AND o.invite_user_id IS NOT NULL AND o.status NOT IN (0, 2) AND o.commission_balance > 0",
        );
    }
    for clause in clauses {
        let OrderFilterClause::Compare { column, op, value } = clause;
        if *op == "like" {
            builder.push(format!(" AND o.\"{column}\"::text ILIKE "));
        } else {
            builder.push(format!(" AND o.\"{column}\" {op} "));
        }
        match value {
            FilterBind::Int(value) => {
                builder.push_bind(*value);
            }
            FilterBind::Text(value) => {
                builder.push_bind(value.clone());
            }
        }
    }
}

pub(in super::super) fn user_column_is_numeric(column: &str) -> bool {
    !matches!(column, "email" | "uuid" | "token" | "remarks")
}

pub(in super::super) fn order_column_is_numeric(column: &str) -> bool {
    !matches!(column, "period" | "trade_no" | "callback_no")
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

/// Conditions accepted by FilterScope::scopeSetFilterAllowKeys.
pub(in super::super) const LOG_FILTER_CONDITIONS: &[&str] =
    &["in", "is", "not", "like", "lt", "gt"];

/// Applies system-log filter[] entries to a builder. `key` is validated to
/// `level`, so the column is fixed. Ports FilterScope's condition mapping
/// (App\Scope\FilterScope): in/is → equality, not → <>, gt/lt → >/<, like → %v%.
pub(in super::super) fn push_log_filters(
    builder: &mut QueryBuilder<Postgres>,
    entries: &[BTreeMap<String, String>],
) {
    for entry in entries {
        let condition = entry
            .get("condition")
            .map(String::as_str)
            .unwrap_or_default();
        let value = entry.get("value").cloned().unwrap_or_default();
        match condition {
            "in" | "is" => {
                builder.push(" AND level = ");
                builder.push_bind(value);
            }
            "not" => {
                builder.push(" AND level <> ");
                builder.push_bind(value);
            }
            "gt" => {
                builder.push(" AND level > ");
                builder.push_bind(value);
            }
            "lt" => {
                builder.push(" AND level < ");
                builder.push_bind(value);
            }
            "like" => {
                builder.push(" AND level LIKE ");
                builder.push_bind(format!("%{value}%"));
            }
            _ => {}
        }
    }
}

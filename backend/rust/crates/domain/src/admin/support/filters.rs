use std::sync::LazyLock;

use super::*;

// ---------------------------------------------------------------------------
// Admin user filtering / sorting.
// Ports UserController::filter (laravel .../Admin/UserController.php:36-62) and
// the sort/sort_type parsing in fetch (:66-69). All dynamic SQL is guarded by
// column and operator whitelists to stay injection-safe.
// ---------------------------------------------------------------------------

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

/// §7.1 filter whitelist for `GET users` (docs/api-dialect.md §7.1: "the
/// guarded `user_column` list"). It enumerates exactly the columns of the
/// legacy `user_column` guard (kept for the still-legacy staff path). The 0/1
/// flag columns (`banned`, `is_admin`, `is_staff`) resolve to boolean
/// predicates so the §7.1 JSON-boolean value type binds correctly against the
/// SMALLINT storage; `t` stays filterable (it is in the whitelist) even though
/// the W12 response projection drops it.
pub(in super::super) const USER_FILTER_COLUMNS: &[filter_dsl::FilterColumn] = &[
    column("id", "u.id", filter_dsl::ColumnKind::Integer),
    column("email", "u.email", filter_dsl::ColumnKind::Email),
    column(
        "telegram_id",
        "u.telegram_id",
        filter_dsl::ColumnKind::Integer,
    ),
    column("balance", "u.balance", filter_dsl::ColumnKind::Integer),
    column("discount", "u.discount", filter_dsl::ColumnKind::Integer),
    column(
        "commission_type",
        "u.commission_type",
        filter_dsl::ColumnKind::Integer,
    ),
    column(
        "commission_rate",
        "u.commission_rate",
        filter_dsl::ColumnKind::Integer,
    ),
    column(
        "commission_balance",
        "u.commission_balance",
        filter_dsl::ColumnKind::Integer,
    ),
    column("t", "u.t", filter_dsl::ColumnKind::Integer),
    column("u", "u.u", filter_dsl::ColumnKind::Integer),
    column("d", "u.d", filter_dsl::ColumnKind::Integer),
    column(
        "transfer_enable",
        "u.transfer_enable",
        filter_dsl::ColumnKind::Integer,
    ),
    column(
        "device_limit",
        "u.device_limit",
        filter_dsl::ColumnKind::Integer,
    ),
    column("banned", "(u.banned <> 0)", filter_dsl::ColumnKind::Boolean),
    column(
        "is_admin",
        "(u.is_admin <> 0)",
        filter_dsl::ColumnKind::Boolean,
    ),
    column(
        "is_staff",
        "(u.is_staff <> 0)",
        filter_dsl::ColumnKind::Boolean,
    ),
    column(
        "last_login_at",
        "u.last_login_at",
        filter_dsl::ColumnKind::Timestamp,
    ),
    column("uuid", "u.uuid", filter_dsl::ColumnKind::Text),
    column("group_id", "u.group_id", filter_dsl::ColumnKind::Integer),
    column("plan_id", "u.plan_id", filter_dsl::ColumnKind::Integer),
    column(
        "speed_limit",
        "u.speed_limit",
        filter_dsl::ColumnKind::Integer,
    ),
    column("token", "u.token", filter_dsl::ColumnKind::Text),
    column(
        "expired_at",
        "u.expired_at",
        filter_dsl::ColumnKind::Timestamp,
    ),
    column("remarks", "u.remarks", filter_dsl::ColumnKind::Text),
    column(
        "invite_user_id",
        "u.invite_user_id",
        filter_dsl::ColumnKind::Integer,
    ),
    column(
        "created_at",
        "u.created_at",
        filter_dsl::ColumnKind::Timestamp,
    ),
    column(
        "updated_at",
        "u.updated_at",
        filter_dsl::ColumnKind::Timestamp,
    ),
];

const fn sort_column(field: &'static str, expr: &'static str) -> filter_dsl::SortColumn {
    filter_dsl::SortColumn { field, expr }
}

/// §7.2 sort whitelist for `GET users`: the same per-endpoint field list as
/// the filters (raw column expressions so ordering is numeric/lexical, not the
/// filter's boolean cast) plus the computed `total_used = u + d` (§7.2) and the
/// `created_at` default.
pub(in super::super) const USER_SORT_COLUMNS: &[filter_dsl::SortColumn] = &[
    sort_column("id", "u.id"),
    sort_column("email", "u.email"),
    sort_column("telegram_id", "u.telegram_id"),
    sort_column("balance", "u.balance"),
    sort_column("discount", "u.discount"),
    sort_column("commission_type", "u.commission_type"),
    sort_column("commission_rate", "u.commission_rate"),
    sort_column("commission_balance", "u.commission_balance"),
    sort_column("t", "u.t"),
    sort_column("u", "u.u"),
    sort_column("d", "u.d"),
    sort_column("transfer_enable", "u.transfer_enable"),
    sort_column("device_limit", "u.device_limit"),
    sort_column("banned", "u.banned"),
    sort_column("is_admin", "u.is_admin"),
    sort_column("is_staff", "u.is_staff"),
    sort_column("last_login_at", "u.last_login_at"),
    sort_column("uuid", "u.uuid"),
    sort_column("group_id", "u.group_id"),
    sort_column("plan_id", "u.plan_id"),
    sort_column("speed_limit", "u.speed_limit"),
    sort_column("token", "u.token"),
    sort_column("expired_at", "u.expired_at"),
    sort_column("remarks", "u.remarks"),
    sort_column("invite_user_id", "u.invite_user_id"),
    sort_column("created_at", "u.created_at"),
    sort_column("updated_at", "u.updated_at"),
    sort_column(
        "total_used",
        "(CAST(u.u AS NUMERIC(65,0)) + CAST(u.d AS NUMERIC(65,0)))",
    ),
];

#[cfg(test)]
mod user_dsl_whitelist_tests {
    use super::*;

    /// Resolves one §7 DSL clause against the `GET users` whitelist and returns
    /// the full SQL the builder would emit for it (values bound, never
    /// interpolated).
    fn sql_for(field: &str, op: &str, value: serde_json::Value) -> String {
        let raw = serde_json::json!([{ "field": field, "op": op, "value": value }]).to_string();
        let clauses = filter_dsl::parse_filter_param(&raw).expect("parse user filter clause");
        let filters = filter_dsl::resolve_filters(&clauses, USER_FILTER_COLUMNS)
            .expect("resolve user filter");
        let mut builder = QueryBuilder::<Postgres>::new("SELECT 1 FROM users u WHERE 1 = 1");
        filter_dsl::push_filter_where(&mut builder, &filters);
        builder.sql().as_str().to_string()
    }

    #[test]
    fn every_whitelisted_user_column_resolves_and_binds_its_expression() {
        use filter_dsl::ColumnKind;
        for column in USER_FILTER_COLUMNS {
            let value = match column.kind {
                ColumnKind::Integer => serde_json::json!(1),
                ColumnKind::Boolean => serde_json::json!(true),
                ColumnKind::Timestamp => serde_json::json!("2023-11-14T22:13:20Z"),
                ColumnKind::Text | ColumnKind::Email => serde_json::json!("x"),
            };
            let sql = sql_for(column.field, "eq", value);
            assert!(
                sql.contains(column.expr),
                "field {} must resolve to its whitelisted expression {}",
                column.field,
                column.expr
            );
            assert!(
                sql.contains("$1"),
                "field {} must bind its value rather than interpolate it",
                column.field
            );
        }
    }

    #[test]
    fn user_operators_map_to_the_expected_sql() {
        const BASE: &str = "SELECT 1 FROM users u WHERE 1 = 1";
        // Integer comparisons across the range operators.
        assert_eq!(
            sql_for("balance", "gt", serde_json::json!(5)),
            format!("{BASE} AND u.balance > $1")
        );
        assert_eq!(
            sql_for("balance", "gte", serde_json::json!(5)),
            format!("{BASE} AND u.balance >= $1")
        );
        assert_eq!(
            sql_for("balance", "lt", serde_json::json!(5)),
            format!("{BASE} AND u.balance < $1")
        );
        assert_eq!(
            sql_for("balance", "lte", serde_json::json!(5)),
            format!("{BASE} AND u.balance <= $1")
        );
        assert_eq!(
            sql_for("id", "neq", serde_json::json!(5)),
            format!("{BASE} AND u.id <> $1")
        );
        assert_eq!(
            sql_for("plan_id", "in", serde_json::json!([1, 2])),
            format!("{BASE} AND u.plan_id = ANY($1)")
        );
        // The `'null'` sentinel is now a JSON null (§7.1).
        assert_eq!(
            sql_for("plan_id", "eq", serde_json::json!(null)),
            format!("{BASE} AND u.plan_id IS NULL")
        );
        assert_eq!(
            sql_for("plan_id", "neq", serde_json::json!(null)),
            format!("{BASE} AND u.plan_id IS NOT NULL")
        );
        // Email keeps the trimmed/lowercased equality and the literal ILIKE.
        assert_eq!(
            sql_for("email", "eq", serde_json::json!("A@B")),
            format!("{BASE} AND lower(btrim(u.email)) = lower(btrim($1))")
        );
        assert_eq!(
            sql_for("email", "like", serde_json::json!("gmail")),
            format!("{BASE} AND u.email ILIKE $1")
        );
        // Boolean flags bind a bool against the SMALLINT-guard expression.
        assert_eq!(
            sql_for("banned", "eq", serde_json::json!(true)),
            format!("{BASE} AND (u.banned <> 0) = $1")
        );
        // Integer `like` keeps the legacy `::text` substring search.
        assert_eq!(
            sql_for("id", "like", serde_json::json!("7")),
            format!("{BASE} AND u.id::text ILIKE $1")
        );
        // Timestamp columns compare on the stored epoch.
        assert_eq!(
            sql_for(
                "created_at",
                "gte",
                serde_json::json!("2023-11-14T22:13:20Z")
            ),
            format!("{BASE} AND u.created_at >= $1")
        );
    }

    #[test]
    fn total_used_is_sort_only_and_sort_defaults_to_created_at() {
        // `total_used` is a computed §7.2 sort field, not a filter column.
        let filter =
            filter_dsl::parse_filter_param(r#"[{"field":"total_used","op":"gt","value":1}]"#)
                .unwrap();
        assert!(filter_dsl::resolve_filters(&filter, USER_FILTER_COLUMNS).is_err());
        // It is sortable, on the NUMERIC(65,0) sum.
        let sort =
            filter_dsl::resolve_sort(Some("total_used"), Some("desc"), USER_SORT_COLUMNS).unwrap();
        assert_eq!(
            sort.order_by(),
            "(CAST(u.u AS NUMERIC(65,0)) + CAST(u.d AS NUMERIC(65,0))) DESC NULLS LAST"
        );
        // The default sort is created_at desc; boolean flags sort on the raw
        // column, not the boolean-cast filter expression.
        assert_eq!(
            filter_dsl::resolve_sort(None, None, USER_SORT_COLUMNS)
                .unwrap()
                .order_by(),
            "u.created_at DESC NULLS LAST"
        );
        assert_eq!(
            filter_dsl::resolve_sort(Some("banned"), Some("asc"), USER_SORT_COLUMNS)
                .unwrap()
                .order_by(),
            "u.banned ASC NULLS FIRST"
        );
    }
}

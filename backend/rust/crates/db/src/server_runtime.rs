use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::NaiveDate;
use rust_decimal::{Decimal, RoundingStrategy, prelude::ToPrimitive};
use sqlx::{AssertSqlSafe, FromRow, PgPool, Postgres, QueryBuilder, Transaction};
use v2board_analytics::{
    AnalyticsAdmissionError, AnalyticsEvent, IdentityKind, OutboxError, ReportedTrafficEvent,
    TrafficEventCore, enqueue_events,
};
use v2board_application::{
    RepositoryError,
    server_runtime::{
        PersistTrafficError, PersistTrafficReport, RepositoryResult, RuntimeServerNode,
        RuntimeServerRoute, RuntimeServerUser, RuntimeTrafficEntry, ServerRuntimeRepository,
    },
};
use v2board_domain_model::ServerKind;

const SQL_BATCH_SIZE: usize = 500;

#[derive(Clone, Debug)]
pub struct PostgresServerRuntimeRepository {
    pool: PgPool,
}

impl PostgresServerRuntimeRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn repository_error(operation: &'static str, error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::new(operation, error)
}

#[derive(Debug, FromRow)]
struct NodeRow {
    id: i32,
    group_ids: String,
    route_ids: Option<String>,
    rate: String,
    host: String,
    server_port: i32,
    created_at: i64,
    listen_ip: Option<String>,
    protocol: Option<String>,
    version: Option<i32>,
    tls: Option<i16>,
    tls_settings_json: Option<String>,
    flow: Option<String>,
    network: Option<String>,
    network_settings_json: Option<String>,
    encryption: Option<String>,
    encryption_settings_json: Option<String>,
    zero_rtt_handshake: Option<i16>,
    congestion_control: Option<String>,
    cipher: Option<String>,
    obfs: Option<String>,
    obfs_settings_json: Option<String>,
    obfs_password: Option<String>,
    padding_scheme_json: Option<String>,
    server_name: Option<String>,
    up_mbps: Option<i32>,
    down_mbps: Option<i32>,
    dns_settings_json: Option<String>,
    rule_settings_json: Option<String>,
}

#[derive(Debug, FromRow)]
struct RouteRow {
    id: i32,
    match_json: String,
    action: String,
    action_value_json: Option<String>,
}

impl ServerRuntimeRepository for PostgresServerRuntimeRepository {
    async fn credential_epoch(
        &self,
        kind: ServerKind,
        node_id: i32,
    ) -> RepositoryResult<Option<i64>> {
        sqlx::query_scalar(
            "SELECT credential_epoch FROM server_credential \
             WHERE node_type = $1 AND node_id = $2 LIMIT 1",
        )
        .bind(kind.as_str())
        .bind(node_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| repository_error("load server credential epoch", error))
    }

    async fn node(
        &self,
        kind: ServerKind,
        node_id: i32,
    ) -> RepositoryResult<Option<RuntimeServerNode>> {
        sqlx::query_as::<_, NodeRow>(AssertSqlSafe(node_sql(kind).to_string()))
            .bind(node_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| repository_error("load server runtime node", error))?
            .map(runtime_node)
            .transpose()
    }

    async fn available_users(
        &self,
        group_ids: &[i32],
        now: i64,
    ) -> RepositoryResult<Vec<RuntimeServerUser>> {
        if group_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut builder = QueryBuilder::<Postgres>::new(
            "SELECT id, uuid, speed_limit, device_limit FROM users WHERE group_id IN (",
        );
        {
            let mut separated = builder.separated(", ");
            for group_id in group_ids {
                separated.push_bind(*group_id);
            }
        }
        builder.push(
            ") AND CAST(u AS DECIMAL(30,0)) + CAST(d AS DECIMAL(30,0)) \
             < CAST(transfer_enable AS DECIMAL(30,0)) AND (expired_at >= ",
        );
        builder.push_bind(now);
        builder.push(" OR expired_at IS NULL) AND banned = 0");
        builder
            .build_query_as::<(i64, String, Option<i32>, Option<i32>)>()
            .fetch_all(&self.pool)
            .await
            .map(|rows| {
                rows.into_iter()
                    .map(|(id, uuid, speed_limit, device_limit)| RuntimeServerUser {
                        id,
                        uuid,
                        speed_limit,
                        device_limit,
                    })
                    .collect()
            })
            .map_err(|error| repository_error("load available server users", error))
    }

    async fn routes(&self, route_ids: &[i32]) -> RepositoryResult<Vec<RuntimeServerRoute>> {
        let mut positions = HashMap::with_capacity(route_ids.len());
        let mut unique_ids = Vec::with_capacity(route_ids.len());
        for route_id in route_ids {
            if positions.contains_key(route_id) {
                continue;
            }
            positions.insert(*route_id, unique_ids.len());
            unique_ids.push(*route_id);
        }
        let mut rows = Vec::with_capacity(unique_ids.len());
        for chunk in unique_ids.chunks(SQL_BATCH_SIZE) {
            let mut builder = QueryBuilder::<Postgres>::new(
                "SELECT id, \"match\"::text AS match_json, action, \
                 action_value::text AS action_value_json FROM server_route WHERE id IN (",
            );
            let mut separated = builder.separated(", ");
            for route_id in chunk {
                separated.push_bind(*route_id);
            }
            separated.push_unseparated(")");
            rows.extend(
                builder
                    .build_query_as::<RouteRow>()
                    .fetch_all(&self.pool)
                    .await
                    .map_err(|error| repository_error("load server routes", error))?,
            );
        }
        rows.sort_by_key(|row| positions.get(&row.id).copied().unwrap_or(usize::MAX));
        Ok(rows
            .into_iter()
            .map(|row| RuntimeServerRoute {
                id: row.id,
                match_json: row.match_json,
                action: row.action,
                action_value_json: row.action_value_json,
            })
            .collect())
    }

    async fn alive_user_ids(&self, now: i64) -> RepositoryResult<Vec<i64>> {
        sqlx::query_scalar(
            r#"
            SELECT id
            FROM users
            WHERE CAST(u AS DECIMAL(30,0)) + CAST(d AS DECIMAL(30,0))
                  < CAST(transfer_enable AS DECIMAL(30,0))
              AND (expired_at >= $1 OR expired_at IS NULL)
              AND banned = 0
              AND device_limit > 0
            "#,
        )
        .bind(now)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| repository_error("load alive-list users", error))
    }

    async fn persist_traffic(
        &self,
        report: PersistTrafficReport,
    ) -> Result<(), PersistTrafficError> {
        persist_traffic_report(&self.pool, report).await
    }
}

fn runtime_node(row: NodeRow) -> RepositoryResult<RuntimeServerNode> {
    Ok(RuntimeServerNode {
        id: row.id,
        group_ids: parse_i32_list(&row.group_ids)
            .map_err(|error| repository_error("decode server runtime group ids", error))?,
        route_ids: row
            .route_ids
            .as_deref()
            .map(parse_i32_list)
            .transpose()
            .map_err(|error| repository_error("decode server runtime route ids", error))?
            .unwrap_or_default(),
        rate: row.rate,
        host: row.host,
        server_port: row.server_port,
        created_at: row.created_at,
        listen_ip: row.listen_ip,
        protocol: row.protocol,
        version: row.version,
        tls: row.tls,
        tls_settings_json: row.tls_settings_json,
        flow: row.flow,
        network: row.network,
        network_settings_json: row.network_settings_json,
        encryption: row.encryption,
        encryption_settings_json: row.encryption_settings_json,
        zero_rtt_handshake: row.zero_rtt_handshake,
        congestion_control: row.congestion_control,
        cipher: row.cipher,
        obfs: row.obfs,
        obfs_settings_json: row.obfs_settings_json,
        obfs_password: row.obfs_password,
        padding_scheme_json: row.padding_scheme_json,
        server_name: row.server_name,
        up_mbps: row.up_mbps,
        down_mbps: row.down_mbps,
        dns_settings_json: row.dns_settings_json,
        rule_settings_json: row.rule_settings_json,
    })
}

fn parse_i32_list(value: &str) -> Result<Vec<i32>, serde_json::Error> {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("null") {
        return Ok(Vec::new());
    }
    if let Ok(single) = value.parse::<i32>() {
        return Ok(vec![single]);
    }
    serde_json::from_str::<Vec<serde_json::Value>>(value).map(|items| {
        items
            .into_iter()
            .filter_map(|item| {
                item.as_i64()
                    .and_then(|value| i32::try_from(value).ok())
                    .or_else(|| item.as_str().and_then(|value| value.parse().ok()))
            })
            .collect()
    })
}

fn node_sql(kind: ServerKind) -> &'static str {
    match kind {
        ServerKind::Shadowsocks => shadowsocks_node_sql(),
        ServerKind::Vmess => vmess_node_sql(),
        ServerKind::Trojan => trojan_node_sql(),
        ServerKind::Tuic => tuic_node_sql(),
        ServerKind::Hysteria => hysteria_node_sql(),
        ServerKind::Vless => vless_node_sql(),
        ServerKind::Anytls => anytls_node_sql(),
        ServerKind::V2node => v2node_node_sql(),
    }
}

fn shadowsocks_node_sql() -> &'static str {
    "SELECT id, group_id::text AS group_ids, route_id::text AS route_ids, rate, host, \
     server_port, created_at, NULL AS listen_ip, NULL AS protocol, NULL::INTEGER AS version, \
     NULL::SMALLINT AS tls, NULL AS tls_settings_json, NULL AS flow, NULL AS network, \
     NULL AS network_settings_json, NULL AS encryption, NULL AS encryption_settings_json, \
     NULL::SMALLINT AS zero_rtt_handshake, NULL AS congestion_control, cipher, obfs, \
     obfs_settings::text AS obfs_settings_json, NULL AS obfs_password, \
     NULL AS padding_scheme_json, NULL AS server_name, NULL::INTEGER AS up_mbps, \
     NULL::INTEGER AS down_mbps, NULL AS dns_settings_json, NULL AS rule_settings_json \
     FROM server_shadowsocks WHERE id = $1 LIMIT 1"
}

fn vmess_node_sql() -> &'static str {
    "SELECT id, group_id::text AS group_ids, route_id::text AS route_ids, rate, host, \
     server_port, created_at, NULL AS listen_ip, NULL AS protocol, NULL::INTEGER AS version, tls, \
     \"tlsSettings\"::text AS tls_settings_json, NULL AS flow, network, \
     \"networkSettings\"::text AS network_settings_json, NULL AS encryption, \
     NULL AS encryption_settings_json, NULL::SMALLINT AS zero_rtt_handshake, \
     NULL AS congestion_control, NULL AS cipher, NULL AS obfs, NULL AS obfs_settings_json, \
     NULL AS obfs_password, NULL AS padding_scheme_json, NULL AS server_name, \
     NULL::INTEGER AS up_mbps, NULL::INTEGER AS down_mbps, \
     \"dnsSettings\"::text AS dns_settings_json, \"ruleSettings\"::text AS rule_settings_json \
     FROM server_vmess WHERE id = $1 LIMIT 1"
}

fn trojan_node_sql() -> &'static str {
    "SELECT id, group_id::text AS group_ids, route_id::text AS route_ids, rate, host, \
     server_port, created_at, NULL AS listen_ip, NULL AS protocol, NULL::INTEGER AS version, \
     NULL::SMALLINT AS tls, NULL AS tls_settings_json, NULL AS flow, network, \
     network_settings::text AS network_settings_json, NULL AS encryption, \
     NULL AS encryption_settings_json, NULL::SMALLINT AS zero_rtt_handshake, \
     NULL AS congestion_control, NULL AS cipher, NULL AS obfs, NULL AS obfs_settings_json, \
     NULL AS obfs_password, NULL AS padding_scheme_json, server_name, \
     NULL::INTEGER AS up_mbps, NULL::INTEGER AS down_mbps, NULL AS dns_settings_json, \
     NULL AS rule_settings_json FROM server_trojan WHERE id = $1 LIMIT 1"
}

fn tuic_node_sql() -> &'static str {
    "SELECT id, group_id::text AS group_ids, route_id::text AS route_ids, rate, host, \
     server_port, created_at, NULL AS listen_ip, NULL AS protocol, NULL::INTEGER AS version, \
     NULL::SMALLINT AS tls, NULL AS tls_settings_json, NULL AS flow, NULL AS network, \
     NULL AS network_settings_json, NULL AS encryption, NULL AS encryption_settings_json, \
     zero_rtt_handshake, congestion_control, NULL AS cipher, NULL AS obfs, \
     NULL AS obfs_settings_json, NULL AS obfs_password, NULL AS padding_scheme_json, server_name, \
     NULL::INTEGER AS up_mbps, NULL::INTEGER AS down_mbps, NULL AS dns_settings_json, \
     NULL AS rule_settings_json FROM server_tuic WHERE id = $1 LIMIT 1"
}

fn hysteria_node_sql() -> &'static str {
    "SELECT id, group_id::text AS group_ids, route_id::text AS route_ids, rate, host, \
     server_port, created_at, NULL AS listen_ip, NULL AS protocol, version, NULL::SMALLINT AS tls, \
     NULL AS tls_settings_json, NULL AS flow, NULL AS network, NULL AS network_settings_json, \
     NULL AS encryption, NULL AS encryption_settings_json, NULL::SMALLINT AS zero_rtt_handshake, \
     NULL AS congestion_control, NULL AS cipher, obfs, NULL AS obfs_settings_json, obfs_password, \
     NULL AS padding_scheme_json, server_name, up_mbps, down_mbps, NULL AS dns_settings_json, \
     NULL AS rule_settings_json FROM server_hysteria WHERE id = $1 LIMIT 1"
}

fn vless_node_sql() -> &'static str {
    "SELECT id, group_id::text AS group_ids, route_id::text AS route_ids, rate, host, \
     server_port, created_at, NULL AS listen_ip, NULL AS protocol, NULL::INTEGER AS version, tls, \
     tls_settings::text AS tls_settings_json, flow, network, \
     network_settings::text AS network_settings_json, encryption, \
     encryption_settings::text AS encryption_settings_json, \
     NULL::SMALLINT AS zero_rtt_handshake, NULL AS congestion_control, NULL AS cipher, \
     NULL AS obfs, NULL AS obfs_settings_json, NULL AS obfs_password, \
     NULL AS padding_scheme_json, NULL AS server_name, NULL::INTEGER AS up_mbps, \
     NULL::INTEGER AS down_mbps, NULL AS dns_settings_json, NULL AS rule_settings_json \
     FROM server_vless WHERE id = $1 LIMIT 1"
}

fn anytls_node_sql() -> &'static str {
    "SELECT id, group_id::text AS group_ids, route_id::text AS route_ids, rate, host, \
     server_port, created_at, NULL AS listen_ip, NULL AS protocol, NULL::INTEGER AS version, \
     NULL::SMALLINT AS tls, NULL AS tls_settings_json, NULL AS flow, NULL AS network, \
     NULL AS network_settings_json, NULL AS encryption, NULL AS encryption_settings_json, \
     NULL::SMALLINT AS zero_rtt_handshake, NULL AS congestion_control, NULL AS cipher, \
     NULL AS obfs, NULL AS obfs_settings_json, NULL AS obfs_password, \
     padding_scheme::text AS padding_scheme_json, server_name, NULL::INTEGER AS up_mbps, \
     NULL::INTEGER AS down_mbps, NULL AS dns_settings_json, NULL AS rule_settings_json \
     FROM server_anytls WHERE id = $1 LIMIT 1"
}

fn v2node_node_sql() -> &'static str {
    "SELECT id, group_id::text AS group_ids, route_id::text AS route_ids, rate, host, \
     server_port, created_at, listen_ip, protocol, NULL::INTEGER AS version, tls, \
     tls_settings::text AS tls_settings_json, flow, network, \
     network_settings::text AS network_settings_json, encryption, \
     encryption_settings::text AS encryption_settings_json, zero_rtt_handshake, \
     congestion_control, cipher, obfs, NULL AS obfs_settings_json, obfs_password, \
     padding_scheme::text AS padding_scheme_json, NULL AS server_name, up_mbps, down_mbps, \
     NULL AS dns_settings_json, NULL AS rule_settings_json \
     FROM server_v2node WHERE id = $1 LIMIT 1"
}

async fn persist_traffic_report(
    pool: &PgPool,
    report: PersistTrafficReport,
) -> Result<(), PersistTrafficError> {
    let rate = parse_server_rate(&report.rate);
    let rate_text = canonical_rate_text(&report.rate, rate);
    let rate_decimal = rate_decimal_10_2(rate)?;
    let accounting_date = NaiveDate::parse_from_str(&report.accounting_date, "%Y-%m-%d")
        .map_err(|error| repository_error("parse traffic accounting date", error))?;
    let identity_kind = if report.report_key.starts_with("i-") {
        IdentityKind::Implicit
    } else {
        IdentityKind::Explicit
    };
    let mut transaction = pool
        .begin()
        .await
        .map_err(|error| repository_error("begin traffic report", error))?;
    let inserted = match sqlx::query(
        r#"
        INSERT INTO server_traffic_report
            (report_key, payload_hash, node_id, node_type, rate_text, rate_decimal_10_2,
             identity_kind, accepted_at, accounting_date, applied_at, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NULL, $10, $11)
        "#,
    )
    .bind(&report.report_key)
    .bind(&report.payload_hash)
    .bind(report.node_id)
    .bind(report.node_kind.as_str())
    .bind(&rate_text)
    .bind(rate_decimal)
    .bind(identity_kind_value(identity_kind))
    .bind(report.accepted_at)
    .bind(accounting_date)
    .bind(report.accepted_at)
    .bind(report.accepted_at)
    .execute(&mut *transaction)
    .await
    {
        Ok(_) => true,
        Err(error)
            if error
                .as_database_error()
                .is_some_and(|error| error.is_unique_violation()) =>
        {
            false
        }
        Err(error) => return Err(repository_error("insert traffic report", error).into()),
    };
    if !inserted {
        let existing_hash: String = sqlx::query_scalar(
            "SELECT payload_hash FROM server_traffic_report WHERE report_key = $1 FOR UPDATE",
        )
        .bind(&report.report_key)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|error| repository_error("lock existing traffic report", error))?;
        if existing_hash != report.payload_hash {
            return Err(PersistTrafficError::IdempotencyConflict);
        }
        transaction
            .commit()
            .await
            .map_err(|error| repository_error("commit duplicate traffic report", error))?;
        return Ok(());
    }

    let epochs = lock_report_users(&mut transaction, &report.group_ids, &report.entries).await?;
    let mut accepted = Vec::with_capacity(report.entries.len());
    for entry in &report.entries {
        let epoch = *epochs
            .get(&entry.user_id)
            .ok_or(PersistTrafficError::UnauthorizedUser)?;
        accepted.push(AcceptedTrafficItem {
            user_id: entry.user_id,
            traffic_epoch: epoch,
            raw_u: entry.upload,
            raw_d: entry.download,
            charged_u: charged_bytes(entry.upload, rate)?,
            charged_d: charged_bytes(entry.download, rate)?,
        });
    }
    for chunk in accepted.chunks(SQL_BATCH_SIZE) {
        let mut builder = QueryBuilder::<Postgres>::new(
            "INSERT INTO server_traffic_report_item \
             (report_key, user_id, traffic_epoch, raw_u, raw_d, charged_u, charged_d) ",
        );
        builder.push_values(chunk, |mut row, item| {
            row.push_bind(&report.report_key)
                .push_bind(item.user_id)
                .push_bind(item.traffic_epoch)
                .push_bind(item.raw_u)
                .push_bind(item.raw_d)
                .push_bind(item.charged_u)
                .push_bind(item.charged_d);
        });
        builder
            .build()
            .execute(&mut *transaction)
            .await
            .map_err(|error| repository_error("insert traffic report items", error))?;
    }

    let rate_decimal_text = decimal_with_scale(rate_decimal, 2);
    let mut events = Vec::<AnalyticsEvent>::with_capacity(accepted.len());
    for item in &accepted {
        let event = ReportedTrafficEvent::new(TrafficEventCore {
            installation_id: report.installation_id.clone(),
            report_key: report.report_key.clone(),
            payload_hash: report.payload_hash.clone(),
            identity_kind,
            user_id: item.user_id.to_string(),
            traffic_epoch: item.traffic_epoch.to_string(),
            server_id: report.node_id.to_string(),
            server_type: report.node_kind.as_str().to_string(),
            rate_text: rate_text.clone(),
            rate_decimal_10_2: rate_decimal_text.clone(),
            raw_u: item.raw_u.to_string(),
            raw_d: item.raw_d.to_string(),
            charged_u: item.charged_u.to_string(),
            charged_d: item.charged_d.to_string(),
            accepted_at: report.accepted_at,
            accounting_date: report.accounting_date.clone(),
            accounting_timezone: "Asia/Shanghai".to_string(),
        })
        .and_then(ReportedTrafficEvent::into_outbox)
        .map_err(|_| PersistTrafficError::AnalyticsEventInvalid)?;
        events.push(event);
    }
    enqueue_events(&mut transaction, &events, report.accepted_at)
        .await
        .map_err(map_outbox_error)?;
    persist_traffic_stats(&mut transaction, &report, rate).await?;
    transaction
        .commit()
        .await
        .map_err(|error| repository_error("commit traffic report", error))?;
    Ok(())
}

#[derive(Clone, Copy)]
struct AcceptedTrafficItem {
    user_id: i64,
    traffic_epoch: i64,
    raw_u: i64,
    raw_d: i64,
    charged_u: i64,
    charged_d: i64,
}

async fn lock_report_users(
    transaction: &mut Transaction<'_, Postgres>,
    group_ids: &[i32],
    entries: &[RuntimeTrafficEntry],
) -> Result<BTreeMap<i64, i64>, PersistTrafficError> {
    let user_ids = entries
        .iter()
        .map(|entry| entry.user_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if user_ids.is_empty() {
        return Ok(BTreeMap::new());
    }
    if group_ids.is_empty() {
        return Err(PersistTrafficError::UnauthorizedUser);
    }
    let mut epochs = BTreeMap::new();
    for user_chunk in user_ids.chunks(SQL_BATCH_SIZE) {
        let mut builder =
            QueryBuilder::<Postgres>::new("SELECT id, traffic_epoch FROM users WHERE id IN (");
        {
            let mut separated = builder.separated(", ");
            for user_id in user_chunk {
                separated.push_bind(*user_id);
            }
        }
        builder.push(") AND group_id IN (");
        {
            let mut separated = builder.separated(", ");
            for group_id in group_ids {
                separated.push_bind(*group_id);
            }
        }
        builder.push(") ORDER BY id FOR UPDATE");
        for (user_id, epoch) in builder
            .build_query_as::<(i64, i64)>()
            .fetch_all(&mut **transaction)
            .await
            .map_err(|error| repository_error("lock traffic report users", error))?
        {
            epochs.insert(user_id, epoch);
        }
    }
    if epochs.len() != user_ids.len() {
        return Err(PersistTrafficError::UnauthorizedUser);
    }
    Ok(epochs)
}

async fn persist_traffic_stats(
    transaction: &mut Transaction<'_, Postgres>,
    report: &PersistTrafficReport,
    rate: Decimal,
) -> Result<(), PersistTrafficError> {
    let mut total_u = 0_i64;
    let mut total_d = 0_i64;
    for entry in &report.entries {
        total_u = total_u
            .checked_add(entry.upload)
            .ok_or(PersistTrafficError::TotalOutOfRange)?;
        total_d = total_d
            .checked_add(entry.download)
            .ok_or(PersistTrafficError::TotalOutOfRange)?;
    }
    for chunk in report.entries.chunks(SQL_BATCH_SIZE) {
        let mut builder = QueryBuilder::<Postgres>::new(
            "INSERT INTO user_traffic \
             (user_id, server_rate, u, d, record_type, record_at, created_at, updated_at) ",
        );
        builder.push_values(chunk, |mut row, entry| {
            row.push_bind(entry.user_id)
                .push_bind(rate)
                .push_bind(entry.upload)
                .push_bind(entry.download)
                .push_bind("d")
                .push_bind(report.accounting_record_at)
                .push_bind(report.accepted_at)
                .push_bind(report.accepted_at);
        });
        builder.push(
            " ON CONFLICT (server_rate, user_id, record_at) DO UPDATE SET \
             u = user_traffic.u + EXCLUDED.u, d = user_traffic.d + EXCLUDED.d, \
             updated_at = EXCLUDED.updated_at",
        );
        builder
            .build()
            .execute(&mut **transaction)
            .await
            .map_err(map_stat_error)?;
    }
    sqlx::query(
        r#"
        INSERT INTO server_traffic
            (server_id, server_type, u, d, record_type, record_at, created_at, updated_at)
        VALUES ($1, $2, $3, $4, 'd', $5, $6, $7)
        ON CONFLICT (server_id, server_type, record_at) DO UPDATE SET
            u = server_traffic.u + EXCLUDED.u,
            d = server_traffic.d + EXCLUDED.d,
            updated_at = EXCLUDED.updated_at
        "#,
    )
    .bind(report.node_id)
    .bind(report.node_kind.as_str())
    .bind(total_u)
    .bind(total_d)
    .bind(report.accounting_record_at)
    .bind(report.accepted_at)
    .bind(report.accepted_at)
    .execute(&mut **transaction)
    .await
    .map_err(map_stat_error)?;
    Ok(())
}

fn parse_server_rate(rate: &str) -> Decimal {
    rate.parse().unwrap_or(Decimal::ZERO)
}

fn canonical_rate_text(raw: &str, parsed: Decimal) -> String {
    if raw.trim().parse::<Decimal>().is_ok() {
        raw.trim().to_owned()
    } else {
        parsed.normalize().to_string()
    }
}

fn rate_decimal_10_2(rate: Decimal) -> Result<Decimal, PersistTrafficError> {
    let rounded = rate.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
    let maximum = Decimal::new(9_999_999_999, 2);
    if rounded.is_sign_negative() || rounded > maximum {
        return Err(PersistTrafficError::RateOutOfRange);
    }
    Ok(rounded)
}

fn charged_bytes(bytes: i64, rate: Decimal) -> Result<i64, PersistTrafficError> {
    Decimal::from(bytes)
        .checked_mul(rate)
        .and_then(|value| {
            value
                .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
                .to_i64()
        })
        .ok_or(PersistTrafficError::ChargeOutOfRange)
}

fn decimal_with_scale(mut value: Decimal, scale: u32) -> String {
    value.rescale(scale);
    value.to_string()
}

fn identity_kind_value(kind: IdentityKind) -> &'static str {
    match kind {
        IdentityKind::Explicit => "explicit",
        IdentityKind::Implicit => "implicit",
    }
}

fn map_outbox_error(error: OutboxError) -> PersistTrafficError {
    match error {
        OutboxError::Admission(AnalyticsAdmissionError::SoftRateLimited) => {
            PersistTrafficError::AnalyticsRateLimited
        }
        OutboxError::Admission(
            AnalyticsAdmissionError::HardStop
            | AnalyticsAdmissionError::MissingOrMismatchedPolicy
            | AnalyticsAdmissionError::InvalidState,
        ) => PersistTrafficError::AnalyticsUnavailable,
        error => PersistTrafficError::Repository(repository_error(
            "enqueue traffic analytics event",
            error,
        )),
    }
}

fn map_stat_error(error: sqlx::Error) -> PersistTrafficError {
    let overflow = error
        .as_database_error()
        .and_then(|error| error.code())
        .is_some_and(|code| code.as_ref() == "22003");
    if overflow {
        PersistTrafficError::TotalOutOfRange
    } else {
        PersistTrafficError::Repository(repository_error("write traffic statistics", error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_compatibility_and_charge_rounding_match_external_contract() {
        assert_eq!(parse_server_rate("not-a-number"), Decimal::ZERO);
        assert_eq!(charged_bytes(3, Decimal::new(15, 1)).unwrap(), 5);
        assert!(rate_decimal_10_2(Decimal::NEGATIVE_ONE).is_err());
    }

    #[test]
    fn persisted_json_id_lists_accept_numeric_strings() {
        assert_eq!(parse_i32_list(r#"["1",2,"bad"]"#).unwrap(), vec![1, 2]);
        assert_eq!(parse_i32_list("3").unwrap(), vec![3]);
    }
}

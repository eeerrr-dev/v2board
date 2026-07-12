use std::collections::HashMap;

use chrono::Utc;
use sqlx::{AssertSqlSafe, Postgres, QueryBuilder};
use v2board_compat::ApiError;
use v2board_db::DbPool;

use crate::runtime::AppState;

use super::{
    ServerNodeRow, ServerUserRow,
    request::{normalize_server_node_type, required_i32_param},
};

pub(super) async fn load_uniproxy_node(
    state: &AppState,
    params: &HashMap<String, String>,
) -> Result<(String, ServerNodeRow), ApiError> {
    let node_type = params
        .get("node_type")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(normalize_server_node_type)
        .ok_or_else(|| ApiError::legacy("server is not exist"))?;
    let node_id = required_i32_param(params, "node_id")?;
    let node = load_server_node(&state.db, &node_type, node_id)
        .await?
        .ok_or_else(|| ApiError::legacy("server is not exist"))?;
    Ok((node_type, node))
}

pub(super) async fn load_server_node(
    db: &DbPool,
    node_type: &str,
    node_id: i32,
) -> Result<Option<ServerNodeRow>, ApiError> {
    let Some(sql) = server_node_sql(node_type) else {
        return Ok(None);
    };
    Ok(
        sqlx::query_as::<_, ServerNodeRow>(AssertSqlSafe(sql.to_string()))
            .bind(node_id)
            .fetch_optional(db)
            .await?,
    )
}

fn server_node_sql(node_type: &str) -> Option<&'static str> {
    match node_type {
        "shadowsocks" => Some(
            r#"
            SELECT id, group_id::text AS group_id, route_id::text AS route_id,
                   name, rate, host, CAST(port AS TEXT) AS port,
                   server_port, created_at,
                   NULL AS listen_ip, NULL AS protocol, NULL::INTEGER AS version,
                   NULL::SMALLINT AS tls,
                   NULL AS tls_settings, NULL AS flow, NULL AS network, NULL AS network_settings,
                   NULL AS encryption, NULL AS encryption_settings,
                   NULL::SMALLINT AS disable_sni,
                   NULL AS udp_relay_mode, NULL::SMALLINT AS zero_rtt_handshake,
                   NULL AS congestion_control,
                   cipher, obfs, obfs_settings::text AS obfs_settings,
                   NULL AS obfs_password, NULL AS padding_scheme,
                   NULL::SMALLINT AS allow_insecure, NULL AS server_name,
                   NULL::INTEGER AS up_mbps, NULL::INTEGER AS down_mbps
            FROM v2_server_shadowsocks
            WHERE id = $1
            LIMIT 1
            "#,
        ),
        "vmess" => Some(
            r#"
            SELECT id, group_id::text AS group_id, route_id::text AS route_id,
                   name, rate, host, CAST(port AS TEXT) AS port,
                   server_port, created_at,
                   NULL AS listen_ip, NULL AS protocol, NULL::INTEGER AS version, tls,
                   "tlsSettings"::text AS tls_settings, NULL AS flow, network,
                   "networkSettings"::text AS network_settings, NULL AS encryption,
                   NULL AS encryption_settings, NULL::SMALLINT AS disable_sni,
                   NULL AS udp_relay_mode, NULL::SMALLINT AS zero_rtt_handshake,
                   NULL AS congestion_control, NULL AS cipher,
                   NULL AS obfs, NULL AS obfs_settings, NULL AS obfs_password,
                   NULL AS padding_scheme, NULL::SMALLINT AS allow_insecure, NULL AS server_name,
                   NULL::INTEGER AS up_mbps, NULL::INTEGER AS down_mbps
            FROM v2_server_vmess
            WHERE id = $1
            LIMIT 1
            "#,
        ),
        "trojan" => Some(
            r#"
            SELECT id, group_id::text AS group_id, route_id::text AS route_id,
                   name, rate, host, CAST(port AS TEXT) AS port,
                   server_port, created_at,
                   NULL AS listen_ip, NULL AS protocol, NULL::INTEGER AS version,
                   NULL::SMALLINT AS tls,
                   NULL AS tls_settings, NULL AS flow, network,
                   network_settings::text AS network_settings,
                   NULL AS encryption, NULL AS encryption_settings,
                   NULL::SMALLINT AS disable_sni, NULL AS udp_relay_mode,
                   NULL::SMALLINT AS zero_rtt_handshake,
                   NULL AS congestion_control, NULL AS cipher, NULL AS obfs,
                   NULL AS obfs_settings, NULL AS obfs_password, NULL AS padding_scheme,
                   allow_insecure, server_name,
                   NULL::INTEGER AS up_mbps, NULL::INTEGER AS down_mbps
            FROM v2_server_trojan
            WHERE id = $1
            LIMIT 1
            "#,
        ),
        "vless" => Some(
            r#"
            SELECT id, group_id::text AS group_id, route_id::text AS route_id,
                   name, rate, host, CAST(port AS TEXT) AS port,
                   server_port, created_at,
                   NULL AS listen_ip, NULL AS protocol, NULL::INTEGER AS version, tls,
                   tls_settings::text AS tls_settings, flow, network,
                   network_settings::text AS network_settings, encryption,
                   encryption_settings::text AS encryption_settings,
                   NULL::SMALLINT AS disable_sni, NULL AS udp_relay_mode,
                   NULL::SMALLINT AS zero_rtt_handshake, NULL AS congestion_control,
                   NULL AS cipher,
                   NULL AS obfs, NULL AS obfs_settings, NULL AS obfs_password,
                   NULL AS padding_scheme, NULL::SMALLINT AS allow_insecure, NULL AS server_name,
                   NULL::INTEGER AS up_mbps, NULL::INTEGER AS down_mbps
            FROM v2_server_vless
            WHERE id = $1
            LIMIT 1
            "#,
        ),
        "tuic" => Some(
            r#"
            SELECT id, group_id::text AS group_id, route_id::text AS route_id,
                   name, rate, host, CAST(port AS TEXT) AS port,
                   server_port, created_at,
                   NULL AS listen_ip, NULL AS protocol, NULL::INTEGER AS version,
                   NULL::SMALLINT AS tls,
                   NULL AS tls_settings, NULL AS flow, NULL AS network, NULL AS network_settings,
                   NULL AS encryption, NULL AS encryption_settings, disable_sni,
                   udp_relay_mode, zero_rtt_handshake, congestion_control, NULL AS cipher,
                   NULL AS obfs, NULL AS obfs_settings, NULL AS obfs_password,
                   NULL AS padding_scheme, insecure AS allow_insecure, server_name,
                   NULL::INTEGER AS up_mbps, NULL::INTEGER AS down_mbps
            FROM v2_server_tuic
            WHERE id = $1
            LIMIT 1
            "#,
        ),
        "hysteria" => Some(
            r#"
            SELECT id, group_id::text AS group_id, route_id::text AS route_id,
                   name, rate, host, CAST(port AS TEXT) AS port,
                   server_port, created_at,
                   NULL AS listen_ip, NULL AS protocol, version, NULL::SMALLINT AS tls,
                   NULL AS tls_settings, NULL AS flow, NULL AS network, NULL AS network_settings,
                   NULL AS encryption, NULL AS encryption_settings,
                   NULL::SMALLINT AS disable_sni,
                   NULL AS udp_relay_mode, NULL::SMALLINT AS zero_rtt_handshake,
                   NULL AS congestion_control,
                   NULL AS cipher, obfs, NULL AS obfs_settings, obfs_password,
                   NULL AS padding_scheme, insecure AS allow_insecure, server_name,
                   up_mbps, down_mbps
            FROM v2_server_hysteria
            WHERE id = $1
            LIMIT 1
            "#,
        ),
        "anytls" => Some(
            r#"
            SELECT id, group_id::text AS group_id, route_id::text AS route_id,
                   name, rate, host, CAST(port AS TEXT) AS port,
                   server_port, created_at,
                   NULL AS listen_ip, NULL AS protocol, NULL::INTEGER AS version,
                   NULL::SMALLINT AS tls,
                   NULL AS tls_settings, NULL AS flow, NULL AS network, NULL AS network_settings,
                   NULL AS encryption, NULL AS encryption_settings,
                   NULL::SMALLINT AS disable_sni,
                   NULL AS udp_relay_mode, NULL::SMALLINT AS zero_rtt_handshake,
                   NULL AS congestion_control,
                   NULL AS cipher, NULL AS obfs, NULL AS obfs_settings, NULL AS obfs_password,
                   padding_scheme::text AS padding_scheme, insecure AS allow_insecure, server_name,
                   NULL::INTEGER AS up_mbps, NULL::INTEGER AS down_mbps
            FROM v2_server_anytls
            WHERE id = $1
            LIMIT 1
            "#,
        ),
        "v2node" => Some(
            r#"
            SELECT id, group_id::text AS group_id, route_id::text AS route_id,
                   name, rate, host, CAST(port AS TEXT) AS port,
                   server_port, created_at, listen_ip, protocol, NULL::INTEGER AS version, tls,
                   tls_settings::text AS tls_settings, flow, network,
                   network_settings::text AS network_settings, encryption,
                   encryption_settings::text AS encryption_settings,
                   disable_sni, udp_relay_mode, zero_rtt_handshake,
                   congestion_control, cipher, obfs, NULL AS obfs_settings, obfs_password,
                   padding_scheme::text AS padding_scheme,
                   NULL::SMALLINT AS allow_insecure, NULL AS server_name,
                   up_mbps, down_mbps
            FROM v2_server_v2node
            WHERE id = $1
            LIMIT 1
            "#,
        ),
        _ => None,
    }
}

pub(super) async fn server_available_users(
    db: &DbPool,
    group_ids: Vec<i32>,
) -> Result<Vec<ServerUserRow>, ApiError> {
    if group_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut builder = QueryBuilder::<Postgres>::new(
        "SELECT id, uuid, speed_limit, device_limit FROM v2_user WHERE group_id IN (",
    );
    {
        let mut separated = builder.separated(", ");
        for group_id in group_ids {
            separated.push_bind(group_id);
        }
    }
    builder.push(
        ") AND CAST(u AS DECIMAL(30,0)) + CAST(d AS DECIMAL(30,0)) \
         < CAST(transfer_enable AS DECIMAL(30,0)) AND (expired_at >= ",
    );
    builder.push_bind(Utc::now().timestamp());
    builder.push(" OR expired_at IS NULL) AND banned = 0");
    Ok(builder
        .build_query_as::<ServerUserRow>()
        .fetch_all(db)
        .await?)
}

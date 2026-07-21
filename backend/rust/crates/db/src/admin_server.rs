use serde_json::{Map, Value};
use sqlx::{
    AssertSqlSafe, FromRow, PgPool, Postgres, QueryBuilder, Transaction, postgres::PgDatabaseError,
    types::Json,
};
use v2board_application::{
    RepositoryError,
    server_management::{
        DeleteGroupOutcome, PreparedServerWrite, ServerColumnValue, ServerGroup,
        ServerGroupReference, ServerManagementRepository, ServerNode, ServerNodeCommon,
        ServerNodeDetails, ServerPersistenceOutcome, ServerRoute, ServerRouteChanges,
        ServerRouteCreateInput, ServerSettingValue, ServerSortUpdate, StoredServerNode,
        UpdateOutcome,
    },
};
use v2board_domain_model::{ServerKind, ServerRouteAction};

const GROUP_LOCK_BATCH: usize = 500;

#[derive(Clone)]
pub struct PostgresServerManagementRepository {
    pool: PgPool,
}

impl PostgresServerManagementRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn repository_error(operation: &'static str, error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::new(operation, error)
}

const fn table(kind: ServerKind) -> &'static str {
    match kind {
        ServerKind::Shadowsocks => "server_shadowsocks",
        ServerKind::Vmess => "server_vmess",
        ServerKind::Trojan => "server_trojan",
        ServerKind::Tuic => "server_tuic",
        ServerKind::Hysteria => "server_hysteria",
        ServerKind::Vless => "server_vless",
        ServerKind::Anytls => "server_anytls",
        ServerKind::V2node => "server_v2node",
    }
}

const fn copy_columns(kind: ServerKind) -> &'static [&'static str] {
    match kind {
        ServerKind::Shadowsocks => &[
            "group_id",
            "route_id",
            "parent_id",
            "tags",
            "name",
            "rate",
            "host",
            "port",
            "server_port",
            "cipher",
            "obfs",
            "obfs_settings",
            "show",
            "sort",
        ],
        ServerKind::Vmess => &[
            "group_id",
            "route_id",
            "name",
            "parent_id",
            "host",
            "port",
            "server_port",
            "tls",
            "tags",
            "rate",
            "network",
            "rules",
            "networkSettings",
            "tlsSettings",
            "ruleSettings",
            "dnsSettings",
            "show",
            "sort",
        ],
        ServerKind::Trojan => &[
            "group_id",
            "route_id",
            "parent_id",
            "tags",
            "name",
            "rate",
            "host",
            "port",
            "server_port",
            "network",
            "network_settings",
            "allow_insecure",
            "server_name",
            "show",
            "sort",
        ],
        ServerKind::Tuic => &[
            "group_id",
            "route_id",
            "name",
            "parent_id",
            "host",
            "port",
            "server_port",
            "tags",
            "rate",
            "show",
            "sort",
            "server_name",
            "insecure",
            "disable_sni",
            "udp_relay_mode",
            "zero_rtt_handshake",
            "congestion_control",
        ],
        ServerKind::Hysteria => &[
            "version",
            "group_id",
            "route_id",
            "name",
            "parent_id",
            "host",
            "port",
            "server_port",
            "tags",
            "rate",
            "show",
            "sort",
            "up_mbps",
            "down_mbps",
            "obfs",
            "obfs_password",
            "server_name",
            "insecure",
        ],
        ServerKind::Vless => &[
            "group_id",
            "route_id",
            "name",
            "parent_id",
            "host",
            "port",
            "server_port",
            "tls",
            "tls_settings",
            "flow",
            "network",
            "network_settings",
            "encryption",
            "encryption_settings",
            "tags",
            "rate",
            "show",
            "sort",
        ],
        ServerKind::Anytls => &[
            "group_id",
            "route_id",
            "name",
            "parent_id",
            "host",
            "port",
            "server_port",
            "tags",
            "rate",
            "show",
            "sort",
            "server_name",
            "insecure",
            "padding_scheme",
        ],
        ServerKind::V2node => &[
            "group_id",
            "route_id",
            "name",
            "parent_id",
            "host",
            "listen_ip",
            "port",
            "server_port",
            "tags",
            "rate",
            "show",
            "sort",
            "protocol",
            "tls",
            "tls_settings",
            "flow",
            "network",
            "network_settings",
            "encryption",
            "encryption_settings",
            "disable_sni",
            "udp_relay_mode",
            "zero_rtt_handshake",
            "congestion_control",
            "cipher",
            "up_mbps",
            "down_mbps",
            "obfs",
            "obfs_password",
            "padding_scheme",
        ],
    }
}

#[derive(FromRow)]
struct GroupRow {
    id: i32,
    name: String,
    user_count: i64,
    server_count: i64,
    created_at: i64,
    updated_at: i64,
}

const GROUP_SELECT: &str = r#"
SELECT g.id, g.name, g.created_at, g.updated_at,
       (SELECT COUNT(*) FROM users u WHERE u.group_id = g.id) AS user_count,
       ((SELECT COUNT(*) FROM server_shadowsocks s WHERE s.group_id @> jsonb_build_array(g.id) OR s.group_id @> jsonb_build_array(g.id::text)) +
        (SELECT COUNT(*) FROM server_vmess s WHERE s.group_id @> jsonb_build_array(g.id) OR s.group_id @> jsonb_build_array(g.id::text)) +
        (SELECT COUNT(*) FROM server_trojan s WHERE s.group_id @> jsonb_build_array(g.id) OR s.group_id @> jsonb_build_array(g.id::text)) +
        (SELECT COUNT(*) FROM server_tuic s WHERE s.group_id @> jsonb_build_array(g.id) OR s.group_id @> jsonb_build_array(g.id::text)) +
        (SELECT COUNT(*) FROM server_hysteria s WHERE s.group_id @> jsonb_build_array(g.id) OR s.group_id @> jsonb_build_array(g.id::text)) +
        (SELECT COUNT(*) FROM server_vless s WHERE s.group_id @> jsonb_build_array(g.id) OR s.group_id @> jsonb_build_array(g.id::text)) +
        (SELECT COUNT(*) FROM server_anytls s WHERE s.group_id @> jsonb_build_array(g.id) OR s.group_id @> jsonb_build_array(g.id::text)) +
        (SELECT COUNT(*) FROM server_v2node s WHERE s.group_id @> jsonb_build_array(g.id) OR s.group_id @> jsonb_build_array(g.id::text))) AS server_count
FROM server_group g
"#;

#[derive(FromRow)]
struct RouteRow {
    id: i32,
    remarks: String,
    match_rules: Json<Vec<String>>,
    action: String,
    action_value: Option<Json<Value>>,
    created_at: i64,
    updated_at: i64,
}

#[allow(async_fn_in_trait)]
impl ServerManagementRepository for PostgresServerManagementRepository {
    async fn groups(&self, id: Option<i32>) -> Result<Vec<ServerGroup>, RepositoryError> {
        let mut query = QueryBuilder::<Postgres>::new(GROUP_SELECT);
        if let Some(id) = id {
            query.push(" WHERE g.id = ").push_bind(id);
        }
        query.push(" ORDER BY g.id ASC");
        query
            .build_query_as::<GroupRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(|error| repository_error("server.groups", error))
            .map(|rows| {
                rows.into_iter()
                    .map(|row| ServerGroup {
                        id: row.id,
                        name: row.name,
                        user_count: row.user_count,
                        server_count: row.server_count,
                        created_at: row.created_at,
                        updated_at: row.updated_at,
                    })
                    .collect()
            })
    }

    async fn create_group(&self, name: &str, now: i64) -> Result<i32, RepositoryError> {
        sqlx::query_scalar("INSERT INTO server_group (name, created_at, updated_at) VALUES ($1, $2, $2) RETURNING id")
            .bind(name).bind(now).fetch_one(&self.pool).await
            .map_err(|error| repository_error("server.create_group", error))
    }

    async fn patch_group(
        &self,
        id: i32,
        name: &str,
        now: i64,
    ) -> Result<UpdateOutcome, RepositoryError> {
        sqlx::query("UPDATE server_group SET name = $1, updated_at = $2 WHERE id = $3")
            .bind(name)
            .bind(now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|error| repository_error("server.patch_group", error))
            .map(|done| {
                if done.rows_affected() == 1 {
                    UpdateOutcome::Updated
                } else {
                    UpdateOutcome::NotFound
                }
            })
    }

    async fn delete_group(&self, id: i32) -> Result<DeleteGroupOutcome, RepositoryError> {
        delete_group(&self.pool, id).await
    }

    async fn routes(&self) -> Result<Vec<ServerRoute>, RepositoryError> {
        sqlx::query_as::<_, RouteRow>(r#"SELECT id, remarks, "match" AS match_rules, action, action_value, created_at, updated_at FROM server_route ORDER BY id ASC"#)
            .fetch_all(&self.pool).await
            .map_err(|error| repository_error("server.routes", error))?
            .into_iter().map(route_from_row).collect()
    }

    async fn create_route(
        &self,
        input: ServerRouteCreateInput,
        now: i64,
    ) -> Result<i32, RepositoryError> {
        sqlx::query_scalar(r#"INSERT INTO server_route (remarks, "match", action, action_value, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $5) RETURNING id"#)
            .bind(input.remarks).bind(Json(input.match_rules)).bind(input.action.as_str())
            .bind(input.action_value.map(|value| Json(Value::String(value))))
            .bind(now).fetch_one(&self.pool).await
            .map_err(|error| repository_error("server.create_route", error))
    }

    async fn patch_route(
        &self,
        id: i32,
        changes: ServerRouteChanges,
    ) -> Result<UpdateOutcome, RepositoryError> {
        let mut query = QueryBuilder::<Postgres>::new("UPDATE server_route SET updated_at = ");
        query.push_bind(changes.updated_at);
        if let Some(remarks) = changes.remarks {
            query.push(", remarks = ").push_bind(remarks);
        }
        if let Some(matches) = changes.match_rules {
            query.push(", \"match\" = ").push_bind(Json(matches));
        }
        if let Some(action) = changes.action {
            query.push(", action = ").push_bind(action.as_str());
        }
        if let Some(action_value) = changes.action_value {
            query
                .push(", action_value = ")
                .push_bind(action_value.map(|value| Json(Value::String(value))));
        }
        query.push(" WHERE id = ").push_bind(id);
        query
            .build()
            .execute(&self.pool)
            .await
            .map_err(|error| repository_error("server.patch_route", error))
            .map(|done| {
                if done.rows_affected() == 1 {
                    UpdateOutcome::Updated
                } else {
                    UpdateOutcome::NotFound
                }
            })
    }

    async fn delete_route(&self, id: i32) -> Result<UpdateOutcome, RepositoryError> {
        sqlx::query("DELETE FROM server_route WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|error| repository_error("server.delete_route", error))
            .map(|done| {
                if done.rows_affected() == 1 {
                    UpdateOutcome::Updated
                } else {
                    UpdateOutcome::NotFound
                }
            })
    }

    async fn nodes(&self) -> Result<Vec<StoredServerNode>, RepositoryError> {
        let mut nodes = Vec::new();
        for kind in ServerKind::ALL {
            let table = table(kind);
            let sql = AssertSqlSafe(format!(
                "SELECT to_jsonb(s) || jsonb_build_object('type', '{}', 'credential_epoch', c.credential_epoch) FROM {table} s LEFT JOIN server_credential c ON c.node_type = '{}' AND c.node_id = s.id ORDER BY s.sort ASC NULLS FIRST",
                kind.as_str(),
                kind.as_str()
            ));
            let rows = sqlx::query_scalar::<_, Json<Value>>(sql)
                .fetch_all(&self.pool)
                .await
                .map_err(|error| repository_error("server.nodes", error))?;
            for row in rows {
                nodes.push(
                    parse_node(row.0)
                        .map_err(|error| repository_error("server.nodes.decode", error))?,
                );
            }
        }
        Ok(nodes)
    }

    async fn sort_nodes(&self, updates: &[ServerSortUpdate]) -> Result<(), RepositoryError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| repository_error("server.sort.begin", error))?;
        for update in updates {
            let sql = AssertSqlSafe(format!(
                "UPDATE {} SET sort = $1 WHERE id = $2",
                table(update.kind)
            ));
            sqlx::query(sql)
                .bind(update.sort)
                .bind(update.id)
                .execute(&mut *tx)
                .await
                .map_err(|error| repository_error("server.sort.update", error))?;
        }
        tx.commit()
            .await
            .map_err(|error| repository_error("server.sort.commit", error))
    }

    async fn create_server(
        &self,
        kind: ServerKind,
        write: PreparedServerWrite,
    ) -> Result<Result<i32, ServerPersistenceOutcome>, RepositoryError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| repository_error("server.create.begin", error))?;
        if !lock_groups(&mut tx, &write.group_ids)
            .await
            .map_err(|error| repository_error("server.create.groups", error))?
        {
            return Ok(Err(ServerPersistenceOutcome::ServerGroupNotFound));
        }
        let table = table(kind);
        let mut query = QueryBuilder::<Postgres>::new(format!("INSERT INTO {table} ("));
        {
            let mut columns = query.separated(", ");
            for (column, _) in &write.values {
                columns.push(format!("\"{column}\""));
            }
            columns.push("created_at");
            columns.push("updated_at");
        }
        query.push(") VALUES (");
        {
            let mut values = query.separated(", ");
            for (column, value) in &write.values {
                push_value_separated(&mut values, column, value);
            }
            values.push_bind(write.updated_at);
            values.push_bind(write.updated_at);
        }
        query.push(") RETURNING id");
        let id = query
            .build_query_scalar::<i32>()
            .fetch_one(&mut *tx)
            .await
            .map_err(|error| repository_error("server.create.insert", error))?;
        upsert_credential(&mut tx, kind, id, write.rotate_credential, write.updated_at)
            .await
            .map_err(|error| repository_error("server.create.credential", error))?;
        tx.commit()
            .await
            .map_err(|error| repository_error("server.create.commit", error))?;
        Ok(Ok(id))
    }

    async fn patch_server(
        &self,
        kind: ServerKind,
        id: i32,
        write: PreparedServerWrite,
    ) -> Result<ServerPersistenceOutcome, RepositoryError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| repository_error("server.patch.begin", error))?;
        if !write.group_ids.is_empty()
            && !lock_groups(&mut tx, &write.group_ids)
                .await
                .map_err(|error| repository_error("server.patch.groups", error))?
        {
            return Ok(ServerPersistenceOutcome::ServerGroupNotFound);
        }
        let table = table(kind);
        let exists = if write.values.is_empty() {
            sqlx::query_scalar::<_, i32>(AssertSqlSafe(format!(
                "SELECT id FROM {table} WHERE id = $1"
            )))
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|error| repository_error("server.patch.exists", error))?
            .is_some()
        } else {
            let mut query = QueryBuilder::<Postgres>::new(format!("UPDATE {table} SET "));
            for (index, (column, value)) in write.values.iter().enumerate() {
                if index > 0 {
                    query.push(", ");
                }
                query.push(format!("\"{column}\" = "));
                push_value(&mut query, column, value);
            }
            query
                .push(", updated_at = ")
                .push_bind(write.updated_at)
                .push(" WHERE id = ")
                .push_bind(id);
            query
                .build()
                .execute(&mut *tx)
                .await
                .map_err(|error| repository_error("server.patch.update", error))?
                .rows_affected()
                == 1
        };
        if !exists {
            return Ok(ServerPersistenceOutcome::ServerNotFound);
        }
        upsert_credential(&mut tx, kind, id, write.rotate_credential, write.updated_at)
            .await
            .map_err(|error| repository_error("server.patch.credential", error))?;
        tx.commit()
            .await
            .map_err(|error| repository_error("server.patch.commit", error))?;
        Ok(ServerPersistenceOutcome::Applied)
    }

    async fn delete_server(
        &self,
        kind: ServerKind,
        id: i32,
    ) -> Result<ServerPersistenceOutcome, RepositoryError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| repository_error("server.delete.begin", error))?;
        let result = sqlx::query(AssertSqlSafe(format!(
            "DELETE FROM {} WHERE id = $1",
            table(kind)
        )))
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|error| repository_error("server.delete.row", error))?;
        if result.rows_affected() == 0 {
            return Ok(ServerPersistenceOutcome::ServerNotFound);
        }
        sqlx::query("DELETE FROM server_credential WHERE node_type = $1 AND node_id = $2")
            .bind(kind.as_str())
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|error| repository_error("server.delete.credential", error))?;
        tx.commit()
            .await
            .map_err(|error| repository_error("server.delete.commit", error))?;
        Ok(ServerPersistenceOutcome::Applied)
    }

    async fn copy_server(
        &self,
        kind: ServerKind,
        id: i32,
        now: i64,
    ) -> Result<Result<i32, ServerPersistenceOutcome>, RepositoryError> {
        let table = table(kind);
        let group_json: Option<Json<Value>> = sqlx::query_scalar(AssertSqlSafe(format!(
            "SELECT group_id FROM {table} WHERE id = $1"
        )))
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| repository_error("server.copy.source", error))?;
        let Some(group_json) = group_json else {
            return Ok(Err(ServerPersistenceOutcome::ServerNotFound));
        };
        let group_ids = positive_group_ids(&group_json.0)
            .map_err(|error| repository_error("server.copy.groups", error))?;
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| repository_error("server.copy.begin", error))?;
        if !lock_groups(&mut tx, &group_ids)
            .await
            .map_err(|error| repository_error("server.copy.lock_groups", error))?
        {
            return Ok(Err(ServerPersistenceOutcome::ServerGroupNotFound));
        }
        let columns = copy_columns(kind);
        let quoted = columns
            .iter()
            .map(|column| format!("\"{column}\""))
            .collect::<Vec<_>>();
        let selected = columns
            .iter()
            .map(|column| {
                if *column == "show" {
                    "0::SMALLINT".to_string()
                } else {
                    format!("\"{column}\"")
                }
            })
            .collect::<Vec<_>>();
        let sql = AssertSqlSafe(format!(
            "INSERT INTO {table} ({}, created_at, updated_at) SELECT {}, created_at, updated_at FROM {table} WHERE id = $1 AND group_id = $2 RETURNING id",
            quoted.join(", "),
            selected.join(", ")
        ));
        let copied = sqlx::query_scalar::<_, i32>(sql)
            .bind(id)
            .bind(group_json)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|error| repository_error("server.copy.insert", error))?;
        let Some(copied) = copied else {
            return Ok(Err(ServerPersistenceOutcome::ServerNotFound));
        };
        upsert_credential(&mut tx, kind, copied, false, now)
            .await
            .map_err(|error| repository_error("server.copy.credential", error))?;
        tx.commit()
            .await
            .map_err(|error| repository_error("server.copy.commit", error))?;
        Ok(Ok(copied))
    }
}

async fn delete_group(pool: &PgPool, id: i32) -> Result<DeleteGroupOutcome, RepositoryError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|error| repository_error("server.delete_group.begin", error))?;
    let exists = sqlx::query_scalar::<_, i32>("SELECT id FROM server_group WHERE id = $1")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| repository_error("server.delete_group.exists", error))?;
    if exists.is_none() {
        return Ok(DeleteGroupOutcome::NotFound);
    }
    if let Some(reference) = find_group_reference(&mut tx, id, true)
        .await
        .map_err(|error| repository_error("server.delete_group.preflight", error))?
    {
        return Ok(DeleteGroupOutcome::InUse(reference));
    }
    let locked =
        sqlx::query_scalar::<_, i32>("SELECT id FROM server_group WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|error| repository_error("server.delete_group.lock", error))?;
    if locked.is_none() {
        return Ok(DeleteGroupOutcome::NotFound);
    }
    if let Some(reference) = find_group_reference(&mut tx, id, false)
        .await
        .map_err(|error| repository_error("server.delete_group.recheck", error))?
    {
        return Ok(DeleteGroupOutcome::InUse(reference));
    }
    match sqlx::query("DELETE FROM server_group WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
    {
        Ok(done) if done.rows_affected() == 1 => {}
        Ok(_) => return Ok(DeleteGroupOutcome::NotFound),
        Err(error) => {
            if let Some(reference) = group_reference_from_database_error(error.as_database_error())
            {
                return Ok(DeleteGroupOutcome::InUse(reference));
            }
            return Err(repository_error("server.delete_group.row", error));
        }
    }
    match tx.commit().await {
        Ok(()) => Ok(DeleteGroupOutcome::Deleted),
        Err(error) => group_reference_from_database_error(error.as_database_error())
            .map(DeleteGroupOutcome::InUse)
            .ok_or_else(|| repository_error("server.delete_group.commit", error)),
    }
}

async fn find_group_reference(
    tx: &mut Transaction<'_, Postgres>,
    id: i32,
    lock_children: bool,
) -> Result<Option<ServerGroupReference>, sqlx::Error> {
    let lock = if lock_children { " FOR SHARE" } else { "" };
    for kind in ServerKind::ALL {
        let sql = AssertSqlSafe(format!(
            "SELECT id FROM {} WHERE group_id @> jsonb_build_array($1::integer) OR group_id @> jsonb_build_array($1::text) LIMIT 1{lock}",
            table(kind)
        ));
        if sqlx::query_scalar::<_, i32>(sql)
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
            .is_some()
        {
            return Ok(Some(ServerGroupReference::Server));
        }
    }
    for (sql, reference) in [
        (
            format!("SELECT id::bigint FROM plan WHERE group_id = $1 LIMIT 1{lock}"),
            ServerGroupReference::Plan,
        ),
        (
            format!("SELECT id::bigint FROM users WHERE group_id = $1 LIMIT 1{lock}"),
            ServerGroupReference::User,
        ),
    ] {
        if sqlx::query_scalar::<_, i64>(AssertSqlSafe(sql))
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
            .is_some()
        {
            return Ok(Some(reference));
        }
    }
    Ok(None)
}

fn group_reference_from_database_error(
    error: Option<&dyn sqlx::error::DatabaseError>,
) -> Option<ServerGroupReference> {
    let error = error?.try_downcast_ref::<PgDatabaseError>()?;
    if error.code() != "23503" {
        return None;
    }
    Some(match error.constraint() {
        Some("plan_group_id_fkey") => ServerGroupReference::Plan,
        Some("users_group_id_fkey") => ServerGroupReference::User,
        _ => ServerGroupReference::Unknown,
    })
}

async fn lock_groups(tx: &mut Transaction<'_, Postgres>, ids: &[i32]) -> Result<bool, sqlx::Error> {
    let mut found = 0;
    for chunk in ids.chunks(GROUP_LOCK_BATCH) {
        let mut query = QueryBuilder::<Postgres>::new("SELECT id FROM server_group WHERE id IN (");
        let mut separated = query.separated(", ");
        for id in chunk {
            separated.push_bind(*id);
        }
        separated.push_unseparated(") ORDER BY id FOR SHARE");
        found += query
            .build_query_scalar::<i32>()
            .fetch_all(&mut **tx)
            .await?
            .len();
    }
    Ok(found == ids.len())
}

async fn upsert_credential(
    tx: &mut Transaction<'_, Postgres>,
    kind: ServerKind,
    id: i32,
    rotate: bool,
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(r#"INSERT INTO server_credential (node_type, node_id, credential_epoch, updated_at) VALUES ($1, $2, 0, $3) ON CONFLICT (node_type, node_id) DO UPDATE SET credential_epoch = CASE WHEN $4 THEN server_credential.credential_epoch + 1 ELSE server_credential.credential_epoch END, updated_at = EXCLUDED.updated_at"#)
        .bind(kind.as_str()).bind(id).bind(now).bind(rotate).execute(&mut **tx).await?;
    Ok(())
}

fn integer_cast(column: &str) -> &'static str {
    match column {
        "allow_insecure" | "disable_sni" | "insecure" | "show" | "tls" | "zero_rtt_handshake" => {
            " AS SMALLINT)"
        }
        "down_mbps" | "parent_id" | "port" | "server_port" | "sort" | "up_mbps" | "version" => {
            " AS INTEGER)"
        }
        _ => " AS BIGINT)",
    }
}

fn push_value_separated(
    query: &mut sqlx::query_builder::Separated<'_, Postgres, &str>,
    column: &str,
    value: &ServerColumnValue,
) {
    match value {
        ServerColumnValue::Text(value) => {
            query.push_bind(value.clone());
        }
        ServerColumnValue::Integer(value) => {
            query.push("CAST(");
            query.push_bind_unseparated(*value);
            query.push_unseparated(integer_cast(column));
        }
        ServerColumnValue::Structured(value) => {
            query.push_bind(value.as_ref().map(|value| Json(setting_to_json(value))));
        }
    }
}

fn push_value(query: &mut QueryBuilder<Postgres>, column: &str, value: &ServerColumnValue) {
    match value {
        ServerColumnValue::Text(value) => {
            query.push_bind(value.clone());
        }
        ServerColumnValue::Integer(value) => {
            query.push("CAST(");
            query.push_bind(*value);
            query.push(integer_cast(column));
        }
        ServerColumnValue::Structured(value) => {
            query.push_bind(value.as_ref().map(|value| Json(setting_to_json(value))));
        }
    }
}

fn route_from_row(row: RouteRow) -> Result<ServerRoute, RepositoryError> {
    let action = ServerRouteAction::try_from(row.action.as_str())
        .map_err(|_| repository_error("server.routes.decode", "stored route action is invalid"))?;
    let action_value = match row.action_value.map(|value| value.0) {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => Some(value),
        Some(_) => {
            return Err(repository_error(
                "server.routes.decode",
                "stored action_value is not a string",
            ));
        }
    };
    Ok(ServerRoute {
        id: row.id,
        remarks: row.remarks,
        match_rules: row.match_rules.0,
        action,
        action_value,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn setting_to_json(value: &ServerSettingValue) -> Value {
    match value {
        ServerSettingValue::Null => Value::Null,
        ServerSettingValue::Bool(value) => Value::Bool(*value),
        ServerSettingValue::Integer(value) => Value::from(*value),
        ServerSettingValue::Decimal(value) => value
            .parse::<serde_json::Number>()
            .map(Value::Number)
            .unwrap_or_else(|_| Value::String(value.clone())),
        ServerSettingValue::String(value) => Value::String(value.clone()),
        ServerSettingValue::Array(values) => {
            Value::Array(values.iter().map(setting_to_json).collect())
        }
        ServerSettingValue::Object(values) => Value::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), setting_to_json(value)))
                .collect(),
        ),
    }
}

fn setting_from_json(value: Value) -> ServerSettingValue {
    match value {
        Value::Null => ServerSettingValue::Null,
        Value::Bool(value) => ServerSettingValue::Bool(value),
        Value::Number(value) => value
            .as_i64()
            .map(ServerSettingValue::Integer)
            .unwrap_or_else(|| ServerSettingValue::Decimal(value.to_string())),
        Value::String(value) => ServerSettingValue::String(value),
        Value::Array(values) => {
            ServerSettingValue::Array(values.into_iter().map(setting_from_json).collect())
        }
        Value::Object(values) => ServerSettingValue::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, setting_from_json(value)))
                .collect(),
        ),
    }
}

fn positive_group_ids(value: &Value) -> Result<Vec<i32>, &'static str> {
    let Value::Array(values) = value else {
        return Err("group_id is not an array");
    };
    let mut ids = values
        .iter()
        .map(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str()?.parse().ok())
                .and_then(|id| i32::try_from(id).ok())
                .filter(|id| *id > 0)
        })
        .collect::<Option<Vec<_>>>()
        .ok_or("group_id contains an invalid id")?;
    ids.sort_unstable();
    ids.dedup();
    if ids.is_empty() {
        Err("group_id is empty")
    } else {
        Ok(ids)
    }
}

fn parse_node(value: Value) -> Result<StoredServerNode, &'static str> {
    let Value::Object(mut object) = value else {
        return Err("node row is not an object");
    };
    let kind = take_string(&mut object, "type")?;
    let kind = ServerKind::try_from(kind.as_str()).map_err(|_| "node type is invalid")?;
    let credential_epoch = take_optional_i64(&mut object, "credential_epoch")?;
    let common = ServerNodeCommon {
        id: take_i32(&mut object, "id")?,
        group_id: take_i64_array(&mut object, "group_id")?.ok_or("group_id is null")?,
        route_id: take_i64_array(&mut object, "route_id")?,
        parent_id: take_optional_i32(&mut object, "parent_id")?,
        tags: take_string_array(&mut object, "tags")?,
        name: take_string(&mut object, "name")?,
        rate: take_f64(&mut object, "rate")?,
        host: take_string(&mut object, "host")?,
        port: take_f64(&mut object, "port")?,
        server_port: take_i32(&mut object, "server_port")?,
        show: take_boolish(&mut object, "show")?,
        sort: take_optional_i32(&mut object, "sort")?,
        created_at: take_i64(&mut object, "created_at")?,
        updated_at: take_i64(&mut object, "updated_at")?,
        online: None,
        last_check_at: None,
        last_push_at: None,
        available_status: -1,
        api_key: None,
    };
    let details = match kind {
        ServerKind::Shadowsocks => ServerNodeDetails::Shadowsocks {
            cipher: take_string(&mut object, "cipher")?,
            obfs: take_optional_string(&mut object, "obfs")?,
            obfs_settings: take_setting(&mut object, "obfs_settings")?,
        },
        ServerKind::Vmess => ServerNodeDetails::Vmess {
            tls: take_i16(&mut object, "tls")?,
            network: take_string(&mut object, "network")?,
            rules: take_setting(&mut object, "rules")?,
            network_settings: take_setting(&mut object, "networkSettings")?,
            tls_settings: take_setting(&mut object, "tlsSettings")?,
            rule_settings: take_setting(&mut object, "ruleSettings")?,
            dns_settings: take_setting(&mut object, "dnsSettings")?,
        },
        ServerKind::Trojan => ServerNodeDetails::Trojan {
            network: take_optional_string(&mut object, "network")?,
            network_settings: take_setting(&mut object, "network_settings")?,
            allow_insecure: take_boolish(&mut object, "allow_insecure")?,
            server_name: take_optional_string(&mut object, "server_name")?,
        },
        ServerKind::Tuic => ServerNodeDetails::Tuic {
            server_name: take_optional_string(&mut object, "server_name")?,
            insecure: take_boolish(&mut object, "insecure")?,
            disable_sni: take_boolish(&mut object, "disable_sni")?,
            udp_relay_mode: take_optional_string(&mut object, "udp_relay_mode")?,
            zero_rtt_handshake: take_boolish(&mut object, "zero_rtt_handshake")?,
            congestion_control: take_optional_string(&mut object, "congestion_control")?,
        },
        ServerKind::Hysteria => ServerNodeDetails::Hysteria {
            version: take_i32(&mut object, "version")?,
            up_mbps: take_i32(&mut object, "up_mbps")?,
            down_mbps: take_i32(&mut object, "down_mbps")?,
            obfs: take_optional_string(&mut object, "obfs")?,
            obfs_password: take_optional_string(&mut object, "obfs_password")?,
            server_name: take_optional_string(&mut object, "server_name")?,
            insecure: take_boolish(&mut object, "insecure")?,
        },
        ServerKind::Vless => ServerNodeDetails::Vless {
            tls: take_i16(&mut object, "tls")?,
            tls_settings: take_setting(&mut object, "tls_settings")?,
            flow: take_optional_string(&mut object, "flow")?,
            network: take_string(&mut object, "network")?,
            network_settings: take_setting(&mut object, "network_settings")?,
            encryption: take_optional_string(&mut object, "encryption")?,
            encryption_settings: take_setting(&mut object, "encryption_settings")?,
        },
        ServerKind::Anytls => ServerNodeDetails::Anytls {
            server_name: take_optional_string(&mut object, "server_name")?,
            insecure: take_boolish(&mut object, "insecure")?,
            padding_scheme: take_setting(&mut object, "padding_scheme")?,
        },
        ServerKind::V2node => ServerNodeDetails::V2node {
            listen_ip: take_string(&mut object, "listen_ip")?,
            protocol: take_string(&mut object, "protocol")?,
            tls: take_i16(&mut object, "tls")?,
            tls_settings: take_setting(&mut object, "tls_settings")?,
            flow: take_optional_string(&mut object, "flow")?,
            network: take_string(&mut object, "network")?,
            network_settings: take_setting(&mut object, "network_settings")?,
            encryption: take_optional_string(&mut object, "encryption")?,
            encryption_settings: take_setting(&mut object, "encryption_settings")?,
            disable_sni: take_boolish(&mut object, "disable_sni")?,
            udp_relay_mode: take_optional_string(&mut object, "udp_relay_mode")?,
            zero_rtt_handshake: take_boolish(&mut object, "zero_rtt_handshake")?,
            congestion_control: take_optional_string(&mut object, "congestion_control")?,
            cipher: take_optional_string(&mut object, "cipher")?,
            up_mbps: take_i32(&mut object, "up_mbps")?,
            down_mbps: take_i32(&mut object, "down_mbps")?,
            obfs: take_optional_string(&mut object, "obfs")?,
            obfs_password: take_optional_string(&mut object, "obfs_password")?,
            padding_scheme: take_setting(&mut object, "padding_scheme")?,
            install_command: String::new(),
        },
    };
    Ok(StoredServerNode {
        node: ServerNode { common, details },
        credential_epoch,
    })
}

fn take(object: &mut Map<String, Value>, key: &'static str) -> Result<Value, &'static str> {
    object.remove(key).ok_or(key)
}
fn take_i64(object: &mut Map<String, Value>, key: &'static str) -> Result<i64, &'static str> {
    take(object, key)?.as_i64().ok_or(key)
}
fn take_i32(object: &mut Map<String, Value>, key: &'static str) -> Result<i32, &'static str> {
    i32::try_from(take_i64(object, key)?).map_err(|_| key)
}
fn take_i16(object: &mut Map<String, Value>, key: &'static str) -> Result<i16, &'static str> {
    i16::try_from(take_i64(object, key)?).map_err(|_| key)
}
fn take_optional_i64(
    object: &mut Map<String, Value>,
    key: &'static str,
) -> Result<Option<i64>, &'static str> {
    match take(object, key)? {
        Value::Null => Ok(None),
        value => value.as_i64().map(Some).ok_or(key),
    }
}
fn take_optional_i32(
    object: &mut Map<String, Value>,
    key: &'static str,
) -> Result<Option<i32>, &'static str> {
    take_optional_i64(object, key)?
        .map(i32::try_from)
        .transpose()
        .map_err(|_| key)
}
fn take_string(object: &mut Map<String, Value>, key: &'static str) -> Result<String, &'static str> {
    take(object, key)?
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or(key)
}
fn take_optional_string(
    object: &mut Map<String, Value>,
    key: &'static str,
) -> Result<Option<String>, &'static str> {
    match take(object, key)? {
        Value::Null => Ok(None),
        Value::String(value) => Ok(Some(value)),
        _ => Err(key),
    }
}
fn take_f64(object: &mut Map<String, Value>, key: &'static str) -> Result<f64, &'static str> {
    match take(object, key)? {
        Value::Number(value) => value.as_f64().ok_or(key),
        Value::String(value) => value
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .ok_or(key),
        _ => Err(key),
    }
}
fn take_boolish(object: &mut Map<String, Value>, key: &'static str) -> Result<bool, &'static str> {
    match take(object, key)? {
        Value::Bool(value) => Ok(value),
        Value::Number(value) => value.as_i64().map(|value| value != 0).ok_or(key),
        _ => Err(key),
    }
}
fn take_setting(
    object: &mut Map<String, Value>,
    key: &'static str,
) -> Result<Option<ServerSettingValue>, &'static str> {
    match take(object, key)? {
        Value::Null => Ok(None),
        value => Ok(Some(setting_from_json(value))),
    }
}
fn take_i64_array(
    object: &mut Map<String, Value>,
    key: &'static str,
) -> Result<Option<Vec<i64>>, &'static str> {
    match take(object, key)? {
        Value::Null => Ok(None),
        Value::Array(values) => values
            .into_iter()
            .map(|value| {
                value
                    .as_i64()
                    .or_else(|| value.as_str()?.parse().ok())
                    .ok_or(key)
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        _ => Err(key),
    }
}
fn take_string_array(
    object: &mut Map<String, Value>,
    key: &'static str,
) -> Result<Option<Vec<String>>, &'static str> {
    match take(object, key)? {
        Value::Null => Ok(None),
        Value::Array(values) => values
            .into_iter()
            .map(|value| value.as_str().map(ToOwned::to_owned).ok_or(key))
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        _ => Err(key),
    }
}

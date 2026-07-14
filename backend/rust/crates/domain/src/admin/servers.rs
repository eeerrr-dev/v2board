use super::*;

const SERVER_GROUP_LOCK_BATCH_SIZE: usize = 500;

fn parse_server_group_ids(raw: &str) -> Result<Vec<i64>, ApiError> {
    let Value::Array(values) = serde_json::from_str::<Value>(raw)
        .map_err(|_| ApiError::validation_field("group_id", "节点组格式不正确"))?
    else {
        return Err(ApiError::validation_field("group_id", "节点组格式不正确"));
    };
    let mut ids = Vec::with_capacity(values.len());
    for value in values {
        let id = value
            .as_i64()
            .or_else(|| value.as_str().and_then(|value| value.parse::<i64>().ok()))
            .filter(|id| *id > 0)
            .ok_or_else(|| ApiError::validation_field("group_id", "节点组格式不正确"))?;
        ids.push(id);
    }
    ids.sort_unstable();
    ids.dedup();
    if ids.is_empty() {
        return Err(ApiError::validation_field("group_id", "节点组不能为空"));
    }
    Ok(ids)
}

fn requested_server_group_ids(params: &HashMap<String, String>) -> Result<Vec<i64>, ApiError> {
    parse_server_group_ids(&required_json_array_string(params, "group_id")?)
}

async fn lock_server_groups(tx: &mut DbTransaction<'_>, group_ids: &[i64]) -> Result<(), ApiError> {
    let mut found = 0_usize;
    for chunk in group_ids.chunks(SERVER_GROUP_LOCK_BATCH_SIZE) {
        let mut builder =
            QueryBuilder::<Postgres>::new("SELECT id::bigint FROM server_group WHERE id IN (");
        let mut ids = builder.separated(", ");
        for id in chunk {
            ids.push_bind(*id);
        }
        ids.push_unseparated(") ORDER BY id FOR SHARE");
        found += builder
            .build_query_scalar::<i64>()
            .fetch_all(&mut **tx)
            .await?
            .len();
    }
    if found != group_ids.len() {
        return Err(ApiError::legacy("节点组不存在"));
    }
    Ok(())
}

async fn server_table_uses_group(
    tx: &mut DbTransaction<'_>,
    table: &str,
    group_id: i64,
) -> Result<bool, ApiError> {
    let sql = AssertSqlSafe(format!(
        "SELECT id::bigint FROM {table} \
         WHERE group_id @> jsonb_build_array($1::bigint)
            OR group_id @> jsonb_build_array($1::text) \
         LIMIT 1 FOR SHARE"
    ));
    Ok(sqlx::query_scalar::<_, i64>(sql)
        .bind(group_id)
        .fetch_optional(&mut **tx)
        .await?
        .is_some())
}

impl AdminService {
    pub(super) async fn server_drop(
        &self,
        path: &str,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let table = server_table_from_path(path)?;
        let kind = server_kind_from_path(path)?;
        let id = required_i64(params, "id")?;
        let mut tx = self.db.begin().await?;
        let result = sqlx::query(AssertSqlSafe(format!("DELETE FROM {table} WHERE id = $1")))
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if result.rows_affected() == 0 {
            return Err(ApiError::legacy("节点ID不存在"));
        }
        sqlx::query("DELETE FROM server_credential WHERE node_type = $1 AND node_id = $2")
            .bind(kind)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn server_group_drop(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // Reject while any node, plan, or user still references the group.  The
        // group row is the serialization point: node/plan writers first take a
        // shared group lock, so none can create a late reference after these
        // checks and before the delete.
        let id = required_i64(params, "id")?;
        let mut tx = self.db.begin().await?;
        let exists: Option<i32> =
            sqlx::query_scalar("SELECT id FROM server_group WHERE id = $1 LIMIT 1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if exists.is_none() {
            return Err(ApiError::legacy("组不存在"));
        }
        for (_, table) in SERVER_TABLES {
            if server_table_uses_group(&mut tx, table, id).await? {
                return Err(ApiError::legacy("该组已被节点所使用，无法删除"));
            }
        }
        let plan_used: Option<i32> =
            sqlx::query_scalar("SELECT id FROM plan WHERE group_id = $1 LIMIT 1 FOR SHARE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if plan_used.is_some() {
            return Err(ApiError::legacy("该组已被订阅所使用，无法删除"));
        }
        let user_used: Option<i64> =
            sqlx::query_scalar("SELECT id FROM users WHERE group_id = $1 LIMIT 1 FOR SHARE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if user_used.is_some() {
            return Err(ApiError::legacy("该组已被用户所使用，无法删除"));
        }
        let deleted = sqlx::query("DELETE FROM server_group WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if deleted.rows_affected() != 1 {
            return Err(ApiError::legacy("组不存在"));
        }
        tx.commit().await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    /// Loads the raw `group_id` JSON of every configured server across all node
    /// tables, for the group `server_count` / drop-guard membership checks.
    async fn all_server_group_ids(&self) -> Result<Vec<String>, ApiError> {
        let mut group_ids = Vec::new();
        for (_, table) in SERVER_TABLES {
            let rows: Vec<String> =
                sqlx::query_scalar(AssertSqlSafe(format!("SELECT group_id::text FROM {table}")))
                    .fetch_all(&self.db)
                    .await?;
            group_ids.extend(rows);
        }
        Ok(group_ids)
    }

    pub(super) async fn server_group_fetch(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // GroupController::fetch short-circuits when group_id is truthy, returning the
        // single raw group wrapped in a one-element array (no user_count/server_count
        // enrichment, `[null]` when the id is not found).
        if let Some(group_id) = optional_i64(params, "group_id").filter(|id| *id != 0) {
            let group = fetch_json_one(
                &self.db,
                r#"
                SELECT jsonb_build_object(
                    'id', id, 'name', name, 'created_at', created_at, 'updated_at', updated_at
                )
                FROM server_group
                WHERE id = $1
                LIMIT 1
                "#,
                group_id,
            )
            .await?
            .unwrap_or(Value::Null);
            return Ok(AdminOutput::Data(json!([group])));
        }
        // server_count counts nodes whose group_id array includes the group,
        // mirroring GroupController::fetch over ServerService::getAllServers.
        // Laravel returns ServerGroup::get() in natural (id-ascending) order.
        let mut groups = fetch_json_list(
            &self.db,
            r#"
            SELECT jsonb_build_object(
                'id', id, 'name', name, 'created_at', created_at, 'updated_at', updated_at,
                'user_count', (SELECT COUNT(*) FROM users WHERE group_id = server_group.id),
                'server_count', 0
            )
            FROM server_group
            ORDER BY id ASC
            "#,
        )
        .await?;
        let group_ids = self.all_server_group_ids().await?;
        for group in &mut groups {
            let Some(object) = group.as_object_mut() else {
                continue;
            };
            let id = object.get("id").and_then(Value::as_i64).unwrap_or_default();
            let count = group_ids
                .iter()
                .filter(|group_id| group_id_contains(group_id, id))
                .count() as i64;
            object.insert("server_count".to_string(), json!(count));
        }
        Ok(AdminOutput::Data(json!(groups)))
    }

    pub(super) async fn server_group_save(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let now = Utc::now().timestamp();
        if let Some(id) = optional_i64(params, "id") {
            sqlx::query("UPDATE server_group SET name = $1, updated_at = $2 WHERE id = $3")
                .bind(required_string(params, "name")?)
                .bind(now)
                .bind(id)
                .execute(&self.db)
                .await?;
        } else {
            sqlx::query(
                "INSERT INTO server_group (name, created_at, updated_at) VALUES ($1, $2, $3)",
            )
            .bind(required_string(params, "name")?)
            .bind(now)
            .bind(now)
            .execute(&self.db)
            .await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn server_route_fetch(&self) -> Result<AdminOutput, ApiError> {
        Ok(AdminOutput::Data(json!(
            fetch_json_list(
                &self.db,
                r#"
            SELECT jsonb_build_object(
                'id', id, 'remarks', remarks, 'match', CAST("match" AS JSONB),
                'action', action, 'action_value', action_value,
                'created_at', created_at, 'updated_at', updated_at
            )
            FROM server_route
            ORDER BY id ASC
            "#
            )
            .await?
        )))
    }

    pub(super) async fn server_route_save(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        if let Some(error) = route_save_validation(params) {
            return Err(error);
        }
        let now = Utc::now().timestamp();
        // RouteController::save: `default_out` forces an empty match set, everything
        // else array_filter()s out empty match entries before json_encode.
        let action = required_string(params, "action")?;
        let matches = if action == "default_out" {
            Value::Array(Vec::new())
        } else {
            let filtered = route_match_values(params)
                .into_iter()
                .filter(|value| !php_falsy(value))
                .collect::<Vec<_>>();
            Value::Array(filtered)
        };
        let action_value = optional_string(params, "action_value").map(Value::String);
        if let Some(id) = optional_i64(params, "id") {
            sqlx::query(
                "UPDATE server_route SET remarks = $1, \"match\" = $2, action = $3, action_value = $4, updated_at = $5 WHERE id = $6",
            )
            .bind(required_string(params, "remarks")?)
            .bind(Json(matches))
            .bind(&action)
            .bind(action_value.clone().map(Json))
            .bind(now)
            .bind(id)
            .execute(&self.db)
            .await?;
        } else {
            sqlx::query(
                "INSERT INTO server_route (remarks, \"match\", action, action_value, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(required_string(params, "remarks")?)
            .bind(Json(matches))
            .bind(&action)
            .bind(action_value.map(Json))
            .bind(now)
            .bind(now)
            .execute(&self.db)
            .await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn server_nodes(&self) -> Result<AdminOutput, ApiError> {
        // Ports ServerService::getAllServers (:424-440): each getAll<Protocol> getter
        // returns every model column (with array casts applied) plus `type`, ordered by
        // sort; the tables are concatenated in SERVER_TABLES order and later stable-sorted.
        let mut nodes = Vec::new();
        for (kind, table) in SERVER_TABLES {
            let rows = fetch_json_list(&self.db, &server_node_select(kind, table)).await?;
            nodes.extend(rows);
        }
        // getAllV2node (:381-405) appends a node install script per v2node using the
        // node API host (server_api_url ?? app_url) and token, shell-escaped.
        let install_api_host = self
            .config
            .server_api_url
            .clone()
            .or_else(|| self.config.app_url.clone())
            .unwrap_or_default();
        let credential_rows = sqlx::query_as::<_, (String, i32, i64)>(
            "SELECT node_type, node_id, credential_epoch FROM server_credential",
        )
        .fetch_all(&self.db)
        .await?
        .into_iter()
        .map(|(node_type, node_id, epoch)| ((node_type, i64::from(node_id)), epoch))
        .collect::<HashMap<_, _>>();
        let credential_master = self.config.server_token.as_deref().unwrap_or_default();
        // Hydrate node health from the cache keys the node API writes, keyed on
        // `parent_id ?? id`. Ports ServerService::mergeData (:407-421); the read is
        // best-effort so a Redis outage still returns the node list. Fetch every
        // field with one MGET instead of issuing three sequential round trips per
        // node.
        let identities = nodes
            .iter()
            .map(|node| {
                let node_type = node
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_ascii_uppercase();
                let id = node.get("id").and_then(Value::as_i64).unwrap_or_default();
                let check_id = node.get("parent_id").and_then(Value::as_i64).unwrap_or(id);
                (node_type, id, check_id)
            })
            .collect::<Vec<_>>();
        let health_keys = identities
            .iter()
            .flat_map(|(node_type, _, check_id)| {
                [
                    self.redis_key(&format!("SERVER_{node_type}_ONLINE_USER_{check_id}")),
                    self.redis_key(&format!("SERVER_{node_type}_LAST_CHECK_AT_{check_id}")),
                    self.redis_key(&format!("SERVER_{node_type}_LAST_PUSH_AT_{check_id}")),
                ]
            })
            .collect::<Vec<_>>();
        let mut health_values = vec![None; health_keys.len()];
        if !health_keys.is_empty() {
            match self.redis.get_multiplexed_async_connection().await {
                Ok(mut conn) => {
                    let mut malformed = 0_usize;
                    for (batch_index, keys) in health_keys.chunks(REDIS_MGET_BATCH_SIZE).enumerate()
                    {
                        match conn.mget::<_, Vec<Option<String>>>(keys).await {
                            Ok(values) => {
                                let offset = batch_index * REDIS_MGET_BATCH_SIZE;
                                for (index, value) in values.into_iter().enumerate() {
                                    health_values[offset + index] =
                                        value.and_then(|value| match value.parse::<i64>() {
                                            Ok(value) => Some(value),
                                            Err(_) => {
                                                malformed += 1;
                                                None
                                            }
                                        });
                                }
                            }
                            Err(error) => {
                                tracing::warn!(
                                    ?error,
                                    "admin server health-cache batch read failed"
                                );
                                break;
                            }
                        }
                    }
                    if malformed > 0 {
                        tracing::warn!(
                            malformed,
                            "admin server health cache contained invalid integers"
                        );
                    }
                }
                Err(error) => {
                    tracing::warn!(?error, "admin server health-cache connection unavailable");
                }
            }
        }
        let now = Utc::now().timestamp();
        for ((node, (node_type, id, _)), health) in nodes
            .iter_mut()
            .zip(identities)
            .zip(health_values.chunks_exact(3))
        {
            let Some(object) = node.as_object_mut() else {
                continue;
            };
            let [online, last_check_at, last_push_at] = health else {
                unreachable!("each server health cache tuple has exactly three values")
            };
            // ServerService::mergeData (:407-421) sets exactly these four cache-derived
            // fields keyed on parent_id ?? id; it does not add is_online.
            let available_status = node_available_status(now, *last_check_at, *last_push_at);
            object.insert("online".to_string(), json!(online));
            object.insert("last_check_at".to_string(), json!(last_check_at));
            object.insert("last_push_at".to_string(), json!(last_push_at));
            object.insert("available_status".to_string(), json!(available_status));
            let normalized_type = node_type.to_ascii_lowercase();
            let scoped_token = credential_rows
                .get(&(normalized_type.clone(), id))
                .and_then(|epoch| {
                    crate::server_credentials::derive_node_token(
                        credential_master,
                        &normalized_type,
                        i32::try_from(id).ok()?,
                        *epoch,
                    )
                });
            object.insert("api_key".to_string(), json!(scoped_token.as_deref()));
            if node_type == "V2NODE" {
                let install_command = format!(
                    "wget -N https://raw.githubusercontent.com/wyx2685/v2node/master/script/install.sh && bash install.sh --api-host {} --node-id {} --api-key {}",
                    escapeshellarg(&install_api_host),
                    id,
                    escapeshellarg(scoped_token.as_deref().unwrap_or_default())
                );
                object.insert("install_command".to_string(), json!(install_command));
            }
        }
        // array_multisort($tmp, SORT_ASC, $servers) over the `sort` column; PHP 8's
        // sort is stable and treats a null sort as 0, so key null -> 0 and rely on the
        // stable sort to preserve the concatenation tie order.
        nodes.sort_by_key(|node| node.get("sort").and_then(Value::as_i64).unwrap_or(0));
        Ok(AdminOutput::Data(json!(nodes)))
    }

    pub(super) async fn server_save(
        &self,
        path: &str,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let table = server_table_from_path(path)?;
        let kind = server_kind_from_path(path)?;
        let now = Utc::now().timestamp();
        let group_ids = requested_server_group_ids(params)?;
        let values = server_save_values(kind, params)?;
        let rotate_credential = truthy(params.get("rotate_credential"));
        let mut tx = self.db.begin().await?;
        lock_server_groups(&mut tx, &group_ids).await?;
        let node_id = if let Some(id) = optional_i64(params, "id") {
            let mut builder = QueryBuilder::<Postgres>::new(format!("UPDATE {table} SET "));
            let mut first = true;
            for (column, value) in &values {
                if !first {
                    builder.push(", ");
                }
                first = false;
                builder.push(format!("\"{column}\" = "));
                push_admin_sql_bind(&mut builder, column, value);
            }
            builder.push(", \"updated_at\" = ");
            builder.push_bind(now);
            builder.push(" WHERE id = ");
            builder.push_bind(id);
            let result = builder.build().execute(&mut *tx).await?;
            if result.rows_affected() == 0 {
                return Err(ApiError::legacy("服务器不存在"));
            }
            i32::try_from(id).map_err(|_| ApiError::legacy("服务器不存在"))?
        } else {
            let mut builder = QueryBuilder::<Postgres>::new(format!("INSERT INTO {table} ("));
            let mut columns = builder.separated(", ");
            for (column, _) in &values {
                columns.push(format!("\"{column}\""));
            }
            columns.push("\"created_at\"");
            columns.push("\"updated_at\"");
            builder.push(") VALUES (");
            let mut placeholders = builder.separated(", ");
            for (column, value) in &values {
                push_admin_sql_value(&mut placeholders, column, value);
            }
            placeholders.push_bind(now);
            placeholders.push_bind(now);
            builder.push(") RETURNING id");
            builder
                .build_query_scalar::<i32>()
                .fetch_one(&mut *tx)
                .await?
        };
        sqlx::query(
            r#"
            INSERT INTO server_credential
                (node_type, node_id, credential_epoch, updated_at)
            VALUES ($1, $2, 0, $3)
            ON CONFLICT (node_type, node_id) DO UPDATE SET
                credential_epoch = CASE WHEN $4
                    THEN server_credential.credential_epoch + 1
                    ELSE server_credential.credential_epoch END,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(kind)
        .bind(node_id)
        .bind(now)
        .bind(rotate_credential)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn server_copy(
        &self,
        path: &str,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let table = server_table_from_path(path)?;
        let kind = server_kind_from_path(path)?;
        let id = required_i64(params, "id")?;
        let columns = server_copy_columns(kind)?;
        let source_group_ids: Option<String> = sqlx::query_scalar(AssertSqlSafe(format!(
            "SELECT group_id::text FROM {table} WHERE id = $1 LIMIT 1"
        )))
        .bind(id)
        .fetch_optional(&self.db)
        .await?;
        let source_group_ids = source_group_ids.ok_or_else(|| ApiError::legacy("服务器不存在"))?;
        let group_ids = parse_server_group_ids(&source_group_ids)?;
        let mut builder = QueryBuilder::<Postgres>::new(format!("INSERT INTO {table} ("));
        let mut insert_columns = builder.separated(", ");
        for column in columns {
            insert_columns.push(format!("\"{column}\""));
        }
        insert_columns.push("\"created_at\"");
        insert_columns.push("\"updated_at\"");
        builder.push(") SELECT ");
        let mut select_columns = builder.separated(", ");
        for column in columns {
            if *column == "show" {
                select_columns.push("0::SMALLINT");
            } else {
                select_columns.push(format!("\"{column}\""));
            }
        }
        // Laravel's copy replicates the row via create($server->toArray()): because
        // created_at/updated_at are fillable (guarded = ['id']) they are set from the
        // source row, so updateTimestamps() leaves them untouched. Preserve the
        // original timestamps rather than stamping now().
        select_columns.push("\"created_at\"");
        select_columns.push("\"updated_at\"");
        builder.push(format!(" FROM {table} WHERE id = "));
        builder.push_bind(id);
        builder.push(" AND group_id = ");
        builder.push_bind(Json(
            serde_json::from_str::<Value>(&source_group_ids)
                .map_err(|_| ApiError::internal("stored server group_id is invalid"))?,
        ));
        builder.push(" RETURNING id");
        let mut tx = self.db.begin().await?;
        lock_server_groups(&mut tx, &group_ids).await?;
        let node_id = builder
            .build_query_scalar::<i32>()
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| ApiError::legacy("服务器不存在"))?;
        sqlx::query(
            "INSERT INTO server_credential \
             (node_type, node_id, credential_epoch, updated_at) VALUES ($1, $2, 0, $3)",
        )
        .bind(kind)
        .bind(node_id)
        .bind(Utc::now().timestamp())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn server_sort(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        for (key, value) in params {
            let Some((kind, raw_id)) = key.split_once('[') else {
                continue;
            };
            let id = raw_id.trim_end_matches(']');
            let Some((_, table)) = SERVER_TABLES.iter().find(|(item, _)| *item == kind) else {
                continue;
            };
            if let (Ok(id), Ok(sort)) = (id.parse::<i64>(), value.parse::<i64>()) {
                sqlx::query(AssertSqlSafe(format!(
                    "UPDATE {table} SET sort = CAST($1::BIGINT AS INTEGER) WHERE id = $2::BIGINT"
                )))
                .bind(sort)
                .bind(id)
                .execute(&self.db)
                .await?;
            }
        }
        Ok(AdminOutput::Data(json!(true)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_group_references_are_nonempty_positive_and_canonicalized() {
        assert_eq!(parse_server_group_ids(r#"[3,"1",3]"#).unwrap(), vec![1, 3]);
        for invalid in ["[]", "{}", "1", r#"["missing"]"#, "[0]", "[-1]"] {
            assert!(
                parse_server_group_ids(invalid).is_err(),
                "accepted {invalid}"
            );
        }
    }

    #[test]
    fn group_mutations_use_one_lock_protocol_for_every_node_table() {
        let source = include_str!("servers.rs");
        assert!(source.contains("lock_server_groups(&mut tx, &group_ids)"));
        assert!(source.contains("for (_, table) in SERVER_TABLES"));
        assert!(source.contains("group_id @> jsonb_build_array($1::bigint)"));
        assert!(source.contains("LIMIT 1 FOR SHARE"));
    }
}

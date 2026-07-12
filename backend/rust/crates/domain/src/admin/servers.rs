use super::*;

impl AdminService {
    pub(super) async fn server_group_drop(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // Ports GroupController::drop (:58-90): reject while any vmess/vless node,
        // plan, or user still references the group.
        let id = required_i64(params, "id")?;
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT id FROM v2_server_group WHERE id = ? LIMIT 1")
                .bind(id)
                .fetch_optional(&self.db)
                .await?;
        if exists.is_none() {
            return Err(ApiError::legacy("组不存在"));
        }
        for table in ["v2_server_vmess", "v2_server_vless"] {
            let group_ids: Vec<String> =
                sqlx::query_scalar(AssertSqlSafe(format!("SELECT group_id FROM {table}")))
                    .fetch_all(&self.db)
                    .await?;
            if group_ids
                .iter()
                .any(|group_id| group_id_contains(group_id, id))
            {
                return Err(ApiError::legacy("该组已被节点所使用，无法删除"));
            }
        }
        let plan_used: Option<i64> =
            sqlx::query_scalar("SELECT id FROM v2_plan WHERE group_id = ? LIMIT 1")
                .bind(id)
                .fetch_optional(&self.db)
                .await?;
        if plan_used.is_some() {
            return Err(ApiError::legacy("该组已被订阅所使用，无法删除"));
        }
        let user_used: Option<i64> =
            sqlx::query_scalar("SELECT id FROM v2_user WHERE group_id = ? LIMIT 1")
                .bind(id)
                .fetch_optional(&self.db)
                .await?;
        if user_used.is_some() {
            return Err(ApiError::legacy("该组已被用户所使用，无法删除"));
        }
        sqlx::query("DELETE FROM v2_server_group WHERE id = ?")
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    /// Loads the raw `group_id` JSON of every configured server across all node
    /// tables, for the group `server_count` / drop-guard membership checks.
    async fn all_server_group_ids(&self) -> Result<Vec<String>, ApiError> {
        let mut group_ids = Vec::new();
        for (_, table) in SERVER_TABLES {
            let rows: Vec<String> =
                sqlx::query_scalar(AssertSqlSafe(format!("SELECT group_id FROM {table}")))
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
                SELECT JSON_OBJECT(
                    'id', id, 'name', name, 'created_at', created_at, 'updated_at', updated_at
                )
                FROM v2_server_group
                WHERE id = ?
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
            SELECT JSON_OBJECT(
                'id', id, 'name', name, 'created_at', created_at, 'updated_at', updated_at,
                'user_count', (SELECT COUNT(*) FROM v2_user WHERE group_id = v2_server_group.id),
                'server_count', 0
            )
            FROM v2_server_group
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
            sqlx::query("UPDATE v2_server_group SET name = ?, updated_at = ? WHERE id = ?")
                .bind(required_string(params, "name")?)
                .bind(now)
                .bind(id)
                .execute(&self.db)
                .await?;
        } else {
            sqlx::query(
                "INSERT INTO v2_server_group (name, created_at, updated_at) VALUES (?, ?, ?)",
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
            SELECT JSON_OBJECT(
                'id', id, 'remarks', remarks, 'match', CAST(`match` AS JSON),
                'action', action, 'action_value', action_value,
                'created_at', created_at, 'updated_at', updated_at
            )
            FROM v2_server_route
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
            "[]".to_string()
        } else {
            let filtered = route_match_values(params)
                .into_iter()
                .filter(|value| !php_falsy(value))
                .collect::<Vec<_>>();
            json_string(&Value::Array(filtered))
        };
        if let Some(id) = optional_i64(params, "id") {
            sqlx::query(
                "UPDATE v2_server_route SET remarks = ?, `match` = ?, action = ?, action_value = ?, updated_at = ? WHERE id = ?",
            )
            .bind(required_string(params, "remarks")?)
            .bind(matches)
            .bind(&action)
            .bind(optional_string(params, "action_value"))
            .bind(now)
            .bind(id)
            .execute(&self.db)
            .await?;
        } else {
            sqlx::query(
                "INSERT INTO v2_server_route (remarks, `match`, action, action_value, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(required_string(params, "remarks")?)
            .bind(matches)
            .bind(&action)
            .bind(optional_string(params, "action_value"))
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
        let install_api_key = self.config.server_token.clone().unwrap_or_default();
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
                    format!("SERVER_{node_type}_ONLINE_USER_{check_id}"),
                    format!("SERVER_{node_type}_LAST_CHECK_AT_{check_id}"),
                    format!("SERVER_{node_type}_LAST_PUSH_AT_{check_id}"),
                ]
            })
            .collect::<Vec<_>>();
        let mut health_values = vec![None; health_keys.len()];
        if !health_keys.is_empty() {
            match self.redis.get_multiplexed_async_connection().await {
                Ok(mut conn) => match conn.mget::<_, Vec<Option<String>>>(&health_keys).await {
                    Ok(values) => {
                        let mut malformed = 0_usize;
                        health_values = values
                            .into_iter()
                            .map(|value| {
                                value.and_then(|value| match value.parse::<i64>() {
                                    Ok(value) => Some(value),
                                    Err(_) => {
                                        malformed += 1;
                                        None
                                    }
                                })
                            })
                            .collect();
                        if malformed > 0 {
                            tracing::warn!(
                                malformed,
                                "admin server health cache contained invalid integers"
                            );
                        }
                    }
                    Err(error) => {
                        tracing::warn!(?error, "admin server health-cache batch read failed");
                    }
                },
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
            if node_type == "V2NODE" {
                let install_command = format!(
                    "wget -N https://raw.githubusercontent.com/wyx2685/v2node/master/script/install.sh && bash install.sh --api-host {} --node-id {} --api-key {}",
                    escapeshellarg(&install_api_host),
                    id,
                    escapeshellarg(&install_api_key)
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
        let values = server_save_values(kind, params)?;
        if let Some(id) = optional_i64(params, "id") {
            let mut builder = QueryBuilder::<MySql>::new(format!("UPDATE {table} SET "));
            let mut first = true;
            for (column, value) in &values {
                if !first {
                    builder.push(", ");
                }
                first = false;
                builder.push(format!("`{column}` = "));
                push_admin_sql_bind(&mut builder, value);
            }
            builder.push(", `updated_at` = ");
            builder.push_bind(now);
            builder.push(" WHERE id = ");
            builder.push_bind(id);
            let result = builder.build().execute(&self.db).await?;
            if result.rows_affected() == 0 {
                return Err(ApiError::legacy("服务器不存在"));
            }
        } else {
            let mut builder = QueryBuilder::<MySql>::new(format!("INSERT INTO {table} ("));
            let mut columns = builder.separated(", ");
            for (column, _) in &values {
                columns.push(format!("`{column}`"));
            }
            columns.push("`created_at`");
            columns.push("`updated_at`");
            builder.push(") VALUES (");
            let mut placeholders = builder.separated(", ");
            for (_, value) in &values {
                push_admin_sql_value(&mut placeholders, value);
            }
            placeholders.push_bind(now);
            placeholders.push_bind(now);
            builder.push(")");
            builder.build().execute(&self.db).await?;
        }
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
        let mut builder = QueryBuilder::<MySql>::new(format!("INSERT INTO {table} ("));
        let mut insert_columns = builder.separated(", ");
        for column in columns {
            insert_columns.push(format!("`{column}`"));
        }
        insert_columns.push("`created_at`");
        insert_columns.push("`updated_at`");
        builder.push(") SELECT ");
        let mut select_columns = builder.separated(", ");
        for column in columns {
            if *column == "show" {
                select_columns.push("0");
            } else {
                select_columns.push(format!("`{column}`"));
            }
        }
        // Laravel's copy replicates the row via create($server->toArray()): because
        // created_at/updated_at are fillable (guarded = ['id']) they are set from the
        // source row, so updateTimestamps() leaves them untouched. Preserve the
        // original timestamps rather than stamping now().
        select_columns.push("`created_at`");
        select_columns.push("`updated_at`");
        builder.push(format!(" FROM {table} WHERE id = "));
        builder.push_bind(id);
        let result = builder.build().execute(&self.db).await?;
        if result.rows_affected() == 0 {
            return Err(ApiError::legacy("服务器不存在"));
        }
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
                    "UPDATE {table} SET sort = ? WHERE id = ?"
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

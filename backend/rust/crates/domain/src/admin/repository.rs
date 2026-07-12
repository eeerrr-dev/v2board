use super::*;

impl AdminService {
    /// Existence probe mirroring the `Model::find($id)` + `if (!$model) abort(...)`
    /// guard every admin drop/show/update runs before mutating. Each caller supplies
    /// its resource-specific not-found error (message and status differ per resource,
    /// e.g. giftcard is a 404). `table` must already be validated by ensure_safe_table.
    /// A row-value probe keeps not-found behavior independent of UPDATE row counts.
    /// 0 affected rows when the new value equals the current one, which would falsely
    /// read as "not found" for an idempotent show/set-show.
    pub(super) async fn ensure_row_exists(
        &self,
        table: &str,
        id: i64,
        not_found: ApiError,
    ) -> Result<(), ApiError> {
        let exists: Option<i64> = sqlx::query_scalar(AssertSqlSafe(format!(
            "SELECT id::bigint FROM {table} WHERE id = $1"
        )))
        .bind(id)
        .fetch_optional(&self.db)
        .await?;
        if exists.is_none() {
            return Err(not_found);
        }
        Ok(())
    }

    pub(super) async fn delete_by_id(
        &self,
        table: &str,
        id: i64,
        not_found: ApiError,
    ) -> Result<AdminOutput, ApiError> {
        ensure_safe_table(table)?;
        self.ensure_row_exists(table, id, not_found).await?;
        sqlx::query(AssertSqlSafe(format!("DELETE FROM {table} WHERE id = $1")))
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn toggle(
        &self,
        table: &str,
        column: &str,
        id: i64,
        not_found: ApiError,
    ) -> Result<AdminOutput, ApiError> {
        ensure_safe_table(table)?;
        ensure_toggle_column(column)?;
        self.ensure_row_exists(table, id, not_found).await?;
        sqlx::query(AssertSqlSafe(format!(
            "UPDATE {table} SET \"{column}\" = CASE WHEN \"{column}\" = 1 THEN 0::SMALLINT ELSE 1::SMALLINT END, updated_at = $1 WHERE id = $2::BIGINT"
        )))
        .bind(Utc::now().timestamp())
        .bind(id)
        .execute(&self.db)
        .await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn toggle_or_set_show(
        &self,
        table: &str,
        id: i64,
        params: &HashMap<String, String>,
        not_found: ApiError,
    ) -> Result<AdminOutput, ApiError> {
        ensure_safe_table(table)?;
        self.ensure_row_exists(table, id, not_found).await?;
        let show = optional_i64(params, "show").unwrap_or(1);
        sqlx::query(AssertSqlSafe(format!(
            "UPDATE {table} SET \"show\" = CAST($1::BIGINT AS SMALLINT), updated_at = $2 WHERE id = $3::BIGINT"
        )))
        .bind(show)
        .bind(Utc::now().timestamp())
        .bind(id)
        .execute(&self.db)
        .await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn sort_ids(&self, table: &str, ids: &[i64]) -> Result<AdminOutput, ApiError> {
        ensure_safe_table(table)?;
        for (index, id) in ids.iter().enumerate() {
            sqlx::query(AssertSqlSafe(format!(
                "UPDATE {table} SET sort = CAST($1::BIGINT AS INTEGER) WHERE id = $2::BIGINT"
            )))
            .bind((index + 1) as i64)
            .bind(id)
            .execute(&self.db)
            .await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }
}

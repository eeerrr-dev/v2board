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
    ) -> Result<(), ApiError> {
        ensure_safe_table(table)?;
        self.ensure_row_exists(table, id, not_found).await?;
        sqlx::query(AssertSqlSafe(format!("DELETE FROM {table} WHERE id = $1")))
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub(super) async fn sort_ids(&self, table: &str, ids: &[i64]) -> Result<(), ApiError> {
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
        Ok(())
    }

    /// Shared §4.4 PATCH executor: dynamic `UPDATE ... SET ..., updated_at
    /// WHERE id = ?` over the provided columns, reporting a path-identified
    /// miss as the caller's 404 problem. An all-absent body retains every
    /// column but still 404s on a missing id.
    pub(super) async fn patch_row(
        &self,
        table: &str,
        id: i64,
        values: &[(&str, AdminSqlValue)],
        not_found: ApiError,
    ) -> Result<(), ApiError> {
        ensure_safe_table(table)?;
        if values.is_empty() {
            return self.ensure_row_exists(table, id, not_found).await;
        }
        let mut builder = QueryBuilder::<Postgres>::new(format!("UPDATE {table} SET "));
        for (column, value) in values {
            builder.push(format!("\"{column}\" = "));
            push_admin_sql_bind(&mut builder, column, value);
            builder.push(", ");
        }
        builder.push("\"updated_at\" = ");
        builder.push_bind(Utc::now().timestamp());
        builder.push(" WHERE id = ");
        builder.push_bind(id);
        let result = builder.build().execute(&self.db).await?;
        if result.rows_affected() == 0 {
            return Err(not_found);
        }
        Ok(())
    }
}

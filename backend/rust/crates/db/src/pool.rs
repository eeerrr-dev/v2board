use sqlx::{MySqlPool, mysql::MySqlPoolOptions};

pub type DbPool = MySqlPool;

pub async fn connect_mysql(database_url: &str) -> Result<DbPool, sqlx::Error> {
    MySqlPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}

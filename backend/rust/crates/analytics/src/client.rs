/// Build the reusable official HTTP client. URL/database/user validation is
/// owned by `v2board-config`; this function deliberately does not log or expose
/// the supplied secret.
pub fn clickhouse_client(
    url: &str,
    database: &str,
    username: &str,
    password: Option<&str>,
) -> clickhouse::Client {
    let client = clickhouse::Client::default()
        .with_url(url)
        .with_database(database)
        .with_user(username)
        // ClickHouse 26.3 enables async inserts by default. Both the schema
        // ledger and projection protocol require request-scoped durability and
        // visibility before PostgreSQL can advance, so opt out explicitly.
        .with_setting("async_insert", "0")
        .with_setting("wait_end_of_query", "1");
    match password {
        Some(password) => client.with_password(password),
        None => client,
    }
}

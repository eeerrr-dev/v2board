use sqlx::PgPool;
use tokio::task::JoinSet;
use v2board_application::server_management::{
    DeleteGroupOutcome, PreparedServerWrite, ServerColumnValue, ServerManagementRepository,
    ServerPersistenceOutcome, ServerSettingValue, ServerSortUpdate,
};
use v2board_application::server_runtime::ServerRuntimeRepository;
use v2board_db::{
    admin_server::PostgresServerManagementRepository,
    server_runtime::PostgresServerRuntimeRepository,
};
use v2board_domain_model::ServerKind;

static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations-postgres");

// Each test runs against its own throwaway database (sqlx::test creates,
// migrates, and drops it automatically), so tests are safe to run in
// parallel and no longer need hand-written DELETE cleanup.
#[sqlx::test(migrator = "POSTGRES_MIGRATOR")]
#[ignore = "requires DATABASE_URL; run via `make rust-integration`"]
async fn sorting_updates_multiple_protocol_tables_as_one_repository_command(pool: PgPool) {
    let marker = uuid::Uuid::new_v4().simple().to_string();
    let group_id = insert_group(&pool, &marker).await;
    let shadowsocks_id = insert_shadowsocks(&pool, group_id, &format!("ss-{marker}"), 40).await;
    let vmess_id = insert_vmess(&pool, group_id, &format!("vmess-{marker}"), 50).await;
    let repository = PostgresServerManagementRepository::new(pool.clone());

    repository
        .sort_nodes(&[
            ServerSortUpdate {
                kind: ServerKind::Shadowsocks,
                id: shadowsocks_id,
                sort: 2,
            },
            ServerSortUpdate {
                kind: ServerKind::Vmess,
                id: vmess_id,
                sort: 1,
            },
        ])
        .await
        .expect("sort both protocol rows through one repository command");

    let shadowsocks_sort: i32 =
        sqlx::query_scalar("SELECT sort FROM server_shadowsocks WHERE id = $1")
            .bind(shadowsocks_id)
            .fetch_one(&pool)
            .await
            .expect("load shadowsocks sort");
    let vmess_sort: i32 = sqlx::query_scalar("SELECT sort FROM server_vmess WHERE id = $1")
        .bind(vmess_id)
        .fetch_one(&pool)
        .await
        .expect("load vmess sort");
    assert_eq!((shadowsocks_sort, vmess_sort), (2, 1));
}

#[sqlx::test(migrator = "POSTGRES_MIGRATOR")]
#[ignore = "requires DATABASE_URL; run via `make rust-integration`"]
async fn concurrent_group_delete_and_node_create_never_leave_an_orphan(pool: PgPool) {
    for attempt in 0..8 {
        let marker = format!("{}-{attempt}", uuid::Uuid::new_v4().simple());
        let group_id = insert_group(&pool, &marker).await;
        let repository = PostgresServerManagementRepository::new(pool.clone());
        let mut tasks = JoinSet::new();

        let create_repository = repository.clone();
        tasks.spawn(async move {
            let write = shadowsocks_write(group_id, &format!("race-node-{attempt}"));
            (
                "create",
                create_repository
                    .create_server(ServerKind::Shadowsocks, write)
                    .await,
            )
        });
        let delete_repository = repository.clone();
        tasks.spawn(async move {
            let outcome = delete_repository.delete_group(group_id).await;
            (
                "delete",
                outcome.map(|outcome| match outcome {
                    DeleteGroupOutcome::Deleted => Ok(-1),
                    DeleteGroupOutcome::InUse(_) => Err(ServerPersistenceOutcome::Applied),
                    DeleteGroupOutcome::NotFound => Err(ServerPersistenceOutcome::ServerNotFound),
                }),
            )
        });

        let mut create = None;
        let mut delete = None;
        while let Some(result) = tasks.join_next().await {
            let (kind, result) = result.expect("race task completes");
            if kind == "create" {
                create = Some(result.expect("create repository call"));
            } else {
                delete = Some(result.expect("delete repository call"));
            }
        }
        let create = create.expect("create outcome");
        let delete = delete.expect("delete outcome");
        let group_exists: bool =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM server_group WHERE id = $1)")
                .bind(group_id)
                .fetch_one(&pool)
                .await
                .expect("check group after race");
        let node_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM server_shadowsocks WHERE group_id @> jsonb_build_array($1::integer)",
        )
        .bind(group_id)
        .fetch_one(&pool)
        .await
        .expect("check node after race");

        match (group_exists, node_count, create, delete) {
            (true, 1, Ok(_), Err(ServerPersistenceOutcome::Applied)) => {}
            (false, 0, Err(ServerPersistenceOutcome::ServerGroupNotFound), Ok(_)) => {}
            state => panic!("group/node race violated reference integrity: {state:?}"),
        }
    }
}

#[sqlx::test(migrator = "POSTGRES_MIGRATOR")]
#[ignore = "requires DATABASE_URL; run via `make rust-integration`"]
async fn external_runtime_reads_credentials_nodes_routes_and_authorized_users_through_its_port(
    pool: PgPool,
) {
    let marker = uuid::Uuid::new_v4().simple().to_string();
    let group_id = insert_group(&pool, &marker).await;
    let node_id = insert_shadowsocks(&pool, group_id, &format!("runtime-{marker}"), 1).await;
    let route_id: i32 = sqlx::query_scalar(
        "INSERT INTO server_route (remarks, \"match\", action, action_value, created_at, updated_at) \
         VALUES ($1, '[\"example.test\"]'::jsonb, 'block', NULL, 1, 1) RETURNING id",
    )
    .bind(format!("runtime-{marker}"))
    .fetch_one(&pool)
    .await
    .expect("insert runtime route");
    sqlx::query(
        "UPDATE server_shadowsocks SET route_id = jsonb_build_array($1::integer) WHERE id = $2",
    )
    .bind(route_id)
    .bind(node_id)
    .execute(&pool)
    .await
    .expect("bind runtime route");
    sqlx::query(
        "INSERT INTO server_credential (node_type, node_id, credential_epoch, updated_at) \
         VALUES ('shadowsocks', $1, 3, 1)",
    )
    .bind(node_id)
    .execute(&pool)
    .await
    .expect("insert runtime credential");
    let user_id: i64 = sqlx::query_scalar(
        "INSERT INTO users \
         (email, password, uuid, token, group_id, u, d, transfer_enable, device_limit, expired_at, created_at, updated_at) \
         VALUES ($1, 'unused', $2, $3, $4, 1, 2, 100, 1, 100, 1, 1) RETURNING id",
    )
    .bind(format!("runtime-{marker}@example.test"))
    .bind(marker.clone())
    .bind(marker.clone())
    .bind(group_id)
    .fetch_one(&pool)
    .await
    .expect("insert runtime user");

    let repository = PostgresServerRuntimeRepository::new(pool.clone());
    assert_eq!(
        repository
            .credential_epoch(ServerKind::Shadowsocks, node_id)
            .await
            .expect("load credential epoch"),
        Some(3)
    );
    let node = repository
        .node(ServerKind::Shadowsocks, node_id)
        .await
        .expect("load runtime node")
        .expect("runtime node exists");
    assert_eq!(node.group_ids, vec![group_id]);
    assert_eq!(node.route_ids, vec![route_id]);
    assert_eq!(node.cipher.as_deref(), Some("aes-128-gcm"));
    let users = repository
        .available_users(&node.group_ids, 2)
        .await
        .expect("load authorized node users");
    assert_eq!(
        users.iter().map(|user| user.id).collect::<Vec<_>>(),
        vec![user_id]
    );
    let routes = repository
        .routes(&[route_id])
        .await
        .expect("load ordered node routes");
    assert_eq!(
        (routes[0].id, routes[0].action.as_str()),
        (route_id, "block")
    );
    assert!(
        repository
            .alive_user_ids(2)
            .await
            .expect("load alive-list users")
            .contains(&user_id)
    );
}

fn shadowsocks_write(group_id: i32, name: &str) -> PreparedServerWrite {
    PreparedServerWrite {
        values: vec![
            (
                "group_id",
                ServerColumnValue::Structured(Some(ServerSettingValue::Array(vec![
                    ServerSettingValue::Integer(i64::from(group_id)),
                ]))),
            ),
            ("name", ServerColumnValue::Text(Some(name.to_string()))),
            ("rate", ServerColumnValue::Text(Some("1".into()))),
            ("host", ServerColumnValue::Text(Some("race.test".into()))),
            ("port", ServerColumnValue::Text(Some("443".into()))),
            ("server_port", ServerColumnValue::Integer(Some(443))),
            (
                "cipher",
                ServerColumnValue::Text(Some("aes-128-gcm".into())),
            ),
        ],
        group_ids: vec![group_id],
        rotate_credential: false,
        updated_at: 1_700_000_000,
    }
}

async fn insert_group(pool: &PgPool, marker: &str) -> i32 {
    sqlx::query_scalar(
        "INSERT INTO server_group (name, created_at, updated_at) VALUES ($1, 1, 1) RETURNING id",
    )
    .bind(format!("server-management-{marker}"))
    .fetch_one(pool)
    .await
    .expect("insert server group")
}

async fn insert_shadowsocks(pool: &PgPool, group_id: i32, name: &str, sort: i32) -> i32 {
    sqlx::query_scalar(
        r#"
        INSERT INTO server_shadowsocks
            (group_id, name, rate, host, port, server_port, cipher, show, sort, created_at, updated_at)
        VALUES (jsonb_build_array($1::integer), $2, '1', 'ss.test', '443', 443,
                'aes-128-gcm', 1, $3, 1, 1)
        RETURNING id
        "#,
    )
    .bind(group_id)
    .bind(name)
    .bind(sort)
    .fetch_one(pool)
    .await
    .expect("insert shadowsocks node")
}

async fn insert_vmess(pool: &PgPool, group_id: i32, name: &str, sort: i32) -> i32 {
    sqlx::query_scalar(
        r#"
        INSERT INTO server_vmess
            (group_id, name, host, port, server_port, tls, rate, network, show, sort, created_at, updated_at)
        VALUES (jsonb_build_array($1::integer), $2, 'vmess.test', '443', 443, 0,
                '1', 'tcp', 1, $3, 1, 1)
        RETURNING id
        "#,
    )
    .bind(group_id)
    .bind(name)
    .bind(sort)
    .fetch_one(pool)
    .await
    .expect("insert vmess node")
}

use std::{fs, path::Path};

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("domain-model lives under <workspace>/crates")
}

fn source(relative: &str) -> String {
    fs::read_to_string(workspace_root().join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"))
}

#[test]
fn order_http_and_worker_runners_cannot_reach_business_persistence() {
    for relative in [
        "crates/api/src/commerce.rs",
        "crates/workers/src/orders.rs",
        "crates/workers/src/commission.rs",
        "crates/workers/src/renewal.rs",
    ] {
        let source = source(relative);
        for forbidden in ["v2board_db", "sqlx::", "query_as(", "query_scalar("] {
            assert!(
                !source.contains(forbidden),
                "{relative} must call application use cases instead of {forbidden}"
            );
        }
    }
}

#[test]
fn scheduled_order_use_cases_and_postgres_adapters_have_one_way_dependencies() {
    let application = source("crates/application/src/order_jobs.rs");
    for forbidden in ["sqlx", "chrono", "v2board_config", "v2board_api_contract"] {
        assert!(
            !application.contains(forbidden),
            "order application use cases must not depend on {forbidden}"
        );
    }
    assert!(application.contains("pub trait CommissionRepository"));
    assert!(application.contains("pub trait RenewalRepository"));

    let database = source("crates/db/src/order_jobs.rs");
    assert!(database.contains("impl CommissionRepository for PostgresOrderJobsRepository"));
    assert!(database.contains("impl RenewalRepository for PostgresOrderJobsRepository"));
    assert!(database.contains("FOR UPDATE SKIP LOCKED"));

    let composition = source("crates/order-adapters/src/lib.rs");
    assert!(composition.contains("runtime_commission_service"));
    assert!(composition.contains("runtime_renewal_service"));
}

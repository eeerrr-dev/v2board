mod mysql_import;

pub use mysql_import::execute as execute_mysql_import;

/// Test-infrastructure entry point used by the Docker-only browser E2E gate.
///
/// It deliberately reuses the production PostgreSQL and Redis runtime ACL
/// installers. Keeping the entry point here makes the black-box gate exercise
/// the same least-privilege registry as a real import without adding a second
/// grant list or exposing a test command from the shipped lifecycle CLI.
#[cfg(feature = "real-stack-e2e")]
#[doc(hidden)]
pub async fn prepare_real_stack_e2e_from_env() -> anyhow::Result<()> {
    mysql_import::real_stack_e2e::prepare_from_env().await
}

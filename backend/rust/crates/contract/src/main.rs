use anyhow::{Result, bail};

mod golden_responses;
mod production_invariants;
mod route_audit;
mod sql_schema_prepare;
mod worker_reconcile;

#[tokio::main]
async fn main() -> Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("route-audit") => route_audit::run(),
        Some("production-invariants") => production_invariants::run().await,
        Some("golden-responses") => golden_responses::run().await,
        Some("sql-inventory") => sql_schema_prepare::audit_dynamic_inventory(),
        Some("worker-reconcile") => worker_reconcile::run().await,
        Some(command) => {
            bail!(
                "unknown command `{command}`; expected `route-audit`, `production-invariants`, `golden-responses`, `sql-inventory`, or `worker-reconcile`"
            )
        }
        None => bail!(
            "missing command; expected `route-audit`, `production-invariants`, `golden-responses`, `sql-inventory`, or `worker-reconcile`"
        ),
    }
}

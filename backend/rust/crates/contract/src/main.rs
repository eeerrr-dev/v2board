use anyhow::{Result, bail};

mod route_audit;
mod worker_reconcile;

#[tokio::main]
async fn main() -> Result<()> {
    match std::env::args().nth(1).as_deref() {
        Some("route-audit") => route_audit::run(),
        Some("worker-reconcile") => worker_reconcile::run().await,
        Some(command) => {
            bail!("unknown command `{command}`; expected `route-audit` or `worker-reconcile`")
        }
        None => bail!("missing command; expected `route-audit` or `worker-reconcile`"),
    }
}

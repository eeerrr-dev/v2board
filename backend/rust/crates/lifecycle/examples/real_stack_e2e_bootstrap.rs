//! Docker-only entry point for the fixture-free browser E2E environment.
//!
//! Keeping this as an example target prevents the native `v2board-contract`
//! tool (and therefore its production audit graph) from depending on the
//! lifecycle/MySQL installation stack.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    v2board_lifecycle::prepare_real_stack_e2e_from_env().await
}

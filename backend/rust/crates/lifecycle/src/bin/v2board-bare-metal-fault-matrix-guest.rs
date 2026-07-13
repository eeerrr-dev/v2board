#[path = "../matrix_guest.rs"]
mod matrix_guest;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    matrix_guest::main().await
}

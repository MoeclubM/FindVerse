#[tokio::main]
async fn main() -> anyhow::Result<()> {
    findverse_api::run_bootstrap().await
}

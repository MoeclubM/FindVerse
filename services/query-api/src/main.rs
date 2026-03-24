#[tokio::main]
async fn main() -> anyhow::Result<()> {
    findverse_api::run_query_api().await
}

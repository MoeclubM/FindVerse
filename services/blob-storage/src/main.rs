#[tokio::main]
async fn main() -> anyhow::Result<()> {
    findverse_api::run_blob_storage().await
}

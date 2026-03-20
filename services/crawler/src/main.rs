mod discover;
mod extract;
mod fetch;
mod models;
mod worker;

use clap::Parser;
use tracing::info;

use models::{Cli, Command, WorkerConfig};

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Discover {
            config,
            output,
            limit_per_seed,
        } => discover::discover(config, output, limit_per_seed).await?,
        Command::Fetch {
            frontier,
            output_dir,
            limit,
        } => discover::fetch(frontier, output_dir, limit).await?,
        Command::BuildIndex { input_dir, output } => {
            discover::build_index(input_dir, output).await?
        }
        Command::Worker {
            server,
            crawler_id,
            crawler_key,
            api_key,
            join_key,
            max_jobs,
            poll_interval_secs,
            once,
            concurrency,
            allowed_domains,
            proxy,
        } => {
            let mut client_builder = reqwest::Client::builder()
                .user_agent("FindVerseCrawlerWorker/0.1")
                .cookie_store(true);

            if let Some(ref proxy_url) = proxy {
                client_builder = client_builder.proxy(reqwest::Proxy::all(proxy_url)?);
            }

            let client = client_builder.build()?;

            let (resolved_id, auth_token) = match (crawler_id, crawler_key, api_key, join_key) {
                (Some(id), Some(key), None, None) => (id, key),
                (None, None, Some(key), None) => {
                    let hello = worker::crawler_hello(&client, &server, &key).await?;
                    info!(
                        "registered as crawler {} ({})",
                        hello.name, hello.crawler_id
                    );
                    (hello.crawler_id, key)
                }
                (None, None, None, Some(jk)) => {
                    let join = worker::crawler_join(&client, &server, &jk).await?;
                    info!("joined as crawler {} ({})", join.name, join.crawler_id);
                    (join.crawler_id, join.crawler_key)
                }
                _ => anyhow::bail!(
                    "provide --api-key for auto-registration, \
                     --join-key for join-based registration, \
                     or both --crawler-id and --crawler-key for manual setup"
                ),
            };

            let parsed_domains: Vec<String> = allowed_domains
                .map(|d| {
                    d.split(',')
                        .map(|s| s.trim().to_lowercase())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default();

            let config = WorkerConfig {
                server,
                crawler_id: resolved_id,
                auth_token,
                max_jobs,
                poll_interval_secs,
                once,
                concurrency,
                allowed_domains: parsed_domains,
            };
            worker::run_worker(config, proxy).await?;
        }
    }

    Ok(())
}

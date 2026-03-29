mod discover;
mod extract;
mod fetch;
mod js_render;
mod llm_filter;
mod models;
mod sitemap;
mod url_normalize;
mod worker;

use clap::Parser;
use tracing::warn;

use models::{Cli, Command, LlmFilterConfig, WorkerConfig};

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
            max_jobs,
            poll_interval_secs,
            once,
            concurrency,
            allowed_domains,
            proxy,
            tor_socks_url,
            llm_base_url,
            llm_api_key,
            llm_model,
            llm_min_score,
            llm_max_body_chars,
            stealth_ua,
        } => {
            if stealth_ua {
                warn!(
                    "--stealth-ua is deprecated and ignored; FindVerse always identifies as a public crawler now"
                );
            }

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
                crawler_id,
                auth_token: crawler_key,
                max_jobs: max_jobs.max(concurrency),
                poll_interval_secs,
                once,
                concurrency,
                allowed_domains: parsed_domains,
                tor_socks_url: Some(tor_socks_url),
                llm_filter: match (llm_base_url, llm_model) {
                    (Some(base_url), Some(model)) => Some(LlmFilterConfig {
                        base_url,
                        api_key: llm_api_key,
                        model,
                        min_score: llm_min_score.clamp(0.0, 1.0),
                        max_body_chars: llm_max_body_chars.clamp(500, 20_000),
                    }),
                    _ => None,
                },
                stealth_ua,
            };
            worker::run_worker(config, proxy).await?;
        }
    }

    Ok(())
}

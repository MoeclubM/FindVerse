mod discover;
mod extract;
mod fetch;
mod js_render;
mod models;
mod site_profile;
mod sitemap;
mod url_normalize;
mod worker;

use clap::Parser;

use models::{Cli, Command, CrawlerCapabilities, WorkerConfig};

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
            crawler_name,
            crawler_key,
            max_jobs,
            poll_interval_secs,
            once,
            concurrency,
            js_render_concurrency,
            allowed_domains,
            proxy,
            tor_socks_url,
        } => {
            let parsed_domains: Vec<String> = allowed_domains
                .map(|d| {
                    d.split(',')
                        .map(|s| s.trim().to_lowercase())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default();

            let js_capable = js_render::detect_js_capability();
            if js_capable {
                tracing::info!("chromium detected — JS rendering capability enabled");
            } else {
                tracing::warn!(
                    "no chromium available — JS rendering capability disabled; SPA pages will be re-queued for a capable node"
                );
            }

            let config = WorkerConfig {
                server,
                crawler_id,
                crawler_name: crawler_name
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                auth_token: crawler_key,
                max_jobs: max_jobs.max(1),
                poll_interval_secs,
                once,
                concurrency,
                js_render_concurrency: js_render_concurrency.max(1),
                allowed_domains: parsed_domains,
                tor_socks_url: Some(tor_socks_url),
                capabilities: CrawlerCapabilities {
                    js_render: js_capable,
                },
            };
            worker::run_worker(config, proxy).await?;
        }
    }

    Ok(())
}

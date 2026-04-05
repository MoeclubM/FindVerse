use std::{env, path::PathBuf};

use clap::{Parser, Subcommand};
use findverse_api::migration::{LegacyMigrationConfig, migrate_legacy_control_plane_data};

#[derive(Parser)]
#[command(name = "findverse-control-api")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Serve,
    MigrateLegacy {
        #[arg(long)]
        postgres_url: Option<String>,
        #[arg(long)]
        postgres_max_connections: Option<u32>,
        #[arg(long)]
        postgres_acquire_timeout_secs: Option<u64>,
        #[arg(long)]
        blob_storage_url: Option<String>,
        #[arg(long)]
        dev_auth_store: Option<PathBuf>,
        #[arg(long)]
        developer_store: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match Cli::parse().command.unwrap_or(Command::Serve) {
        Command::Serve => findverse_api::run_control_api().await,
        Command::MigrateLegacy {
            postgres_url,
            postgres_max_connections,
            postgres_acquire_timeout_secs,
            blob_storage_url,
            dev_auth_store,
            developer_store,
        } => {
            let summary = migrate_legacy_control_plane_data(LegacyMigrationConfig {
                postgres_url: postgres_url.unwrap_or_else(|| {
                    env::var("FINDVERSE_POSTGRES_URL").unwrap_or_else(|_| {
                        "postgres://postgres:postgres@localhost:5432/findverse".to_string()
                    })
                }),
                postgres_max_connections: postgres_max_connections.unwrap_or_else(|| {
                    env::var("FINDVERSE_POSTGRES_MAX_CONNECTIONS")
                        .ok()
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(10)
                }),
                postgres_acquire_timeout_secs: postgres_acquire_timeout_secs.unwrap_or_else(|| {
                    env::var("FINDVERSE_POSTGRES_ACQUIRE_TIMEOUT_SECS")
                        .ok()
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(5)
                }),
                blob_storage_url: blob_storage_url
                    .or_else(|| env::var("FINDVERSE_BLOB_STORAGE_URL").ok()),
                dev_auth_store_path: dev_auth_store,
                developer_store_path: developer_store,
            })
            .await?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
            Ok(())
        }
    }
}

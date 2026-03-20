use std::{env, net::SocketAddr, path::PathBuf};

use anyhow::Context;

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub index_path: PathBuf,
    pub developer_store_path: PathBuf,
    pub crawler_store_path: PathBuf,
    pub dev_auth_store_path: PathBuf,
    pub frontend_origin: String,
    pub local_admin_username: String,
    pub local_admin_password: String,
    pub crawler_maintenance_interval_secs: u64,
    pub crawler_claim_timeout_secs: u64,
    pub crawler_join_key: Option<String>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let bind_addr = env::var("FINDVERSE_API_BIND")
            .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
            .parse()
            .context("invalid FINDVERSE_API_BIND")?;

        let index_path = env::var("FINDVERSE_INDEX_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("services/api/fixtures/bootstrap_documents.json"));

        let developer_store_path = env::var("FINDVERSE_DEVELOPER_STORE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("services/api/fixtures/developer_store.json"));

        let crawler_store_path = env::var("FINDVERSE_CRAWLER_STORE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("services/api/fixtures/crawler_store.json"));

        let dev_auth_store_path = env::var("FINDVERSE_DEV_AUTH_STORE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("services/api/fixtures/dev_auth_store.json"));

        let frontend_origin = env::var("FINDVERSE_FRONTEND_ORIGIN")
            .unwrap_or_else(|_| "http://localhost:3000".to_string());

        let local_admin_username = env::var("FINDVERSE_LOCAL_ADMIN_USERNAME")
            .unwrap_or_else(|_| "admin".to_string());

        let local_admin_password = env::var("FINDVERSE_LOCAL_ADMIN_PASSWORD")
            .unwrap_or_else(|_| "change-me".to_string());

        let crawler_maintenance_interval_secs = env::var(
            "FINDVERSE_CRAWLER_MAINTENANCE_INTERVAL_SECS",
        )
        .unwrap_or_else(|_| "15".to_string())
        .parse()
        .context("invalid FINDVERSE_CRAWLER_MAINTENANCE_INTERVAL_SECS")?;

        let crawler_claim_timeout_secs = env::var("FINDVERSE_CRAWLER_CLAIM_TIMEOUT_SECS")
            .unwrap_or_else(|_| "300".to_string())
            .parse()
            .context("invalid FINDVERSE_CRAWLER_CLAIM_TIMEOUT_SECS")?;

        let crawler_join_key = env::var("FINDVERSE_CRAWLER_JOIN_KEY").ok().filter(|v| !v.is_empty());

        Ok(Self {
            bind_addr,
            index_path,
            developer_store_path,
            crawler_store_path,
            dev_auth_store_path,
            frontend_origin,
            local_admin_username,
            local_admin_password,
            crawler_maintenance_interval_secs,
            crawler_claim_timeout_secs,
            crawler_join_key,
        })
    }
}

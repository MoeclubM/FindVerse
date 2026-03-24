use std::{env, net::SocketAddr, path::PathBuf};

use anyhow::Context;

#[derive(Debug, Clone, Copy)]
pub enum ServiceKind {
    Control,
    Query,
}

impl ServiceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Control => "control-api",
            Self::Query => "query-api",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub index_path: PathBuf,
    pub developer_store_path: PathBuf,
    pub dev_auth_store_path: PathBuf,
    pub frontend_origin: String,
    pub local_admin_username: String,
    pub local_admin_password: String,
    pub postgres_url: String,
    pub postgres_max_connections: u32,
    pub postgres_acquire_timeout_secs: u64,
    pub redis_url: String,
    pub opensearch_url: String,
    pub opensearch_index: String,
    pub bootstrap_admin_enabled: bool,
    pub crawler_maintenance_interval_secs: u64,
    pub crawler_claim_timeout_secs: u64,
    pub crawler_join_key: Option<String>,
}

impl Config {
    pub fn from_env(service_kind: ServiceKind) -> anyhow::Result<Self> {
        let bind_addr = resolve_bind_addr(service_kind)?
            .parse()
            .with_context(|| format!("invalid bind address for {}", service_kind.as_str()))?;

        let index_path = env::var("FINDVERSE_INDEX_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("services/api/fixtures/bootstrap_documents.json"));

        let developer_store_path = env::var("FINDVERSE_DEVELOPER_STORE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("services/api/fixtures/developer_store.json"));

        let dev_auth_store_path = env::var("FINDVERSE_DEV_AUTH_STORE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("services/api/fixtures/dev_auth_store.json"));

        let frontend_origin = env::var("FINDVERSE_FRONTEND_ORIGIN")
            .unwrap_or_else(|_| "http://localhost:3000".to_string());

        let local_admin_username =
            env::var("FINDVERSE_LOCAL_ADMIN_USERNAME").unwrap_or_else(|_| "admin".to_string());

        let local_admin_password =
            env::var("FINDVERSE_LOCAL_ADMIN_PASSWORD").unwrap_or_else(|_| "change-me".to_string());

        let postgres_url = env::var("FINDVERSE_POSTGRES_URL").unwrap_or_else(|_| {
            "postgres://postgres:postgres@localhost:5432/findverse".to_string()
        });

        let postgres_max_connections = env::var("FINDVERSE_POSTGRES_MAX_CONNECTIONS")
            .unwrap_or_else(|_| "10".to_string())
            .parse()
            .context("invalid FINDVERSE_POSTGRES_MAX_CONNECTIONS")?;

        let postgres_acquire_timeout_secs = env::var("FINDVERSE_POSTGRES_ACQUIRE_TIMEOUT_SECS")
            .unwrap_or_else(|_| "5".to_string())
            .parse()
            .context("invalid FINDVERSE_POSTGRES_ACQUIRE_TIMEOUT_SECS")?;

        let redis_url = env::var("FINDVERSE_REDIS_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379/0".to_string());

        let opensearch_url = env::var("FINDVERSE_OPENSEARCH_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:9200".to_string());

        let opensearch_index = env::var("FINDVERSE_OPENSEARCH_INDEX")
            .unwrap_or_else(|_| "findverse-documents".to_string());

        let bootstrap_admin_enabled = env::var("FINDVERSE_BOOTSTRAP_ADMIN_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .parse()
            .context("invalid FINDVERSE_BOOTSTRAP_ADMIN_ENABLED")?;

        let crawler_maintenance_interval_secs =
            env::var("FINDVERSE_CRAWLER_MAINTENANCE_INTERVAL_SECS")
                .unwrap_or_else(|_| "15".to_string())
                .parse()
                .context("invalid FINDVERSE_CRAWLER_MAINTENANCE_INTERVAL_SECS")?;

        let crawler_claim_timeout_secs = env::var("FINDVERSE_CRAWLER_CLAIM_TIMEOUT_SECS")
            .unwrap_or_else(|_| "300".to_string())
            .parse()
            .context("invalid FINDVERSE_CRAWLER_CLAIM_TIMEOUT_SECS")?;

        let crawler_join_key = env::var("FINDVERSE_CRAWLER_JOIN_KEY")
            .ok()
            .filter(|v| !v.is_empty());

        Ok(Self {
            bind_addr,
            index_path,
            developer_store_path,
            dev_auth_store_path,
            frontend_origin,
            local_admin_username,
            local_admin_password,
            postgres_url,
            postgres_max_connections,
            postgres_acquire_timeout_secs,
            redis_url,
            opensearch_url,
            opensearch_index,
            bootstrap_admin_enabled,
            crawler_maintenance_interval_secs,
            crawler_claim_timeout_secs,
            crawler_join_key,
        })
    }
}

fn resolve_bind_addr(service_kind: ServiceKind) -> anyhow::Result<String> {
    let value = match service_kind {
        ServiceKind::Control => {
            env::var("FINDVERSE_CONTROL_API_BIND").unwrap_or_else(|_| "0.0.0.0:8080".to_string())
        }
        ServiceKind::Query => {
            env::var("FINDVERSE_QUERY_API_BIND").unwrap_or_else(|_| "0.0.0.0:8081".to_string())
        }
    };

    if value.trim().is_empty() {
        anyhow::bail!("bind address must not be empty");
    }

    Ok(value)
}

use std::{env, net::SocketAddr, path::PathBuf};

use anyhow::Context;

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub index_path: PathBuf,
    pub developer_store_path: PathBuf,
    pub crawler_store_path: PathBuf,
    pub frontend_origin: String,
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

        let frontend_origin = env::var("FINDVERSE_FRONTEND_ORIGIN")
            .unwrap_or_else(|_| "http://localhost:3000".to_string());

        Ok(Self {
            bind_addr,
            index_path,
            developer_store_path,
            crawler_store_path,
            frontend_origin,
        })
    }
}

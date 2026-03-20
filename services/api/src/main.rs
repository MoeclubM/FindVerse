mod admin;
mod crawler;
mod config;
mod dev_auth;
mod error;
mod handlers;
mod models;
mod store;

use std::{sync::Arc, time::Duration};

use admin::AdminAuth;
use axum::{
    Router,
    routing::{delete, get, patch, post},
};
use dev_auth::DevAuthStore;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::{error, info};

use crate::{
    config::Config,
    crawler::CrawlerStore,
    store::{DeveloperStore, SearchIndex},
};

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) search_index: SearchIndex,
    pub(crate) developer_store: DeveloperStore,
    pub(crate) crawler_store: CrawlerStore,
    pub(crate) admin_auth: AdminAuth,
    pub(crate) dev_auth: DevAuthStore,
    pub(crate) crawler_claim_timeout_secs: u64,
    pub(crate) crawler_join_key: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "findverse_api=debug,tower_http=info".to_string()),
        )
        .init();

    let config = Config::from_env()?;
    let search_index = SearchIndex::load(config.index_path.clone()).await?;
    let developer_store = DeveloperStore::load(config.developer_store_path.clone()).await?;
    let crawler_store = CrawlerStore::load(config.crawler_store_path.clone()).await?;
    let dev_auth = DevAuthStore::load(config.dev_auth_store_path.clone()).await?;
    let admin_auth = AdminAuth::new(
        config.local_admin_username.clone(),
        config.local_admin_password.clone(),
    );

    let state = Arc::new(AppState {
        search_index,
        developer_store,
        crawler_store,
        admin_auth,
        dev_auth,
        crawler_claim_timeout_secs: config.crawler_claim_timeout_secs.max(1),
        crawler_join_key: config.crawler_join_key.clone(),
    });

    let maintenance_state = Arc::clone(&state);
    let maintenance_interval = Duration::from_secs(config.crawler_maintenance_interval_secs.max(1));
    let claim_timeout = Duration::from_secs(config.crawler_claim_timeout_secs.max(1));
    tokio::spawn(async move {
        if let Err(error) = maintenance_state
            .crawler_store
            .run_maintenance(claim_timeout)
            .await
        {
            error!(?error, "initial crawler maintenance pass failed");
        }

        let mut ticker = tokio::time::interval(maintenance_interval);
        loop {
            ticker.tick().await;
            if let Err(error) = maintenance_state
                .crawler_store
                .run_maintenance(claim_timeout)
                .await
            {
                error!(?error, "crawler maintenance pass failed");
            }
        }
    });

    let app = Router::new()
        .route("/healthz", get(handlers::search::healthz))
        .route("/v1/search", get(handlers::search::browser_search))
        .route("/v1/developer/search", get(handlers::search::search))
        .route("/v1/suggest", get(handlers::search::suggest))
        .route("/v1/admin/session/login", post(handlers::admin::admin_login))
        .route("/v1/admin/session/me", get(handlers::admin::admin_session_me))
        .route("/v1/admin/session/logout", post(handlers::admin::admin_logout))
        .route("/v1/admin/usage", get(handlers::admin::admin_usage))
        .route("/v1/admin/api-keys", post(handlers::admin::admin_create_key))
        .route(
            "/v1/admin/api-keys/{id}",
            delete(handlers::admin::admin_revoke_key),
        )
        .route("/v1/admin/crawlers", post(handlers::admin::admin_create_crawler))
        .route("/v1/admin/frontier/seed", post(handlers::admin::admin_seed_frontier))
        .route(
            "/v1/admin/crawl/overview",
            get(handlers::admin::admin_crawl_overview),
        )
        .route("/v1/admin/crawl/rules", post(handlers::admin::admin_create_rule))
        .route(
            "/v1/admin/crawl/rules/{id}",
            patch(handlers::admin::admin_update_rule)
                .delete(handlers::admin::admin_delete_rule),
        )
        .route("/v1/admin/documents", get(handlers::admin::admin_list_documents))
        .route(
            "/v1/admin/documents/{id}",
            delete(handlers::admin::admin_delete_document),
        )
        .route(
            "/v1/admin/documents/purge-site",
            post(handlers::admin::admin_purge_site),
        )
        .route(
            "/v1/admin/crawler-join-key",
            get(handlers::admin::admin_get_join_key).put(handlers::admin::admin_set_join_key),
        )
        .route("/v1/dev/register", post(handlers::developer::dev_register))
        .route("/v1/dev/login", post(handlers::developer::dev_login))
        .route("/v1/dev/me", get(handlers::developer::dev_me))
        .route("/v1/dev/logout", post(handlers::developer::dev_logout))
        .route(
            "/v1/dev/keys",
            get(handlers::developer::dev_list_keys).post(handlers::developer::dev_create_key),
        )
        .route(
            "/v1/dev/keys/{id}",
            delete(handlers::developer::dev_revoke_key),
        )
        .route(
            "/v1/admin/developers",
            get(handlers::admin::admin_list_developers),
        )
        .route(
            "/v1/admin/developers/{user_id}",
            patch(handlers::admin::admin_update_developer),
        )
        .route("/internal/crawlers/claim", post(handlers::crawler::claim_jobs))
        .route(
            "/internal/crawlers/report",
            post(handlers::crawler::submit_crawl_report),
        )
        .route("/internal/crawlers/hello", post(handlers::crawler::crawler_hello))
        .route("/internal/crawlers/join", post(handlers::crawler::crawler_join))
        .layer(
            CorsLayer::new()
                .allow_origin(config.frontend_origin.parse::<axum::http::HeaderValue>()?)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    info!("findverse api listening on {}", config.bind_addr);
    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

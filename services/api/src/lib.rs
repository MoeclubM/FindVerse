pub mod admin;
pub mod auth_support;
pub mod blob_store;
pub mod config;
pub mod crawl;
pub mod crawler;
pub mod db;
pub mod dev_auth;
pub mod error;
pub mod handlers;
pub mod indexing;
pub mod migration;
pub mod models;
pub mod quality;
pub mod query;
pub mod ranking;
pub mod rate_limit;
pub mod store;

use std::time::Duration;

use admin::AdminAuth;
use axum::{
    Router,
    extract::FromRef,
    routing::{delete, get, patch, post, put},
};
use config::{Config, ServiceKind};
use crawler::{ControlCrawlerStore, SchedulerCrawlerStore, TaskCrawlerStore};
use db::DatabaseBackends;
use dev_auth::DevAuthStore;
use sqlx::migrate;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::{error, info};

use crate::{
    blob_store::BlobStore,
    store::{DeveloperStore, SearchIndex},
};

#[derive(Clone)]
pub struct QueryState {
    pub search_index: SearchIndex,
    pub developer_store: DeveloperStore,
    pub db: DatabaseBackends,
}

#[derive(Clone)]
pub struct ControlState {
    pub query: QueryState,
    pub crawl_store: ControlCrawlerStore,
    pub admin_auth: AdminAuth,
    pub dev_auth: DevAuthStore,
    pub default_crawler_owner_id: String,
}

#[derive(Clone)]
pub struct TaskState {
    pub crawl_store: TaskCrawlerStore,
    pub db: DatabaseBackends,
    pub default_crawler_owner_id: String,
}

#[derive(Clone)]
struct SchedulerState {
    crawl_store: SchedulerCrawlerStore,
    search_index: SearchIndex,
}

impl FromRef<ControlState> for QueryState {
    fn from_ref(state: &ControlState) -> Self {
        state.query.clone()
    }
}

pub async fn run_control_api() -> anyhow::Result<()> {
    init_tracing();

    let config = Config::from_env(ServiceKind::Control)?;
    let state = bootstrap_control_state(&config).await?;

    info!(
        service = ServiceKind::Control.as_str(),
        postgres_url = %config.postgres_url,
        redis_url = %config.redis_url,
        opensearch_url = %config.opensearch_url,
        bootstrap_admin_enabled = config.bootstrap_admin_enabled,
        "findverse backends ready"
    );
    info!(
        service = ServiceKind::Control.as_str(),
        "findverse api listening on {}",
        config.bind_addr.expect("control-api bind addr")
    );

    let listener =
        tokio::net::TcpListener::bind(config.bind_addr.expect("control-api bind addr")).await?;
    axum::serve(listener, build_control_router(&config, state)).await?;

    Ok(())
}

pub async fn run_task_api() -> anyhow::Result<()> {
    init_tracing();

    let config = Config::from_env(ServiceKind::Task)?;
    let state = bootstrap_task_state(&config).await?;

    info!(
        service = ServiceKind::Task.as_str(),
        postgres_url = %config.postgres_url,
        redis_url = %config.redis_url,
        "findverse backends ready"
    );
    info!(
        service = ServiceKind::Task.as_str(),
        "findverse api listening on {}",
        config.bind_addr.expect("task-api bind addr")
    );

    let listener =
        tokio::net::TcpListener::bind(config.bind_addr.expect("task-api bind addr")).await?;
    axum::serve(listener, build_task_router(state)).await?;

    Ok(())
}

pub async fn run_query_api() -> anyhow::Result<()> {
    init_tracing();

    let config = Config::from_env(ServiceKind::Query)?;
    let state = bootstrap_query_state(&config).await?;

    info!(
        service = ServiceKind::Query.as_str(),
        postgres_url = %config.postgres_url,
        redis_url = %config.redis_url,
        opensearch_url = %config.opensearch_url,
        "findverse backends ready"
    );
    info!(
        service = ServiceKind::Query.as_str(),
        "findverse api listening on {}",
        config.bind_addr.expect("query-api bind addr")
    );

    let listener =
        tokio::net::TcpListener::bind(config.bind_addr.expect("query-api bind addr")).await?;
    axum::serve(listener, build_query_router(&config, state)).await?;

    Ok(())
}

pub async fn run_scheduler() -> anyhow::Result<()> {
    init_tracing();

    let config = Config::from_env(ServiceKind::Scheduler)?;
    let state = bootstrap_scheduler_state(&config).await?;
    let maintenance_interval = Duration::from_secs(config.crawler_maintenance_interval_secs.max(1));
    let default_claim_timeout_secs = config.crawler_claim_timeout_secs;

    info!(
        service = "scheduler",
        postgres_url = %config.postgres_url,
        redis_url = %config.redis_url,
        opensearch_url = %config.opensearch_url,
        interval_secs = maintenance_interval.as_secs(),
        "findverse scheduler ready"
    );

    let mut ticker = tokio::time::interval(maintenance_interval);
    loop {
        ticker.tick().await;
        let timeout_secs = state
            .crawl_store
            .get_system_config("crawler.claim_timeout_secs")
            .await
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(default_claim_timeout_secs);
        let claim_timeout = Duration::from_secs(timeout_secs.max(1));
        if let Err(error) = state
            .crawl_store
            .run_maintenance(claim_timeout, &state.search_index)
            .await
        {
            error!(?error, "scheduler maintenance pass failed");
        }
    }
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "findverse_api=debug,tower_http=info".to_string()),
        )
        .try_init();
}

async fn connect_backends(config: &Config) -> anyhow::Result<DatabaseBackends> {
    let db = DatabaseBackends::connect(config).await?;
    migrate!("./migrations").run(&db.pg_pool).await?;
    Ok(db)
}

async fn bootstrap_query_state(config: &Config) -> anyhow::Result<QueryState> {
    let db = connect_backends(config).await?;
    let search_index = SearchIndex::connect(
        db.pg_pool.clone(),
        config.opensearch_url.clone(),
        config.opensearch_index.clone(),
        config.blob_store_dir.clone(),
        db.redis_client.clone(),
    )
    .await?;

    Ok(QueryState {
        search_index,
        developer_store: DeveloperStore::new(db.pg_pool.clone()),
        db,
    })
}

async fn bootstrap_control_state(config: &Config) -> anyhow::Result<ControlState> {
    let db = connect_backends(config).await?;
    db.prepare_control_plane(config).await?;
    let blob_store = BlobStore::new(db.pg_pool.clone(), config.blob_store_dir.clone());
    blob_store.ensure_ready().await?;

    let search_index = SearchIndex::connect(
        db.pg_pool.clone(),
        config.opensearch_url.clone(),
        config.opensearch_index.clone(),
        config.blob_store_dir.clone(),
        db.redis_client.clone(),
    )
    .await?;
    search_index
        .bootstrap_from_path(config.index_path.clone())
        .await?;

    Ok(ControlState {
        query: QueryState {
            search_index,
            developer_store: DeveloperStore::new(db.pg_pool.clone()),
            db: db.clone(),
        },
        crawl_store: ControlCrawlerStore::new(db.pg_pool.clone(), blob_store),
        admin_auth: AdminAuth::new(db.pg_pool.clone()),
        dev_auth: DevAuthStore::new(db.pg_pool.clone()),
        default_crawler_owner_id: format!("local:{}", config.local_admin_username),
    })
}

async fn bootstrap_task_state(config: &Config) -> anyhow::Result<TaskState> {
    let db = connect_backends(config).await?;
    let blob_store = BlobStore::new(db.pg_pool.clone(), config.blob_store_dir.clone());
    blob_store.ensure_ready().await?;

    Ok(TaskState {
        crawl_store: TaskCrawlerStore::new(db.pg_pool.clone(), blob_store),
        db,
        default_crawler_owner_id: format!("local:{}", config.local_admin_username),
    })
}

async fn bootstrap_scheduler_state(config: &Config) -> anyhow::Result<SchedulerState> {
    let db = connect_backends(config).await?;
    let blob_store = BlobStore::new(db.pg_pool.clone(), config.blob_store_dir.clone());
    blob_store.ensure_ready().await?;
    let search_index = SearchIndex::connect(
        db.pg_pool.clone(),
        config.opensearch_url.clone(),
        config.opensearch_index.clone(),
        config.blob_store_dir.clone(),
        db.redis_client.clone(),
    )
    .await?;

    Ok(SchedulerState {
        crawl_store: SchedulerCrawlerStore::new(db.pg_pool.clone(), blob_store),
        search_index,
    })
}

fn build_query_router(config: &Config, state: QueryState) -> Router {
    Router::new()
        .route("/healthz", get(handlers::search::healthz))
        .route("/readyz", get(handlers::search::readyz))
        .route("/v1/search", get(handlers::search::browser_search))
        .route("/v1/developer/search", get(handlers::search::search))
        .route("/v1/suggest", get(handlers::search::suggest))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            rate_limit::rate_limit_middleware,
        ))
        .layer(shared_cors(config))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

fn build_control_router(config: &Config, state: ControlState) -> Router {
    Router::new()
        .route("/healthz", get(handlers::search::healthz))
        .route("/readyz", get(handlers::search::readyz))
        .route(
            "/v1/admin/session/login",
            post(handlers::admin::admin_login),
        )
        .route(
            "/v1/admin/session/me",
            get(handlers::admin::admin_session_me),
        )
        .route(
            "/v1/admin/session/logout",
            post(handlers::admin::admin_logout),
        )
        .route(
            "/v1/admin/developers/{user_id}/keys",
            get(handlers::admin::admin_list_developer_keys)
                .post(handlers::admin::admin_create_developer_key),
        )
        .route(
            "/v1/admin/developers/{user_id}/keys/{key_id}",
            delete(handlers::admin::admin_revoke_developer_key),
        )
        .route(
            "/v1/admin/crawlers/{id}",
            patch(handlers::admin::admin_update_crawler)
                .delete(handlers::admin::admin_delete_crawler),
        )
        .route(
            "/v1/admin/frontier/seed",
            post(handlers::admin::admin_seed_frontier),
        )
        .route(
            "/v1/admin/crawl/overview",
            get(handlers::admin::admin_crawl_overview),
        )
        .route(
            "/v1/admin/crawl/rules",
            post(handlers::admin::admin_create_rule),
        )
        .route(
            "/v1/admin/crawl/rules/{id}",
            patch(handlers::admin::admin_update_rule).delete(handlers::admin::admin_delete_rule),
        )
        .route(
            "/v1/admin/documents",
            get(handlers::admin::admin_list_documents),
        )
        .route(
            "/v1/admin/documents/{id}",
            delete(handlers::admin::admin_delete_document),
        )
        .route(
            "/v1/admin/documents/purge-site",
            post(handlers::admin::admin_purge_site),
        )
        .route(
            "/v1/admin/system-config",
            get(handlers::admin::admin_list_system_config),
        )
        .route(
            "/v1/admin/system-config/{key}",
            put(handlers::admin::admin_set_system_config),
        )
        .route(
            "/v1/admin/crawl/jobs",
            get(handlers::admin::admin_list_jobs),
        )
        .route(
            "/v1/admin/crawl/jobs/stats",
            get(handlers::admin::admin_job_stats),
        )
        .route(
            "/v1/admin/crawl/origins",
            get(handlers::admin::admin_list_origins),
        )
        .route(
            "/v1/admin/domains/inspect",
            get(handlers::admin::admin_domain_insight),
        )
        .route(
            "/v1/admin/crawl/jobs/retry",
            post(handlers::admin::admin_retry_failed_jobs),
        )
        .route(
            "/v1/admin/crawl/jobs/stop",
            post(handlers::admin::admin_stop_all_jobs),
        )
        .route(
            "/v1/admin/crawl/jobs/completed",
            delete(handlers::admin::admin_cleanup_completed_jobs),
        )
        .route(
            "/v1/admin/crawl/jobs/failed",
            delete(handlers::admin::admin_cleanup_failed_jobs),
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
            "/v1/dev/domains/inspect",
            get(handlers::developer::dev_domain_insight),
        )
        .route(
            "/v1/dev/domains/submit",
            post(handlers::developer::dev_submit_domain),
        )
        .route(
            "/v1/admin/developers",
            get(handlers::admin::admin_list_developers),
        )
        .route(
            "/v1/admin/developers/{user_id}",
            patch(handlers::admin::admin_update_developer)
                .delete(handlers::admin::admin_delete_developer),
        )
        .layer(shared_cors(config))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

fn build_task_router(state: TaskState) -> Router {
    Router::new()
        .route("/healthz", get(handlers::task::healthz))
        .route("/readyz", get(handlers::task::readyz))
        .route(
            "/internal/crawlers/claim",
            post(handlers::crawler::claim_jobs),
        )
        .route(
            "/internal/crawlers/report",
            post(handlers::crawler::submit_crawl_report),
        )
        .route(
            "/internal/crawlers/heartbeat",
            post(handlers::crawler::heartbeat_crawler),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

fn shared_cors(config: &Config) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(
            config
                .frontend_origin
                .split(',')
                .filter_map(|o| o.trim().parse::<axum::http::HeaderValue>().ok())
                .collect::<Vec<_>>(),
        )
        .allow_methods(Any)
        .allow_headers(Any)
}

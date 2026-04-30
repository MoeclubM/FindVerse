use std::time::Duration;

use tracing::{error, info};

use crate::{
    blob_store::{self, BlobStore},
    config::{Config, ServiceKind},
    migration,
    routes::{build_control_router, build_query_router, build_task_router},
    startup::{
        bootstrap_control_state, bootstrap_projector_state, bootstrap_query_state,
        bootstrap_scheduler_state, bootstrap_task_state, connect_backends,
        seed_default_system_config,
    },
    store::SearchIndex,
};

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
            .run_scheduler_maintenance(claim_timeout)
            .await
        {
            error!(?error, "scheduler maintenance pass failed");
        }
    }
}

pub async fn run_projector() -> anyhow::Result<()> {
    init_tracing();

    let config = Config::from_env(ServiceKind::Projector)?;
    let state = bootstrap_projector_state(&config).await?;

    info!(
        service = "projector",
        postgres_url = %config.postgres_url,
        redis_url = %config.redis_url,
        opensearch_url = %config.opensearch_url,
        interval_secs = state.interval.as_secs(),
        batch_size = state.batch_size,
        "findverse projector ready"
    );

    loop {
        let message_ids = state
            .task_bus
            .read_batch(state.batch_size, state.interval, state.interval)
            .await?;
        let timeout_secs = state
            .crawl_store
            .get_system_config("crawler.claim_timeout_secs")
            .await
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(state.default_claim_timeout_secs);
        let claim_timeout = Duration::from_secs(timeout_secs.max(1));

        if let Err(error) = state.crawl_store.recover_stale_ingests(claim_timeout).await {
            error!(?error, "projector stale ingest recovery failed");
            continue;
        }

        if let Err(error) = state
            .crawl_store
            .cleanup_blacklisted_domains(&state.search_index)
            .await
        {
            error!(?error, "projector blacklist cleanup failed");
            continue;
        }

        if let Err(error) = state
            .crawl_store
            .process_pending_ingests(&state.search_index, state.batch_size)
            .await
        {
            error!(?error, "projector ingest pass failed");
            continue;
        }

        state.task_bus.ack(&message_ids).await?;
    }
}

pub async fn run_bootstrap() -> anyhow::Result<()> {
    init_tracing();

    let config = Config::from_env(ServiceKind::Bootstrap)?;
    let db = connect_backends(&config, true).await?;
    db.prepare_control_plane(&config).await?;

    seed_default_system_config(&db.pg_pool, &config).await?;

    let blob_store = BlobStore::new(db.pg_pool.clone(), config.blob_storage_url.clone());
    let search_index = SearchIndex::connect(
        db.pg_pool.clone(),
        config.opensearch_url.clone(),
        config.opensearch_index.clone(),
        blob_store.clone(),
        db.redis_client.clone(),
    )
    .await?;
    let blob_backfill = migration::backfill_blob_storage(&db.pg_pool, &blob_store).await?;
    search_index.bootstrap_storage().await?;
    let reindexed_documents = search_index.reindex_existing_documents(256).await?;
    info!(
        document_text_blobs_backfilled = blob_backfill.document_text_blobs_backfilled,
        crawl_result_blobs_backfilled = blob_backfill.crawl_result_blobs_backfilled,
        reindexed_documents,
        "findverse bootstrap migration pass completed"
    );
    search_index
        .bootstrap_from_path(config.index_path.clone())
        .await?;

    info!("findverse bootstrap completed");
    Ok(())
}

pub async fn run_blob_storage() -> anyhow::Result<()> {
    init_tracing();
    blob_store::run_blob_storage().await
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "findverse_api=debug,tower_http=info".to_string()),
        )
        .try_init();
}

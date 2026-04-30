use std::time::Duration;

use sqlx::migrate;

use crate::{
    blob_store::BlobStore,
    config::Config,
    crawler::{
        ControlCrawlerStore, ProjectorCrawlerStore, SchedulerCrawlerStore, TaskCrawlerStore,
    },
    db::DatabaseBackends,
    dev_auth::DevAuthStore,
    state::{ControlState, ProjectorState, QueryState, SchedulerState, TaskState},
    store::{DeveloperStore, SearchIndex},
    task_bus::TaskBus,
};

pub(crate) async fn connect_backends(
    config: &Config,
    apply_migrations: bool,
) -> anyhow::Result<DatabaseBackends> {
    let db = DatabaseBackends::connect(config).await?;
    if apply_migrations {
        migrate!("./migrations").run(&db.pg_pool).await?;
    }
    Ok(db)
}

pub(crate) async fn bootstrap_query_state(config: &Config) -> anyhow::Result<QueryState> {
    let db = connect_backends(config, false).await?;
    let blob_store = BlobStore::new(db.pg_pool.clone(), config.blob_storage_url.clone());
    let search_index = SearchIndex::connect(
        db.pg_pool.clone(),
        config.opensearch_url.clone(),
        config.opensearch_index.clone(),
        blob_store,
        db.redis_client.clone(),
    )
    .await?;

    Ok(QueryState {
        search_index,
        developer_store: DeveloperStore::new(db.pg_pool.clone()),
        db,
    })
}

pub(crate) async fn bootstrap_control_state(config: &Config) -> anyhow::Result<ControlState> {
    let db = connect_backends(config, false).await?;
    let blob_store = BlobStore::new(db.pg_pool.clone(), config.blob_storage_url.clone());

    let search_index = SearchIndex::connect(
        db.pg_pool.clone(),
        config.opensearch_url.clone(),
        config.opensearch_index.clone(),
        blob_store.clone(),
        db.redis_client.clone(),
    )
    .await?;

    Ok(ControlState {
        query: QueryState {
            search_index,
            developer_store: DeveloperStore::new(db.pg_pool.clone()),
            db: db.clone(),
        },
        crawl_store: ControlCrawlerStore::new(db.pg_pool.clone(), blob_store),
        dev_auth: DevAuthStore::new(db.pg_pool.clone()),
        default_crawler_owner_id: format!("local:{}", config.local_admin_username),
    })
}

pub(crate) async fn bootstrap_task_state(config: &Config) -> anyhow::Result<TaskState> {
    let db = connect_backends(config, false).await?;
    let blob_store = BlobStore::new(db.pg_pool.clone(), config.blob_storage_url.clone());

    Ok(TaskState {
        crawl_store: TaskCrawlerStore::new(db.pg_pool.clone(), blob_store),
        db: db.clone(),
        default_crawler_owner_id: format!("local:{}", config.local_admin_username),
        task_bus: TaskBus::new(db.redis_client.clone()),
    })
}

pub(crate) async fn bootstrap_scheduler_state(config: &Config) -> anyhow::Result<SchedulerState> {
    let db = connect_backends(config, false).await?;
    let blob_store = BlobStore::new(db.pg_pool.clone(), config.blob_storage_url.clone());

    Ok(SchedulerState {
        crawl_store: SchedulerCrawlerStore::new(db.pg_pool.clone(), blob_store),
    })
}

pub(crate) async fn bootstrap_projector_state(config: &Config) -> anyhow::Result<ProjectorState> {
    let db = connect_backends(config, false).await?;
    let blob_store = BlobStore::new(db.pg_pool.clone(), config.blob_storage_url.clone());
    let search_index = SearchIndex::connect(
        db.pg_pool.clone(),
        config.opensearch_url.clone(),
        config.opensearch_index.clone(),
        blob_store.clone(),
        db.redis_client.clone(),
    )
    .await?;

    Ok(ProjectorState {
        crawl_store: ProjectorCrawlerStore::new(db.pg_pool.clone(), blob_store),
        search_index,
        default_claim_timeout_secs: config.crawler_claim_timeout_secs,
        batch_size: config.projector_batch_size.max(1),
        interval: Duration::from_secs(config.projector_interval_secs.max(1)),
        task_bus: TaskBus::new(db.redis_client.clone()),
    })
}

pub(crate) async fn seed_default_system_config(
    pg_pool: &sqlx::PgPool,
    config: &Config,
) -> anyhow::Result<()> {
    let defaults = [
        (
            "crawler.claim_timeout_secs",
            config.crawler_claim_timeout_secs.to_string(),
        ),
        ("crawler.total_concurrency", "16".to_string()),
        ("crawler.js_render_concurrency", "1".to_string()),
        ("crawler.max_jobs", "16".to_string()),
    ];

    for (key, value) in defaults {
        sqlx::query(
            "insert into system_config (key, value, updated_at)
             values ($1, $2, now())
             on conflict (key) do nothing",
        )
        .bind(key)
        .bind(value)
        .execute(pg_pool)
        .await?;
    }

    Ok(())
}

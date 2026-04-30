use std::time::Duration;

use axum::extract::FromRef;

use crate::{
    crawler::{
        ControlCrawlerStore, ProjectorCrawlerStore, SchedulerCrawlerStore, TaskCrawlerStore,
    },
    db::DatabaseBackends,
    dev_auth::DevAuthStore,
    store::{DeveloperStore, SearchIndex},
    task_bus::TaskBus,
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
    pub dev_auth: DevAuthStore,
    pub default_crawler_owner_id: String,
}

#[derive(Clone)]
pub struct TaskState {
    pub crawl_store: TaskCrawlerStore,
    pub db: DatabaseBackends,
    pub default_crawler_owner_id: String,
    pub task_bus: TaskBus,
}

#[derive(Clone)]
pub(crate) struct SchedulerState {
    pub(crate) crawl_store: SchedulerCrawlerStore,
}

#[derive(Clone)]
pub(crate) struct ProjectorState {
    pub(crate) crawl_store: ProjectorCrawlerStore,
    pub(crate) search_index: SearchIndex,
    pub(crate) default_claim_timeout_secs: u64,
    pub(crate) batch_size: usize,
    pub(crate) interval: Duration,
    pub(crate) task_bus: TaskBus,
}

impl FromRef<ControlState> for QueryState {
    fn from_ref(state: &ControlState) -> Self {
        state.query.clone()
    }
}

use axum::{Json, extract::State};
use serde::Serialize;

use crate::TaskState;

#[derive(Serialize)]
pub struct TaskHealthResponse {
    pub status: &'static str,
}

#[derive(Serialize)]
pub struct TaskReadyResponse {
    pub status: &'static str,
    pub postgres: bool,
    pub redis: bool,
}

pub async fn healthz() -> Json<TaskHealthResponse> {
    Json(TaskHealthResponse { status: "ok" })
}

pub async fn readyz(State(state): State<TaskState>) -> Json<TaskReadyResponse> {
    let postgres = state.db.ping_postgres().await;
    let redis = state.db.ping_redis().await;

    Json(TaskReadyResponse {
        status: if postgres && redis {
            "ready"
        } else {
            "degraded"
        },
        postgres,
        redis,
    })
}

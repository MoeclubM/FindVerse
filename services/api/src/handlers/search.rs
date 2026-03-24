use std::collections::HashMap;

use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
};

use crate::{
    QueryState,
    error::ApiError,
    models::{HealthResponse, ReadyResponse, SearchParams, SearchResponse, SuggestResponse},
};

pub async fn healthz(State(state): State<QueryState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        documents: state.search_index.total_documents().await,
    })
}

pub async fn readyz(State(state): State<QueryState>) -> Json<ReadyResponse> {
    let postgres_ready = state.db.ping_postgres().await;
    let redis_ready = state.db.ping_redis().await;
    Json(
        state
            .search_index
            .readiness(postgres_ready, redis_ready)
            .await,
    )
}

pub async fn browser_search(
    State(state): State<QueryState>,
    Query(params): Query<SearchParams>,
) -> Result<Json<SearchResponse>, ApiError> {
    if params.q.trim().is_empty() {
        return Err(ApiError::BadRequest("query must not be empty".to_string()));
    }
    Ok(Json(state.search_index.search(params).await))
}

pub async fn search(
    State(state): State<QueryState>,
    headers: HeaderMap,
    Query(params): Query<SearchParams>,
) -> Result<Json<SearchResponse>, ApiError> {
    state
        .developer_store
        .validate_and_track_developer_key(
            headers
                .get("authorization")
                .and_then(|value| value.to_str().ok()),
        )
        .await?;

    if params.q.trim().is_empty() {
        return Err(ApiError::BadRequest("query must not be empty".to_string()));
    }
    Ok(Json(state.search_index.search(params).await))
}

pub async fn suggest(
    State(state): State<QueryState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<SuggestResponse>, ApiError> {
    let query = params
        .get("q")
        .cloned()
        .ok_or_else(|| ApiError::BadRequest("query parameter q is required".to_string()))?;

    if query.trim().is_empty() {
        return Err(ApiError::BadRequest("query must not be empty".to_string()));
    }

    Ok(Json(state.search_index.suggest(&query).await))
}

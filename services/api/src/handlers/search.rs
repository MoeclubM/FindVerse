use std::{collections::HashMap, sync::Arc};

use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
};

use crate::{
    AppState,
    error::ApiError,
    models::{HealthResponse, SearchParams, SearchResponse, SuggestResponse},
    store::SearchIndex,
};

pub async fn healthz(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        documents: state.search_index.total_documents(),
    })
}

pub async fn browser_search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParams>,
) -> Result<Json<SearchResponse>, ApiError> {
    Ok(Json(run_search(&state.search_index, params)?))
}

pub async fn search(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<SearchParams>,
) -> Result<Json<SearchResponse>, ApiError> {
    state
        .developer_store
        .validate_and_track(headers.get("authorization").and_then(|value| value.to_str().ok()))
        .await?;

    Ok(Json(run_search(&state.search_index, params)?))
}

pub async fn suggest(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<SuggestResponse>, ApiError> {
    let query = params
        .get("q")
        .cloned()
        .ok_or_else(|| ApiError::BadRequest("query parameter q is required".to_string()))?;

    if query.trim().is_empty() {
        return Err(ApiError::BadRequest("query must not be empty".to_string()));
    }

    Ok(Json(state.search_index.suggest(&query)))
}

fn run_search(search_index: &SearchIndex, params: SearchParams) -> Result<SearchResponse, ApiError> {
    if params.q.trim().is_empty() {
        return Err(ApiError::BadRequest("query must not be empty".to_string()));
    }

    Ok(search_index.search(params))
}

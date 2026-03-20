use std::sync::Arc;

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};

use crate::{
    AppState,
    error::ApiError,
    models::{
        ClaimJobsRequest, ClaimJobsResponse, HelloCrawlerRequest, HelloCrawlerResponse,
        JoinCrawlerRequest, SubmitCrawlReportRequest, SubmitCrawlReportResponse,
    },
};

pub async fn claim_jobs(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<ClaimJobsRequest>,
) -> Result<Json<ClaimJobsResponse>, ApiError> {
    let crawler_id = crawler_id_from_headers(&headers)?;
    Ok(Json(
        state
            .crawler_store
            .claim_jobs(
                &crawler_id,
                headers.get("authorization").and_then(|value| value.to_str().ok()),
                request,
            )
            .await?,
    ))
}

pub async fn submit_crawl_report(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<SubmitCrawlReportRequest>,
) -> Result<Json<SubmitCrawlReportResponse>, ApiError> {
    let crawler_id = crawler_id_from_headers(&headers)?;
    Ok(Json(
        state
            .crawler_store
            .submit_report(
                &crawler_id,
                headers.get("authorization").and_then(|value| value.to_str().ok()),
                request,
                &state.search_index,
            )
            .await?,
    ))
}

pub async fn crawler_hello(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<HelloCrawlerRequest>,
) -> Result<Json<HelloCrawlerResponse>, ApiError> {
    let auth = headers.get("authorization").and_then(|v| v.to_str().ok());
    let developer_id = state.developer_store.validate_api_key_for_identity(auth).await?;
    let token = auth
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(str::trim)
        .unwrap_or("");
    let api_key_hash = {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(token.as_bytes());
        format!("{:x}", hasher.finalize())
    };
    Ok(Json(
        state
            .crawler_store
            .hello(&developer_id, &api_key_hash, request)
            .await?,
    ))
}

pub async fn crawler_join(
    State(state): State<Arc<AppState>>,
    Json(request): Json<JoinCrawlerRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let response = state
        .crawler_store
        .join(state.crawler_join_key.as_deref(), request)
        .await?;
    Ok((StatusCode::CREATED, Json(response)))
}

fn crawler_id_from_headers(headers: &HeaderMap) -> Result<String, ApiError> {
    headers
        .get("x-crawler-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| ApiError::Unauthorized("missing x-crawler-id header".to_string()))
}

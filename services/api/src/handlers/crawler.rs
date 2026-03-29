use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};

use crate::{
    ControlState,
    error::ApiError,
    models::{
        ClaimJobsRequest, ClaimJobsResponse, SubmitCrawlReportRequest, SubmitCrawlReportResponse,
    },
};

pub async fn claim_jobs(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Json(request): Json<ClaimJobsRequest>,
) -> Result<Json<ClaimJobsResponse>, ApiError> {
    let crawler_id = crawler_id_from_headers(&headers)?;
    let crawler_name = crawler_name_from_headers(&headers);
    Ok(Json(
        state
            .crawler_store
            .claim_jobs(
                &crawler_id,
                crawler_name.as_deref(),
                headers
                    .get("authorization")
                    .and_then(|value| value.to_str().ok()),
                &state.default_crawler_owner_id,
                request,
            )
            .await?,
    ))
}

pub async fn submit_crawl_report(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Json(request): Json<SubmitCrawlReportRequest>,
) -> Result<Json<SubmitCrawlReportResponse>, ApiError> {
    let crawler_id = crawler_id_from_headers(&headers)?;
    let crawler_name = crawler_name_from_headers(&headers);
    Ok(Json(
        state
            .crawler_store
            .submit_report(
                &crawler_id,
                crawler_name.as_deref(),
                headers
                    .get("authorization")
                    .and_then(|value| value.to_str().ok()),
                &state.default_crawler_owner_id,
                request,
                &state.query.search_index,
            )
            .await?,
    ))
}

pub async fn heartbeat_crawler(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let crawler_id = crawler_id_from_headers(&headers)?;
    let crawler_name = crawler_name_from_headers(&headers);
    state
        .crawler_store
        .heartbeat_crawler(
            &crawler_id,
            crawler_name.as_deref(),
            headers
                .get("authorization")
                .and_then(|value| value.to_str().ok()),
            &state.default_crawler_owner_id,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
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

fn crawler_name_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-crawler-name")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

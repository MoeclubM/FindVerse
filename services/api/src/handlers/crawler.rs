use axum::{Json, extract::State, http::HeaderMap};

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
    Ok(Json(
        state
            .crawler_store
            .claim_jobs(
                &crawler_id,
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
    Ok(Json(
        state
            .crawler_store
            .submit_report(
                &crawler_id,
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

fn crawler_id_from_headers(headers: &HeaderMap) -> Result<String, ApiError> {
    headers
        .get("x-crawler-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| ApiError::Unauthorized("missing x-crawler-id header".to_string()))
}

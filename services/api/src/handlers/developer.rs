use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};

use crate::{
    ControlState,
    dev_auth::DevUserIdentity,
    error::ApiError,
    models::{
        CreateKeyRequest, DevLoginRequest, DevRegisterRequest, DevSessionResponse,
        DeveloperDomainInsightQuery, DeveloperDomainInsightResponse, DeveloperDomainSubmitRequest,
        DeveloperDomainSubmitResponse, DeveloperUsageResponse,
    },
};

pub async fn dev_register(
    State(state): State<ControlState>,
    Json(request): Json<DevRegisterRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let session = state.dev_auth.register(request).await?;
    Ok((StatusCode::CREATED, Json(session)))
}

pub async fn dev_login(
    State(state): State<ControlState>,
    Json(request): Json<DevLoginRequest>,
) -> Result<Json<DevSessionResponse>, ApiError> {
    Ok(Json(state.dev_auth.login(request).await?))
}

pub async fn dev_me(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<Json<DevSessionResponse>, ApiError> {
    Ok(Json(
        state
            .dev_auth
            .current_session(headers.get("authorization").and_then(|v| v.to_str().ok()))
            .await?,
    ))
}

pub async fn dev_logout(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    state
        .dev_auth
        .logout(headers.get("authorization").and_then(|v| v.to_str().ok()))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn dev_list_keys(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<Json<DeveloperUsageResponse>, ApiError> {
    let dev = authorize_dev(&state, &headers).await?;
    Ok(Json(
        state
            .query
            .developer_store
            .developer_usage(&dev.user_id)
            .await?,
    ))
}

pub async fn dev_create_key(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Json(request): Json<CreateKeyRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let dev = authorize_dev(&state, &headers).await?;
    let created = state
        .query
        .developer_store
        .create_developer_key(&dev.user_id, request)
        .await?;
    Ok((StatusCode::CREATED, Json(created)))
}

pub async fn dev_revoke_key(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let dev = authorize_dev(&state, &headers).await?;
    state
        .query
        .developer_store
        .revoke_developer_key(&dev.user_id, &id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn dev_domain_insight(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Query(query): Query<DeveloperDomainInsightQuery>,
) -> Result<Json<DeveloperDomainInsightResponse>, ApiError> {
    let _ = authorize_dev(&state, &headers).await?;
    Ok(Json(state.crawl_store.domain_insight(&query.domain).await?))
}

pub async fn dev_submit_domain(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Json(request): Json<DeveloperDomainSubmitRequest>,
) -> Result<Json<DeveloperDomainSubmitResponse>, ApiError> {
    let dev = authorize_dev(&state, &headers).await?;
    Ok(Json(
        state
            .crawl_store
            .submit_domain_urls(&state.default_crawler_owner_id, &dev.user_id, request)
            .await?,
    ))
}

async fn authorize_dev(
    state: &ControlState,
    headers: &HeaderMap,
) -> Result<DevUserIdentity, ApiError> {
    state
        .dev_auth
        .authorize(headers.get("authorization").and_then(|v| v.to_str().ok()))
        .await
}

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};

use crate::{
    ControlState,
    dev_auth::UserIdentity,
    error::ApiError,
    models::{
        CreateKeyRequest, UserLoginRequest, UserRegisterRequest, UserSessionResponse,
        DeveloperDomainInsightQuery, DeveloperDomainInsightResponse, DeveloperDomainSubmitRequest,
        DeveloperDomainSubmitResponse, DeveloperUsageResponse,
    },
};

pub async fn user_register(
    State(state): State<ControlState>,
    Json(request): Json<UserRegisterRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let session = state.dev_auth.register(request).await?;
    Ok((StatusCode::CREATED, Json(session)))
}

pub async fn user_login(
    State(state): State<ControlState>,
    Json(request): Json<UserLoginRequest>,
) -> Result<Json<UserSessionResponse>, ApiError> {
    Ok(Json(state.dev_auth.login(request).await?))
}

pub async fn user_me(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<Json<UserSessionResponse>, ApiError> {
    Ok(Json(
        state
            .dev_auth
            .current_session(headers.get("authorization").and_then(|v| v.to_str().ok()))
            .await?,
    ))
}

pub async fn user_logout(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    state
        .dev_auth
        .logout(headers.get("authorization").and_then(|v| v.to_str().ok()))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn user_list_keys(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<Json<DeveloperUsageResponse>, ApiError> {
    let user = authorize_user(&state, &headers).await?;
    Ok(Json(
        state
            .query
            .developer_store
            .developer_usage(&user.user_id)
            .await?,
    ))
}

pub async fn user_create_key(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Json(request): Json<CreateKeyRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let user = authorize_user(&state, &headers).await?;
    let created = state
        .query
        .developer_store
        .create_developer_key(&user.user_id, request)
        .await?;
    Ok((StatusCode::CREATED, Json(created)))
}

pub async fn user_revoke_key(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let user = authorize_user(&state, &headers).await?;
    state
        .query
        .developer_store
        .revoke_developer_key(&user.user_id, &id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn user_domain_insight(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Query(query): Query<DeveloperDomainInsightQuery>,
) -> Result<Json<DeveloperDomainInsightResponse>, ApiError> {
    let _ = authorize_user(&state, &headers).await?;
    Ok(Json(state.crawl_store.domain_insight(&query.domain).await?))
}

pub async fn user_submit_domain(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Json(request): Json<DeveloperDomainSubmitRequest>,
) -> Result<Json<DeveloperDomainSubmitResponse>, ApiError> {
    let user = authorize_user(&state, &headers).await?;
    Ok(Json(
        state
            .crawl_store
            .submit_domain_urls(&state.default_crawler_owner_id, &user.user_id, request)
            .await?,
    ))
}

async fn authorize_user(
    state: &ControlState,
    headers: &HeaderMap,
) -> Result<UserIdentity, ApiError> {
    state
        .dev_auth
        .authorize(headers.get("authorization").and_then(|v| v.to_str().ok()))
        .await
}

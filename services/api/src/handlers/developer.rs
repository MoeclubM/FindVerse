use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};

use crate::{
    AppState,
    dev_auth::DevUserIdentity,
    error::ApiError,
    models::{
        CreateKeyRequest, DevLoginRequest, DevRegisterRequest, DevSessionResponse,
        DeveloperUsageResponse,
    },
};

pub async fn dev_register(
    State(state): State<Arc<AppState>>,
    Json(request): Json<DevRegisterRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let session = state.dev_auth.register(request).await?;
    Ok((StatusCode::CREATED, Json(session)))
}

pub async fn dev_login(
    State(state): State<Arc<AppState>>,
    Json(request): Json<DevLoginRequest>,
) -> Result<Json<DevSessionResponse>, ApiError> {
    Ok(Json(state.dev_auth.login(request).await?))
}

pub async fn dev_me(
    State(state): State<Arc<AppState>>,
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
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    state
        .dev_auth
        .logout(headers.get("authorization").and_then(|v| v.to_str().ok()))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn dev_list_keys(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<DeveloperUsageResponse>, ApiError> {
    let dev = authorize_dev(&state, &headers).await?;
    Ok(Json(state.developer_store.usage(&dev.user_id).await?))
}

pub async fn dev_create_key(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<CreateKeyRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let dev = authorize_dev(&state, &headers).await?;
    let created = state
        .developer_store
        .create_key(&dev.user_id, request)
        .await?;
    Ok((StatusCode::CREATED, Json(created)))
}

pub async fn dev_revoke_key(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let dev = authorize_dev(&state, &headers).await?;
    state.developer_store.revoke_key(&dev.user_id, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn authorize_dev(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<DevUserIdentity, ApiError> {
    state
        .dev_auth
        .authorize(headers.get("authorization").and_then(|v| v.to_str().ok()))
        .await
}

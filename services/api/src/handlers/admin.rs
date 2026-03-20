use std::{sync::Arc, time::Duration};

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};

use crate::{
    AppState,
    admin::AdminIdentity,
    error::ApiError,
    models::{
        AdminDeveloperRecord, AdminLoginRequest, AdminSessionResponse, CrawlOverviewResponse,
        CrawlRule, CrawlerJoinKeyResponse, CreateCrawlRuleRequest, CreateCrawlerRequest,
        CreateKeyRequest, CreatedCrawlerResponse, DeveloperUsageResponse, DocumentListParams,
        DocumentListResponse, PurgeSiteRequest, PurgeSiteResponse, SeedFrontierRequest,
        SeedFrontierResponse, UpdateCrawlRuleRequest, UpdateDeveloperRequest,
    },
    store::DeveloperStore,
};

pub async fn admin_login(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AdminLoginRequest>,
) -> Result<Json<AdminSessionResponse>, ApiError> {
    Ok(Json(state.admin_auth.login(request).await?))
}

pub async fn admin_session_me(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<AdminSessionResponse>, ApiError> {
    Ok(Json(
        state
            .admin_auth
            .current_session(headers.get("authorization").and_then(|value| value.to_str().ok()))
            .await?,
    ))
}

pub async fn admin_logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    state
        .admin_auth
        .logout(headers.get("authorization").and_then(|value| value.to_str().ok()))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn admin_usage(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<DeveloperUsageResponse>, ApiError> {
    let admin = authorize_admin(&state, &headers).await?;
    Ok(Json(state.developer_store.usage(&admin.developer_id).await?))
}

pub async fn admin_create_key(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<CreateKeyRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let admin = authorize_admin(&state, &headers).await?;
    let created = state
        .developer_store
        .create_key(&admin.developer_id, request)
        .await?;
    state
        .crawler_store
        .record_admin_event(
            &admin.developer_id,
            "api-key-created",
            "ok",
            format!("created api key {}", created.name),
            None,
            None,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(created)))
}

pub async fn admin_revoke_key(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let admin = authorize_admin(&state, &headers).await?;
    state
        .developer_store
        .revoke_key(&admin.developer_id, &id)
        .await?;
    state
        .crawler_store
        .record_admin_event(
            &admin.developer_id,
            "api-key-revoked",
            "ok",
            format!("revoked api key {id}"),
            None,
            None,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn admin_create_crawler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<CreateCrawlerRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let admin = authorize_admin(&state, &headers).await?;
    let created: CreatedCrawlerResponse = state
        .crawler_store
        .create_crawler(&admin.developer_id, request)
        .await?;
    Ok((StatusCode::CREATED, Json(created)))
}

pub async fn admin_seed_frontier(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<SeedFrontierRequest>,
) -> Result<Json<SeedFrontierResponse>, ApiError> {
    let admin = authorize_admin(&state, &headers).await?;
    Ok(Json(
        state
            .crawler_store
            .seed_frontier(&admin.developer_id, request)
            .await?,
    ))
}

pub async fn admin_crawl_overview(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<CrawlOverviewResponse>, ApiError> {
    let admin = authorize_admin(&state, &headers).await?;
    Ok(Json(
        state
            .crawler_store
            .overview(&admin.developer_id, state.search_index.total_documents())
            .await?,
    ))
}

pub async fn admin_create_rule(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<CreateCrawlRuleRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let admin = authorize_admin(&state, &headers).await?;
    let created = state
        .crawler_store
        .create_rule(&admin.developer_id, request)
        .await?;
    if created.enabled {
        state
            .crawler_store
            .run_maintenance(Duration::from_secs(state.crawler_claim_timeout_secs))
            .await?;
    }
    Ok((StatusCode::CREATED, Json(created)))
}

pub async fn admin_update_rule(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<UpdateCrawlRuleRequest>,
) -> Result<Json<CrawlRule>, ApiError> {
    let admin = authorize_admin(&state, &headers).await?;
    Ok(Json(
        state
            .crawler_store
            .update_rule(&admin.developer_id, &id, request)
            .await?,
    ))
}

pub async fn admin_delete_rule(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let admin = authorize_admin(&state, &headers).await?;
    state
        .crawler_store
        .delete_rule(&admin.developer_id, &id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn admin_list_documents(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<DocumentListParams>,
) -> Result<Json<DocumentListResponse>, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    Ok(Json(state.search_index.list_documents(params)))
}

pub async fn admin_delete_document(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let admin = authorize_admin(&state, &headers).await?;
    let deleted = state.search_index.delete_document(&id).await?;
    if !deleted {
        return Err(ApiError::NotFound("document not found".to_string()));
    }

    state
        .crawler_store
        .record_admin_event(
            &admin.developer_id,
            "document-deleted",
            "ok",
            format!("deleted indexed document {id}"),
            None,
            None,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn admin_purge_site(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<PurgeSiteRequest>,
) -> Result<Json<PurgeSiteResponse>, ApiError> {
    let admin = authorize_admin(&state, &headers).await?;
    let response = state.search_index.purge_site(&request.site).await?;
    state
        .crawler_store
        .record_admin_event(
            &admin.developer_id,
            "site-purged",
            "ok",
            format!(
                "purged {} documents for site {}",
                response.deleted_documents, request.site
            ),
            Some(request.site),
            None,
        )
        .await?;
    Ok(Json(response))
}

pub async fn admin_get_join_key(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<CrawlerJoinKeyResponse>, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    let key = state
        .crawler_store
        .get_join_key(state.crawler_join_key.as_deref())
        .await;
    Ok(Json(CrawlerJoinKeyResponse { join_key: key }))
}

pub async fn admin_set_join_key(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<CrawlerJoinKeyResponse>,
) -> Result<StatusCode, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    state
        .crawler_store
        .set_join_key(body.join_key.filter(|k| !k.is_empty()))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn admin_list_developers(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<AdminDeveloperRecord>>, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    let accounts = state.dev_auth.list_accounts().await;
    let usages = state.developer_store.list_all_usage().await;
    let usage_map: std::collections::HashMap<String, &DeveloperUsageResponse> =
        usages.iter().map(|u| (u.developer_id.clone(), u)).collect();

    let records = accounts
        .iter()
        .map(|account| {
            let default_usage = DeveloperUsageResponse {
                developer_id: account.user_id.clone(),
                qps_limit: 5,
                daily_limit: 10_000,
                used_today: 0,
                keys: vec![],
            };
            let usage = usage_map.get(&account.user_id).copied().unwrap_or(&default_usage);
            DeveloperStore::build_admin_developer_record(
                usage,
                &account.username,
                account.enabled,
                account.created_at,
            )
        })
        .collect();

    Ok(Json(records))
}

pub async fn admin_update_developer(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(request): Json<UpdateDeveloperRequest>,
) -> Result<StatusCode, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    // Toggle enabled/disabled in dev auth store
    if let Some(enabled) = request.enabled {
        state.dev_auth.set_enabled(&user_id, enabled).await?;
    }
    // Update quota in developer store (only if quota fields are provided)
    if request.qps_limit.is_some() || request.daily_limit.is_some() {
        let quota_request = UpdateDeveloperRequest {
            qps_limit: request.qps_limit,
            daily_limit: request.daily_limit,
            enabled: None,
        };
        // create empty record if needed by getting usage first (upserts the record)
        let _ = state.developer_store.usage(&user_id).await;
        state.developer_store.update_quota(&user_id, quota_request).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn authorize_admin(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<AdminIdentity, ApiError> {
    state
        .admin_auth
        .authorize(headers.get("authorization").and_then(|value| value.to_str().ok()))
        .await
}

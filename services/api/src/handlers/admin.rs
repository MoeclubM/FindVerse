use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};

use crate::{
    ControlState,
    admin::AdminIdentity,
    error::ApiError,
    models::{
        AdminDeveloperRecord, AdminLoginRequest, AdminSessionResponse, CrawlJobListParams,
        CrawlJobListResponse, CrawlJobStats, CrawlOriginState, CrawlOverviewResponse, CrawlRule,
        CreateCrawlRuleRequest, CreateKeyRequest, CreatedKeyResponse, DeveloperDomainInsightQuery,
        DeveloperDomainInsightResponse, DeveloperUsageResponse, DocumentListParams,
        DocumentListResponse, PurgeSiteRequest, PurgeSiteResponse, SeedFrontierRequest,
        SeedFrontierResponse, SetSystemConfigRequest, SystemConfigResponse, UpdateCrawlRuleRequest,
        UpdateCrawlerRequest, UpdateDeveloperRequest,
    },
    store::DeveloperStore,
};

pub async fn admin_login(
    State(state): State<ControlState>,
    Json(request): Json<AdminLoginRequest>,
) -> Result<Json<AdminSessionResponse>, ApiError> {
    Ok(Json(state.admin_auth.login(request).await?))
}

pub async fn admin_session_me(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<Json<AdminSessionResponse>, ApiError> {
    Ok(Json(
        state
            .admin_auth
            .current_session(
                headers
                    .get("authorization")
                    .and_then(|value| value.to_str().ok()),
            )
            .await?,
    ))
}

pub async fn admin_logout(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    state
        .admin_auth
        .logout(
            headers
                .get("authorization")
                .and_then(|value| value.to_str().ok()),
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn admin_list_developer_keys(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> Result<Json<DeveloperUsageResponse>, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    Ok(Json(
        state
            .query
            .developer_store
            .list_developer_keys(&user_id)
            .await?,
    ))
}

pub async fn admin_create_developer_key(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(request): Json<CreateKeyRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    let created: CreatedKeyResponse = state
        .query
        .developer_store
        .create_developer_key(&user_id, request)
        .await?;
    state
        .crawler_store
        .record_admin_event(
            &state.default_crawler_owner_id,
            "developer-api-key-created",
            "ok",
            format!("created api key {} for developer {user_id}", created.name),
            None,
            None,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(created)))
}

pub async fn admin_revoke_developer_key(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path((user_id, key_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    state
        .query
        .developer_store
        .revoke_developer_key(&user_id, &key_id)
        .await?;
    state
        .crawler_store
        .record_admin_event(
            &state.default_crawler_owner_id,
            "developer-api-key-revoked",
            "ok",
            format!("revoked api key {key_id} for developer {user_id}"),
            None,
            None,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn admin_update_crawler(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<UpdateCrawlerRequest>,
) -> Result<StatusCode, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    state
        .crawler_store
        .update_crawler(&state.default_crawler_owner_id, &id, request)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn admin_delete_crawler(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    state
        .crawler_store
        .delete_crawler(&state.default_crawler_owner_id, &id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn admin_seed_frontier(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Json(request): Json<SeedFrontierRequest>,
) -> Result<Json<SeedFrontierResponse>, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    Ok(Json(
        state
            .crawler_store
            .seed_frontier(&state.default_crawler_owner_id, request)
            .await?,
    ))
}

pub async fn admin_crawl_overview(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<Json<CrawlOverviewResponse>, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    Ok(Json(
        state
            .crawler_store
            .overview(
                &state.default_crawler_owner_id,
                state.query.search_index.total_documents().await,
            )
            .await?,
    ))
}

pub async fn admin_create_rule(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Json(request): Json<CreateCrawlRuleRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    let created = state
        .crawler_store
        .create_rule(&state.default_crawler_owner_id, request)
        .await?;
    Ok((StatusCode::CREATED, Json(created)))
}

pub async fn admin_update_rule(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<UpdateCrawlRuleRequest>,
) -> Result<Json<CrawlRule>, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    Ok(Json(
        state
            .crawler_store
            .update_rule(&state.default_crawler_owner_id, &id, request)
            .await?,
    ))
}

pub async fn admin_delete_rule(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    state
        .crawler_store
        .delete_rule(&state.default_crawler_owner_id, &id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn admin_list_documents(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Query(params): Query<DocumentListParams>,
) -> Result<Json<DocumentListResponse>, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    Ok(Json(state.query.search_index.list_documents(params).await))
}

pub async fn admin_delete_document(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    let deleted = state.query.search_index.delete_document(&id).await?;
    if !deleted {
        return Err(ApiError::NotFound("document not found".to_string()));
    }

    state
        .crawler_store
        .record_admin_event(
            &state.default_crawler_owner_id,
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
    State(state): State<ControlState>,
    headers: HeaderMap,
    Json(request): Json<PurgeSiteRequest>,
) -> Result<Json<PurgeSiteResponse>, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    let response = state.query.search_index.purge_site(&request.site).await?;
    state
        .crawler_store
        .record_admin_event(
            &state.default_crawler_owner_id,
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

pub async fn admin_list_system_config(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<Json<SystemConfigResponse>, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    let entries = state.crawler_store.get_all_system_config().await?;
    Ok(Json(SystemConfigResponse { entries }))
}

pub async fn admin_set_system_config(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(key): Path<String>,
    Json(body): Json<SetSystemConfigRequest>,
) -> Result<StatusCode, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    let allowed = matches!(
        key.as_str(),
        "crawler.auth_key"
            | "crawler.claim_timeout_secs"
            | "crawler.total_concurrency"
            | "crawler.js_render_concurrency"
            | "crawler.max_attempts"
            | "crawler.tor_proxy_url"
            | "crawler.tor_enabled"
    );
    if !allowed {
        return Err(ApiError::BadRequest(format!("unknown config key: {key}")));
    }
    if matches!(
        key.as_str(),
        "crawler.total_concurrency" | "crawler.js_render_concurrency"
    ) {
        if let Some(value) = body.value.as_deref() {
            let parsed = value
                .trim()
                .parse::<usize>()
                .map_err(|_| ApiError::BadRequest(format!("{key} must be a positive integer")))?;
            if parsed == 0 {
                return Err(ApiError::BadRequest(format!(
                    "{key} must be a positive integer"
                )));
            }
        }
    }
    state
        .crawler_store
        .set_system_config(&key, body.value)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn admin_list_developers(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<Json<Vec<AdminDeveloperRecord>>, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    let accounts = state.dev_auth.list_accounts().await;
    let usages = state.query.developer_store.list_all_developer_usage().await;
    let usage_map: std::collections::HashMap<String, &DeveloperUsageResponse> =
        usages.iter().map(|u| (u.developer_id.clone(), u)).collect();

    let records = accounts
        .iter()
        .map(|account| {
            let default_usage = DeveloperUsageResponse {
                developer_id: account.user_id.clone(),
                daily_limit: 10_000,
                used_today: 0,
                keys: vec![],
            };
            let usage = usage_map
                .get(&account.user_id)
                .copied()
                .unwrap_or(&default_usage);
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
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
    Json(request): Json<UpdateDeveloperRequest>,
) -> Result<StatusCode, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    // Toggle enabled/disabled in dev auth store
    if let Some(enabled) = request.enabled {
        state.dev_auth.set_enabled(&user_id, enabled).await?;
    }
    if request.daily_limit.is_some() {
        let quota_request = UpdateDeveloperRequest {
            daily_limit: request.daily_limit,
            enabled: None,
            password: None,
        };
        // ensure the developer account exists before applying quota changes
        let _ = state
            .query
            .developer_store
            .developer_usage(&user_id)
            .await?;
        state
            .query
            .developer_store
            .update_developer_quota(&user_id, quota_request)
            .await?;
    }
    if let Some(password) = request.password.as_deref() {
        state.dev_auth.update_password(&user_id, password).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

pub async fn admin_delete_developer(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Path(user_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    state.dev_auth.delete_account(&user_id).await?;
    state
        .crawler_store
        .record_admin_event(
            &state.default_crawler_owner_id,
            "developer-deleted",
            "ok",
            format!("deleted developer {user_id}"),
            None,
            None,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn admin_list_jobs(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Query(params): Query<CrawlJobListParams>,
) -> Result<Json<CrawlJobListResponse>, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    let limit = params.limit.clamp(1, 100) as i64;
    let offset = params.offset as i64;
    Ok(Json(
        state
            .crawler_store
            .list_jobs(
                &state.default_crawler_owner_id,
                params.status.as_deref(),
                limit,
                offset,
            )
            .await?,
    ))
}

pub async fn admin_job_stats(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<Json<CrawlJobStats>, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    Ok(Json(
        state
            .crawler_store
            .job_stats(&state.default_crawler_owner_id)
            .await?,
    ))
}

pub async fn admin_list_origins(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<Json<Vec<CrawlOriginState>>, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    Ok(Json(
        state
            .crawler_store
            .list_origins(&state.default_crawler_owner_id)
            .await?,
    ))
}

pub async fn admin_domain_insight(
    State(state): State<ControlState>,
    headers: HeaderMap,
    Query(query): Query<DeveloperDomainInsightQuery>,
) -> Result<Json<DeveloperDomainInsightResponse>, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    Ok(Json(
        state.crawler_store.domain_insight(&query.domain).await?,
    ))
}

pub async fn admin_retry_failed_jobs(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    let count = state
        .crawler_store
        .retry_failed_jobs(&state.default_crawler_owner_id)
        .await?;
    Ok(Json(serde_json::json!({ "retried": count })))
}

pub async fn admin_cleanup_completed_jobs(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    let count = state
        .crawler_store
        .cleanup_completed_jobs(&state.default_crawler_owner_id)
        .await?;
    Ok(Json(serde_json::json!({ "cleaned": count })))
}

pub async fn admin_cleanup_failed_jobs(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    let count = state
        .crawler_store
        .cleanup_failed_jobs(&state.default_crawler_owner_id)
        .await?;
    Ok(Json(serde_json::json!({ "cleaned": count })))
}

pub async fn admin_stop_all_jobs(
    State(state): State<ControlState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    let (disabled_rules, removed_jobs) = state
        .crawler_store
        .stop_all_jobs(&state.default_crawler_owner_id)
        .await?;
    Ok(Json(serde_json::json!({
        "disabled_rules": disabled_rules,
        "removed_jobs": removed_jobs
    })))
}

async fn authorize_admin(
    state: &ControlState,
    headers: &HeaderMap,
) -> Result<AdminIdentity, ApiError> {
    let identity = state
        .admin_auth
        .authorize(
            headers
                .get("authorization")
                .and_then(|value| value.to_str().ok()),
        )
        .await?;
    let _ = &identity.user_id;
    Ok(identity)
}

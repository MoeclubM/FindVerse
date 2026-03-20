mod admin;
mod crawler;
mod config;
mod error;
mod models;
mod store;

use std::{collections::HashMap, sync::Arc};

use admin::AdminAuth;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    routing::{delete, get, patch, post},
};
use models::{
    AdminLoginRequest, ClaimJobsRequest, ClaimJobsResponse, CrawlOverviewResponse,
    CreatedCrawlerResponse, DeveloperUsageResponse, DocumentListParams, DocumentListResponse,
    HealthResponse, PurgeSiteRequest, PurgeSiteResponse, SearchParams, SearchResponse,
    SeedFrontierRequest, SeedFrontierResponse, SubmitCrawlReportRequest,
    SubmitCrawlReportResponse, SuggestResponse,
};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::info;

use crate::{
    config::Config,
    crawler::CrawlerStore,
    error::ApiError,
    models::{CreateCrawlRuleRequest, CreateCrawlerRequest, CreateKeyRequest, UpdateCrawlRuleRequest},
    store::{DeveloperStore, SearchIndex},
};

#[derive(Clone)]
struct AppState {
    search_index: SearchIndex,
    developer_store: DeveloperStore,
    crawler_store: CrawlerStore,
    admin_auth: AdminAuth,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "findverse_api=debug,tower_http=info".to_string()),
        )
        .init();

    let config = Config::from_env()?;
    let search_index = SearchIndex::load(config.index_path.clone()).await?;
    let developer_store = DeveloperStore::load(config.developer_store_path.clone()).await?;
    let crawler_store = CrawlerStore::load(config.crawler_store_path.clone()).await?;
    let admin_auth = AdminAuth::new(
        config.local_admin_username.clone(),
        config.local_admin_password.clone(),
    );

    let state = AppState {
        search_index,
        developer_store,
        crawler_store,
        admin_auth,
    };

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/search", get(search))
        .route("/v1/suggest", get(suggest))
        .route("/v1/admin/session/login", post(admin_login))
        .route("/v1/admin/session/me", get(admin_session_me))
        .route("/v1/admin/session/logout", post(admin_logout))
        .route("/v1/admin/usage", get(admin_usage))
        .route("/v1/admin/api-keys", post(admin_create_key))
        .route("/v1/admin/api-keys/{id}", delete(admin_revoke_key))
        .route("/v1/admin/crawlers", post(admin_create_crawler))
        .route("/v1/admin/frontier/seed", post(admin_seed_frontier))
        .route("/v1/admin/crawl/overview", get(admin_crawl_overview))
        .route("/v1/admin/crawl/rules", post(admin_create_rule))
        .route(
            "/v1/admin/crawl/rules/{id}",
            patch(admin_update_rule).delete(admin_delete_rule),
        )
        .route("/v1/admin/documents", get(admin_list_documents))
        .route("/v1/admin/documents/{id}", delete(admin_delete_document))
        .route("/v1/admin/documents/purge-site", post(admin_purge_site))
        .route("/internal/crawlers/claim", post(claim_jobs))
        .route("/internal/crawlers/report", post(submit_crawl_report))
        .layer(
            CorsLayer::new()
                .allow_origin(config.frontend_origin.parse::<axum::http::HeaderValue>()?)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(Arc::new(state));

    info!("findverse api listening on {}", config.bind_addr);
    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn healthz(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        documents: state.search_index.total_documents(),
    })
}

async fn search(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<SearchParams>,
) -> Result<Json<SearchResponse>, ApiError> {
    if params.q.trim().is_empty() {
        return Err(ApiError::BadRequest("query must not be empty".to_string()));
    }

    state
        .developer_store
        .validate_and_track(headers.get("authorization").and_then(|value| value.to_str().ok()))
        .await?;

    Ok(Json(state.search_index.search(params)))
}

async fn suggest(
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

async fn admin_login(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AdminLoginRequest>,
) -> Result<Json<crate::models::AdminSessionResponse>, ApiError> {
    Ok(Json(state.admin_auth.login(request).await?))
}

async fn admin_session_me(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<crate::models::AdminSessionResponse>, ApiError> {
    Ok(Json(
        state
            .admin_auth
            .current_session(headers.get("authorization").and_then(|value| value.to_str().ok()))
            .await?,
    ))
}

async fn admin_logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<axum::http::StatusCode, ApiError> {
    state
        .admin_auth
        .logout(headers.get("authorization").and_then(|value| value.to_str().ok()))
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn admin_usage(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<DeveloperUsageResponse>, ApiError> {
    let admin = authorize_admin(&state, &headers).await?;
    Ok(Json(state.developer_store.usage(&admin.developer_id).await?))
}

async fn admin_create_key(
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
    Ok((axum::http::StatusCode::CREATED, Json(created)))
}

async fn admin_revoke_key(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<axum::http::StatusCode, ApiError> {
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
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn admin_create_crawler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<CreateCrawlerRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let admin = authorize_admin(&state, &headers).await?;
    let created: CreatedCrawlerResponse = state
        .crawler_store
        .create_crawler(&admin.developer_id, request)
        .await?;
    Ok((axum::http::StatusCode::CREATED, Json(created)))
}

async fn admin_seed_frontier(
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

async fn admin_crawl_overview(
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

async fn admin_create_rule(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<CreateCrawlRuleRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let admin = authorize_admin(&state, &headers).await?;
    let created = state
        .crawler_store
        .create_rule(&admin.developer_id, request)
        .await?;
    Ok((axum::http::StatusCode::CREATED, Json(created)))
}

async fn admin_update_rule(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(request): Json<UpdateCrawlRuleRequest>,
) -> Result<Json<crate::models::CrawlRule>, ApiError> {
    let admin = authorize_admin(&state, &headers).await?;
    Ok(Json(
        state
            .crawler_store
            .update_rule(&admin.developer_id, &id, request)
            .await?,
    ))
}

async fn admin_delete_rule(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<axum::http::StatusCode, ApiError> {
    let admin = authorize_admin(&state, &headers).await?;
    state
        .crawler_store
        .delete_rule(&admin.developer_id, &id)
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn admin_list_documents(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<DocumentListParams>,
) -> Result<Json<DocumentListResponse>, ApiError> {
    let _admin = authorize_admin(&state, &headers).await?;
    Ok(Json(state.search_index.list_documents(params)))
}

async fn admin_delete_document(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<axum::http::StatusCode, ApiError> {
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
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn admin_purge_site(
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

async fn claim_jobs(
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

async fn submit_crawl_report(
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

async fn authorize_admin(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<admin::AdminIdentity, ApiError> {
    state
        .admin_auth
        .authorize(headers.get("authorization").and_then(|value| value.to_str().ok()))
        .await
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

mod crawler;
mod config;
mod error;
mod models;
mod store;

use std::{collections::HashMap, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post},
};
use models::{
    ClaimJobsRequest, CreateCrawlerRequest, CreateKeyRequest, HealthResponse, SearchParams,
    SeedFrontierRequest, SubmitCrawlReportRequest,
};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing::info;

use crate::{
    crawler::CrawlerStore,
    config::Config,
    error::ApiError,
    models::{
        ClaimJobsResponse, CrawlOverviewResponse, DeveloperUsageResponse, SearchResponse,
        SeedFrontierResponse, SubmitCrawlReportResponse, SuggestResponse,
    },
    store::{DeveloperStore, SearchIndex},
};

#[derive(Clone)]
struct AppState {
    search_index: SearchIndex,
    developer_store: DeveloperStore,
    crawler_store: CrawlerStore,
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

    let state = AppState {
        search_index,
        developer_store,
        crawler_store,
    };

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/search", get(search))
        .route("/v1/suggest", get(suggest))
        .route("/v1/developer/keys", post(create_key))
        .route("/v1/developer/keys/{id}", delete(revoke_key))
        .route("/v1/developer/usage", get(developer_usage))
        .route(
            "/v1/developer/crawlers",
            get(list_crawlers).post(create_crawler),
        )
        .route("/v1/developer/frontier/seed", post(seed_frontier))
        .route("/v1/developer/crawl/overview", get(crawl_overview))
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

async fn create_key(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<CreateKeyRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let developer_id = developer_id_from_headers(&headers)?;
    let response = state.developer_store.create_key(&developer_id, request).await?;

    Ok((StatusCode::CREATED, Json(response)))
}

async fn revoke_key(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let developer_id = developer_id_from_headers(&headers)?;
    state.developer_store.revoke_key(&developer_id, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn developer_usage(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<DeveloperUsageResponse>, ApiError> {
    let developer_id = developer_id_from_headers(&headers)?;
    let usage = state.developer_store.usage(&developer_id).await?;
    Ok(Json(usage))
}

async fn list_crawlers(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<CrawlOverviewResponse>, ApiError> {
    let developer_id = developer_id_from_headers(&headers)?;
    let overview = state
        .crawler_store
        .overview(&developer_id, state.search_index.total_documents())
        .await?;
    Ok(Json(overview))
}

async fn create_crawler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<CreateCrawlerRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let developer_id = developer_id_from_headers(&headers)?;
    let created = state
        .crawler_store
        .create_crawler(&developer_id, request)
        .await?;
    Ok((StatusCode::CREATED, Json(created)))
}

async fn seed_frontier(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<SeedFrontierRequest>,
) -> Result<Json<SeedFrontierResponse>, ApiError> {
    let developer_id = developer_id_from_headers(&headers)?;
    let seeded = state
        .crawler_store
        .seed_frontier(&developer_id, request)
        .await?;
    Ok(Json(seeded))
}

async fn crawl_overview(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<CrawlOverviewResponse>, ApiError> {
    let developer_id = developer_id_from_headers(&headers)?;
    let overview = state
        .crawler_store
        .overview(&developer_id, state.search_index.total_documents())
        .await?;
    Ok(Json(overview))
}

async fn claim_jobs(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<ClaimJobsRequest>,
) -> Result<Json<ClaimJobsResponse>, ApiError> {
    let crawler_id = crawler_id_from_headers(&headers)?;
    let claimed = state
        .crawler_store
        .claim_jobs(
            &crawler_id,
            headers.get("authorization").and_then(|value| value.to_str().ok()),
            request,
        )
        .await?;
    Ok(Json(claimed))
}

async fn submit_crawl_report(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<SubmitCrawlReportRequest>,
) -> Result<Json<SubmitCrawlReportResponse>, ApiError> {
    let crawler_id = crawler_id_from_headers(&headers)?;
    let response = state
        .crawler_store
        .submit_report(
            &crawler_id,
            headers.get("authorization").and_then(|value| value.to_str().ok()),
            request,
            &state.search_index,
        )
        .await?;
    Ok(Json(response))
}

fn developer_id_from_headers(headers: &HeaderMap) -> Result<String, ApiError> {
    headers
        .get("x-developer-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| ApiError::Unauthorized("missing x-developer-id header".to_string()))
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

use std::env;
use std::path::{Component, Path, PathBuf};

use axum::{
    Router,
    body::Bytes,
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header::CONTENT_TYPE},
    response::IntoResponse,
    routing::get,
};
use reqwest::Client;
use serde::Deserialize;
use sqlx::PgPool;
use tokio::fs;
use uuid::Uuid;

use crate::{
    config::{Config, ServiceKind},
    error::ApiError,
    models::CrawlResultInput,
};

#[derive(Debug, Clone)]
pub struct BlobStore {
    pg_pool: PgPool,
    http_client: Client,
    base_url: String,
}

#[derive(Debug, Clone)]
struct BlobStorageState {
    root_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
struct PutBlobQuery {
    content_type: Option<String>,
}

pub async fn run_blob_storage() -> anyhow::Result<()> {
    let config = Config::from_env(ServiceKind::BlobStorage)?;
    let state = BlobStorageState {
        root_dir: env::var("FINDVERSE_BLOB_STORE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("data/blobs")),
    };
    fs::create_dir_all(&state.root_dir).await?;

    let listener =
        tokio::net::TcpListener::bind(config.bind_addr.expect("blob-storage bind addr")).await?;
    axum::serve(listener, build_blob_storage_router(state)).await?;
    Ok(())
}

impl BlobStore {
    pub fn new(pg_pool: PgPool, base_url: String) -> Self {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("blob store http client");

        Self {
            pg_pool,
            http_client,
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    pub async fn write_result(
        &self,
        owner_developer_id: &str,
        crawler_id: &str,
        lease_id: &str,
        result: &CrawlResultInput,
    ) -> Result<String, ApiError> {
        let body = serde_json::to_vec(result).map_err(|error| ApiError::Internal(error.into()))?;
        let body_len = body.len();
        let blob_key = format!("crawl-results/{lease_id}/{}.json", result.job_id);
        self.write_blob_bytes(&blob_key, body, "application/json")
            .await?;

        sqlx::query_scalar(
            "insert into crawl_result_blobs (
                id,
                owner_developer_id,
                crawler_id,
                crawl_job_id,
                lease_id,
                blob_key,
                blob_size_bytes,
                blob_content_type,
                created_at
             )
             values ($1, $2, $3, $4, $5, $6, $7, $8, now())
             on conflict (lease_id, crawl_job_id) do update
             set owner_developer_id = excluded.owner_developer_id,
                 crawler_id = excluded.crawler_id,
                 blob_key = excluded.blob_key,
                 blob_size_bytes = excluded.blob_size_bytes,
                 blob_content_type = excluded.blob_content_type
             returning id",
        )
        .bind(Uuid::now_v7().to_string())
        .bind(owner_developer_id)
        .bind(crawler_id)
        .bind(&result.job_id)
        .bind(lease_id)
        .bind(&blob_key)
        .bind(body_len as i64)
        .bind("application/json")
        .fetch_one(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))
    }

    pub async fn load_result(&self, blob_id: &str) -> Result<CrawlResultInput, ApiError> {
        let row = sqlx::query_as::<_, StoredBlobRow>(
            "select blob_key from crawl_result_blobs where id = $1",
        )
        .bind(blob_id)
        .fetch_one(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;
        let blob_key = row.blob_key.ok_or_else(|| {
            ApiError::Internal(anyhow::anyhow!(
                "crawl result blob {blob_id} is missing blob_key; run bootstrap migration first"
            ))
        })?;
        let bytes = self.read_blob_bytes(&blob_key).await?;
        serde_json::from_slice(&bytes).map_err(|error| ApiError::Internal(error.into()))
    }

    pub async fn write_text_blob(&self, blob_id: &str, body: &str) -> Result<(), ApiError> {
        self.write_blob_bytes(blob_id, body.as_bytes().to_vec(), "text/plain")
            .await
    }

    pub async fn load_text_blob(&self, blob_id: &str) -> Result<String, ApiError> {
        String::from_utf8(self.read_blob_bytes(blob_id).await?)
            .map_err(|error| ApiError::Internal(error.into()))
    }

    pub async fn delete_blob(&self, blob_id: &str) {
        let response = self
            .http_client
            .delete(self.blob_endpoint(blob_id))
            .send()
            .await;

        match response {
            Ok(response)
                if response.status().is_success() || response.status() == StatusCode::NOT_FOUND => {
            }
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                tracing::warn!(%status, %body, blob_id, "failed to delete blob");
            }
            Err(error) => tracing::warn!(?error, blob_id, "failed to delete blob"),
        }
    }

    pub(crate) async fn write_blob_bytes(
        &self,
        blob_id: &str,
        body: Vec<u8>,
        content_type: &str,
    ) -> Result<(), ApiError> {
        let response = self
            .http_client
            .put(self.blob_endpoint_with_query(blob_id, content_type))
            .body(body)
            .send()
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

        if response.status().is_success() {
            return Ok(());
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(ApiError::Internal(anyhow::anyhow!(
            "blob write failed: {status} {body}"
        )))
    }

    async fn read_blob_bytes(&self, blob_id: &str) -> Result<Vec<u8>, ApiError> {
        let response = self
            .http_client
            .get(self.blob_endpoint(blob_id))
            .send()
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

        if response.status().is_success() {
            return response
                .bytes()
                .await
                .map(|bytes| bytes.to_vec())
                .map_err(|error| ApiError::Internal(error.into()));
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(ApiError::Internal(anyhow::anyhow!(
            "blob read failed: {status} {body}"
        )))
    }

    fn blob_endpoint(&self, blob_id: &str) -> String {
        let encoded = url::form_urlencoded::byte_serialize(blob_id.as_bytes()).collect::<String>();
        format!("{}/internal/blobs/{}", self.base_url, encoded)
    }

    fn blob_endpoint_with_query(&self, blob_id: &str, content_type: &str) -> String {
        let encoded =
            url::form_urlencoded::byte_serialize(content_type.as_bytes()).collect::<String>();
        format!("{}?content_type={encoded}", self.blob_endpoint(blob_id))
    }
}

fn build_blob_storage_router(state: BlobStorageState) -> Router {
    Router::new()
        .route("/healthz", get(blob_healthz))
        .route("/readyz", get(blob_readyz))
        .route(
            "/internal/blobs/{*blob_id}",
            get(get_blob)
                .head(head_blob)
                .put(put_blob)
                .delete(delete_blob_handler),
        )
        .with_state(state)
}

async fn blob_healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn blob_readyz(State(state): State<BlobStorageState>) -> Result<StatusCode, ApiError> {
    fs::create_dir_all(&state.root_dir).await?;
    Ok(StatusCode::OK)
}

async fn put_blob(
    State(state): State<BlobStorageState>,
    AxumPath(blob_id): AxumPath<String>,
    Query(query): Query<PutBlobQuery>,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    let path = blob_path(&state.root_dir, &blob_id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    fs::write(path, body).await?;

    if let Some(content_type) = query.content_type {
        let sidecar_path = blob_content_type_path(&state.root_dir, &blob_id)?;
        fs::write(sidecar_path, content_type).await?;
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn get_blob(
    State(state): State<BlobStorageState>,
    AxumPath(blob_id): AxumPath<String>,
) -> Result<impl IntoResponse, ApiError> {
    let path = blob_path(&state.root_dir, &blob_id)?;
    let body = match fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(ApiError::NotFound("blob not found".to_string()));
        }
        Err(error) => return Err(error.into()),
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        content_type_header(&state.root_dir, &blob_id)?,
    );
    Ok((headers, body))
}

async fn head_blob(
    State(state): State<BlobStorageState>,
    AxumPath(blob_id): AxumPath<String>,
) -> Result<impl IntoResponse, ApiError> {
    let path = blob_path(&state.root_dir, &blob_id)?;
    let metadata = match fs::metadata(&path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(ApiError::NotFound("blob not found".to_string()));
        }
        Err(error) => return Err(error.into()),
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        content_type_header(&state.root_dir, &blob_id)?,
    );
    headers.insert(
        axum::http::header::CONTENT_LENGTH,
        HeaderValue::from_str(&metadata.len().to_string())
            .map_err(|error| ApiError::Internal(error.into()))?,
    );
    Ok((StatusCode::OK, headers))
}

async fn delete_blob_handler(
    State(state): State<BlobStorageState>,
    AxumPath(blob_id): AxumPath<String>,
) -> Result<StatusCode, ApiError> {
    let path = blob_path(&state.root_dir, &blob_id)?;
    match fs::remove_file(path).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    let sidecar_path = blob_content_type_path(&state.root_dir, &blob_id)?;
    match fs::remove_file(sidecar_path).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    Ok(StatusCode::NO_CONTENT)
}

fn blob_path(root_dir: &Path, blob_id: &str) -> Result<PathBuf, ApiError> {
    let relative = normalize_blob_id(blob_id)?;
    Ok(root_dir.join(relative))
}

fn blob_content_type_path(root_dir: &Path, blob_id: &str) -> Result<PathBuf, ApiError> {
    let mut path = blob_path(root_dir, blob_id)?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| ApiError::BadRequest("invalid blob id".to_string()))?;
    path.set_file_name(format!("{file_name}.content-type"));
    Ok(path)
}

fn content_type_header(root_dir: &Path, blob_id: &str) -> Result<HeaderValue, ApiError> {
    if let Ok(saved) = std::fs::read_to_string(blob_content_type_path(root_dir, blob_id)?) {
        return HeaderValue::from_str(saved.trim())
            .map_err(|error| ApiError::Internal(error.into()));
    }

    let content_type = if blob_id.ends_with(".json") {
        "application/json"
    } else if blob_id.ends_with(".txt") {
        "text/plain; charset=utf-8"
    } else {
        "application/octet-stream"
    };
    HeaderValue::from_str(content_type).map_err(|error| ApiError::Internal(error.into()))
}

fn normalize_blob_id(blob_id: &str) -> Result<PathBuf, ApiError> {
    let path = PathBuf::from(blob_id);
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::Prefix(_) | Component::RootDir => {
                return Err(ApiError::BadRequest("invalid blob id".to_string()));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(ApiError::BadRequest("invalid blob id".to_string()));
    }

    Ok(normalized)
}

#[derive(sqlx::FromRow)]
struct StoredBlobRow {
    blob_key: Option<String>,
}

use std::path::{Path, PathBuf};

use sqlx::PgPool;
use tokio::fs;
use uuid::Uuid;

use crate::{error::ApiError, models::CrawlResultInput};

#[derive(Debug, Clone)]
pub struct BlobStore {
    pg_pool: PgPool,
    root_dir: PathBuf,
}

impl BlobStore {
    pub fn new(pg_pool: PgPool, root_dir: PathBuf) -> Self {
        Self { pg_pool, root_dir }
    }

    pub async fn ensure_ready(&self) -> Result<(), ApiError> {
        fs::create_dir_all(&self.root_dir)
            .await
            .map_err(|error| ApiError::Internal(error.into()))
    }

    pub async fn write_result(
        &self,
        owner_developer_id: &str,
        crawler_id: &str,
        lease_id: &str,
        result: &CrawlResultInput,
    ) -> Result<String, ApiError> {
        let body = serde_json::to_vec(result).map_err(|error| ApiError::Internal(error.into()))?;
        let blob_key = format!("crawl-results/{lease_id}/{}.json", result.job_id);
        let absolute_path = self.absolute_path(&blob_key);
        if let Some(parent) = absolute_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|error| ApiError::Internal(error.into()))?;
        }
        fs::write(&absolute_path, &body)
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;
        let payload = serde_json::json!({
            "blob_key": blob_key,
            "blob_size_bytes": body.len() as u64,
            "blob_content_type": "application/json",
        });

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
                payload,
                created_at
             )
             values ($1, $2, $3, $4, $5, $6, $7, $8, $9, now())
             on conflict (lease_id, crawl_job_id) do update
             set owner_developer_id = excluded.owner_developer_id,
                 crawler_id = excluded.crawler_id,
                 blob_key = excluded.blob_key,
                 blob_size_bytes = excluded.blob_size_bytes,
                 blob_content_type = excluded.blob_content_type,
                 payload = excluded.payload
             returning id",
        )
        .bind(Uuid::now_v7().to_string())
        .bind(owner_developer_id)
        .bind(crawler_id)
        .bind(&result.job_id)
        .bind(lease_id)
        .bind(&blob_key)
        .bind(body.len() as i64)
        .bind("application/json")
        .bind(payload)
        .fetch_one(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))
    }

    pub async fn load_result(&self, blob_id: &str) -> Result<CrawlResultInput, ApiError> {
        let row = sqlx::query_as::<_, StoredBlobRow>(
            "select blob_key, payload from crawl_result_blobs where id = $1",
        )
        .bind(blob_id)
        .fetch_one(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

        if let Some(blob_key) = row.blob_key {
            let bytes = fs::read(self.absolute_path(&blob_key))
                .await
                .map_err(|error| ApiError::Internal(error.into()))?;
            return serde_json::from_slice(&bytes)
                .map_err(|error| ApiError::Internal(error.into()));
        }

        serde_json::from_value(row.payload).map_err(|error| ApiError::Internal(error.into()))
    }

    fn absolute_path(&self, blob_key: &str) -> PathBuf {
        self.root_dir.join(normalize_blob_key(blob_key))
    }
}

#[derive(sqlx::FromRow)]
struct StoredBlobRow {
    blob_key: Option<String>,
    payload: serde_json::Value,
}

fn normalize_blob_key(blob_key: &str) -> &Path {
    Path::new(blob_key)
}

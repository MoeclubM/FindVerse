use sqlx::PgPool;
use uuid::Uuid;

use crate::{error::ApiError, models::CrawlResultInput};

#[derive(Debug, Clone)]
pub struct BlobStore {
    pg_pool: PgPool,
}

impl BlobStore {
    pub fn new(pg_pool: PgPool) -> Self {
        Self { pg_pool }
    }

    pub async fn write_result(
        &self,
        owner_developer_id: &str,
        crawler_id: &str,
        lease_id: &str,
        result: &CrawlResultInput,
    ) -> Result<String, ApiError> {
        let payload =
            serde_json::to_value(result).map_err(|error| ApiError::Internal(error.into()))?;

        sqlx::query_scalar(
            "insert into crawl_result_blobs (
                id,
                owner_developer_id,
                crawler_id,
                crawl_job_id,
                lease_id,
                payload,
                created_at
             )
             values ($1, $2, $3, $4, $5, $6, now())
             on conflict (lease_id, crawl_job_id) do update
             set owner_developer_id = excluded.owner_developer_id,
                 crawler_id = excluded.crawler_id,
                 payload = excluded.payload
             returning id",
        )
        .bind(Uuid::now_v7().to_string())
        .bind(owner_developer_id)
        .bind(crawler_id)
        .bind(&result.job_id)
        .bind(lease_id)
        .bind(payload)
        .fetch_one(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))
    }

    pub async fn load_result(&self, blob_id: &str) -> Result<CrawlResultInput, ApiError> {
        let payload: serde_json::Value =
            sqlx::query_scalar("select payload from crawl_result_blobs where id = $1")
                .bind(blob_id)
                .fetch_one(&self.pg_pool)
                .await
                .map_err(|error| ApiError::Internal(error.into()))?;

        serde_json::from_value(payload).map_err(|error| ApiError::Internal(error.into()))
    }
}

use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    blob_store::BlobStore, crawl::frontier::FrontierService, error::ApiError,
    models::CrawlResultInput,
};

#[derive(Debug, Clone)]
pub struct IngestService {
    pg_pool: PgPool,
    blob_store: BlobStore,
}

#[derive(Debug, Clone)]
pub struct StageReportOutcome {
    pub staged_results: usize,
    pub pending_results: usize,
}

#[derive(Debug, Clone)]
pub struct PendingIngestItem {
    pub item_id: String,
    pub lease_id: String,
    pub owner_developer_id: String,
    pub crawler_id: String,
    pub crawl_job_id: String,
    pub blob_id: String,
}

impl IngestService {
    pub fn new(pg_pool: PgPool, blob_store: BlobStore) -> Self {
        Self {
            pg_pool,
            blob_store,
        }
    }

    pub async fn stage_report(
        &self,
        frontier: &FrontierService,
        owner_developer_id: &str,
        crawler_id: &str,
        lease_id: &str,
        results: Vec<CrawlResultInput>,
    ) -> Result<StageReportOutcome, ApiError> {
        sqlx::query(
            "insert into crawl_ingest_batches (
                lease_id,
                owner_developer_id,
                crawler_id,
                status,
                result_count,
                created_at
             )
             values ($1, $2, $3, 'pending', 0, now())
             on conflict (lease_id) do nothing",
        )
        .bind(lease_id)
        .bind(owner_developer_id)
        .bind(crawler_id)
        .execute(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

        let mut staged_job_ids = Vec::with_capacity(results.len());
        for result in &results {
            let expected_url = frontier
                .report_job_url(owner_developer_id, crawler_id, lease_id, &result.job_id)
                .await?;
            if let Some(url) = expected_url {
                if url != result.url {
                    return Err(ApiError::BadRequest(
                        "crawl report contained a job not assigned to this crawler".to_string(),
                    ));
                }
            } else {
                let exists: Option<String> = sqlx::query_scalar(
                    "select id
                     from crawl_ingest_items
                     where lease_id = $1 and crawl_job_id = $2",
                )
                .bind(lease_id)
                .bind(&result.job_id)
                .fetch_optional(&self.pg_pool)
                .await
                .map_err(|error| ApiError::Internal(error.into()))?;

                if exists.is_none() {
                    return Err(ApiError::BadRequest(
                        "crawl report contained a job not assigned to this lease".to_string(),
                    ));
                }
            }

            let blob_id = self
                .blob_store
                .write_result(owner_developer_id, crawler_id, lease_id, result)
                .await?;

            sqlx::query(
                "insert into crawl_ingest_items (
                    id,
                    lease_id,
                    owner_developer_id,
                    crawler_id,
                    crawl_job_id,
                    blob_id,
                    status,
                    created_at
                 )
                 values ($1, $2, $3, $4, $5, $6, 'pending', now())
                 on conflict (lease_id, crawl_job_id) do update
                 set owner_developer_id = excluded.owner_developer_id,
                     crawler_id = excluded.crawler_id,
                     blob_id = excluded.blob_id",
            )
            .bind(Uuid::now_v7().to_string())
            .bind(lease_id)
            .bind(owner_developer_id)
            .bind(crawler_id)
            .bind(&result.job_id)
            .bind(&blob_id)
            .execute(&self.pg_pool)
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

            staged_job_ids.push(result.job_id.clone());
        }

        frontier
            .accept_report(owner_developer_id, crawler_id, lease_id, &staged_job_ids)
            .await?;

        let staged_results = self.lease_result_count(lease_id).await?;
        sqlx::query(
            "update crawl_ingest_batches
             set owner_developer_id = $2,
                 crawler_id = $3,
                 status = case when status = 'completed' then status else 'pending' end,
                 result_count = $4,
                 error_message = null
             where lease_id = $1",
        )
        .bind(lease_id)
        .bind(owner_developer_id)
        .bind(crawler_id)
        .bind(staged_results as i32)
        .execute(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

        Ok(StageReportOutcome {
            staged_results,
            pending_results: self.pending_result_count(owner_developer_id).await?,
        })
    }

    pub async fn claim_pending_items(
        &self,
        limit: usize,
    ) -> Result<Vec<PendingIngestItem>, ApiError> {
        let now = Utc::now();
        let items = sqlx::query_as::<_, PendingIngestItemRow>(
            "with picked as (
                 select id
                 from crawl_ingest_items
                 where status = 'pending'
                 order by created_at asc
                 limit $1
                 for update skip locked
             )
             update crawl_ingest_items item
             set status = 'processing',
                 started_at = $2,
                 error_message = null
             from picked
             where item.id = picked.id
             returning item.id, item.lease_id, item.owner_developer_id, item.crawler_id, item.crawl_job_id, item.blob_id",
        )
        .bind(limit.clamp(1, 256) as i64)
        .bind(now)
        .fetch_all(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

        if items.is_empty() {
            return Ok(Vec::new());
        }

        let lease_ids = items
            .iter()
            .map(|item| item.lease_id.clone())
            .collect::<Vec<_>>();
        sqlx::query(
            "update crawl_ingest_batches
             set status = 'processing',
                 started_at = coalesce(started_at, $2),
                 error_message = null
             where lease_id = any($1)",
        )
        .bind(&lease_ids)
        .bind(now)
        .execute(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

        Ok(items
            .into_iter()
            .map(|item| PendingIngestItem {
                item_id: item.id,
                lease_id: item.lease_id,
                owner_developer_id: item.owner_developer_id,
                crawler_id: item.crawler_id,
                crawl_job_id: item.crawl_job_id,
                blob_id: item.blob_id,
            })
            .collect())
    }

    pub async fn mark_item_completed(&self, item: &PendingIngestItem) -> Result<(), ApiError> {
        let now = Utc::now();
        sqlx::query(
            "update crawl_ingest_items
             set status = 'completed',
                 finished_at = $2,
                 error_message = null
             where id = $1",
        )
        .bind(&item.item_id)
        .bind(now)
        .execute(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

        self.refresh_batch_status(&item.lease_id, None, now).await
    }

    pub async fn mark_item_failed(
        &self,
        item: &PendingIngestItem,
        error_message: &str,
    ) -> Result<(), ApiError> {
        let now = Utc::now();
        sqlx::query(
            "update crawl_ingest_items
             set status = 'failed',
                 finished_at = $2,
                 error_message = $3
             where id = $1",
        )
        .bind(&item.item_id)
        .bind(now)
        .bind(error_message)
        .execute(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

        self.refresh_batch_status(&item.lease_id, Some(error_message), now)
            .await
    }

    pub async fn pending_result_count(&self, owner_developer_id: &str) -> Result<usize, ApiError> {
        let count: i64 = sqlx::query_scalar(
            "select count(*)
             from crawl_ingest_items
             where owner_developer_id = $1
               and status in ('pending', 'processing')",
        )
        .bind(owner_developer_id)
        .fetch_one(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

        Ok(count.max(0) as usize)
    }

    async fn lease_result_count(&self, lease_id: &str) -> Result<usize, ApiError> {
        let count: i64 =
            sqlx::query_scalar("select count(*) from crawl_ingest_items where lease_id = $1")
                .bind(lease_id)
                .fetch_one(&self.pg_pool)
                .await
                .map_err(|error| ApiError::Internal(error.into()))?;

        Ok(count.max(0) as usize)
    }

    async fn refresh_batch_status(
        &self,
        lease_id: &str,
        error_message: Option<&str>,
        now: chrono::DateTime<Utc>,
    ) -> Result<(), ApiError> {
        let (pending_count, failed_count): (i64, i64) = sqlx::query_as(
            "select
                 count(*) filter (where status in ('pending', 'processing')) as pending_count,
                 count(*) filter (where status = 'failed') as failed_count
             from crawl_ingest_items
             where lease_id = $1",
        )
        .bind(lease_id)
        .fetch_one(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

        let status = if failed_count > 0 {
            "failed"
        } else if pending_count > 0 {
            "processing"
        } else {
            "completed"
        };

        sqlx::query(
            "update crawl_ingest_batches
             set status = $2,
                 finished_at = case when $2 = 'completed' or $2 = 'failed' then $3 else null end,
                 error_message = $4
             where lease_id = $1",
        )
        .bind(lease_id)
        .bind(status)
        .bind(now)
        .bind(error_message)
        .execute(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

        Ok(())
    }
}

#[derive(sqlx::FromRow)]
struct PendingIngestItemRow {
    id: String,
    lease_id: String,
    owner_developer_id: String,
    crawler_id: String,
    crawl_job_id: String,
    blob_id: String,
}

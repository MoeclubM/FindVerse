use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{error::ApiError, models::CrawlJob};

#[derive(Debug, Clone)]
pub struct FrontierService {
    pg_pool: PgPool,
}

#[derive(Debug)]
pub struct FrontierClaim {
    pub lease_id: String,
    pub jobs: Vec<CrawlJob>,
}

impl FrontierService {
    pub fn new(pg_pool: PgPool) -> Self {
        Self { pg_pool }
    }

    pub async fn claim_jobs(
        &self,
        owner_developer_id: &str,
        crawler_id: &str,
        max_jobs: usize,
        crawler_has_js_render: bool,
        now: DateTime<Utc>,
    ) -> Result<FrontierClaim, ApiError> {
        let lease_id = Uuid::now_v7().to_string();
        let claimed_rows = sqlx::query_as::<_, ClaimedJobRow>(
            "with active_counts as (
                 select origin_key, count(*)::integer as active_count
                 from crawl_jobs
                 where owner_developer_id = $4
                   and status in ('claimed', 'ingesting')
                 group by origin_key
             ),
             ranked_jobs as (
                 select
                     j.id,
                     j.origin_key,
                     o.next_allowed_at,
                     j.priority,
                     j.discovered_at,
                     coalesce(active_counts.active_count, 0) as active_count,
                     greatest(j.same_origin_concurrency, 1) as same_origin_concurrency,
                     row_number() over (
                         partition by j.origin_key
                         order by j.priority desc, j.discovered_at asc
                     ) as origin_rank
                 from crawl_jobs j
                 join crawl_origins o
                   on o.owner_developer_id = j.owner_developer_id
                  and o.origin_key = j.origin_key
                 left join active_counts
                   on active_counts.origin_key = j.origin_key
                 where j.owner_developer_id = $4
                   and j.status = 'queued'
                   and (j.next_retry_at is null or j.next_retry_at <= $2)
                   and o.next_allowed_at <= $2
                   and ($6 or not j.requires_js)
             ),
             candidate_jobs as (
                 select j.id, ranked_jobs.origin_key
                 from crawl_jobs j
                 join ranked_jobs on ranked_jobs.id = j.id
                 where ranked_jobs.active_count + ranked_jobs.origin_rank <= ranked_jobs.same_origin_concurrency
                 order by ranked_jobs.next_allowed_at asc, ranked_jobs.priority desc, ranked_jobs.discovered_at asc
                 limit $5
                 for update of j skip locked
             ),
             claimed as (
                 update crawl_jobs
                 set status = 'claimed',
                     claimed_by = $1,
                     claimed_at = $2,
                     lease_id = $3,
                     attempt_count = attempt_count + 1
                 where id in (select id from candidate_jobs)
                 returning id, url, origin_key, source, depth, max_depth, attempt_count, discovered_at, network
             ),
             touched_origins as (
                 update crawl_origins origin
                 set in_flight_count = origin.in_flight_count + touched.claimed_count,
                     updated_at = $2
                 from (
                     select origin_key, count(*)::integer as claimed_count
                     from claimed
                     group by origin_key
                 ) touched
                 where origin.owner_developer_id = $4
                   and origin.origin_key = touched.origin_key
             )
             select id, url, origin_key, source, depth, max_depth, attempt_count, discovered_at, network
             from claimed",
        )
        .bind(crawler_id)
        .bind(now)
        .bind(&lease_id)
        .bind(owner_developer_id)
        .bind(max_jobs.clamp(1, 100) as i64)
        .bind(crawler_has_js_render)
        .fetch_all(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

        let jobs = claimed_rows
            .into_iter()
            .map(|row| CrawlJob {
                job_id: row.id,
                url: row.url,
                origin_key: row.origin_key,
                source: row.source,
                depth: row.depth as u32,
                max_depth: row.max_depth as u32,
                attempt_count: row.attempt_count as u32,
                discovered_at: row.discovered_at,
                network: row.network,
                etag: None,
                last_modified: None,
            })
            .collect();

        Ok(FrontierClaim { lease_id, jobs })
    }

    pub async fn report_job_url(
        &self,
        owner_developer_id: &str,
        crawler_id: &str,
        lease_id: &str,
        job_id: &str,
    ) -> Result<Option<String>, ApiError> {
        sqlx::query_scalar(
            "select url
             from crawl_jobs
             where owner_developer_id = $1
               and id = $2
               and claimed_by = $3
               and lease_id = $4",
        )
        .bind(owner_developer_id)
        .bind(job_id)
        .bind(crawler_id)
        .bind(lease_id)
        .fetch_optional(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))
    }

    pub async fn accept_report(
        &self,
        owner_developer_id: &str,
        crawler_id: &str,
        lease_id: &str,
        job_ids: &[String],
    ) -> Result<(), ApiError> {
        if job_ids.is_empty() {
            return Ok(());
        }

        sqlx::query(
            "update crawl_jobs
             set status = 'ingesting'
             where owner_developer_id = $1
               and claimed_by = $2
               and lease_id = $3
               and status in ('claimed', 'ingesting')
               and id = any($4)",
        )
        .bind(owner_developer_id)
        .bind(crawler_id)
        .bind(lease_id)
        .bind(job_ids)
        .execute(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

        Ok(())
    }

    pub async fn frontier_depth(&self, owner_developer_id: &str) -> usize {
        let depth: i64 = sqlx::query_scalar(
            "select count(*) from crawl_jobs where owner_developer_id = $1 and status = 'queued'",
        )
        .bind(owner_developer_id)
        .fetch_one(&self.pg_pool)
        .await
        .unwrap_or(0);

        depth.max(0) as usize
    }
}

#[derive(sqlx::FromRow)]
struct ClaimedJobRow {
    id: String,
    url: String,
    origin_key: String,
    source: String,
    depth: i32,
    max_depth: i32,
    attempt_count: i32,
    discovered_at: DateTime<Utc>,
    network: String,
}

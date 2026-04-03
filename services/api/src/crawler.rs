use std::time::Duration;

use chrono::{DateTime, Utc};
use findverse_common::{DiscoveryScope, host_matches_scope, origin_key};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    blob_store::BlobStore,
    crawl::{
        frontier::FrontierService,
        ingest::{IngestService, PendingIngestItem},
        projection::ProjectionRunner,
    },
    error::ApiError,
    models::{
        ClaimJobsRequest, ClaimJobsResponse, CrawlEvent, CrawlJobDetail, CrawlJobListResponse,
        CrawlJobStats, CrawlOriginState, CrawlOverviewResponse, CrawlResultInput, CrawlRule,
        CrawlerCapabilities, CrawlerMetadata, CreateCrawlRuleRequest, DeveloperDomainDocument,
        DeveloperDomainFacet, DeveloperDomainInsightResponse, DeveloperDomainJob,
        DeveloperDomainSubmitRequest, DeveloperDomainSubmitResponse, IndexedDocument,
        SeedFrontierRequest, SeedFrontierResponse, SubmitCrawlReportRequest,
        SubmitCrawlReportResponse, UpdateCrawlRuleRequest, UpdateCrawlerRequest,
    },
    store::{
        CURRENT_INDEX_VERSION, CURRENT_PARSER_VERSION, CURRENT_SCHEMA_VERSION, SearchIndex,
        content_hash, derive_terms, display_url, extract_host, hash_token, normalize_url,
        stable_document_id, word_count,
    },
};

const DEFAULT_CRAWLER_CLAIM_TIMEOUT_SECS: u64 = 300;

#[derive(Debug, Clone)]
pub(crate) struct CrawlerStore {
    pg_pool: PgPool,
    frontier: FrontierService,
    ingest: IngestService,
    projection: ProjectionRunner,
}

#[derive(Debug, Clone)]
pub struct ControlCrawlerStore {
    inner: CrawlerStore,
}

#[derive(Debug, Clone)]
pub struct TaskCrawlerStore {
    inner: CrawlerStore,
}

#[derive(Debug, Clone)]
pub struct SchedulerCrawlerStore {
    inner: CrawlerStore,
}

impl ControlCrawlerStore {
    pub fn new(pg_pool: PgPool, blob_store: BlobStore) -> Self {
        Self {
            inner: CrawlerStore::new(pg_pool, blob_store),
        }
    }

    pub async fn update_crawler(
        &self,
        developer_id: &str,
        crawler_id: &str,
        request: UpdateCrawlerRequest,
    ) -> Result<(), ApiError> {
        self.inner
            .update_crawler(developer_id, crawler_id, request)
            .await
    }

    pub async fn delete_crawler(
        &self,
        developer_id: &str,
        crawler_id: &str,
    ) -> Result<(), ApiError> {
        self.inner.delete_crawler(developer_id, crawler_id).await
    }

    pub async fn get_all_system_config(
        &self,
    ) -> Result<Vec<crate::models::SystemConfigEntry>, ApiError> {
        self.inner.get_all_system_config().await
    }

    pub async fn set_system_config(
        &self,
        key: &str,
        value: Option<String>,
    ) -> Result<(), ApiError> {
        self.inner.set_system_config(key, value).await
    }

    pub async fn create_rule(
        &self,
        developer_id: &str,
        request: CreateCrawlRuleRequest,
    ) -> Result<CrawlRule, ApiError> {
        self.inner.create_rule(developer_id, request).await
    }

    pub async fn update_rule(
        &self,
        developer_id: &str,
        rule_id: &str,
        request: UpdateCrawlRuleRequest,
    ) -> Result<CrawlRule, ApiError> {
        self.inner.update_rule(developer_id, rule_id, request).await
    }

    pub async fn delete_rule(&self, developer_id: &str, rule_id: &str) -> Result<(), ApiError> {
        self.inner.delete_rule(developer_id, rule_id).await
    }

    pub async fn overview(
        &self,
        developer_id: &str,
        total_documents: usize,
    ) -> Result<CrawlOverviewResponse, ApiError> {
        self.inner.overview(developer_id, total_documents).await
    }

    pub async fn domain_insight(
        &self,
        domain: &str,
    ) -> Result<DeveloperDomainInsightResponse, ApiError> {
        self.inner.domain_insight(domain).await
    }

    pub async fn submit_domain_urls(
        &self,
        owner_developer_id: &str,
        submitter_id: &str,
        request: DeveloperDomainSubmitRequest,
    ) -> Result<DeveloperDomainSubmitResponse, ApiError> {
        self.inner
            .submit_domain_urls(owner_developer_id, submitter_id, request)
            .await
    }

    pub async fn seed_frontier(
        &self,
        developer_id: &str,
        request: SeedFrontierRequest,
    ) -> Result<SeedFrontierResponse, ApiError> {
        self.inner.seed_frontier(developer_id, request).await
    }

    pub async fn record_admin_event(
        &self,
        developer_id: &str,
        kind: &str,
        status: &str,
        message: String,
        host: Option<String>,
        crawler_id: Option<String>,
    ) -> Result<(), ApiError> {
        self.inner
            .record_admin_event(developer_id, kind, status, message, host, crawler_id)
            .await
    }

    pub async fn list_jobs(
        &self,
        developer_id: &str,
        status: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<CrawlJobListResponse, ApiError> {
        self.inner
            .list_jobs(developer_id, status, limit, offset)
            .await
    }

    pub async fn retry_failed_jobs(&self, developer_id: &str) -> Result<usize, ApiError> {
        self.inner.retry_failed_jobs(developer_id).await
    }

    pub async fn cleanup_completed_jobs(&self, developer_id: &str) -> Result<usize, ApiError> {
        self.inner.cleanup_completed_jobs(developer_id).await
    }

    pub async fn cleanup_failed_jobs(&self, developer_id: &str) -> Result<usize, ApiError> {
        self.inner.cleanup_failed_jobs(developer_id).await
    }

    pub async fn stop_all_jobs(&self, developer_id: &str) -> Result<(usize, usize), ApiError> {
        self.inner.stop_all_jobs(developer_id).await
    }

    pub async fn job_stats(&self, developer_id: &str) -> Result<CrawlJobStats, ApiError> {
        self.inner.job_stats(developer_id).await
    }

    pub async fn list_origins(
        &self,
        developer_id: &str,
    ) -> Result<Vec<CrawlOriginState>, ApiError> {
        self.inner.list_origins(developer_id).await
    }
}

impl TaskCrawlerStore {
    pub fn new(pg_pool: PgPool, blob_store: BlobStore) -> Self {
        Self {
            inner: CrawlerStore::new(pg_pool, blob_store),
        }
    }

    pub async fn claim_jobs(
        &self,
        crawler_id: &str,
        crawler_name: Option<&str>,
        auth_header: Option<&str>,
        default_owner_developer_id: &str,
        request: ClaimJobsRequest,
        capabilities: Option<&CrawlerCapabilities>,
    ) -> Result<ClaimJobsResponse, ApiError> {
        self.inner
            .claim_jobs(
                crawler_id,
                crawler_name,
                auth_header,
                default_owner_developer_id,
                request,
                capabilities,
            )
            .await
    }

    pub async fn submit_report(
        &self,
        crawler_id: &str,
        crawler_name: Option<&str>,
        auth_header: Option<&str>,
        default_owner_developer_id: &str,
        request: SubmitCrawlReportRequest,
    ) -> Result<SubmitCrawlReportResponse, ApiError> {
        self.inner
            .submit_report(
                crawler_id,
                crawler_name,
                auth_header,
                default_owner_developer_id,
                request,
            )
            .await
    }

    pub async fn heartbeat_crawler(
        &self,
        crawler_id: &str,
        crawler_name: Option<&str>,
        auth_header: Option<&str>,
        default_owner_developer_id: &str,
        capabilities: Option<&CrawlerCapabilities>,
    ) -> Result<crate::models::CrawlerHeartbeatResponse, ApiError> {
        self.inner
            .heartbeat_crawler(
                crawler_id,
                crawler_name,
                auth_header,
                default_owner_developer_id,
                capabilities,
            )
            .await
    }
}

impl SchedulerCrawlerStore {
    pub fn new(pg_pool: PgPool, blob_store: BlobStore) -> Self {
        Self {
            inner: CrawlerStore::new(pg_pool, blob_store),
        }
    }

    pub async fn get_system_config(&self, key: &str) -> Option<String> {
        self.inner.get_system_config(key).await
    }

    pub async fn run_maintenance(
        &self,
        claim_timeout: Duration,
        search_index: &SearchIndex,
    ) -> Result<(), ApiError> {
        self.inner
            .run_maintenance(claim_timeout, search_index)
            .await
    }
}

impl CrawlerStore {
    pub fn new(pg_pool: PgPool, blob_store: BlobStore) -> Self {
        let frontier = FrontierService::new(pg_pool.clone());
        let ingest = IngestService::new(pg_pool.clone(), blob_store.clone());
        let projection = ProjectionRunner::new(ingest.clone(), blob_store);

        Self {
            pg_pool,
            frontier,
            ingest,
            projection,
        }
    }

    pub async fn rename_crawler(
        &self,
        developer_id: &str,
        crawler_id: &str,
        new_name: &str,
    ) -> Result<(), ApiError> {
        let clean = new_name.trim();
        if clean.len() < 2 {
            return Err(ApiError::BadRequest(
                "crawler name must contain at least 2 characters".to_string(),
            ));
        }

        let result =
            sqlx::query("UPDATE crawlers SET name = $3 WHERE id = $1 AND owner_developer_id = $2")
                .bind(crawler_id)
                .bind(developer_id)
                .bind(clean)
                .execute(&self.pg_pool)
                .await
                .map_err(|e| ApiError::Internal(e.into()))?;

        if result.rows_affected() == 0 {
            return Err(ApiError::NotFound("crawler not found".to_string()));
        }

        self.push_event(
            developer_id,
            "crawler-renamed",
            "ok",
            format!("renamed crawler {crawler_id} to {clean}"),
            None,
            Some(crawler_id.to_string()),
        )
        .await?;

        Ok(())
    }

    pub async fn update_crawler(
        &self,
        developer_id: &str,
        crawler_id: &str,
        request: UpdateCrawlerRequest,
    ) -> Result<(), ApiError> {
        let worker_concurrency = match request.worker_concurrency {
            Some(0) => {
                return Err(ApiError::BadRequest(
                    "worker_concurrency must be a positive integer".to_string(),
                ));
            }
            Some(value) => Some(value),
            None => None,
        };
        let js_render_concurrency = match request.js_render_concurrency {
            Some(0) => {
                return Err(ApiError::BadRequest(
                    "js_render_concurrency must be a positive integer".to_string(),
                ));
            }
            Some(value) => Some(value),
            None => None,
        };
        let has_runtime_update = worker_concurrency.is_some() || js_render_concurrency.is_some();

        if request.name.is_none() && !has_runtime_update {
            return Err(ApiError::BadRequest(
                "no crawler fields provided".to_string(),
            ));
        }

        if let Some(name) = request.name.as_deref() {
            self.rename_crawler(developer_id, crawler_id, name).await?;
        }

        if !has_runtime_update {
            return Ok(());
        }

        let mut metadata_patch = serde_json::Map::new();
        if let Some(value) = worker_concurrency {
            metadata_patch.insert("worker_concurrency".to_string(), serde_json::json!(value));
        }
        if let Some(value) = js_render_concurrency {
            metadata_patch.insert(
                "js_render_concurrency".to_string(),
                serde_json::json!(value),
            );
        }

        let result = sqlx::query(
            "update crawlers
             set metadata = metadata || $3::jsonb
             where id = $1 and owner_developer_id = $2",
        )
        .bind(crawler_id)
        .bind(developer_id)
        .bind(serde_json::Value::Object(metadata_patch))
        .execute(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

        if result.rows_affected() == 0 {
            return Err(ApiError::NotFound("crawler not found".to_string()));
        }

        self.push_event(
            developer_id,
            "crawler-runtime-updated",
            "ok",
            format!("updated runtime config for crawler {crawler_id}"),
            None,
            Some(crawler_id.to_string()),
        )
        .await?;

        Ok(())
    }

    pub async fn delete_crawler(
        &self,
        developer_id: &str,
        crawler_id: &str,
    ) -> Result<(), ApiError> {
        let row = sqlx::query_as::<_, CrawlerDeleteRow>(
            "select id, name, last_seen_at
             from crawlers
             where id = $1 and owner_developer_id = $2",
        )
        .bind(crawler_id)
        .bind(developer_id)
        .fetch_optional(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?
        .ok_or_else(|| ApiError::NotFound("crawler not found".to_string()))?;

        let in_flight_jobs = sqlx::query_as::<_, CrawlerDeleteInFlightRow>(
            "select
                 count(*) filter (where status = 'claimed') as claimed_jobs,
                 count(*) filter (where status = 'ingesting') as ingesting_jobs
             from crawl_jobs
             where owner_developer_id = $1
               and claimed_by = $2
               and status in ('claimed', 'ingesting')",
        )
        .bind(developer_id)
        .bind(crawler_id)
        .fetch_one(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;
        let claimed_jobs = in_flight_jobs.claimed_jobs.max(0) as usize;
        let ingesting_jobs = in_flight_jobs.ingesting_jobs.max(0) as usize;

        if claimed_jobs > 0 || ingesting_jobs > 0 {
            let claim_timeout_secs = self
                .get_system_config("crawler.claim_timeout_secs")
                .await
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(DEFAULT_CRAWLER_CLAIM_TIMEOUT_SECS);
            let now = Utc::now();
            let claim_timeout = Duration::from_secs(claim_timeout_secs.max(1));

            if crawler_seen_within_timeout(row.last_seen_at, now, claim_timeout) {
                return Err(ApiError::Conflict(
                    "crawler still has in-flight jobs".to_string(),
                ));
            }

            let (requeued_count, dead_letter_count) = if claimed_jobs > 0 {
                self.release_claimed_jobs_for_deleted_crawler(developer_id, crawler_id, now)
                    .await?
            } else {
                (0, 0)
            };

            let detached_message = if claimed_jobs == 0 {
                format!(
                    "deleted offline crawler {crawler_id}; ingesting jobs still finishing: {ingesting_jobs}"
                )
            } else if ingesting_jobs == 0 {
                format!(
                    "deleted offline crawler {crawler_id}; released claimed jobs (requeued: {requeued_count}, dead_letter: {dead_letter_count})"
                )
            } else {
                format!(
                    "deleted offline crawler {crawler_id}; released claimed jobs (requeued: {requeued_count}, dead_letter: {dead_letter_count}), ingesting jobs still finishing: {ingesting_jobs}"
                )
            };

            self.push_event(
                developer_id,
                "crawler-delete-detached-jobs",
                "ok",
                detached_message,
                None,
                Some(crawler_id.to_string()),
            )
            .await?;
        }

        sqlx::query("delete from crawlers where id = $1 and owner_developer_id = $2")
            .bind(crawler_id)
            .bind(developer_id)
            .execute(&self.pg_pool)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        self.push_event(
            developer_id,
            "crawler-deleted",
            "ok",
            format!("deleted crawler {}", row.name),
            None,
            Some(row.id),
        )
        .await?;

        Ok(())
    }

    pub async fn get_all_system_config(
        &self,
    ) -> Result<Vec<crate::models::SystemConfigEntry>, crate::error::ApiError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            key: String,
            value: String,
            updated_at: chrono::DateTime<chrono::Utc>,
        }
        sqlx::query_as::<_, Row>("SELECT key, value, updated_at FROM system_config ORDER BY key")
            .fetch_all(&self.pg_pool)
            .await
            .map_err(|e| crate::error::ApiError::Internal(e.into()))
            .map(|rows| {
                rows.into_iter()
                    .map(|r| crate::models::SystemConfigEntry {
                        key: r.key,
                        value: r.value,
                        updated_at: r.updated_at,
                    })
                    .collect()
            })
    }

    pub async fn get_system_config(&self, key: &str) -> Option<String> {
        self.get_config(key).await.ok().flatten()
    }

    pub async fn set_system_config(
        &self,
        key: &str,
        value: Option<String>,
    ) -> Result<(), crate::error::ApiError> {
        match value {
            Some(v) => self.set_config(key, &v).await,
            None => self.delete_config(key).await,
        }
    }

    pub async fn create_rule(
        &self,
        developer_id: &str,
        request: CreateCrawlRuleRequest,
    ) -> Result<CrawlRule, ApiError> {
        let name = validate_rule_name(&request.name)?;
        let seed_url = normalize_url(&request.seed_url).ok_or_else(|| {
            ApiError::BadRequest("seed_url must be a valid http(s) url".to_string())
        })?;

        let id = Uuid::now_v7().to_string();
        let now = Utc::now();
        let interval_minutes = request.interval_minutes.clamp(1, 10_080) as i64;
        let max_depth = request.max_depth.min(10) as i32;
        let max_pages = request.max_pages.clamp(1, 10_000) as i32;
        let same_origin_concurrency = request.same_origin_concurrency.clamp(1, 32) as i32;
        let max_discovered_urls_per_page =
            request.max_discovered_urls_per_page.clamp(1, 200) as i32;

        sqlx::query(
            "insert into crawl_rules (id, owner_developer_id, owner_user_id, name, seed_url, pattern, status, interval_minutes, max_depth, max_pages, same_origin_concurrency, discovery_scope, max_discovered_urls_per_page, enabled, created_at, updated_at)
             values ($1, $2, null, $3, $4, $4, 'active', $5, $6, $7, $8, $9, $10, $11, $12, $12)",
        )
        .bind(&id)
        .bind(developer_id)
        .bind(&name)
        .bind(&seed_url)
        .bind(interval_minutes)
        .bind(max_depth)
        .bind(max_pages)
        .bind(same_origin_concurrency)
        .bind(request.discovery_scope.as_str())
        .bind(max_discovered_urls_per_page)
        .bind(request.enabled)
        .bind(now)
        .execute(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

        self.push_event(
            developer_id,
            "rule-created",
            "ok",
            format!("created rule {name}"),
            Some(seed_url.clone()),
            None,
        )
        .await?;

        Ok(CrawlRule {
            id,
            name,
            seed_url,
            interval_minutes: interval_minutes as u64,
            max_depth: max_depth as u32,
            max_pages: max_pages as u32,
            same_origin_concurrency: same_origin_concurrency as u32,
            discovery_scope: request.discovery_scope,
            max_discovered_urls_per_page: max_discovered_urls_per_page as u32,
            enabled: request.enabled,
            created_at: now,
            updated_at: now,
            last_enqueued_at: None,
        })
    }

    pub async fn update_rule(
        &self,
        developer_id: &str,
        rule_id: &str,
        request: UpdateCrawlRuleRequest,
    ) -> Result<CrawlRule, ApiError> {
        let row = sqlx::query_as::<_, CrawlRuleRow>(
            "select id, owner_developer_id, name, seed_url, interval_minutes, max_depth, max_pages, same_origin_concurrency, discovery_scope, max_discovered_urls_per_page, enabled, created_at, updated_at, last_enqueued_at
             from crawl_rules where id = $1",
        )
        .bind(rule_id)
        .fetch_optional(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?
        .ok_or_else(|| ApiError::NotFound("crawl rule not found".to_string()))?;

        if row.owner_developer_id != developer_id {
            return Err(ApiError::NotFound("crawl rule not found".to_string()));
        }

        let new_name = match request.name.as_deref() {
            Some(n) => validate_rule_name(n)?,
            None => row.name,
        };
        let new_seed_url = match request.seed_url.as_deref() {
            Some(u) => normalize_url(u).ok_or_else(|| {
                ApiError::BadRequest("seed_url must be a valid http(s) url".to_string())
            })?,
            None => row.seed_url,
        };
        let new_interval = request
            .interval_minutes
            .map(|v| v.clamp(1, 10_080))
            .unwrap_or(row.interval_minutes as u64) as i64;
        let new_max_depth = request
            .max_depth
            .map(|v| v.min(10))
            .unwrap_or(row.max_depth as u32) as i32;
        let new_max_pages = request
            .max_pages
            .map(|value| value.clamp(1, 10_000))
            .unwrap_or(row.max_pages as u32) as i32;
        let new_same_origin_concurrency = request
            .same_origin_concurrency
            .map(|value| value.clamp(1, 32))
            .unwrap_or(row.same_origin_concurrency as u32)
            as i32;
        let new_discovery_scope = request
            .discovery_scope
            .unwrap_or_else(|| DiscoveryScope::from_db_value(&row.discovery_scope));
        let new_max_discovered_urls_per_page = request
            .max_discovered_urls_per_page
            .map(|value| value.clamp(1, 200))
            .unwrap_or(row.max_discovered_urls_per_page as u32)
            as i32;
        let new_enabled = request.enabled.unwrap_or(row.enabled);
        let now = Utc::now();

        sqlx::query(
            "update crawl_rules set name = $2, seed_url = $3, pattern = $3, interval_minutes = $4, max_depth = $5, max_pages = $6, same_origin_concurrency = $7, discovery_scope = $8, max_discovered_urls_per_page = $9, enabled = $10, updated_at = $11
             where id = $1",
        )
        .bind(rule_id)
        .bind(&new_name)
        .bind(&new_seed_url)
        .bind(new_interval)
        .bind(new_max_depth)
        .bind(new_max_pages)
        .bind(new_same_origin_concurrency)
        .bind(new_discovery_scope.as_str())
        .bind(new_max_discovered_urls_per_page)
        .bind(new_enabled)
        .bind(now)
        .execute(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

        self.push_event(
            developer_id,
            "rule-updated",
            "ok",
            format!("updated rule {new_name}"),
            Some(new_seed_url.clone()),
            None,
        )
        .await?;

        Ok(CrawlRule {
            id: rule_id.to_string(),
            name: new_name,
            seed_url: new_seed_url,
            interval_minutes: new_interval as u64,
            max_depth: new_max_depth as u32,
            max_pages: new_max_pages as u32,
            same_origin_concurrency: new_same_origin_concurrency as u32,
            discovery_scope: new_discovery_scope,
            max_discovered_urls_per_page: new_max_discovered_urls_per_page as u32,
            enabled: new_enabled,
            created_at: row.created_at,
            updated_at: now,
            last_enqueued_at: row.last_enqueued_at,
        })
    }

    pub async fn delete_rule(&self, developer_id: &str, rule_id: &str) -> Result<(), ApiError> {
        let row = sqlx::query_as::<_, CrawlRuleRow>(
            "select id, owner_developer_id, name, seed_url, interval_minutes, max_depth, max_pages, same_origin_concurrency, discovery_scope, max_discovered_urls_per_page, enabled, created_at, updated_at, last_enqueued_at
             from crawl_rules where id = $1",
        )
        .bind(rule_id)
        .fetch_optional(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?
        .ok_or_else(|| ApiError::NotFound("crawl rule not found".to_string()))?;

        if row.owner_developer_id != developer_id {
            return Err(ApiError::NotFound("crawl rule not found".to_string()));
        }

        sqlx::query("delete from crawl_rules where id = $1")
            .bind(rule_id)
            .execute(&self.pg_pool)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        self.push_event(
            developer_id,
            "rule-deleted",
            "ok",
            format!("deleted rule {}", row.name),
            Some(row.seed_url),
            None,
        )
        .await?;

        Ok(())
    }

    pub async fn overview(
        &self,
        developer_id: &str,
        indexed_documents: usize,
    ) -> Result<CrawlOverviewResponse, ApiError> {
        let default_worker_concurrency = parse_positive_usize_config(
            self.get_system_config("crawler.total_concurrency").await,
            16,
        );
        let default_js_render_concurrency = parse_positive_usize_config(
            self.get_system_config("crawler.js_render_concurrency")
                .await,
            1,
        );
        let claim_timeout_secs = self
            .get_system_config("crawler.claim_timeout_secs")
            .await
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_CRAWLER_CLAIM_TIMEOUT_SECS);
        let claim_timeout = Duration::from_secs(claim_timeout_secs.max(1));
        let now = Utc::now();
        let crawlers: Vec<CrawlerMetadata> = sqlx::query_as::<_, CrawlerMetadataRow>(
            "select crawlers.id, crawlers.name, crawlers.preview, crawlers.created_at, crawlers.revoked_at, crawlers.last_seen_at, crawlers.last_claimed_at, crawlers.jobs_claimed, crawlers.jobs_reported, coalesce(job_counts.in_flight_jobs, 0) as in_flight_jobs, crawlers.metadata
             from crawlers
             left join (
                 select claimed_by as crawler_id, count(*)::bigint as in_flight_jobs
                 from crawl_jobs
                 where owner_developer_id = $1
                   and claimed_by is not null
                   and status in ('claimed', 'ingesting')
                 group by claimed_by
             ) job_counts on job_counts.crawler_id = crawlers.id
             where crawlers.owner_developer_id = $1
             order by crawlers.created_at desc",
        )
        .bind(developer_id)
        .fetch_all(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?
        .into_iter()
        .map(|r| {
            let metadata = crawler_runtime_metadata(&r.metadata);
            let in_flight_jobs = r.in_flight_jobs.max(0) as u64;
            let online = crawler_seen_within_timeout(r.last_seen_at, now, claim_timeout);
            CrawlerMetadata {
                supports_js_render: metadata.js_render,
                id: r.id,
                name: r.name,
                preview: r.preview,
                created_at: r.created_at,
                revoked_at: r.revoked_at,
                last_seen_at: r.last_seen_at,
                last_claimed_at: r.last_claimed_at,
                online,
                can_delete: in_flight_jobs == 0 || !online,
                in_flight_jobs,
                jobs_claimed: r.jobs_claimed as u64,
                jobs_reported: r.jobs_reported as u64,
                worker_concurrency: metadata
                    .worker_concurrency
                    .filter(|value| *value > 0)
                    .unwrap_or(default_worker_concurrency),
                js_render_concurrency: metadata
                    .js_render_concurrency
                    .filter(|value| *value > 0)
                    .unwrap_or(default_js_render_concurrency),
            }
        })
        .collect();

        let rules: Vec<CrawlRule> = sqlx::query_as::<_, CrawlRuleRow>(
            "select id, owner_developer_id, name, seed_url, interval_minutes, max_depth, max_pages, same_origin_concurrency, discovery_scope, max_discovered_urls_per_page, enabled, created_at, updated_at, last_enqueued_at
             from crawl_rules where owner_developer_id = $1
             order by created_at desc",
        )
        .bind(developer_id)
        .fetch_all(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?
        .into_iter()
        .map(|r| CrawlRule {
            id: r.id,
                name: r.name,
                seed_url: r.seed_url,
                interval_minutes: r.interval_minutes as u64,
                max_depth: r.max_depth as u32,
                max_pages: r.max_pages as u32,
                same_origin_concurrency: r.same_origin_concurrency as u32,
                discovery_scope: DiscoveryScope::from_db_value(&r.discovery_scope),
                max_discovered_urls_per_page: r.max_discovered_urls_per_page as u32,
                enabled: r.enabled,
                created_at: r.created_at,
                updated_at: r.updated_at,
                last_enqueued_at: r.last_enqueued_at,
        })
        .collect();

        let frontier_depth: i64 = sqlx::query_scalar(
            "select count(*) from crawl_jobs where owner_developer_id = $1 and status = 'queued'",
        )
        .bind(developer_id)
        .fetch_one(&self.pg_pool)
        .await
        .unwrap_or(0);

        let known_urls: i64 =
            sqlx::query_scalar("select count(*) from crawl_jobs where owner_developer_id = $1")
                .bind(developer_id)
                .fetch_one(&self.pg_pool)
                .await
                .unwrap_or(0);

        let in_flight_jobs: i64 = sqlx::query_scalar(
            "select count(*) from crawl_jobs where owner_developer_id = $1 and status in ('claimed', 'ingesting')",
        )
        .bind(developer_id)
        .fetch_one(&self.pg_pool)
        .await
        .unwrap_or(0);

        let duplicate_documents: i64 =
            sqlx::query_scalar("select count(*) from documents where duplicate_of is not null")
                .fetch_one(&self.pg_pool)
                .await
                .unwrap_or(0);

        let terminal_failures: i64 = sqlx::query_scalar(
            "select count(*) from crawl_jobs where owner_developer_id = $1 and status in ('failed', 'blocked', 'dead_letter')",
        )
        .bind(developer_id)
        .fetch_one(&self.pg_pool)
        .await
        .unwrap_or(0);

        let recent_events: Vec<CrawlEvent> = sqlx::query_as::<_, CrawlEventRow>(
            "select id, event_type, status, message, url, crawler_id, created_at
             from crawl_events where owner_developer_id = $1
             order by created_at desc
             limit 40",
        )
        .bind(developer_id)
        .fetch_all(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?
        .into_iter()
        .map(|r| CrawlEvent {
            id: r.id,
            kind: r.event_type,
            status: r.status,
            message: r.message,
            url: r.url,
            crawler_id: r.crawler_id,
            created_at: r.created_at,
        })
        .collect();

        Ok(CrawlOverviewResponse {
            owner_id: developer_id.to_string(),
            frontier_depth: frontier_depth as usize,
            known_urls: known_urls as usize,
            in_flight_jobs: in_flight_jobs as usize,
            indexed_documents,
            duplicate_documents: duplicate_documents as usize,
            terminal_failures: terminal_failures as usize,
            crawlers,
            rules,
            recent_events,
        })
    }

    pub async fn domain_insight(
        &self,
        domain_input: &str,
    ) -> Result<DeveloperDomainInsightResponse, ApiError> {
        #[derive(sqlx::FromRow)]
        struct DomainDocumentSummaryRow {
            indexed_documents: i64,
            duplicate_documents: i64,
            last_indexed_at: Option<DateTime<Utc>>,
        }

        #[derive(sqlx::FromRow)]
        struct DomainFacetRow {
            label: Option<String>,
            count: i64,
        }

        #[derive(sqlx::FromRow)]
        struct DomainDocumentRow {
            id: String,
            title: String,
            url: String,
            display_url: String,
            language: String,
            last_crawled_at: DateTime<Utc>,
            word_count: i32,
            content_type: String,
            duplicate_of: Option<String>,
        }

        #[derive(sqlx::FromRow)]
        struct DomainJobSummaryRow {
            pending_jobs: i64,
            successful_jobs: i64,
            filtered_jobs: i64,
            failed_jobs: i64,
            blocked_jobs: i64,
            last_crawled_at: Option<DateTime<Utc>>,
        }

        #[derive(sqlx::FromRow)]
        struct DomainJobRow {
            id: String,
            url: String,
            status: String,
            http_status: Option<i32>,
            depth: i32,
            discovered_at: DateTime<Utc>,
            finished_at: Option<DateTime<Utc>>,
            failure_kind: Option<String>,
            failure_message: Option<String>,
            accepted_document_id: Option<String>,
            render_mode: String,
        }

        let domain = normalize_domain_input(domain_input).ok_or_else(|| {
            ApiError::BadRequest("domain must be a valid host or http(s) url".to_string())
        })?;
        let subdomain_pattern = domain_like_pattern(&domain);
        let job_host_expr = "lower(regexp_replace(split_part(split_part(coalesce(final_url, url), '://', 2), '/', 1), ':[0-9]+$', ''))";

        let document_summary = sqlx::query_as::<_, DomainDocumentSummaryRow>(
            r#"
            select
                count(*) filter (where duplicate_of is null) as indexed_documents,
                count(*) filter (where duplicate_of is not null) as duplicate_documents,
                max(last_crawled_at) filter (where duplicate_of is null) as last_indexed_at
            from documents
            where lower(host) = $1 or lower(host) like $2
            "#,
        )
        .bind(&domain)
        .bind(&subdomain_pattern)
        .fetch_one(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

        let top_languages = sqlx::query_as::<_, DomainFacetRow>(
            r#"
            select nullif(language, '') as label, count(*) as count
            from documents
            where duplicate_of is null
              and (lower(host) = $1 or lower(host) like $2)
            group by language
            order by count desc, label asc
            limit 5
            "#,
        )
        .bind(&domain)
        .bind(&subdomain_pattern)
        .fetch_all(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?
        .into_iter()
        .map(|row| DeveloperDomainFacet {
            label: row.label.unwrap_or_else(|| "unknown".to_string()),
            count: row.count.max(0) as usize,
        })
        .collect();

        let top_content_types = sqlx::query_as::<_, DomainFacetRow>(
            r#"
            select nullif(content_type, '') as label, count(*) as count
            from documents
            where duplicate_of is null
              and (lower(host) = $1 or lower(host) like $2)
            group by content_type
            order by count desc, label asc
            limit 5
            "#,
        )
        .bind(&domain)
        .bind(&subdomain_pattern)
        .fetch_all(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?
        .into_iter()
        .map(|row| DeveloperDomainFacet {
            label: row.label.unwrap_or_else(|| "unknown".to_string()),
            count: row.count.max(0) as usize,
        })
        .collect();

        let recent_documents = sqlx::query_as::<_, DomainDocumentRow>(
            r#"
            select
                id,
                title,
                canonical_url as url,
                display_url,
                language,
                last_crawled_at,
                word_count,
                content_type,
                duplicate_of
            from documents
            where lower(host) = $1 or lower(host) like $2
            order by last_crawled_at desc
            limit 8
            "#,
        )
        .bind(&domain)
        .bind(&subdomain_pattern)
        .fetch_all(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?
        .into_iter()
        .map(|row| DeveloperDomainDocument {
            id: row.id,
            title: row.title,
            url: row.url,
            display_url: row.display_url,
            language: row.language,
            last_crawled_at: row.last_crawled_at,
            word_count: row.word_count.max(0) as u32,
            content_type: row.content_type,
            duplicate_of: row.duplicate_of,
        })
        .collect();

        let job_summary_query = format!(
            r#"
            select
                count(*) filter (where status in ('queued', 'claimed', 'ingesting')) as pending_jobs,
                count(*) filter (where status = 'succeeded' and accepted_document_id is not null) as successful_jobs,
                count(*) filter (where status = 'succeeded' and accepted_document_id is null) as filtered_jobs,
                count(*) filter (where status in ('failed', 'dead_letter')) as failed_jobs,
                count(*) filter (where status = 'blocked') as blocked_jobs,
                max(coalesce(finished_at, discovered_at)) as last_crawled_at
            from crawl_jobs
            where {job_host_expr} = $1 or {job_host_expr} like $2
            "#,
        );
        let job_summary = sqlx::query_as::<_, DomainJobSummaryRow>(&job_summary_query)
            .bind(&domain)
            .bind(&subdomain_pattern)
            .fetch_one(&self.pg_pool)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        let recent_jobs_query = format!(
            r#"
            select
                id,
                coalesce(final_url, url) as url,
                status,
                http_status,
                depth,
                discovered_at,
                finished_at,
                failure_kind,
                failure_message,
                accepted_document_id,
                render_mode
            from crawl_jobs
            where {job_host_expr} = $1 or {job_host_expr} like $2
            order by coalesce(finished_at, discovered_at) desc
            limit 10
            "#,
        );
        let recent_jobs = sqlx::query_as::<_, DomainJobRow>(&recent_jobs_query)
            .bind(&domain)
            .bind(&subdomain_pattern)
            .fetch_all(&self.pg_pool)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?
            .into_iter()
            .map(|row| DeveloperDomainJob {
                id: row.id,
                url: row.url,
                status: row.status,
                http_status: row.http_status.map(|value| value as u16),
                depth: row.depth.max(0) as u32,
                discovered_at: row.discovered_at,
                finished_at: row.finished_at,
                failure_kind: row.failure_kind,
                failure_message: row.failure_message,
                accepted_document_id: row.accepted_document_id,
                render_mode: row.render_mode,
            })
            .collect();

        Ok(DeveloperDomainInsightResponse {
            domain: domain.clone(),
            property_url: format!("https://{domain}/"),
            indexed_documents: document_summary.indexed_documents.max(0) as usize,
            duplicate_documents: document_summary.duplicate_documents.max(0) as usize,
            pending_jobs: job_summary.pending_jobs.max(0) as usize,
            successful_jobs: job_summary.successful_jobs.max(0) as usize,
            filtered_jobs: job_summary.filtered_jobs.max(0) as usize,
            failed_jobs: job_summary.failed_jobs.max(0) as usize,
            blocked_jobs: job_summary.blocked_jobs.max(0) as usize,
            last_indexed_at: document_summary.last_indexed_at,
            last_crawled_at: job_summary.last_crawled_at,
            top_languages,
            top_content_types,
            recent_documents,
            recent_jobs,
        })
    }

    pub async fn submit_domain_urls(
        &self,
        owner_developer_id: &str,
        submitted_by: &str,
        request: DeveloperDomainSubmitRequest,
    ) -> Result<DeveloperDomainSubmitResponse, ApiError> {
        if request.urls.is_empty() {
            return Err(ApiError::BadRequest(
                "at least one property url is required".to_string(),
            ));
        }

        let domain = normalize_domain_input(&request.domain).ok_or_else(|| {
            ApiError::BadRequest("domain must be a valid host or http(s) url".to_string())
        })?;
        let mut urls = Vec::new();
        for raw_url in request.urls {
            let trimmed = raw_url.trim();
            if trimmed.is_empty() {
                continue;
            }
            let normalized = normalize_url(trimmed)
                .ok_or_else(|| ApiError::BadRequest(format!("invalid property url: {trimmed}")))?;
            let host = extract_host(&normalized)
                .ok_or_else(|| ApiError::BadRequest(format!("invalid property url: {trimmed}")))?;
            if !host_matches_scope(&host, &domain, DiscoveryScope::SameDomain) {
                return Err(ApiError::BadRequest(format!(
                    "submitted url host must stay within the {domain} property"
                )));
            }
            urls.push(normalized);
        }

        if urls.is_empty() {
            return Err(ApiError::BadRequest(
                "at least one property url is required".to_string(),
            ));
        }

        let budget_id = Uuid::now_v7().to_string();
        let source = format!("developer-property:{submitted_by}:{domain}");
        let accepted_urls = self
            .enqueue_urls(
                owner_developer_id,
                urls,
                &source,
                &budget_id,
                0,
                request.max_depth.min(10) as i32,
                request.max_pages.clamp(1, 10_000) as i32,
                request.same_origin_concurrency.clamp(1, 32) as i32,
                Some(submitted_by),
                None,
                DiscoveryScope::SameDomain,
                Some(&domain),
                50,
                request.allow_revisit,
            )
            .await?;

        self.push_event(
            owner_developer_id,
            "developer-property-seeded",
            "ok",
            format!("developer {submitted_by} queued {accepted_urls} urls for {domain}"),
            Some(format!("https://{domain}/")),
            None,
        )
        .await?;

        let subdomain_pattern = domain_like_pattern(&domain);
        let job_host_expr = "lower(regexp_replace(split_part(split_part(coalesce(final_url, url), '://', 2), '/', 1), ':[0-9]+$', ''))";
        let counts_query = format!(
            r#"
            select
                count(*) filter (where status in ('queued', 'claimed', 'ingesting')) as queued_domain_jobs,
                count(*) as known_domain_urls
            from crawl_jobs
            where {job_host_expr} = $1 or {job_host_expr} like $2
            "#,
        );
        let (queued_domain_jobs, known_domain_urls): (i64, i64) = sqlx::query_as(&counts_query)
            .bind(&domain)
            .bind(&subdomain_pattern)
            .fetch_one(&self.pg_pool)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        Ok(DeveloperDomainSubmitResponse {
            accepted_urls,
            queued_domain_jobs: queued_domain_jobs.max(0) as usize,
            known_domain_urls: known_domain_urls.max(0) as usize,
        })
    }

    pub async fn seed_frontier(
        &self,
        developer_id: &str,
        request: SeedFrontierRequest,
    ) -> Result<SeedFrontierResponse, ApiError> {
        if request.urls.is_empty() {
            return Err(ApiError::BadRequest(
                "at least one seed url is required".to_string(),
            ));
        }

        let source = request
            .source
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| format!("manual:{developer_id}"));
        let budget_id = Uuid::now_v7().to_string();

        let accepted_urls = self
            .enqueue_urls(
                developer_id,
                request.urls,
                &source,
                &budget_id,
                0,
                request.max_depth.min(10) as i32,
                request.max_pages.clamp(1, 10_000) as i32,
                request.same_origin_concurrency.clamp(1, 32) as i32,
                Some(developer_id),
                None,
                request.discovery_scope,
                None,
                request.max_discovered_urls_per_page.clamp(1, 200) as i32,
                request.allow_revisit,
            )
            .await?;

        self.push_event(
            developer_id,
            "seed-queued",
            "ok",
            format!("queued {accepted_urls} manual seed urls"),
            None,
            None,
        )
        .await?;

        let frontier_depth: i64 = sqlx::query_scalar(
            "select count(*) from crawl_jobs where owner_developer_id = $1 and status = 'queued'",
        )
        .bind(developer_id)
        .fetch_one(&self.pg_pool)
        .await
        .unwrap_or(0);

        let known_urls: i64 =
            sqlx::query_scalar("select count(*) from crawl_jobs where owner_developer_id = $1")
                .bind(developer_id)
                .fetch_one(&self.pg_pool)
                .await
                .unwrap_or(0);

        Ok(SeedFrontierResponse {
            accepted_urls,
            frontier_depth: frontier_depth as usize,
            known_urls: known_urls as usize,
        })
    }

    pub async fn claim_jobs(
        &self,
        crawler_id: &str,
        crawler_name: Option<&str>,
        auth_header: Option<&str>,
        default_owner_developer_id: &str,
        request: ClaimJobsRequest,
        capabilities: Option<&CrawlerCapabilities>,
    ) -> Result<ClaimJobsResponse, ApiError> {
        let token_hash = bearer_hash(auth_header)?;
        let max_jobs = request.max_jobs.clamp(1, 100) as i64;
        let now = Utc::now();

        let crawler = self
            .validate_crawler_auth(
                crawler_id,
                crawler_name,
                &token_hash,
                default_owner_developer_id,
            )
            .await?;

        // Update last_seen_at and last_claimed_at
        if let Some(caps) = capabilities {
            let caps_json = serde_json::to_value(caps).map_err(|e| ApiError::Internal(e.into()))?;
            sqlx::query("update crawlers set last_seen_at = $2, last_claimed_at = $2, metadata = metadata || $3::jsonb where id = $1")
                .bind(crawler_id)
                .bind(now)
                .bind(&caps_json)
                .execute(&self.pg_pool)
                .await
                .map_err(|e| ApiError::Internal(e.into()))?;
        } else {
            sqlx::query(
                "update crawlers set last_seen_at = $2, last_claimed_at = $2 where id = $1",
            )
            .bind(crawler_id)
            .bind(now)
            .execute(&self.pg_pool)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;
        }

        let crawler_has_js_render = capabilities.map(|c| c.js_render).unwrap_or(false);

        let claim = self
            .frontier
            .claim_jobs(
                &crawler.owner_developer_id,
                crawler_id,
                max_jobs as usize,
                crawler_has_js_render,
                now,
            )
            .await?;
        let lease_id = claim.lease_id;
        let mut jobs = claim.jobs;

        // Enrich jobs with conditional request headers from existing documents
        for job in &mut jobs {
            if let Ok(Some(row)) = sqlx::query_as::<_, (Option<String>, Option<String>)>(
                "SELECT http_etag, http_last_modified FROM documents WHERE canonical_url = $1",
            )
            .bind(&job.url)
            .fetch_optional(&self.pg_pool)
            .await
            {
                job.etag = row.0;
                job.last_modified = row.1;
            }
        }

        // Update jobs_claimed counter
        if !jobs.is_empty() {
            sqlx::query("update crawlers set jobs_claimed = jobs_claimed + $2 where id = $1")
                .bind(crawler_id)
                .bind(jobs.len() as i64)
                .execute(&self.pg_pool)
                .await
                .map_err(|e| ApiError::Internal(e.into()))?;

            self.push_event(
                &crawler.owner_developer_id,
                "jobs-claimed",
                "ok",
                format!("crawler {} claimed {} jobs", crawler.name, jobs.len()),
                jobs.first().map(|j| j.url.clone()),
                Some(crawler_id.to_string()),
            )
            .await?;
        }

        let frontier_depth: i64 = sqlx::query_scalar(
            "select count(*) from crawl_jobs where owner_developer_id = $1 and status = 'queued'",
        )
        .bind(&crawler.owner_developer_id)
        .fetch_one(&self.pg_pool)
        .await
        .unwrap_or(0);

        Ok(ClaimJobsResponse {
            crawler_id: crawler_id.to_string(),
            lease_id: (!jobs.is_empty()).then_some(lease_id),
            frontier_depth: frontier_depth as usize,
            jobs,
        })
    }

    pub async fn submit_report(
        &self,
        crawler_id: &str,
        crawler_name: Option<&str>,
        auth_header: Option<&str>,
        default_owner_developer_id: &str,
        request: SubmitCrawlReportRequest,
    ) -> Result<SubmitCrawlReportResponse, ApiError> {
        let token_hash = bearer_hash(auth_header)?;

        let crawler = self
            .validate_crawler_auth(
                crawler_id,
                crawler_name,
                &token_hash,
                default_owner_developer_id,
            )
            .await?;

        sqlx::query("update crawlers set last_seen_at = $2 where id = $1")
            .bind(crawler_id)
            .bind(Utc::now())
            .execute(&self.pg_pool)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        let SubmitCrawlReportRequest { lease_id, results } = request;
        let reported = results.len();
        let stage = self
            .ingest
            .stage_report(
                &self.frontier,
                &crawler.owner_developer_id,
                crawler_id,
                &lease_id,
                results,
            )
            .await?;

        if reported > 0 {
            sqlx::query("update crawlers set jobs_reported = jobs_reported + $2 where id = $1")
                .bind(crawler_id)
                .bind(reported as i64)
                .execute(&self.pg_pool)
                .await
                .map_err(|e| ApiError::Internal(e.into()))?;
        }

        let frontier_depth = self
            .frontier
            .frontier_depth(&crawler.owner_developer_id)
            .await;

        Ok(SubmitCrawlReportResponse {
            lease_id,
            staged_results: stage.staged_results,
            pending_results: stage.pending_results,
            frontier_depth,
        })
    }

    pub(crate) async fn process_pending_ingests(
        &self,
        search_index: &SearchIndex,
        limit: usize,
    ) -> Result<usize, ApiError> {
        let mut processed = 0usize;
        while processed < limit {
            let drained = self
                .projection
                .drain(self, search_index, limit - processed)
                .await?;
            if drained == 0 {
                break;
            }
            processed += drained;
        }
        Ok(processed)
    }

    pub(crate) async fn recover_stale_ingests(&self, timeout: Duration) -> Result<(), ApiError> {
        let recovered = self.ingest.recover_stale_items(timeout).await?;

        for item in recovered.requeued {
            self.push_event(
                &item.owner_developer_id,
                "stale-ingest-requeued",
                "ok",
                format!(
                    "requeued stale ingest item {} for lease {}",
                    item.crawl_job_id, item.lease_id
                ),
                Some(item.url),
                Some(item.crawler_id),
            )
            .await?;
        }

        for item in recovered.failed {
            self.push_event(
                &item.owner_developer_id,
                "stale-ingest-failed",
                "error",
                format!(
                    "stale ingest item {} could not be replayed because crawl job is {}",
                    item.crawl_job_id, item.crawl_job_status
                ),
                Some(item.url),
                Some(item.crawler_id),
            )
            .await?;
        }

        Ok(())
    }

    pub(crate) async fn apply_staged_result(
        &self,
        search_index: &SearchIndex,
        item: &PendingIngestItem,
        result: CrawlResultInput,
    ) -> Result<(), ApiError> {
        let now = Utc::now();
        let in_flight = sqlx::query_as::<_, InFlightJobRow>(
            "select id, url, origin_key, depth, max_depth, max_pages, budget_id, rule_id, attempt_count, max_attempts, discovery_scope, discovery_host, same_origin_concurrency, max_discovered_urls_per_page from crawl_jobs
             where id = $1 and owner_developer_id = $2 and claimed_by = $3 and lease_id = $4 and status = 'ingesting'",
        )
        .bind(&item.crawl_job_id)
        .bind(&item.owner_developer_id)
        .bind(&item.crawler_id)
        .bind(&item.lease_id)
        .fetch_optional(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?
        .ok_or_else(|| ApiError::Conflict("staged crawl job is no longer ingesting".to_string()))?;

        if in_flight.url != result.url {
            return Err(ApiError::BadRequest(
                "crawl report contained a job not assigned to this crawler".to_string(),
            ));
        }

        let finalized_url = result
            .final_url
            .clone()
            .unwrap_or_else(|| result.url.clone());
        let discovery_scope = DiscoveryScope::from_db_value(&in_flight.discovery_scope);
        let scoped_discovered_urls = filter_discovered_urls(
            result.discovered_urls.clone(),
            discovery_scope,
            in_flight.discovery_host.as_deref(),
            in_flight.max_discovered_urls_per_page.max(1) as usize,
        );
        let discovered_count = scoped_discovered_urls.len() as i32;
        let http_status = result.status_code as i32;
        let content_type = result.content_type.clone();
        let llm_decision = summarize_llm_decision(&result);
        let redirect_chain_json = serde_json::to_value(&result.redirect_chain)
            .map_err(|e| ApiError::Internal(e.into()))?;

        match classify_job_outcome(&result, &in_flight) {
            JobOutcome::Succeeded(document) => {
                sqlx::query(
                    "update crawl_jobs
                     set status = 'succeeded',
                         failure_kind = null,
                         failure_message = null,
                         next_retry_at = null,
                         finished_at = $2,
                         final_url = $3,
                         content_type = $4,
                         http_status = $5,
                         discovered_urls_count = $6,
                         accepted_document_id = $7,
                         llm_decision = $8,
                         llm_reason = $9,
                         llm_relevance_score = $10,
                         canonical_hint = $11,
                         canonical_source = $12,
                         render_mode = $13,
                         redirect_chain_json = $14
                     where id = $1",
                )
                .bind(&result.job_id)
                .bind(now)
                .bind(&finalized_url)
                .bind(content_type.as_deref())
                .bind(http_status)
                .bind(discovered_count)
                .bind(&document.id)
                .bind(llm_decision.as_deref())
                .bind(result.llm_reason.as_deref())
                .bind(result.llm_relevance_score)
                .bind(result.canonical_hint.as_deref())
                .bind(result.canonical_source.as_deref())
                .bind(&result.render_mode)
                .bind(&redirect_chain_json)
                .execute(&self.pg_pool)
                .await
                .map_err(|e| ApiError::Internal(e.into()))?;

                if in_flight.depth < in_flight.max_depth {
                    for discovered_url in &scoped_discovered_urls {
                        let _ = sqlx::query(
                            "update documents set inlink_count = inlink_count + 1
                             where canonical_url = $1",
                        )
                        .bind(discovered_url)
                        .execute(&self.pg_pool)
                        .await;
                    }

                    for discovered_url in &scoped_discovered_urls {
                        let _ = sqlx::query(
                            "INSERT INTO link_edges (source_url, target_url, discovered_at)
                             VALUES ($1, $2, now())
                             ON CONFLICT (source_url, target_url) DO UPDATE SET discovered_at = now()",
                        )
                        .bind(&finalized_url)
                        .bind(discovered_url)
                        .execute(&self.pg_pool)
                        .await;
                    }

                    self.enqueue_urls(
                        &item.owner_developer_id,
                        scoped_discovered_urls.clone(),
                        &finalized_url,
                        &in_flight.budget_id,
                        in_flight.depth + 1,
                        in_flight.max_depth,
                        in_flight.max_pages,
                        in_flight.same_origin_concurrency,
                        Some(&item.owner_developer_id),
                        in_flight.rule_id.as_deref(),
                        discovery_scope,
                        in_flight.discovery_host.as_deref(),
                        in_flight.max_discovered_urls_per_page,
                        false,
                    )
                    .await?;
                }

                if let Some(ref domain) = extract_host(&finalized_url) {
                    let content_changed = if let Some(ref body) = result.body {
                        let new_hash = content_hash(body);
                        let existing_hash: Option<String> = sqlx::query_scalar(
                            "SELECT content_hash FROM documents WHERE canonical_url = $1",
                        )
                        .bind(&finalized_url)
                        .fetch_optional(&self.pg_pool)
                        .await
                        .ok()
                        .flatten();
                        existing_hash.as_deref() != Some(&new_hash)
                    } else {
                        false
                    };

                    let _ = sqlx::query(
                        "INSERT INTO domain_crawl_stats (domain, total_pages_indexed, last_success_at, consecutive_failures, updated_at)
                         VALUES ($1, 1, now(), 0, now())
                         ON CONFLICT (domain) DO UPDATE SET
                             total_pages_indexed = domain_crawl_stats.total_pages_indexed + 1,
                             last_success_at = now(),
                             consecutive_failures = 0,
                             health_status = 'healthy',
                             updated_at = now()",
                    )
                    .bind(domain)
                    .execute(&self.pg_pool)
                    .await;

                    let _ = sqlx::query(
                        "UPDATE domain_crawl_stats SET
                            content_checks = content_checks + 1,
                            content_changes = content_changes + CASE WHEN $2 THEN 1 ELSE 0 END,
                            avg_change_frequency_hours = CASE
                                WHEN content_checks > 0 THEN
                                    (content_changes::real + CASE WHEN $2 THEN 1 ELSE 0 END) /
                                    (content_checks::real + 1) * 168.0
                                ELSE NULL
                            END,
                            updated_at = now()
                         WHERE domain = $1",
                    )
                    .bind(domain)
                    .bind(content_changed)
                    .execute(&self.pg_pool)
                    .await;
                }

                search_index.upsert_documents(vec![document]).await?;

                self.push_event(
                    &item.owner_developer_id,
                    "job-succeeded",
                    "ok",
                    format!(
                        "indexed {finalized_url} on attempt {}",
                        in_flight.attempt_count
                    ),
                    Some(finalized_url.clone()),
                    Some(item.crawler_id.clone()),
                )
                .await?;

                if result.http_etag.is_some() || result.http_last_modified.is_some() {
                    let doc_url = result.final_url.as_ref().unwrap_or(&result.url);
                    let doc_id = stable_document_id(doc_url);
                    let _ = sqlx::query(
                        "UPDATE documents SET http_etag = $2, http_last_modified = $3 WHERE id = $1",
                    )
                    .bind(&doc_id)
                    .bind(result.http_etag.as_deref())
                    .bind(result.http_last_modified.as_deref())
                    .execute(&self.pg_pool)
                    .await;
                }
            }
            JobOutcome::Filtered { reason } => {
                sqlx::query(
                    "update crawl_jobs
                     set status = 'succeeded',
                         failure_kind = null,
                         failure_message = null,
                         next_retry_at = null,
                         finished_at = $2,
                         final_url = $3,
                         content_type = $4,
                         http_status = $5,
                         discovered_urls_count = $6,
                         accepted_document_id = null,
                         llm_decision = $7,
                         llm_reason = $8,
                         llm_relevance_score = $9,
                         canonical_hint = $10,
                         canonical_source = $11,
                         render_mode = $12,
                         redirect_chain_json = $13
                     where id = $1",
                )
                .bind(&result.job_id)
                .bind(now)
                .bind(&finalized_url)
                .bind(content_type.as_deref())
                .bind(http_status)
                .bind(discovered_count)
                .bind(llm_decision.as_deref())
                .bind(result.llm_reason.as_deref())
                .bind(result.llm_relevance_score)
                .bind(result.canonical_hint.as_deref())
                .bind(result.canonical_source.as_deref())
                .bind(&result.render_mode)
                .bind(&redirect_chain_json)
                .execute(&self.pg_pool)
                .await
                .map_err(|e| ApiError::Internal(e.into()))?;

                if in_flight.depth < in_flight.max_depth {
                    self.enqueue_urls(
                        &item.owner_developer_id,
                        scoped_discovered_urls.clone(),
                        &finalized_url,
                        &in_flight.budget_id,
                        in_flight.depth + 1,
                        in_flight.max_depth,
                        in_flight.max_pages,
                        in_flight.same_origin_concurrency,
                        Some(&item.owner_developer_id),
                        in_flight.rule_id.as_deref(),
                        discovery_scope,
                        in_flight.discovery_host.as_deref(),
                        in_flight.max_discovered_urls_per_page,
                        false,
                    )
                    .await?;
                }

                self.push_event(
                    &item.owner_developer_id,
                    "job-filtered",
                    "ok",
                    format!("skipped indexing {finalized_url}: {reason}"),
                    Some(finalized_url.clone()),
                    Some(item.crawler_id.clone()),
                )
                .await?;

                if result.status_code == 304 {
                    let doc_url = result.final_url.as_ref().unwrap_or(&result.url);
                    let doc_id = stable_document_id(doc_url);
                    let _ = sqlx::query("UPDATE documents SET last_crawled_at = $2 WHERE id = $1")
                        .bind(&doc_id)
                        .bind(now)
                        .execute(&self.pg_pool)
                        .await;
                }
            }
            JobOutcome::Retryable {
                failure_kind,
                failure_message,
                next_retry_at,
            } => {
                sqlx::query(
                    "update crawl_jobs
                     set status = 'queued',
                         claimed_by = null,
                         claimed_at = null,
                         lease_id = null,
                         next_retry_at = $2,
                         failure_kind = $3,
                         failure_message = $4,
                         finished_at = null,
                         final_url = $5,
                         content_type = $6,
                         http_status = $7,
                         discovered_urls_count = $8,
                         accepted_document_id = null,
                         llm_decision = $9,
                         llm_reason = $10,
                         llm_relevance_score = $11,
                         canonical_hint = $12,
                         canonical_source = $13,
                         render_mode = $14,
                         redirect_chain_json = $15
                     where id = $1",
                )
                .bind(&result.job_id)
                .bind(next_retry_at)
                .bind(&failure_kind)
                .bind(&failure_message)
                .bind(&finalized_url)
                .bind(content_type.as_deref())
                .bind(http_status)
                .bind(discovered_count)
                .bind(llm_decision.as_deref())
                .bind(result.llm_reason.as_deref())
                .bind(result.llm_relevance_score)
                .bind(result.canonical_hint.as_deref())
                .bind(result.canonical_source.as_deref())
                .bind(&result.render_mode)
                .bind(&redirect_chain_json)
                .execute(&self.pg_pool)
                .await
                .map_err(|e| ApiError::Internal(e.into()))?;

                self.push_event(
                    &item.owner_developer_id,
                    "job-requeued",
                    "error",
                    format!(
                        "{}; retrying at {}",
                        failure_message,
                        next_retry_at.to_rfc3339()
                    ),
                    Some(finalized_url.clone()),
                    Some(item.crawler_id.clone()),
                )
                .await?;
            }
            JobOutcome::RequiresJsRender { failure_message } => {
                sqlx::query(
                    "update crawl_jobs
                     set status = 'queued',
                         claimed_by = null,
                         claimed_at = null,
                         lease_id = null,
                         next_retry_at = null,
                         failure_kind = null,
                         failure_message = null,
                         finished_at = null,
                         final_url = $2,
                         content_type = $3,
                         http_status = $4,
                         discovered_urls_count = $5,
                         accepted_document_id = null,
                         llm_decision = $6,
                         llm_reason = $7,
                         llm_relevance_score = $8,
                         canonical_hint = $9,
                         canonical_source = $10,
                         requires_js = true,
                         render_mode = $11,
                         redirect_chain_json = $12
                     where id = $1",
                )
                .bind(&result.job_id)
                .bind(&finalized_url)
                .bind(content_type.as_deref())
                .bind(http_status)
                .bind(discovered_count)
                .bind(llm_decision.as_deref())
                .bind(result.llm_reason.as_deref())
                .bind(result.llm_relevance_score)
                .bind(result.canonical_hint.as_deref())
                .bind(result.canonical_source.as_deref())
                .bind(&result.render_mode)
                .bind(&redirect_chain_json)
                .execute(&self.pg_pool)
                .await
                .map_err(|e| ApiError::Internal(e.into()))?;

                self.push_event(
                    &item.owner_developer_id,
                    "job-requires-js",
                    "ok",
                    format!("{}; re-queuing for JS-capable node", failure_message),
                    Some(finalized_url.clone()),
                    Some(item.crawler_id.clone()),
                )
                .await?;
            }
            JobOutcome::Blocked {
                failure_kind,
                failure_message,
            } => {
                sqlx::query(
                    "update crawl_jobs
                     set status = 'blocked',
                         failure_kind = $2,
                         failure_message = $3,
                         next_retry_at = null,
                         finished_at = $4,
                         final_url = $5,
                         content_type = $6,
                         http_status = $7,
                         discovered_urls_count = $8,
                         accepted_document_id = null,
                         llm_decision = $9,
                         llm_reason = $10,
                         llm_relevance_score = $11,
                         canonical_hint = $12,
                         canonical_source = $13,
                         render_mode = $14,
                         redirect_chain_json = $15
                     where id = $1",
                )
                .bind(&result.job_id)
                .bind(&failure_kind)
                .bind(&failure_message)
                .bind(now)
                .bind(&finalized_url)
                .bind(content_type.as_deref())
                .bind(http_status)
                .bind(discovered_count)
                .bind(llm_decision.as_deref())
                .bind(result.llm_reason.as_deref())
                .bind(result.llm_relevance_score)
                .bind(result.canonical_hint.as_deref())
                .bind(result.canonical_source.as_deref())
                .bind(&result.render_mode)
                .bind(&redirect_chain_json)
                .execute(&self.pg_pool)
                .await
                .map_err(|e| ApiError::Internal(e.into()))?;

                self.push_event(
                    &item.owner_developer_id,
                    "job-blocked",
                    "error",
                    failure_message,
                    Some(finalized_url.clone()),
                    Some(item.crawler_id.clone()),
                )
                .await?;
            }
            JobOutcome::Failed {
                failure_kind,
                failure_message,
            } => {
                sqlx::query(
                    "update crawl_jobs
                     set status = 'failed',
                         failure_kind = $2,
                         failure_message = $3,
                         next_retry_at = null,
                         finished_at = $4,
                         final_url = $5,
                         content_type = $6,
                         http_status = $7,
                         discovered_urls_count = $8,
                         accepted_document_id = null,
                         llm_decision = $9,
                         llm_reason = $10,
                         llm_relevance_score = $11,
                         canonical_hint = $12,
                         canonical_source = $13,
                         render_mode = $14,
                         redirect_chain_json = $15
                     where id = $1",
                )
                .bind(&result.job_id)
                .bind(&failure_kind)
                .bind(&failure_message)
                .bind(now)
                .bind(&finalized_url)
                .bind(content_type.as_deref())
                .bind(http_status)
                .bind(discovered_count)
                .bind(llm_decision.as_deref())
                .bind(result.llm_reason.as_deref())
                .bind(result.llm_relevance_score)
                .bind(result.canonical_hint.as_deref())
                .bind(result.canonical_source.as_deref())
                .bind(&result.render_mode)
                .bind(&redirect_chain_json)
                .execute(&self.pg_pool)
                .await
                .map_err(|e| ApiError::Internal(e.into()))?;

                if let Some(ref domain) = extract_host(&finalized_url) {
                    let _ = sqlx::query(
                        "INSERT INTO domain_crawl_stats (domain, total_pages_failed, last_failure_at, consecutive_failures, updated_at)
                         VALUES ($1, 1, now(), 1, now())
                         ON CONFLICT (domain) DO UPDATE SET
                             total_pages_failed = domain_crawl_stats.total_pages_failed + 1,
                             last_failure_at = now(),
                             consecutive_failures = domain_crawl_stats.consecutive_failures + 1,
                             health_status = CASE
                                 WHEN domain_crawl_stats.consecutive_failures + 1 >= 10 THEN 'unhealthy'
                                 WHEN domain_crawl_stats.consecutive_failures + 1 >= 5 THEN 'degraded'
                                 ELSE domain_crawl_stats.health_status
                             END,
                             updated_at = now()",
                    )
                    .bind(domain)
                    .execute(&self.pg_pool)
                    .await;
                }

                self.push_event(
                    &item.owner_developer_id,
                    "job-failed",
                    "error",
                    failure_message,
                    Some(finalized_url.clone()),
                    Some(item.crawler_id.clone()),
                )
                .await?;
            }
            JobOutcome::DeadLetter {
                failure_kind,
                failure_message,
            } => {
                sqlx::query(
                    "update crawl_jobs
                     set status = 'dead_letter',
                         failure_kind = $2,
                         failure_message = $3,
                         next_retry_at = null,
                         finished_at = $4,
                         final_url = $5,
                         content_type = $6,
                         http_status = $7,
                         discovered_urls_count = $8,
                         accepted_document_id = null,
                         llm_decision = $9,
                         llm_reason = $10,
                         llm_relevance_score = $11,
                         canonical_hint = $12,
                         canonical_source = $13,
                         render_mode = $14,
                         redirect_chain_json = $15
                     where id = $1",
                )
                .bind(&result.job_id)
                .bind(&failure_kind)
                .bind(&failure_message)
                .bind(now)
                .bind(&finalized_url)
                .bind(content_type.as_deref())
                .bind(http_status)
                .bind(discovered_count)
                .bind(llm_decision.as_deref())
                .bind(result.llm_reason.as_deref())
                .bind(result.llm_relevance_score)
                .bind(result.canonical_hint.as_deref())
                .bind(result.canonical_source.as_deref())
                .bind(&result.render_mode)
                .bind(&redirect_chain_json)
                .execute(&self.pg_pool)
                .await
                .map_err(|e| ApiError::Internal(e.into()))?;

                if let Some(ref domain) = extract_host(&finalized_url) {
                    let _ = sqlx::query(
                        "INSERT INTO domain_crawl_stats (domain, total_pages_failed, last_failure_at, consecutive_failures, updated_at)
                         VALUES ($1, 1, now(), 1, now())
                         ON CONFLICT (domain) DO UPDATE SET
                             total_pages_failed = domain_crawl_stats.total_pages_failed + 1,
                             last_failure_at = now(),
                             consecutive_failures = domain_crawl_stats.consecutive_failures + 1,
                             health_status = CASE
                                 WHEN domain_crawl_stats.consecutive_failures + 1 >= 10 THEN 'unhealthy'
                                 WHEN domain_crawl_stats.consecutive_failures + 1 >= 5 THEN 'degraded'
                                 ELSE domain_crawl_stats.health_status
                             END,
                             updated_at = now()",
                    )
                    .bind(domain)
                    .execute(&self.pg_pool)
                    .await;
                }

                self.push_event(
                    &item.owner_developer_id,
                    "job-dead-lettered",
                    "error",
                    failure_message,
                    Some(finalized_url.clone()),
                    Some(item.crawler_id.clone()),
                )
                .await?;
            }
            JobOutcome::Gone {
                failure_kind,
                failure_message,
            } => {
                sqlx::query(
                    "update crawl_jobs
                     set status = 'failed',
                         failure_kind = $2,
                         failure_message = $3,
                         next_retry_at = null,
                         finished_at = $4,
                         final_url = $5,
                         content_type = $6,
                         http_status = $7,
                         discovered_urls_count = $8,
                         accepted_document_id = null,
                         llm_decision = $9,
                         llm_reason = $10,
                         llm_relevance_score = $11,
                         canonical_hint = $12,
                         canonical_source = $13,
                         render_mode = $14,
                         redirect_chain_json = $15
                     where id = $1",
                )
                .bind(&result.job_id)
                .bind(&failure_kind)
                .bind(&failure_message)
                .bind(now)
                .bind(&finalized_url)
                .bind(content_type.as_deref())
                .bind(http_status)
                .bind(discovered_count)
                .bind(llm_decision.as_deref())
                .bind(result.llm_reason.as_deref())
                .bind(result.llm_relevance_score)
                .bind(result.canonical_hint.as_deref())
                .bind(result.canonical_source.as_deref())
                .bind(&result.render_mode)
                .bind(&redirect_chain_json)
                .execute(&self.pg_pool)
                .await
                .map_err(|e| ApiError::Internal(e.into()))?;

                let doc_id = stable_document_id(&finalized_url);
                let _ = search_index.delete_document(&doc_id).await;
                if finalized_url != result.url {
                    let original_doc_id = stable_document_id(&result.url);
                    let _ = search_index.delete_document(&original_doc_id).await;
                }
                if let Some(canonical_hint) = result.canonical_hint.as_deref() {
                    let canonical_doc_id = stable_document_id(canonical_hint);
                    let _ = search_index.delete_document(&canonical_doc_id).await;
                }

                if let Some(ref domain) = extract_host(&finalized_url) {
                    let _ = sqlx::query(
                        "INSERT INTO domain_crawl_stats (domain, total_pages_failed, last_failure_at, consecutive_failures, updated_at)
                         VALUES ($1, 1, now(), 1, now())
                         ON CONFLICT (domain) DO UPDATE SET
                             total_pages_failed = domain_crawl_stats.total_pages_failed + 1,
                             last_failure_at = now(),
                             consecutive_failures = domain_crawl_stats.consecutive_failures + 1,
                             health_status = CASE
                                 WHEN domain_crawl_stats.consecutive_failures + 1 >= 10 THEN 'unhealthy'
                                 WHEN domain_crawl_stats.consecutive_failures + 1 >= 5 THEN 'degraded'
                                 ELSE domain_crawl_stats.health_status
                             END,
                             updated_at = now()",
                    )
                    .bind(domain)
                    .execute(&self.pg_pool)
                    .await;
                }

                self.push_event(
                    &item.owner_developer_id,
                    "job-gone",
                    "error",
                    format!("removed from index: {failure_message}"),
                    Some(finalized_url.clone()),
                    Some(item.crawler_id.clone()),
                )
                .await?;
            }
        }

        self.update_origin_after_report(
            &item.owner_developer_id,
            &in_flight.origin_key,
            &finalized_url,
            &result,
            now,
        )
        .await?;

        Ok(())
    }

    pub(crate) async fn mark_projection_failure(
        &self,
        item: &PendingIngestItem,
        error_message: &str,
    ) -> Result<(), ApiError> {
        let now = Utc::now();
        let failed = sqlx::query_as::<_, ProjectionFailureRow>(
            "update crawl_jobs
             set status = 'failed',
                 claimed_by = null,
                 claimed_at = null,
                 lease_id = null,
                 next_retry_at = null,
                 failure_kind = 'projection_error',
                 failure_message = $5,
                 finished_at = $6
             where owner_developer_id = $1
               and id = $2
               and claimed_by = $3
               and lease_id = $4
               and status = 'ingesting'
             returning origin_key, url",
        )
        .bind(&item.owner_developer_id)
        .bind(&item.crawl_job_id)
        .bind(&item.crawler_id)
        .bind(&item.lease_id)
        .bind(error_message)
        .bind(now)
        .fetch_optional(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

        if let Some(failed) = failed {
            sqlx::query(
                "update crawl_origins
                 set in_flight_count = greatest(in_flight_count - 1, 0),
                     updated_at = $3
                 where owner_developer_id = $1 and origin_key = $2",
            )
            .bind(&item.owner_developer_id)
            .bind(&failed.origin_key)
            .bind(now)
            .execute(&self.pg_pool)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

            self.push_event(
                &item.owner_developer_id,
                "job-projection-failed",
                "error",
                error_message.to_string(),
                Some(failed.url),
                Some(item.crawler_id.clone()),
            )
            .await?;
        }

        Ok(())
    }

    pub async fn heartbeat_crawler(
        &self,
        crawler_id: &str,
        crawler_name: Option<&str>,
        auth_header: Option<&str>,
        default_owner_developer_id: &str,
        capabilities: Option<&CrawlerCapabilities>,
    ) -> Result<crate::models::CrawlerHeartbeatResponse, ApiError> {
        let token_hash = bearer_hash(auth_header)?;
        self.validate_crawler_auth(
            crawler_id,
            crawler_name,
            &token_hash,
            default_owner_developer_id,
        )
        .await?;

        let metadata: serde_json::Value = if let Some(caps) = capabilities {
            let caps_json = serde_json::to_value(caps).map_err(|e| ApiError::Internal(e.into()))?;
            sqlx::query_scalar(
                "update crawlers
                 set last_seen_at = $2,
                     metadata = metadata || $3::jsonb
                 where id = $1
                 returning metadata",
            )
            .bind(crawler_id)
            .bind(Utc::now())
            .bind(&caps_json)
            .fetch_one(&self.pg_pool)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?
        } else {
            sqlx::query_scalar(
                "update crawlers
                 set last_seen_at = $2
                 where id = $1
                 returning metadata",
            )
            .bind(crawler_id)
            .bind(Utc::now())
            .fetch_one(&self.pg_pool)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?
        };

        let metadata = crawler_runtime_metadata(&metadata);
        let default_worker_concurrency = parse_positive_usize_config(
            self.get_system_config("crawler.total_concurrency").await,
            16,
        );
        let default_js_render_concurrency = parse_positive_usize_config(
            self.get_system_config("crawler.js_render_concurrency")
                .await,
            1,
        );

        Ok(crate::models::CrawlerHeartbeatResponse {
            worker_concurrency: metadata
                .worker_concurrency
                .filter(|value| *value > 0)
                .unwrap_or(default_worker_concurrency),
            js_render_concurrency: metadata
                .js_render_concurrency
                .filter(|value| *value > 0)
                .unwrap_or(default_js_render_concurrency),
        })
    }

    async fn schedule_adaptive_recrawl(&self, _now: DateTime<Utc>) -> Result<(), ApiError> {
        let stale_docs = sqlx::query_as::<_, StaleDocRow>(
            "SELECT d.canonical_url, d.host,
                    j.owner_developer_id, j.budget_id, j.rule_id, j.discovery_scope
             FROM documents d
             LEFT JOIN domain_crawl_stats dcs ON dcs.domain = d.host
             JOIN crawl_jobs j ON j.id = d.source_job_id
             WHERE d.duplicate_of IS NULL
               AND d.source_job_id IS NOT NULL
               AND d.last_crawled_at < now() - make_interval(hours => LEAST(GREATEST(COALESCE(dcs.avg_change_frequency_hours, 168), 24), 720)::int)
             ORDER BY d.last_crawled_at ASC
             LIMIT 100"
        )
        .fetch_all(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

        for doc in stale_docs {
            let scope = DiscoveryScope::from_db_value(&doc.discovery_scope);
            let _ = self
                .enqueue_urls(
                    &doc.owner_developer_id,
                    vec![doc.canonical_url.clone()],
                    &doc.canonical_url,
                    &doc.budget_id,
                    0, // depth 0 for recrawl
                    10,
                    1, // max_pages 1 for individual recrawl
                    1, // same_origin_concurrency
                    Some(&doc.owner_developer_id),
                    doc.rule_id.as_deref(),
                    scope,
                    Some(&doc.host),
                    0,    // max_discovered_urls_per_page - no discovery needed
                    true, // allow_revisit
                )
                .await;
        }
        Ok(())
    }

    pub async fn run_maintenance(
        &self,
        claim_timeout: Duration,
        search_index: &SearchIndex,
    ) -> Result<(), ApiError> {
        let now = Utc::now();
        self.apply_due_rules(now).await?;
        self.recover_stale_ingests(claim_timeout).await?;
        self.process_pending_ingests(search_index, 64).await?;
        self.requeue_stale_jobs(now, claim_timeout).await?;
        self.trim_events().await?;
        self.schedule_adaptive_recrawl(now).await?;
        crate::ranking::update_site_authority(&self.pg_pool).await?;
        Ok(())
    }

    pub async fn record_admin_event(
        &self,
        developer_id: &str,
        kind: &str,
        status: &str,
        message: String,
        url: Option<String>,
        crawler_id: Option<String>,
    ) -> Result<(), ApiError> {
        self.push_event(developer_id, kind, status, message, url, crawler_id)
            .await
    }

    pub async fn list_jobs(
        &self,
        developer_id: &str,
        status_filter: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<CrawlJobListResponse, ApiError> {
        let total: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM crawl_jobs
             WHERE owner_developer_id = $1
               AND ($2::text IS NULL OR status = $2)",
        )
        .bind(developer_id)
        .bind(status_filter)
        .fetch_one(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

        let rows = sqlx::query_as::<_, JobListRow>(
            "SELECT id, url, origin_key, final_url, status, depth, max_depth, attempt_count, max_attempts,
                    source, rule_id, claimed_by, discovered_at, claimed_at, next_retry_at,
                    content_type, http_status, discovered_urls_count, accepted_document_id,
                    llm_decision, llm_reason, llm_relevance_score, canonical_hint, canonical_source,
                    failure_kind, failure_message, finished_at, render_mode
             FROM crawl_jobs
             WHERE owner_developer_id = $1
               AND ($2::text IS NULL OR status = $2)
             ORDER BY discovered_at DESC
             LIMIT $3 OFFSET $4",
        )
        .bind(developer_id)
        .bind(status_filter)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

        let jobs: Vec<CrawlJobDetail> = rows
            .into_iter()
            .map(|r| CrawlJobDetail {
                id: r.id,
                url: r.url,
                origin_key: r.origin_key,
                final_url: r.final_url,
                status: r.status,
                depth: r.depth as u32,
                max_depth: r.max_depth as u32,
                attempt_count: r.attempt_count as u32,
                max_attempts: r.max_attempts as u32,
                source: r.source,
                rule_id: r.rule_id,
                claimed_by: r.claimed_by,
                discovered_at: r.discovered_at,
                claimed_at: r.claimed_at,
                next_retry_at: r.next_retry_at,
                content_type: r.content_type,
                http_status: r.http_status.map(|value| value as u16),
                discovered_urls_count: r.discovered_urls_count.max(0) as usize,
                accepted_document_id: r.accepted_document_id,
                llm_decision: r.llm_decision,
                llm_reason: r.llm_reason,
                llm_relevance_score: r.llm_relevance_score,
                canonical_hint: r.canonical_hint,
                canonical_source: r.canonical_source,
                failure_kind: r.failure_kind,
                failure_message: r.failure_message,
                finished_at: r.finished_at,
                render_mode: r.render_mode,
            })
            .collect();

        let next_offset = if (offset as usize) + jobs.len() < total as usize {
            Some((offset as usize) + jobs.len())
        } else {
            None
        };

        Ok(CrawlJobListResponse {
            total: total as usize,
            next_offset,
            jobs,
        })
    }

    pub async fn retry_failed_jobs(&self, developer_id: &str) -> Result<usize, ApiError> {
        let result = sqlx::query(
            "UPDATE crawl_jobs
             SET status = 'queued',
                 claimed_by = NULL,
                 claimed_at = NULL,
                 lease_id = NULL,
                 next_retry_at = NULL,
                 failure_kind = NULL,
                 failure_message = NULL,
                 finished_at = NULL,
                 final_url = NULL,
                 content_type = NULL,
                 http_status = NULL,
                 discovered_urls_count = 0,
                 accepted_document_id = NULL,
                 llm_decision = NULL,
                 llm_reason = NULL,
                 llm_relevance_score = NULL,
                 render_mode = 'static'
             WHERE owner_developer_id = $1 AND status in ('failed', 'dead_letter')",
        )
        .bind(developer_id)
        .execute(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

        let count = result.rows_affected() as usize;

        if count > 0 {
            self.push_event(
                developer_id,
                "jobs-retried",
                "ok",
                format!("retried {count} failed jobs"),
                None,
                None,
            )
            .await?;
        }

        Ok(count)
    }

    pub async fn cleanup_completed_jobs(&self, developer_id: &str) -> Result<usize, ApiError> {
        let result = sqlx::query(
            "DELETE FROM crawl_jobs WHERE owner_developer_id = $1 AND status = 'succeeded'",
        )
        .bind(developer_id)
        .execute(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

        let count = result.rows_affected() as usize;

        if count > 0 {
            self.push_event(
                developer_id,
                "jobs-cleaned",
                "ok",
                format!("cleaned up {count} completed jobs"),
                None,
                None,
            )
            .await?;
        }

        Ok(count)
    }

    pub async fn cleanup_failed_jobs(&self, developer_id: &str) -> Result<usize, ApiError> {
        let result = sqlx::query(
            "DELETE FROM crawl_jobs
             WHERE owner_developer_id = $1
               AND status in ('failed', 'blocked', 'dead_letter')",
        )
        .bind(developer_id)
        .execute(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

        let count = result.rows_affected() as usize;

        if count > 0 {
            self.push_event(
                developer_id,
                "jobs-failed-cleaned",
                "ok",
                format!("cleaned up {count} failed jobs"),
                None,
                None,
            )
            .await?;
        }

        Ok(count)
    }

    pub async fn stop_all_jobs(&self, developer_id: &str) -> Result<(usize, usize), ApiError> {
        let now = Utc::now();
        let disabled_rules = sqlx::query(
            "update crawl_rules
             set enabled = false,
                 updated_at = $2
             where owner_developer_id = $1 and enabled = true",
        )
        .bind(developer_id)
        .bind(now)
        .execute(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?
        .rows_affected() as usize;

        let removed_jobs = sqlx::query(
            "delete from crawl_jobs
             where owner_developer_id = $1 and status in ('queued', 'claimed', 'ingesting')",
        )
        .bind(developer_id)
        .execute(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?
        .rows_affected() as usize;

        sqlx::query(
            "update crawl_origins
             set in_flight_count = 0,
                 next_allowed_at = $2,
                 updated_at = $2
             where owner_developer_id = $1",
        )
        .bind(developer_id)
        .bind(now)
        .execute(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

        if disabled_rules > 0 || removed_jobs > 0 {
            self.push_event(
                developer_id,
                "crawl-stopped",
                "ok",
                format!(
                    "stopped crawl activity: disabled {disabled_rules} rules and removed {removed_jobs} active jobs"
                ),
                None,
                None,
            )
            .await?;
        }

        Ok((disabled_rules, removed_jobs))
    }

    pub async fn job_stats(&self, developer_id: &str) -> Result<CrawlJobStats, ApiError> {
        let stats = sqlx::query_as::<_, JobStatsRow>(
            "SELECT
                 count(*) FILTER (WHERE status = 'queued') AS queued,
                 count(*) FILTER (WHERE status in ('claimed', 'ingesting')) AS claimed,
                 count(*) FILTER (WHERE status = 'succeeded') AS succeeded,
                 count(*) FILTER (WHERE status = 'failed') AS failed,
                 count(*) FILTER (WHERE status = 'blocked') AS blocked,
                 count(*) FILTER (WHERE status = 'dead_letter') AS dead_letter
             FROM crawl_jobs
             WHERE owner_developer_id = $1",
        )
        .bind(developer_id)
        .fetch_one(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

        Ok(CrawlJobStats {
            queued: stats.queued.unwrap_or(0) as usize,
            claimed: stats.claimed.unwrap_or(0) as usize,
            succeeded: stats.succeeded.unwrap_or(0) as usize,
            failed: stats.failed.unwrap_or(0) as usize,
            blocked: stats.blocked.unwrap_or(0) as usize,
            dead_letter: stats.dead_letter.unwrap_or(0) as usize,
        })
    }

    pub async fn list_origins(
        &self,
        developer_id: &str,
    ) -> Result<Vec<crate::models::CrawlOriginState>, ApiError> {
        let rows = sqlx::query_as::<_, CrawlOriginRow>(
            "select origin_key, robots_status, crawl_delay_secs, next_allowed_at, in_flight_count,
                    last_fetch_status, consecutive_failures, robots_sitemaps, updated_at
             from crawl_origins
             where owner_developer_id = $1
             order by next_allowed_at asc, updated_at desc
             limit 200",
        )
        .bind(developer_id)
        .fetch_all(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

        rows.into_iter()
            .map(|row| {
                let robots_sitemaps: Vec<String> = serde_json::from_value(row.robots_sitemaps)
                    .map_err(|e| ApiError::Internal(e.into()))?;
                Ok(crate::models::CrawlOriginState {
                    origin_key: row.origin_key,
                    robots_status: row.robots_status,
                    crawl_delay_secs: row.crawl_delay_secs.map(|value| value.max(0) as u32),
                    next_allowed_at: row.next_allowed_at,
                    in_flight_count: row.in_flight_count.max(0) as u32,
                    last_fetch_status: row.last_fetch_status.map(|value| value as u16),
                    consecutive_failures: row.consecutive_failures.max(0) as u32,
                    robots_sitemaps,
                    updated_at: row.updated_at,
                })
            })
            .collect()
    }

    // ---- private helpers ----

    async fn update_origin_after_report(
        &self,
        developer_id: &str,
        requested_origin_key: &str,
        finalized_url: &str,
        result: &CrawlResultInput,
        now: DateTime<Utc>,
    ) -> Result<(), ApiError> {
        let next_allowed_at = origin_ready_at(now, result);
        let robots_status = result
            .robots_status
            .as_deref()
            .unwrap_or("unknown")
            .to_string();
        let crawl_delay_secs = result
            .applied_crawl_delay_secs
            .map(|value| value.min(i32::MAX as u64) as i32);
        let robots_sitemaps = serde_json::to_value(&result.robots_sitemaps)
            .map_err(|e| ApiError::Internal(e.into()))?;
        let failure_like = matches!(result.status_code, 429 | 500..=599)
            || matches!(result.error_kind.as_deref(), Some("network_error"));

        sqlx::query(
            "insert into crawl_origins (
                owner_developer_id,
                origin_key,
                robots_status,
                crawl_delay_secs,
                next_allowed_at,
                in_flight_count,
                last_fetch_status,
                consecutive_failures,
                robots_sitemaps,
                updated_at
             )
             values ($1, $2, $3, $4, $5, 0, $6, CASE WHEN $7 THEN 1 ELSE 0 END, $8, $9)
             on conflict (owner_developer_id, origin_key) do update set
                robots_status = excluded.robots_status,
                crawl_delay_secs = coalesce(excluded.crawl_delay_secs, crawl_origins.crawl_delay_secs),
                next_allowed_at = greatest(crawl_origins.next_allowed_at, excluded.next_allowed_at),
                in_flight_count = greatest(crawl_origins.in_flight_count - 1, 0),
                last_fetch_status = excluded.last_fetch_status,
                consecutive_failures = CASE
                    WHEN $7 THEN crawl_origins.consecutive_failures + 1
                    ELSE 0
                END,
                robots_sitemaps = excluded.robots_sitemaps,
                updated_at = excluded.updated_at",
        )
        .bind(developer_id)
        .bind(requested_origin_key)
        .bind(&robots_status)
        .bind(crawl_delay_secs)
        .bind(next_allowed_at)
        .bind(result.status_code as i32)
        .bind(failure_like)
        .bind(&robots_sitemaps)
        .bind(now)
        .execute(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

        if let Some(final_origin_key) = origin_key(finalized_url)
            && final_origin_key != requested_origin_key
        {
            sqlx::query(
                "insert into crawl_origins (
                    owner_developer_id,
                    origin_key,
                    robots_status,
                    crawl_delay_secs,
                    next_allowed_at,
                    in_flight_count,
                    last_fetch_status,
                    consecutive_failures,
                    robots_sitemaps,
                    updated_at
                 )
                 values ($1, $2, $3, $4, $5, 0, $6, CASE WHEN $7 THEN 1 ELSE 0 END, $8, $9)
                 on conflict (owner_developer_id, origin_key) do update set
                    robots_status = excluded.robots_status,
                    crawl_delay_secs = coalesce(excluded.crawl_delay_secs, crawl_origins.crawl_delay_secs),
                    next_allowed_at = greatest(crawl_origins.next_allowed_at, excluded.next_allowed_at),
                    last_fetch_status = excluded.last_fetch_status,
                    consecutive_failures = CASE
                        WHEN $7 THEN crawl_origins.consecutive_failures + 1
                        ELSE 0
                    END,
                    robots_sitemaps = excluded.robots_sitemaps,
                    updated_at = excluded.updated_at",
            )
            .bind(developer_id)
            .bind(final_origin_key)
            .bind(&robots_status)
            .bind(crawl_delay_secs)
            .bind(next_allowed_at)
            .bind(result.status_code as i32)
            .bind(failure_like)
            .bind(&robots_sitemaps)
            .bind(now)
            .execute(&self.pg_pool)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;
        }

        Ok(())
    }

    async fn push_event(
        &self,
        developer_id: &str,
        kind: &str,
        status: &str,
        message: String,
        url: Option<String>,
        crawler_id: Option<String>,
    ) -> Result<(), ApiError> {
        let id = Uuid::now_v7().to_string();
        sqlx::query(
            "insert into crawl_events (id, owner_developer_id, crawler_id, event_type, status, message, url, payload, created_at)
             values ($1, $2, $3, $4, $5, $6, $7, '{}'::jsonb, $8)",
        )
        .bind(&id)
        .bind(developer_id)
        .bind(crawler_id.as_deref())
        .bind(kind)
        .bind(status)
        .bind(&message)
        .bind(url.as_deref())
        .bind(Utc::now())
        .execute(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;
        Ok(())
    }

    async fn trim_events(&self) -> Result<(), ApiError> {
        // Keep at most 400 events per owner, using window function (O(n log n))
        sqlx::query(
            "DELETE FROM crawl_events WHERE id IN (
                 SELECT id FROM (
                     SELECT id, ROW_NUMBER() OVER (
                         PARTITION BY owner_developer_id
                         ORDER BY created_at DESC
                     ) AS rn
                     FROM crawl_events
                 ) ranked
                 WHERE rn > 400
             )",
        )
        .execute(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;
        Ok(())
    }

    async fn enqueue_urls(
        &self,
        developer_id: &str,
        urls: Vec<String>,
        source: &str,
        budget_id: &str,
        depth: i32,
        max_depth: i32,
        max_pages: i32,
        same_origin_concurrency: i32,
        submitted_by: Option<&str>,
        rule_id: Option<&str>,
        discovery_scope: DiscoveryScope,
        discovery_host: Option<&str>,
        max_discovered_urls_per_page: i32,
        allow_revisit: bool,
    ) -> Result<usize, ApiError> {
        let mut accepted = 0usize;
        let mut budget_used: i64 = sqlx::query_scalar(
            "select count(*) from crawl_jobs where owner_developer_id = $1 and budget_id = $2",
        )
        .bind(developer_id)
        .bind(budget_id)
        .fetch_one(&self.pg_pool)
        .await
        .unwrap_or(0);
        let max_pages = max_pages.max(1);
        for url in urls {
            if budget_used >= i64::from(max_pages) {
                break;
            }
            let Some(normalized) = normalize_url(&url) else {
                continue;
            };
            let Some(origin_key) = origin_key(&normalized) else {
                continue;
            };
            let resolved_discovery_host = discovery_host
                .map(ToString::to_string)
                .or_else(|| extract_host(&normalized));

            if allow_revisit {
                // Delete any existing completed job so it can be re-queued
                sqlx::query(
                    "delete from crawl_jobs
                     where owner_developer_id = $1
                       and url = $2
                       and status in ('succeeded', 'failed', 'blocked', 'dead_letter')",
                )
                .bind(developer_id)
                .bind(&normalized)
                .execute(&self.pg_pool)
                .await
                .map_err(|e| ApiError::Internal(e.into()))?;
            }

            // 计算优先级：深度越浅优先级越高，种子URL最高
            let priority = if depth == 0 {
                80 // 种子URL
            } else {
                50 + (10 - depth.min(10)) * 3 // 深度越浅优先级越高
            };

            let id = Uuid::now_v7().to_string();
            let network = infer_network(&normalized);
            let result = sqlx::query(
                "insert into crawl_jobs (
                    id,
                    owner_developer_id,
                    url,
                    depth,
                    max_depth,
                    attempt_count,
                    max_attempts,
                    next_retry_at,
                    failure_kind,
                    failure_message,
                    source,
                    budget_id,
                    submitted_by,
                    rule_id,
                    discovery_scope,
                    discovery_host,
                    max_pages,
                    same_origin_concurrency,
                    max_discovered_urls_per_page,
                    origin_key,
                    network,
                    status,
                    priority,
                    discovered_at
                 )
                 values ($1, $2, $3, $4, $5, 0, 3, null, null, null, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, 'queued', $17, now())
                 on conflict (owner_developer_id, url) do nothing",
            )
            .bind(&id)
            .bind(developer_id)
            .bind(&normalized)
            .bind(depth)
            .bind(max_depth)
            .bind(source)
            .bind(budget_id)
            .bind(submitted_by)
            .bind(rule_id)
            .bind(discovery_scope.as_str())
            .bind(resolved_discovery_host.as_deref())
            .bind(max_pages)
            .bind(same_origin_concurrency.max(1))
            .bind(max_discovered_urls_per_page.max(1))
            .bind(&origin_key)
            .bind(network)
            .bind(priority)
            .execute(&self.pg_pool)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

            if result.rows_affected() > 0 {
                sqlx::query(
                    "insert into crawl_origins (
                        owner_developer_id,
                        origin_key,
                        robots_status,
                        next_allowed_at,
                        in_flight_count,
                        updated_at
                     )
                     values ($1, $2, 'unknown', now(), 0, now())
                     on conflict (owner_developer_id, origin_key) do nothing",
                )
                .bind(developer_id)
                .bind(&origin_key)
                .execute(&self.pg_pool)
                .await
                .map_err(|e| ApiError::Internal(e.into()))?;
                accepted += 1;
                budget_used += 1;
            }
        }
        Ok(accepted)
    }

    async fn validate_crawler_auth(
        &self,
        crawler_id: &str,
        crawler_name: Option<&str>,
        token_hash: &str,
        default_owner_developer_id: &str,
    ) -> Result<CrawlerAuthInfo, ApiError> {
        let configured_hash = self.resolve_crawler_auth_key_hash().await?.ok_or_else(|| {
            ApiError::Unauthorized("crawler auth key is not configured".to_string())
        })?;

        if configured_hash != token_hash {
            return Err(ApiError::Unauthorized("invalid crawler key".to_string()));
        }

        self.ensure_crawler_identity(
            crawler_id,
            crawler_name,
            default_owner_developer_id,
            token_hash,
        )
        .await
    }

    async fn apply_due_rules(&self, now: DateTime<Utc>) -> Result<(), ApiError> {
        let due_rules = sqlx::query_as::<_, DueRuleRow>(
            "select id, owner_developer_id, name, seed_url, max_depth, max_pages, same_origin_concurrency, discovery_scope, max_discovered_urls_per_page
             from crawl_rules
             where enabled = true
               and (last_enqueued_at is null
                    or extract(epoch from ($1 - last_enqueued_at)) / 60 >= interval_minutes)",
        )
        .bind(now)
        .fetch_all(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

        for rule in due_rules {
            let budget_id = Uuid::now_v7().to_string();
            sqlx::query("update crawl_rules set last_enqueued_at = $2 where id = $1")
                .bind(&rule.id)
                .bind(now)
                .execute(&self.pg_pool)
                .await
                .map_err(|e| ApiError::Internal(e.into()))?;

            let accepted = self
                .enqueue_urls(
                    &rule.owner_developer_id,
                    vec![rule.seed_url.clone()],
                    &format!("rule:{}", rule.name),
                    &budget_id,
                    0,
                    rule.max_depth,
                    rule.max_pages,
                    rule.same_origin_concurrency,
                    Some(&rule.owner_developer_id),
                    Some(&rule.id),
                    DiscoveryScope::from_db_value(&rule.discovery_scope),
                    None,
                    rule.max_discovered_urls_per_page,
                    true,
                )
                .await?;

            if accepted > 0 {
                self.push_event(
                    &rule.owner_developer_id,
                    "rule-enqueued",
                    "ok",
                    format!("rule {} queued seed {}", rule.name, rule.seed_url),
                    Some(rule.seed_url),
                    None,
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn requeue_stale_jobs(
        &self,
        now: DateTime<Utc>,
        claim_timeout: Duration,
    ) -> Result<(), ApiError> {
        let timeout_secs = claim_timeout.as_secs() as i64;
        let cutoff = now - chrono::Duration::seconds(timeout_secs);
        let next_retry_at = now + chrono::Duration::seconds(30);

        let stale = sqlx::query_as::<_, StaleJobRow>(
            "with stale as (
                 update crawl_jobs
                 set status = case
                         when greatest(max_attempts, 1) <= greatest(attempt_count, 1) then 'dead_letter'
                         else 'queued'
                     end,
                     claimed_by = null,
                     claimed_at = null,
                     lease_id = null,
                     next_retry_at = case
                         when greatest(max_attempts, 1) <= greatest(attempt_count, 1) then null
                         else $2
                     end,
                     failure_kind = case
                         when greatest(max_attempts, 1) <= greatest(attempt_count, 1) then 'claim_timeout'
                         else failure_kind
                     end,
                     failure_message = case
                         when greatest(max_attempts, 1) <= greatest(attempt_count, 1) then concat(
                             'claim timed out after ',
                             greatest(attempt_count, 1),
                             ' attempts'
                         )
                         else failure_message
                     end,
                     finished_at = case
                         when greatest(max_attempts, 1) <= greatest(attempt_count, 1) then $3
                         else null
                     end
                 where status = 'claimed' and claimed_at < $1
                 returning
                     id,
                     owner_developer_id,
                     origin_key,
                     url,
                     claimed_by,
                     status,
                     attempt_count,
                     max_attempts
             ),
             touched_origins as (
                 update crawl_origins origin
                 set in_flight_count = greatest(origin.in_flight_count - touched.stale_count, 0),
                     next_allowed_at = greatest(origin.next_allowed_at, $2),
                     updated_at = $2
                 from (
                     select owner_developer_id, origin_key, count(*)::integer as stale_count
                     from stale
                     group by owner_developer_id, origin_key
                 ) touched
                 where origin.owner_developer_id = touched.owner_developer_id
                   and origin.origin_key = touched.origin_key
             )
             select id, owner_developer_id, url, claimed_by, status, attempt_count, max_attempts from stale",
        )
        .bind(cutoff)
        .bind(next_retry_at)
        .bind(now)
        .fetch_all(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

        for job in stale {
            if job.status == "dead_letter" {
                self.push_event(
                    &job.owner_developer_id,
                    "stale-job-dead-lettered",
                    "error",
                    format!(
                        "moved stale in-flight job {} to dead letter after {}/{} claims",
                        job.id, job.attempt_count, job.max_attempts
                    ),
                    Some(job.url),
                    job.claimed_by,
                )
                .await?;
            } else {
                self.push_event(
                    &job.owner_developer_id,
                    "stale-job-requeued",
                    "ok",
                    format!(
                        "requeued stale in-flight job {} after {}/{} claims",
                        job.id, job.attempt_count, job.max_attempts
                    ),
                    Some(job.url),
                    job.claimed_by,
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn release_claimed_jobs_for_deleted_crawler(
        &self,
        owner_developer_id: &str,
        crawler_id: &str,
        now: DateTime<Utc>,
    ) -> Result<(usize, usize), ApiError> {
        let next_retry_at = now + chrono::Duration::seconds(30);
        let counts: (Option<i64>, Option<i64>) = sqlx::query_as(
            "with released as (
                 update crawl_jobs
                 set status = case
                         when greatest(max_attempts, 1) <= greatest(attempt_count, 1) then 'dead_letter'
                         else 'queued'
                     end,
                     claimed_by = null,
                     claimed_at = null,
                     lease_id = null,
                     next_retry_at = case
                         when greatest(max_attempts, 1) <= greatest(attempt_count, 1) then null
                         else $3
                     end,
                     failure_kind = case
                         when greatest(max_attempts, 1) <= greatest(attempt_count, 1) then 'crawler_deleted'
                         else failure_kind
                     end,
                     failure_message = case
                         when greatest(max_attempts, 1) <= greatest(attempt_count, 1) then 'crawler was deleted while the job was still claimed'
                         else failure_message
                     end,
                     finished_at = case
                         when greatest(max_attempts, 1) <= greatest(attempt_count, 1) then $4
                         else null
                     end
                 where owner_developer_id = $1
                   and claimed_by = $2
                   and status = 'claimed'
                 returning owner_developer_id, origin_key, status
             ),
             touched_origins as (
                 update crawl_origins origin
                 set in_flight_count = greatest(origin.in_flight_count - touched.released_count, 0),
                     next_allowed_at = greatest(origin.next_allowed_at, $3),
                     updated_at = $4
                 from (
                     select owner_developer_id, origin_key, count(*)::integer as released_count
                     from released
                     group by owner_developer_id, origin_key
                 ) touched
                 where origin.owner_developer_id = touched.owner_developer_id
                   and origin.origin_key = touched.origin_key
             )
             select
                 count(*) filter (where status = 'queued') as requeued_count,
                 count(*) filter (where status = 'dead_letter') as dead_letter_count
             from released",
        )
        .bind(owner_developer_id)
        .bind(crawler_id)
        .bind(next_retry_at)
        .bind(now)
        .fetch_one(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

        Ok((
            counts.0.unwrap_or(0).max(0) as usize,
            counts.1.unwrap_or(0).max(0) as usize,
        ))
    }

    async fn get_config(&self, key: &str) -> Result<Option<String>, ApiError> {
        sqlx::query_scalar::<_, String>("select value from system_config where key = $1")
            .bind(key)
            .fetch_optional(&self.pg_pool)
            .await
            .map_err(|e| ApiError::Internal(e.into()))
    }

    async fn resolve_crawler_auth_key_hash(&self) -> Result<Option<String>, ApiError> {
        Ok(self
            .get_config("crawler.auth_key")
            .await?
            .map(|value| hash_token(&value)))
    }

    async fn ensure_crawler_identity(
        &self,
        crawler_id: &str,
        crawler_name: Option<&str>,
        default_owner_developer_id: &str,
        token_hash: &str,
    ) -> Result<CrawlerAuthInfo, ApiError> {
        let row = sqlx::query_as::<_, CrawlerAuthRow>(
            "insert into crawlers (id, owner_developer_id, name, preview, key_hash, created_at, last_seen_at, metadata)
             values ($1, $2, $3, 'shared', $4, now(), now(), '{}'::jsonb)
             on conflict (id) do update
             set owner_developer_id = excluded.owner_developer_id,
                 preview = excluded.preview,
                 key_hash = excluded.key_hash,
                 revoked_at = null
             returning id, owner_developer_id, name",
        )
        .bind(crawler_id)
        .bind(default_owner_developer_id)
        .bind(default_crawler_name(crawler_id, crawler_name))
        .bind(token_hash)
        .fetch_one(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

        Ok(CrawlerAuthInfo {
            owner_developer_id: row.owner_developer_id,
            name: row.name,
        })
    }

    async fn set_config(&self, key: &str, value: &str) -> Result<(), ApiError> {
        sqlx::query(
            "insert into system_config (key, value, updated_at) values ($1, $2, now())
             on conflict (key) do update set value = excluded.value, updated_at = now()",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;
        Ok(())
    }

    async fn delete_config(&self, key: &str) -> Result<(), ApiError> {
        sqlx::query("delete from system_config where key = $1")
            .bind(key)
            .execute(&self.pg_pool)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;
        Ok(())
    }
}

// ---- SQL row types ----

#[derive(sqlx::FromRow)]
struct CrawlerAuthRow {
    #[allow(dead_code)]
    id: String,
    owner_developer_id: String,
    name: String,
}

struct CrawlerAuthInfo {
    owner_developer_id: String,
    name: String,
}

#[derive(sqlx::FromRow)]
struct CrawlerMetadataRow {
    id: String,
    name: String,
    preview: String,
    created_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
    last_seen_at: Option<DateTime<Utc>>,
    last_claimed_at: Option<DateTime<Utc>>,
    jobs_claimed: i64,
    jobs_reported: i64,
    in_flight_jobs: i64,
    metadata: serde_json::Value,
}

#[derive(sqlx::FromRow)]
struct CrawlerDeleteRow {
    id: String,
    name: String,
    last_seen_at: Option<DateTime<Utc>>,
}

#[derive(sqlx::FromRow)]
struct CrawlerDeleteInFlightRow {
    claimed_jobs: i64,
    ingesting_jobs: i64,
}

#[derive(sqlx::FromRow)]
struct CrawlRuleRow {
    id: String,
    owner_developer_id: String,
    name: String,
    seed_url: String,
    interval_minutes: i64,
    max_depth: i32,
    max_pages: i32,
    same_origin_concurrency: i32,
    discovery_scope: String,
    max_discovered_urls_per_page: i32,
    enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    last_enqueued_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Default, serde::Deserialize)]
struct StoredCrawlerMetadata {
    #[serde(default)]
    js_render: bool,
    #[serde(default)]
    worker_concurrency: Option<usize>,
    #[serde(default)]
    js_render_concurrency: Option<usize>,
}

#[derive(sqlx::FromRow)]
struct CrawlEventRow {
    id: String,
    event_type: String,
    status: String,
    message: String,
    url: Option<String>,
    crawler_id: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct InFlightJobRow {
    #[allow(dead_code)]
    id: String,
    url: String,
    origin_key: String,
    depth: i32,
    max_depth: i32,
    max_pages: i32,
    budget_id: String,
    rule_id: Option<String>,
    attempt_count: i32,
    max_attempts: i32,
    discovery_scope: String,
    discovery_host: Option<String>,
    same_origin_concurrency: i32,
    max_discovered_urls_per_page: i32,
}

#[derive(sqlx::FromRow)]
struct DueRuleRow {
    id: String,
    owner_developer_id: String,
    name: String,
    seed_url: String,
    max_depth: i32,
    max_pages: i32,
    same_origin_concurrency: i32,
    discovery_scope: String,
    max_discovered_urls_per_page: i32,
}

#[derive(sqlx::FromRow)]
struct StaleJobRow {
    id: String,
    owner_developer_id: String,
    url: String,
    claimed_by: Option<String>,
    status: String,
    attempt_count: i32,
    max_attempts: i32,
}

#[derive(sqlx::FromRow)]
struct ProjectionFailureRow {
    origin_key: String,
    url: String,
}

#[derive(sqlx::FromRow)]
struct StaleDocRow {
    canonical_url: String,
    host: String,
    owner_developer_id: String,
    budget_id: String,
    rule_id: Option<String>,
    discovery_scope: String,
}

#[derive(sqlx::FromRow)]
struct JobListRow {
    id: String,
    url: String,
    origin_key: String,
    final_url: Option<String>,
    status: String,
    depth: i32,
    max_depth: i32,
    attempt_count: i32,
    max_attempts: i32,
    source: String,
    rule_id: Option<String>,
    claimed_by: Option<String>,
    discovered_at: DateTime<Utc>,
    claimed_at: Option<DateTime<Utc>>,
    next_retry_at: Option<DateTime<Utc>>,
    content_type: Option<String>,
    http_status: Option<i32>,
    discovered_urls_count: i32,
    accepted_document_id: Option<String>,
    llm_decision: Option<String>,
    llm_reason: Option<String>,
    llm_relevance_score: Option<f32>,
    canonical_hint: Option<String>,
    canonical_source: Option<String>,
    failure_kind: Option<String>,
    failure_message: Option<String>,
    finished_at: Option<DateTime<Utc>>,
    render_mode: String,
}

#[derive(sqlx::FromRow)]
struct JobStatsRow {
    queued: Option<i64>,
    claimed: Option<i64>,
    succeeded: Option<i64>,
    failed: Option<i64>,
    blocked: Option<i64>,
    dead_letter: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct CrawlOriginRow {
    origin_key: String,
    robots_status: String,
    crawl_delay_secs: Option<i32>,
    next_allowed_at: DateTime<Utc>,
    in_flight_count: i32,
    last_fetch_status: Option<i32>,
    consecutive_failures: i32,
    robots_sitemaps: serde_json::Value,
    updated_at: DateTime<Utc>,
}

// ---- free functions ----

fn infer_network(url: &str) -> &'static str {
    if url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.ends_with(".onion")))
        .unwrap_or(false)
    {
        "tor"
    } else {
        "clearnet"
    }
}

fn origin_ready_at(now: DateTime<Utc>, result: &CrawlResultInput) -> DateTime<Utc> {
    let crawl_delay_secs = result.applied_crawl_delay_secs.unwrap_or(1).max(1);
    let retry_after_secs = result.retry_after_secs.unwrap_or(0);
    let extra_backoff_secs = match result.status_code {
        429 => retry_after_secs.max(30),
        500..=599 => retry_after_secs.max(60),
        _ if matches!(result.error_kind.as_deref(), Some("network_error")) => 45,
        _ if matches!(result.error_kind.as_deref(), Some("robots")) => 300,
        _ => 0,
    };
    let wait_secs = crawl_delay_secs
        .max(retry_after_secs)
        .max(extra_backoff_secs);
    now + chrono::Duration::seconds(wait_secs.min(i64::MAX as u64) as i64)
}

fn trusted_canonical_url(result: &CrawlResultInput) -> Option<String> {
    let canonical_hint = result.canonical_hint.as_deref().and_then(normalize_url)?;
    let final_url = result
        .final_url
        .as_deref()
        .or(Some(result.url.as_str()))
        .and_then(normalize_url)?;
    let canonical_host = extract_host(&canonical_hint)?;
    let final_host = extract_host(&final_url)?;

    if host_matches_scope(&canonical_host, &final_host, DiscoveryScope::SameDomain)
        || host_matches_scope(&final_host, &canonical_host, DiscoveryScope::SameDomain)
    {
        Some(canonical_hint)
    } else {
        None
    }
}

enum JobOutcome {
    Succeeded(IndexedDocument),
    Filtered {
        reason: String,
    },
    Retryable {
        failure_kind: String,
        failure_message: String,
        next_retry_at: DateTime<Utc>,
    },
    RequiresJsRender {
        failure_message: String,
    },
    Blocked {
        failure_kind: String,
        failure_message: String,
    },
    Failed {
        failure_kind: String,
        failure_message: String,
    },
    DeadLetter {
        failure_kind: String,
        failure_message: String,
    },
    Gone {
        failure_kind: String,
        failure_message: String,
    },
}

fn classify_job_outcome(result: &CrawlResultInput, job: &InFlightJobRow) -> JobOutcome {
    let kind = result
        .error_kind
        .clone()
        .unwrap_or_else(|| infer_failure_kind(result.status_code));
    let message = result
        .error_message
        .clone()
        .unwrap_or_else(|| infer_failure_message(result));
    let retryable = result
        .retryable
        .unwrap_or_else(|| is_retryable_status(result.status_code));
    let attempt_count = job.attempt_count.max(1) as u32;
    let max_attempts = job.max_attempts.max(1) as u32;

    if result.status_code == 304 {
        return JobOutcome::Filtered {
            reason: "304 Not Modified — content unchanged".to_string(),
        };
    }

    if let Some(reason) = filtered_reason(result) {
        return JobOutcome::Filtered { reason };
    }

    if (200..300).contains(&result.status_code) && result.error_kind.is_none() {
        if let Some(document) = build_document(result) {
            return JobOutcome::Succeeded(document);
        }
        return JobOutcome::Failed {
            failure_kind: "unindexable_document".to_string(),
            failure_message: "fetch succeeded but parsing produced no indexable document"
                .to_string(),
        };
    }

    if matches!(result.status_code, 404 | 410) {
        return JobOutcome::Gone {
            failure_kind: kind,
            failure_message: message,
        };
    }

    if is_blocked_result(result.status_code, &kind) {
        return JobOutcome::Blocked {
            failure_kind: kind,
            failure_message: message,
        };
    }

    if result.error_kind.as_deref() == Some("requires_js_render") {
        return JobOutcome::RequiresJsRender {
            failure_message: message,
        };
    }

    if retryable {
        if attempt_count < max_attempts {
            return JobOutcome::Retryable {
                failure_kind: kind,
                failure_message: message,
                next_retry_at: Utc::now()
                    + chrono::Duration::seconds(backoff_seconds_for_attempt(attempt_count)),
            };
        }

        return JobOutcome::DeadLetter {
            failure_kind: kind,
            failure_message: format!("{message}; retries exhausted"),
        };
    }

    JobOutcome::Failed {
        failure_kind: kind,
        failure_message: message,
    }
}

fn filtered_reason(result: &CrawlResultInput) -> Option<String> {
    if !(200..300).contains(&result.status_code) {
        return None;
    }

    match result.error_kind.as_deref() {
        Some("page_noindex") => Some(
            result
                .error_message
                .clone()
                .unwrap_or_else(|| "page filtered by robots directives".to_string()),
        ),
        Some(_) => None,
        None => match result.llm_should_index {
            Some(false) => Some(
                result
                    .llm_reason
                    .clone()
                    .unwrap_or_else(|| "page filtered by llm".to_string()),
            ),
            _ => None,
        },
    }
}

fn build_document(result: &CrawlResultInput) -> Option<IndexedDocument> {
    if !(200..300).contains(&result.status_code) {
        return None;
    }

    let title = result.title.as_ref()?.trim().to_string();
    let body = result.body.as_ref()?.trim().to_string();
    if title.is_empty() || body.is_empty() {
        return None;
    }

    let snippet_source = result.snippet.as_deref().unwrap_or(body.as_str());
    let snippet = snippet_source.trim().chars().take(220).collect::<String>();
    let suggest_terms = derive_terms(&title, &body);
    let resolved_url = result
        .final_url
        .clone()
        .unwrap_or_else(|| result.url.clone());
    let canonical_url = trusted_canonical_url(result).unwrap_or_else(|| resolved_url.clone());
    let host = extract_host(&canonical_url);
    let body_word_count = word_count(&body) as u32;

    let document = IndexedDocument {
        id: stable_document_id(&resolved_url),
        title,
        url: resolved_url.clone(),
        display_url: display_url(&canonical_url),
        snippet: snippet.chars().take(220).collect(),
        body: body.clone(),
        language: result
            .language
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        last_crawled_at: result.fetched_at,
        canonical_url: Some(canonical_url),
        host,
        content_hash: Some(content_hash(&body)),
        suggest_terms,
        site_authority: result.site_authority.unwrap_or(0.5).max(0.5),
        content_type: result
            .content_type
            .clone()
            .unwrap_or_else(|| "text/html".to_string()),
        word_count: body_word_count,
        network: result.network.clone(),
        source_job_id: Some(result.job_id.clone()),
        parser_version: CURRENT_PARSER_VERSION,
        schema_version: CURRENT_SCHEMA_VERSION,
        index_version: CURRENT_INDEX_VERSION,
        duplicate_of: None,
    };

    // 过滤垃圾内容
    if crate::quality::spam_detector::SpamDetector::is_spam(&document) {
        return None;
    }

    Some(document)
}

fn filter_discovered_urls(
    urls: Vec<String>,
    scope: DiscoveryScope,
    anchor_host: Option<&str>,
    max_urls: usize,
) -> Vec<String> {
    if urls.is_empty() {
        return Vec::new();
    }

    let mut filtered = Vec::new();
    for url in urls {
        let allowed = match (scope, anchor_host) {
            (DiscoveryScope::Any, _) => true,
            (_, Some(anchor_host)) => extract_host(&url)
                .map(|host| host_matches_scope(&host, anchor_host, scope))
                .unwrap_or(false),
            _ => false,
        };

        if allowed {
            filtered.push(url);
            if filtered.len() >= max_urls {
                break;
            }
        }
    }

    filtered
}

fn summarize_llm_decision(result: &CrawlResultInput) -> Option<String> {
    match (result.llm_should_index, result.llm_should_discover) {
        (Some(false), Some(true)) => Some("discover_only".to_string()),
        (Some(false), _) => Some("filtered".to_string()),
        (Some(true), Some(false)) => Some("index_only".to_string()),
        (Some(true), _) => Some("indexed".to_string()),
        _ => None,
    }
}

fn bearer_hash(auth_header: Option<&str>) -> Result<String, ApiError> {
    let header = auth_header
        .ok_or_else(|| ApiError::Unauthorized("missing crawler authorization".to_string()))?;
    let token = header
        .strip_prefix("Bearer ")
        .ok_or_else(|| ApiError::Unauthorized("invalid authorization scheme".to_string()))?
        .trim();

    if token.is_empty() {
        return Err(ApiError::Unauthorized("empty crawler key".to_string()));
    }

    Ok(hash_token(token))
}

fn default_crawler_name(crawler_id: &str, crawler_name: Option<&str>) -> String {
    if let Some(name) = crawler_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(120).collect::<String>())
        .filter(|value| !value.is_empty())
    {
        return name;
    }

    let suffix: String = crawler_id.chars().take(8).collect();
    format!("crawler-{suffix}")
}

fn validate_rule_name(input: &str) -> Result<String, ApiError> {
    let clean = input.trim();
    if clean.len() < 2 {
        return Err(ApiError::BadRequest(
            "rule name must contain at least 2 characters".to_string(),
        ));
    }
    Ok(clean.to_string())
}

fn normalize_domain_input(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(host) = extract_host(trimmed) {
        return Some(host.to_ascii_lowercase());
    }

    extract_host(&format!("https://{trimmed}")).map(|host| host.to_ascii_lowercase())
}

fn domain_like_pattern(domain: &str) -> String {
    format!("%.{}", domain.to_ascii_lowercase())
}

fn infer_failure_kind(status_code: u16) -> String {
    match status_code {
        599 => "network_error".to_string(),
        401 | 403 => "blocked".to_string(),
        404 => "http_404".to_string(),
        408 => "timeout".to_string(),
        429 => "http_429".to_string(),
        500..=599 => "http_5xx".to_string(),
        _ => format!("http_{status_code}"),
    }
}

fn infer_failure_message(result: &CrawlResultInput) -> String {
    if let Some(content_type) = result.content_type.as_deref() {
        format!(
            "fetch returned status {} ({content_type})",
            result.status_code
        )
    } else {
        format!("fetch returned status {}", result.status_code)
    }
}

fn is_retryable_status(status_code: u16) -> bool {
    matches!(status_code, 408 | 425 | 429 | 599) || status_code >= 500
}

fn is_blocked_result(status_code: u16, failure_kind: &str) -> bool {
    matches!(status_code, 401 | 403)
        || matches!(failure_kind, "robots" | "blocked" | "robots_blocked")
}

fn backoff_seconds_for_attempt(attempt_count: u32) -> i64 {
    match attempt_count {
        0 | 1 => 30,
        2 => 120,
        _ => 300,
    }
}

fn parse_positive_usize_config(value: Option<String>, default: usize) -> usize {
    value
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn crawler_runtime_metadata(value: &serde_json::Value) -> StoredCrawlerMetadata {
    serde_json::from_value(value.clone()).unwrap_or_default()
}

fn crawler_seen_within_timeout(
    last_seen_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    timeout: Duration,
) -> bool {
    let timeout_secs = timeout.as_secs() as i64;
    let cutoff = now - chrono::Duration::seconds(timeout_secs);
    last_seen_at.is_some_and(|last_seen| last_seen >= cutoff)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::{TimeDelta, Utc};

    use super::{
        InFlightJobRow, JobOutcome, classify_job_outcome, crawler_seen_within_timeout,
        normalize_domain_input, normalize_url, origin_ready_at, trusted_canonical_url,
    };
    use crate::models::CrawlResultInput;

    #[test]
    fn normalize_url_rejects_invalid_schemes_and_fragments() {
        assert!(normalize_url("ftp://example.com/file").is_none());
        assert_eq!(
            normalize_url("https://example.com/a#fragment"),
            Some("https://example.com/a".to_string())
        );
    }

    #[test]
    fn normalize_domain_input_accepts_hosts_and_urls() {
        assert_eq!(
            normalize_domain_input("https://Docs.Example.com/path?q=1"),
            Some("docs.example.com".to_string())
        );
        assert_eq!(
            normalize_domain_input("sub.example.com/docs"),
            Some("sub.example.com".to_string())
        );
        assert_eq!(normalize_domain_input(""), None);
    }

    #[test]
    fn classify_job_outcome_filters_page_noindex() {
        let job = InFlightJobRow {
            id: "job-1".to_string(),
            url: "https://example.com".to_string(),
            origin_key: "https://example.com".to_string(),
            depth: 0,
            max_depth: 1,
            max_pages: 50,
            budget_id: "budget-1".to_string(),
            rule_id: None,
            attempt_count: 1,
            max_attempts: 3,
            discovery_scope: "same_domain".to_string(),
            discovery_host: Some("example.com".to_string()),
            same_origin_concurrency: 1,
            max_discovered_urls_per_page: 50,
        };
        let result = CrawlResultInput {
            job_id: "job-1".to_string(),
            url: "https://example.com".to_string(),
            status_code: 200,
            fetched_at: Utc::now(),
            final_url: Some("https://example.com".to_string()),
            redirect_chain: Vec::new(),
            content_type: Some("text/html".to_string()),
            title: Some("Example".to_string()),
            snippet: Some("Example".to_string()),
            body: None,
            canonical_hint: None,
            canonical_source: None,
            language: Some("eng".to_string()),
            discovered_urls: vec!["https://example.com/docs".to_string()],
            site_authority: Some(0.5),
            llm_should_index: None,
            llm_should_discover: None,
            llm_relevance_score: None,
            llm_reason: None,
            retryable: Some(false),
            error_kind: Some("page_noindex".to_string()),
            error_message: Some("page requested noindex via robots directives".to_string()),
            network: "clearnet".to_string(),
            http_etag: None,
            http_last_modified: None,
            applied_crawl_delay_secs: None,
            retry_after_secs: None,
            robots_status: None,
            robots_sitemaps: Vec::new(),
            render_mode: "static".to_string(),
        };

        match classify_job_outcome(&result, &job) {
            JobOutcome::Filtered { reason } => {
                assert_eq!(reason, "page requested noindex via robots directives");
            }
            _ => panic!("expected filtered outcome"),
        }
    }

    #[test]
    fn crawler_seen_within_timeout_requires_recent_heartbeat() {
        let now = Utc::now();
        let timeout = Duration::from_secs(300);

        assert!(crawler_seen_within_timeout(
            Some(now - TimeDelta::seconds(299)),
            now,
            timeout
        ));
        assert!(!crawler_seen_within_timeout(
            Some(now - TimeDelta::seconds(301)),
            now,
            timeout
        ));
        assert!(!crawler_seen_within_timeout(None, now, timeout));
    }

    #[test]
    fn trusted_canonical_url_rejects_cross_domain_hints() {
        let result = CrawlResultInput {
            job_id: "job-1".to_string(),
            url: "https://example.com/docs".to_string(),
            status_code: 200,
            fetched_at: Utc::now(),
            final_url: Some("https://example.com/docs".to_string()),
            redirect_chain: Vec::new(),
            content_type: Some("text/html".to_string()),
            title: Some("Docs".to_string()),
            snippet: Some("Docs".to_string()),
            body: Some("Hello".to_string()),
            canonical_hint: Some("https://other.example.net/docs".to_string()),
            canonical_source: Some("rel_canonical".to_string()),
            language: Some("eng".to_string()),
            discovered_urls: Vec::new(),
            site_authority: Some(0.5),
            llm_should_index: None,
            llm_should_discover: None,
            llm_relevance_score: None,
            llm_reason: None,
            retryable: Some(false),
            error_kind: None,
            error_message: None,
            network: "clearnet".to_string(),
            http_etag: None,
            http_last_modified: None,
            applied_crawl_delay_secs: Some(5),
            retry_after_secs: None,
            robots_status: Some("fetched".to_string()),
            robots_sitemaps: Vec::new(),
            render_mode: "static".to_string(),
        };

        assert_eq!(trusted_canonical_url(&result), None);
    }

    #[test]
    fn origin_ready_at_prefers_retry_after_for_throttled_hosts() {
        let now = Utc::now();
        let result = CrawlResultInput {
            job_id: "job-1".to_string(),
            url: "https://example.com/docs".to_string(),
            status_code: 429,
            fetched_at: now,
            final_url: Some("https://example.com/docs".to_string()),
            redirect_chain: Vec::new(),
            content_type: Some("text/html".to_string()),
            title: None,
            snippet: None,
            body: None,
            canonical_hint: None,
            canonical_source: None,
            language: None,
            discovered_urls: Vec::new(),
            site_authority: None,
            llm_should_index: None,
            llm_should_discover: None,
            llm_relevance_score: None,
            llm_reason: None,
            retryable: Some(true),
            error_kind: Some("http_429".to_string()),
            error_message: Some("throttled".to_string()),
            network: "clearnet".to_string(),
            http_etag: None,
            http_last_modified: None,
            applied_crawl_delay_secs: Some(3),
            retry_after_secs: Some(120),
            robots_status: Some("fetched".to_string()),
            robots_sitemaps: Vec::new(),
            render_mode: "static".to_string(),
        };

        assert!(origin_ready_at(now, &result) >= now + TimeDelta::seconds(120));
    }
}

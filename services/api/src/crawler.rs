use std::time::Duration;

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::ApiError,
    models::{
        ClaimJobsRequest, ClaimJobsResponse, CrawlEvent, CrawlJob, CrawlJobDetail,
        CrawlJobListResponse, CrawlJobStats, CrawlOverviewResponse, CrawlResultInput, CrawlRule,
        CrawlerMetadata, CreateCrawlRuleRequest, IndexedDocument, JoinCrawlerRequest,
        JoinCrawlerResponse, SeedFrontierRequest, SeedFrontierResponse, SubmitCrawlReportRequest,
        SubmitCrawlReportResponse, UpdateCrawlRuleRequest,
    },
    store::{
        CURRENT_INDEX_VERSION, CURRENT_PARSER_VERSION, CURRENT_SCHEMA_VERSION, SearchIndex,
        content_hash, derive_terms, display_url, extract_host, normalize_url, stable_document_id,
        word_count,
    },
};

#[derive(Debug, Clone)]
pub struct CrawlerStore {
    pg_pool: PgPool,
}

impl CrawlerStore {
    pub fn new(pg_pool: PgPool) -> Self {
        Self { pg_pool }
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

    pub async fn join(
        &self,
        owner_developer_id: &str,
        config_join_key: Option<&str>,
        request: JoinCrawlerRequest,
    ) -> Result<JoinCrawlerResponse, ApiError> {
        let expected_key = self
            .resolve_join_key(config_join_key)
            .await?
            .ok_or_else(|| ApiError::BadRequest("crawler join key is not configured".to_string()))?;

        if request.join_key != expected_key {
            return Err(ApiError::Unauthorized("invalid join key".to_string()));
        }

        let clean_name = request
            .name
            .as_deref()
            .map(str::trim)
            .filter(|n| n.len() >= 2)
            .unwrap_or("join-crawler");

        let key = generate_token("fvc");
        let preview = format!("{}...{}", &key[..8], &key[key.len() - 4..]);
        let id = Uuid::now_v7().to_string();
        let now = Utc::now();

        sqlx::query(
            "insert into crawlers (id, owner_developer_id, name, preview, key_hash, created_at, last_seen_at, metadata)
             values ($1, $2, $3, $4, $5, $6, $6, '{}'::jsonb)",
        )
        .bind(&id)
        .bind(owner_developer_id)
        .bind(clean_name)
        .bind(&preview)
        .bind(hash_token(&key))
        .bind(now)
        .execute(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

        self.push_event(
            owner_developer_id,
            "crawler-joined",
            "ok",
            format!("crawler {clean_name} joined via join key"),
            None,
            Some(id.clone()),
        )
        .await?;

        Ok(JoinCrawlerResponse {
            crawler_id: id,
            crawler_key: key,
            name: clean_name.to_string(),
        })
    }

    pub async fn get_join_key(&self, config_join_key: Option<&str>) -> Option<String> {
        self.resolve_join_key(config_join_key).await.ok().flatten()
    }

    pub async fn set_join_key(&self, new_key: Option<String>) -> Result<(), ApiError> {
        match new_key {
            Some(k) => self.set_config("join_key", &k).await,
            None => self.delete_config("join_key").await,
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

        sqlx::query(
            "insert into crawl_rules (id, owner_developer_id, owner_user_id, name, seed_url, pattern, status, interval_minutes, max_depth, enabled, created_at, updated_at)
             values ($1, $2, null, $3, $4, $4, 'active', $5, $6, $7, $8, $8)",
        )
        .bind(&id)
        .bind(developer_id)
        .bind(&name)
        .bind(&seed_url)
        .bind(interval_minutes)
        .bind(max_depth)
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
            "select id, owner_developer_id, name, seed_url, interval_minutes, max_depth, enabled, created_at, updated_at, last_enqueued_at
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
        let new_enabled = request.enabled.unwrap_or(row.enabled);
        let now = Utc::now();

        sqlx::query(
            "update crawl_rules set name = $2, seed_url = $3, pattern = $3, interval_minutes = $4, max_depth = $5, enabled = $6, updated_at = $7
             where id = $1",
        )
        .bind(rule_id)
        .bind(&new_name)
        .bind(&new_seed_url)
        .bind(new_interval)
        .bind(new_max_depth)
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
            enabled: new_enabled,
            created_at: row.created_at,
            updated_at: now,
            last_enqueued_at: row.last_enqueued_at,
        })
    }

    pub async fn delete_rule(&self, developer_id: &str, rule_id: &str) -> Result<(), ApiError> {
        let row = sqlx::query_as::<_, CrawlRuleRow>(
            "select id, owner_developer_id, name, seed_url, interval_minutes, max_depth, enabled, created_at, updated_at, last_enqueued_at
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
        let crawlers: Vec<CrawlerMetadata> = sqlx::query_as::<_, CrawlerMetadataRow>(
            "select id, name, preview, created_at, revoked_at, last_seen_at, last_claimed_at, jobs_claimed, jobs_reported
             from crawlers where owner_developer_id = $1
             order by created_at desc",
        )
        .bind(developer_id)
        .fetch_all(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?
        .into_iter()
        .map(|r| CrawlerMetadata {
            id: r.id,
            name: r.name,
            preview: r.preview,
            created_at: r.created_at,
            revoked_at: r.revoked_at,
            last_seen_at: r.last_seen_at,
            last_claimed_at: r.last_claimed_at,
            jobs_claimed: r.jobs_claimed as u64,
            jobs_reported: r.jobs_reported as u64,
        })
        .collect();

        let rules: Vec<CrawlRule> = sqlx::query_as::<_, CrawlRuleRow>(
            "select id, owner_developer_id, name, seed_url, interval_minutes, max_depth, enabled, created_at, updated_at, last_enqueued_at
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
            "select count(*) from crawl_jobs where owner_developer_id = $1 and status = 'claimed'",
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

        let accepted_urls = self
            .enqueue_urls(
                developer_id,
                request.urls,
                &source,
                0,
                request.max_depth.min(10) as i32,
                Some(developer_id),
                None,
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
        auth_header: Option<&str>,
        request: ClaimJobsRequest,
    ) -> Result<ClaimJobsResponse, ApiError> {
        let token_hash = bearer_hash(auth_header)?;
        let max_jobs = request.max_jobs.clamp(1, 100) as i64;
        let now = Utc::now();

        let crawler = self.validate_crawler_auth(crawler_id, &token_hash).await?;

        // Update last_seen_at and last_claimed_at
        sqlx::query("update crawlers set last_seen_at = $2, last_claimed_at = $2 where id = $1")
            .bind(crawler_id)
            .bind(now)
            .execute(&self.pg_pool)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        // Claim jobs using FOR UPDATE SKIP LOCKED with priority
        let lease_expires = now + chrono::Duration::minutes(30);
        let claimed_rows = sqlx::query_as::<_, ClaimedJobRow>(
            "update crawl_jobs
             set status = 'claimed',
                 claimed_by = $1,
                 claimed_at = $2,
                 lease_expires_at = $3,
                 attempt_count = attempt_count + 1
             where id in (
                 select id from crawl_jobs
                 where owner_developer_id = $4
                   and status = 'queued'
                   and (next_retry_at is null or next_retry_at <= $2)
                 order by priority desc, discovered_at asc
                 limit $5
                 for update skip locked
             )
             returning id, url, source, depth, max_depth, attempt_count, discovered_at",
        )
        .bind(crawler_id)
        .bind(now)
        .bind(lease_expires)
        .bind(&crawler.owner_developer_id)
        .bind(max_jobs)
        .fetch_all(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

        let jobs: Vec<CrawlJob> = claimed_rows
            .iter()
            .map(|r| CrawlJob {
                job_id: r.id.clone(),
                url: r.url.clone(),
                source: r.source.clone(),
                depth: r.depth as u32,
                max_depth: r.max_depth as u32,
                attempt_count: r.attempt_count as u32,
                discovered_at: r.discovered_at,
            })
            .collect();

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
            frontier_depth: frontier_depth as usize,
            jobs,
        })
    }

    pub async fn submit_report(
        &self,
        crawler_id: &str,
        auth_header: Option<&str>,
        request: SubmitCrawlReportRequest,
        search_index: &SearchIndex,
    ) -> Result<SubmitCrawlReportResponse, ApiError> {
        let token_hash = bearer_hash(auth_header)?;
        let now = Utc::now();

        let crawler = self.validate_crawler_auth(crawler_id, &token_hash).await?;

        sqlx::query("update crawlers set last_seen_at = $2 where id = $1")
            .bind(crawler_id)
            .bind(now)
            .execute(&self.pg_pool)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

        let mut documents = Vec::new();
        let mut discovered_urls = 0usize;
        let mut reported = 0i64;

        for result in request.results {
            let in_flight = sqlx::query_as::<_, InFlightJobRow>(
                "select id, url, depth, max_depth, rule_id, attempt_count, max_attempts from crawl_jobs
                 where id = $1 and claimed_by = $2 and status = 'claimed'",
            )
            .bind(&result.job_id)
            .bind(crawler_id)
            .fetch_optional(&self.pg_pool)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

            let Some(in_flight) = in_flight else {
                continue;
            };

            if in_flight.url != result.url {
                return Err(ApiError::BadRequest(
                    "crawl report contained a job not assigned to this crawler".to_string(),
                ));
            }

            reported += 1;
            let finalized_url = result
                .final_url
                .clone()
                .unwrap_or_else(|| result.url.clone());
            let discovered_count = result.discovered_urls.len() as i32;
            let http_status = result.status_code as i32;
            let content_type = result.content_type.clone();

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
                             accepted_document_id = $7
                         where id = $1",
                    )
                    .bind(&result.job_id)
                    .bind(now)
                    .bind(&finalized_url)
                    .bind(content_type.as_deref())
                    .bind(http_status)
                    .bind(discovered_count)
                    .bind(&document.id)
                    .execute(&self.pg_pool)
                    .await
                    .map_err(|e| ApiError::Internal(e.into()))?;

                    let allowed_discovery = in_flight.depth < in_flight.max_depth;
                    if allowed_discovery {
                        for discovered_url in &result.discovered_urls {
                            let _ = sqlx::query(
                                "update documents set inlink_count = inlink_count + 1
                                 where canonical_url = $1",
                            )
                            .bind(discovered_url)
                            .execute(&self.pg_pool)
                            .await;
                        }

                        discovered_urls += self
                            .enqueue_urls(
                                &crawler.owner_developer_id,
                                result.discovered_urls.clone(),
                                &finalized_url,
                                in_flight.depth + 1,
                                in_flight.max_depth,
                                Some(&crawler.owner_developer_id),
                                in_flight.rule_id.as_deref(),
                                false,
                            )
                            .await?;
                    }

                    self.push_event(
                        &crawler.owner_developer_id,
                        "job-succeeded",
                        "ok",
                        format!(
                            "indexed {finalized_url} on attempt {}",
                            in_flight.attempt_count
                        ),
                        Some(finalized_url.clone()),
                        Some(crawler_id.to_string()),
                    )
                    .await?;

                    documents.push(document);
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
                             lease_expires_at = null,
                             next_retry_at = $2,
                             failure_kind = $3,
                             failure_message = $4,
                             finished_at = null,
                             final_url = $5,
                             content_type = $6,
                             http_status = $7,
                             discovered_urls_count = $8,
                             accepted_document_id = null
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
                    .execute(&self.pg_pool)
                    .await
                    .map_err(|e| ApiError::Internal(e.into()))?;

                    self.push_event(
                        &crawler.owner_developer_id,
                        "job-requeued",
                        "error",
                        format!(
                            "{}; retrying at {}",
                            failure_message,
                            next_retry_at.to_rfc3339()
                        ),
                        Some(finalized_url.clone()),
                        Some(crawler_id.to_string()),
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
                             accepted_document_id = null
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
                    .execute(&self.pg_pool)
                    .await
                    .map_err(|e| ApiError::Internal(e.into()))?;

                    self.push_event(
                        &crawler.owner_developer_id,
                        "job-blocked",
                        "error",
                        failure_message,
                        Some(finalized_url.clone()),
                        Some(crawler_id.to_string()),
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
                             accepted_document_id = null
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
                    .execute(&self.pg_pool)
                    .await
                    .map_err(|e| ApiError::Internal(e.into()))?;

                    self.push_event(
                        &crawler.owner_developer_id,
                        "job-failed",
                        "error",
                        failure_message,
                        Some(finalized_url.clone()),
                        Some(crawler_id.to_string()),
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
                             accepted_document_id = null
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
                    .execute(&self.pg_pool)
                    .await
                    .map_err(|e| ApiError::Internal(e.into()))?;

                    self.push_event(
                        &crawler.owner_developer_id,
                        "job-dead-lettered",
                        "error",
                        failure_message,
                        Some(finalized_url.clone()),
                        Some(crawler_id.to_string()),
                    )
                    .await?;
                }
            }
        }

        if reported > 0 {
            sqlx::query("update crawlers set jobs_reported = jobs_reported + $2 where id = $1")
                .bind(crawler_id)
                .bind(reported)
                .execute(&self.pg_pool)
                .await
                .map_err(|e| ApiError::Internal(e.into()))?;
        }

        let frontier_depth: i64 = sqlx::query_scalar(
            "select count(*) from crawl_jobs where owner_developer_id = $1 and status = 'queued'",
        )
        .bind(&crawler.owner_developer_id)
        .fetch_one(&self.pg_pool)
        .await
        .unwrap_or(0);

        let ingest = search_index.upsert_documents(documents).await?;
        Ok(SubmitCrawlReportResponse {
            accepted_documents: ingest.accepted_documents,
            duplicate_documents: ingest.duplicate_documents,
            skipped_documents: ingest.skipped_documents,
            discovered_urls,
            frontier_depth: frontier_depth as usize,
            indexed_documents: search_index.total_documents().await,
        })
    }

    pub async fn run_maintenance(&self, claim_timeout: Duration) -> Result<(), ApiError> {
        let now = Utc::now();
        self.apply_due_rules(now).await?;
        self.requeue_stale_jobs(now, claim_timeout).await?;
        self.trim_events().await?;
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
            "SELECT id, url, final_url, status, depth, max_depth, attempt_count, max_attempts,
                    source, rule_id, claimed_by, discovered_at, claimed_at, next_retry_at,
                    content_type, http_status, discovered_urls_count, accepted_document_id,
                    failure_kind, failure_message, finished_at
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
                failure_kind: r.failure_kind,
                failure_message: r.failure_message,
                finished_at: r.finished_at,
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
                 lease_expires_at = NULL,
                 next_retry_at = NULL,
                 failure_kind = NULL,
                 failure_message = NULL,
                 finished_at = NULL,
                 final_url = NULL,
                 content_type = NULL,
                 http_status = NULL,
                 discovered_urls_count = 0,
                 accepted_document_id = NULL
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

    pub async fn job_stats(&self, developer_id: &str) -> Result<CrawlJobStats, ApiError> {
        let stats = sqlx::query_as::<_, JobStatsRow>(
            "SELECT
                 count(*) FILTER (WHERE status = 'queued') AS queued,
                 count(*) FILTER (WHERE status = 'claimed') AS claimed,
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

    // ---- private helpers ----

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
        depth: i32,
        max_depth: i32,
        submitted_by: Option<&str>,
        rule_id: Option<&str>,
        allow_revisit: bool,
    ) -> Result<usize, ApiError> {
        let mut accepted = 0usize;
        for url in urls {
            let Some(normalized) = normalize_url(&url) else {
                continue;
            };

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
                    submitted_by,
                    rule_id,
                    status,
                    priority,
                    discovered_at
                 )
                 values ($1, $2, $3, $4, $5, 0, 3, null, null, null, $6, $7, $8, 'queued', $9, now())
                 on conflict (owner_developer_id, url) do nothing",
            )
            .bind(&id)
            .bind(developer_id)
            .bind(&normalized)
            .bind(depth)
            .bind(max_depth)
            .bind(source)
            .bind(submitted_by)
            .bind(rule_id)
            .bind(priority)
            .execute(&self.pg_pool)
            .await
            .map_err(|e| ApiError::Internal(e.into()))?;

            if result.rows_affected() > 0 {
                accepted += 1;
            }
        }
        Ok(accepted)
    }

    async fn validate_crawler_auth(
        &self,
        crawler_id: &str,
        token_hash: &str,
    ) -> Result<CrawlerAuthInfo, ApiError> {
        let row = sqlx::query_as::<_, CrawlerAuthRow>(
            "select id, owner_developer_id, name, key_hash, revoked_at
             from crawlers where id = $1",
        )
        .bind(crawler_id)
        .fetch_optional(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?
        .ok_or_else(|| ApiError::Unauthorized("unknown crawler id".to_string()))?;

        if row.revoked_at.is_some() {
            return Err(ApiError::Unauthorized("crawler key is revoked".to_string()));
        }

        let matches_key = !row.key_hash.is_empty() && row.key_hash == token_hash;

        if !matches_key {
            return Err(ApiError::Unauthorized("invalid crawler key".to_string()));
        }

        Ok(CrawlerAuthInfo {
            owner_developer_id: row.owner_developer_id,
            name: row.name,
        })
    }

    async fn apply_due_rules(&self, now: DateTime<Utc>) -> Result<(), ApiError> {
        let due_rules = sqlx::query_as::<_, DueRuleRow>(
            "select id, owner_developer_id, name, seed_url, max_depth
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
                    0,
                    rule.max_depth,
                    Some(&rule.owner_developer_id),
                    Some(&rule.id),
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

        let stale = sqlx::query_as::<_, StaleJobRow>(
            "update crawl_jobs
             set status = 'queued',
                 claimed_by = null,
                 claimed_at = null,
                 lease_expires_at = null,
                 next_retry_at = $2
             where status = 'claimed' and claimed_at < $1
             returning id, owner_developer_id, url, claimed_by",
        )
        .bind(cutoff)
        .bind(now + chrono::Duration::seconds(30))
        .fetch_all(&self.pg_pool)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

        for job in stale {
            self.push_event(
                &job.owner_developer_id,
                "stale-job-requeued",
                "ok",
                format!("requeued stale in-flight job {}", job.id),
                Some(job.url),
                job.claimed_by,
            )
            .await?;
        }

        Ok(())
    }

    async fn get_config(&self, key: &str) -> Result<Option<String>, ApiError> {
        sqlx::query_scalar::<_, String>("select value from crawler_config where key = $1")
            .bind(key)
            .fetch_optional(&self.pg_pool)
            .await
            .map_err(|e| ApiError::Internal(e.into()))
    }

    async fn resolve_join_key(
        &self,
        config_join_key: Option<&str>,
    ) -> Result<Option<String>, ApiError> {
        if let Some(stored_key) = self.get_config("join_key").await? {
            return Ok(Some(stored_key));
        }

        Ok(config_join_key.map(ToString::to_string))
    }

    async fn set_config(&self, key: &str, value: &str) -> Result<(), ApiError> {
        sqlx::query(
            "insert into crawler_config (key, value, updated_at) values ($1, $2, now())
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
        sqlx::query("delete from crawler_config where key = $1")
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
    key_hash: String,
    revoked_at: Option<DateTime<Utc>>,
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
}

#[derive(sqlx::FromRow)]
struct CrawlRuleRow {
    id: String,
    owner_developer_id: String,
    name: String,
    seed_url: String,
    interval_minutes: i64,
    max_depth: i32,
    enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    last_enqueued_at: Option<DateTime<Utc>>,
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
struct ClaimedJobRow {
    id: String,
    url: String,
    source: String,
    depth: i32,
    max_depth: i32,
    attempt_count: i32,
    discovered_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct InFlightJobRow {
    #[allow(dead_code)]
    id: String,
    url: String,
    depth: i32,
    max_depth: i32,
    rule_id: Option<String>,
    attempt_count: i32,
    max_attempts: i32,
}

#[derive(sqlx::FromRow)]
struct DueRuleRow {
    id: String,
    owner_developer_id: String,
    name: String,
    seed_url: String,
    max_depth: i32,
}

#[derive(sqlx::FromRow)]
struct StaleJobRow {
    id: String,
    owner_developer_id: String,
    url: String,
    claimed_by: Option<String>,
}

#[derive(sqlx::FromRow)]
struct JobListRow {
    id: String,
    url: String,
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
    failure_kind: Option<String>,
    failure_message: Option<String>,
    finished_at: Option<DateTime<Utc>>,
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

// ---- free functions ----

enum JobOutcome {
    Succeeded(IndexedDocument),
    Retryable {
        failure_kind: String,
        failure_message: String,
        next_retry_at: DateTime<Utc>,
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

    if is_blocked_result(result.status_code, &kind) {
        return JobOutcome::Blocked {
            failure_kind: kind,
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
    let host = extract_host(&resolved_url);
    let body_word_count = word_count(&body) as u32;

    let document = IndexedDocument {
        id: stable_document_id(&resolved_url),
        title,
        url: resolved_url.clone(),
        display_url: display_url(&resolved_url),
        snippet: snippet.chars().take(220).collect(),
        body: body.clone(),
        language: result
            .language
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        last_crawled_at: result.fetched_at,
        canonical_url: Some(resolved_url),
        host,
        content_hash: Some(content_hash(&body)),
        suggest_terms,
        site_authority: result.site_authority.unwrap_or(0.5).max(0.5),
        content_type: result
            .content_type
            .clone()
            .unwrap_or_else(|| "text/html".to_string()),
        word_count: body_word_count,
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

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn generate_token(prefix: &str) -> String {
    use rand::{Rng, distr::Alphanumeric};

    let secret = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(40)
        .map(char::from)
        .collect::<String>();
    format!("{prefix}_{secret}")
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

#[cfg(test)]
mod tests {
    use super::normalize_url;

    #[test]
    fn normalize_url_rejects_invalid_schemes_and_fragments() {
        assert!(normalize_url("ftp://example.com/file").is_none());
        assert_eq!(
            normalize_url("https://example.com/a#fragment"),
            Some("https://example.com/a".to_string())
        );
    }
}

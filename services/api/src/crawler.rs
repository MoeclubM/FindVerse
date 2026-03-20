use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::PathBuf,
    sync::Arc,
};

use anyhow::Context;
use chrono::{DateTime, Utc};
use rand::{Rng, distr::Alphanumeric};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::{fs, sync::RwLock};
use url::Url;
use uuid::Uuid;

use crate::{
    error::ApiError,
    models::{
        ClaimJobsRequest, ClaimJobsResponse, CrawlEvent, CrawlJob, CrawlOverviewResponse,
        CrawlResultInput, CrawlRule, CrawlerMetadata, CreateCrawlRuleRequest,
        CreateCrawlerRequest, CreatedCrawlerResponse, IndexedDocument, SeedFrontierRequest,
        SeedFrontierResponse, SubmitCrawlReportRequest, SubmitCrawlReportResponse,
        UpdateCrawlRuleRequest,
    },
    store::{SearchIndex, derive_terms, display_url, stable_document_id},
};

#[derive(Debug, Clone)]
pub struct CrawlerStore {
    path: PathBuf,
    inner: Arc<RwLock<CrawlerStoreState>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CrawlerStoreState {
    #[serde(default)]
    crawlers: HashMap<String, StoredCrawler>,
    #[serde(default)]
    rules: HashMap<String, StoredCrawlRule>,
    #[serde(default)]
    frontier: VecDeque<FrontierRecord>,
    #[serde(default)]
    known_urls: HashSet<String>,
    #[serde(default)]
    in_flight: HashMap<String, InFlightRecord>,
    #[serde(default)]
    events: VecDeque<StoredCrawlEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredCrawler {
    id: String,
    #[serde(default = "default_owner_developer_id")]
    owner_developer_id: String,
    name: String,
    preview: String,
    key_hash: String,
    created_at: DateTime<Utc>,
    #[serde(default)]
    revoked_at: Option<DateTime<Utc>>,
    #[serde(default)]
    last_seen_at: Option<DateTime<Utc>>,
    #[serde(default)]
    last_claimed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    jobs_claimed: u64,
    #[serde(default)]
    jobs_reported: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredCrawlRule {
    id: String,
    #[serde(default = "default_owner_developer_id")]
    owner_developer_id: String,
    name: String,
    seed_url: String,
    interval_minutes: u64,
    #[serde(default = "default_rule_depth")]
    max_depth: u32,
    #[serde(default = "default_true")]
    enabled: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    #[serde(default)]
    last_enqueued_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FrontierRecord {
    job_id: String,
    #[serde(default = "default_owner_developer_id")]
    owner_developer_id: String,
    url: String,
    source: String,
    depth: u32,
    #[serde(default = "default_rule_depth")]
    max_depth: u32,
    discovered_at: DateTime<Utc>,
    #[serde(default)]
    submitted_by: Option<String>,
    #[serde(default)]
    rule_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InFlightRecord {
    crawler_id: String,
    job: FrontierRecord,
    claimed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredCrawlEvent {
    id: String,
    #[serde(default = "default_owner_developer_id")]
    owner_developer_id: String,
    kind: String,
    status: String,
    message: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    crawler_id: Option<String>,
    created_at: DateTime<Utc>,
}

impl CrawlerStore {
    pub async fn load(path: PathBuf) -> anyhow::Result<Self> {
        let empty = serde_json::to_string_pretty(&CrawlerStoreState::default())?;
        ensure_file_with_fallbacks(
            &path,
            &empty,
            &[
                PathBuf::from("/opt/findverse/crawler_store.json"),
                PathBuf::from("services/api/fixtures/crawler_store.json"),
            ],
        )
        .await?;

        let raw = fs::read_to_string(&path)
            .await
            .context("failed to read crawler store file")?;
        let state: CrawlerStoreState =
            serde_json::from_str(&raw).context("failed to parse crawler store file")?;

        Ok(Self {
            path,
            inner: Arc::new(RwLock::new(state)),
        })
    }

    pub async fn create_crawler(
        &self,
        developer_id: &str,
        request: CreateCrawlerRequest,
    ) -> Result<CreatedCrawlerResponse, ApiError> {
        let clean_name = request.name.trim();
        if clean_name.len() < 2 {
            return Err(ApiError::BadRequest(
                "crawler name must contain at least 2 characters".to_string(),
            ));
        }

        let key = generate_token("fvc");
        let preview = format!("{}...{}", &key[..8], &key[key.len() - 4..]);
        let crawler = StoredCrawler {
            id: Uuid::now_v7().to_string(),
            owner_developer_id: developer_id.to_string(),
            name: clean_name.to_string(),
            preview: preview.clone(),
            key_hash: hash_token(&key),
            created_at: Utc::now(),
            revoked_at: None,
            last_seen_at: None,
            last_claimed_at: None,
            jobs_claimed: 0,
            jobs_reported: 0,
        };

        {
            let mut state = self.inner.write().await;
            state.crawlers.insert(crawler.id.clone(), crawler.clone());
            push_event(
                &mut state,
                developer_id,
                "crawler-created",
                "ok",
                format!("created crawler {}", crawler.name),
                None,
                Some(crawler.id.clone()),
            );
            self.persist_locked(&state).await?;
        }

        Ok(CreatedCrawlerResponse {
            id: crawler.id,
            name: crawler.name,
            preview,
            key,
            created_at: crawler.created_at,
        })
    }

    pub async fn create_rule(
        &self,
        developer_id: &str,
        request: CreateCrawlRuleRequest,
    ) -> Result<CrawlRule, ApiError> {
        let name = validate_rule_name(&request.name)?;
        let seed_url = normalize_url(&request.seed_url)
            .ok_or_else(|| ApiError::BadRequest("seed_url must be a valid http(s) url".to_string()))?;

        let rule = StoredCrawlRule {
            id: Uuid::now_v7().to_string(),
            owner_developer_id: developer_id.to_string(),
            name,
            seed_url,
            interval_minutes: request.interval_minutes.clamp(1, 10_080),
            max_depth: request.max_depth.min(10),
            enabled: request.enabled,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_enqueued_at: None,
        };

        let response = to_crawl_rule(&rule);
        {
            let mut state = self.inner.write().await;
            state.rules.insert(rule.id.clone(), rule.clone());
            push_event(
                &mut state,
                developer_id,
                "rule-created",
                "ok",
                format!("created rule {}", rule.name),
                Some(rule.seed_url.clone()),
                None,
            );
            self.persist_locked(&state).await?;
        }

        Ok(response)
    }

    pub async fn update_rule(
        &self,
        developer_id: &str,
        rule_id: &str,
        request: UpdateCrawlRuleRequest,
    ) -> Result<CrawlRule, ApiError> {
        let response = {
            let mut state = self.inner.write().await;
            let rule = state
                .rules
                .get_mut(rule_id)
                .ok_or_else(|| ApiError::NotFound("crawl rule not found".to_string()))?;

            if rule.owner_developer_id != developer_id {
                return Err(ApiError::NotFound("crawl rule not found".to_string()));
            }

            if let Some(name) = request.name.as_deref() {
                rule.name = validate_rule_name(name)?;
            }
            if let Some(seed_url) = request.seed_url.as_deref() {
                rule.seed_url = normalize_url(seed_url).ok_or_else(|| {
                    ApiError::BadRequest("seed_url must be a valid http(s) url".to_string())
                })?;
            }
            if let Some(interval_minutes) = request.interval_minutes {
                rule.interval_minutes = interval_minutes.clamp(1, 10_080);
            }
            if let Some(max_depth) = request.max_depth {
                rule.max_depth = max_depth.min(10);
            }
            if let Some(enabled) = request.enabled {
                rule.enabled = enabled;
            }
            rule.updated_at = Utc::now();

            let response = to_crawl_rule(rule);
            let rule_name = rule.name.clone();
            let rule_seed_url = rule.seed_url.clone();
            push_event(
                &mut state,
                developer_id,
                "rule-updated",
                "ok",
                format!("updated rule {rule_name}"),
                Some(rule_seed_url),
                None,
            );
            self.persist_locked(&state).await?;
            response
        };

        Ok(response)
    }

    pub async fn delete_rule(&self, developer_id: &str, rule_id: &str) -> Result<(), ApiError> {
        let mut state = self.inner.write().await;
        let rule = state
            .rules
            .get(rule_id)
            .cloned()
            .ok_or_else(|| ApiError::NotFound("crawl rule not found".to_string()))?;

        if rule.owner_developer_id != developer_id {
            return Err(ApiError::NotFound("crawl rule not found".to_string()));
        }

        state.rules.remove(rule_id);
        push_event(
            &mut state,
            developer_id,
            "rule-deleted",
            "ok",
            format!("deleted rule {}", rule.name),
            Some(rule.seed_url),
            None,
        );
        self.persist_locked(&state).await?;
        Ok(())
    }

    pub async fn overview(
        &self,
        developer_id: &str,
        indexed_documents: usize,
    ) -> Result<CrawlOverviewResponse, ApiError> {
        let mut state = self.inner.write().await;
        let changed = apply_due_rules(&mut state, Some(developer_id));
        if changed {
            self.persist_locked(&state).await?;
        }

        let mut crawlers = state
            .crawlers
            .values()
            .filter(|crawler| crawler.owner_developer_id == developer_id)
            .map(to_crawler_metadata)
            .collect::<Vec<_>>();
        crawlers.sort_by(|left, right| right.created_at.cmp(&left.created_at));

        let mut rules = state
            .rules
            .values()
            .filter(|rule| rule.owner_developer_id == developer_id)
            .map(to_crawl_rule)
            .collect::<Vec<_>>();
        rules.sort_by(|left, right| right.created_at.cmp(&left.created_at));

        let recent_events = state
            .events
            .iter()
            .rev()
            .filter(|event| event.owner_developer_id == developer_id)
            .take(40)
            .map(to_crawl_event)
            .collect::<Vec<_>>();

        Ok(CrawlOverviewResponse {
            developer_id: developer_id.to_string(),
            frontier_depth: state
                .frontier
                .iter()
                .filter(|job| job.owner_developer_id == developer_id)
                .count(),
            known_urls: state
                .known_urls
                .iter()
                .filter(|key| key.starts_with(&format!("{developer_id}:")))
                .count(),
            in_flight_jobs: state
                .in_flight
                .values()
                .filter(|job| job.job.owner_developer_id == developer_id)
                .count(),
            indexed_documents,
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
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("manual:{developer_id}"));
        let mut state = self.inner.write().await;
        let accepted_urls = enqueue_urls(
            &mut state,
            developer_id,
            request.urls,
            &source,
            0,
            request.max_depth.min(10),
            Some(developer_id.to_string()),
            None,
            request.allow_revisit,
        );
        push_event(
            &mut state,
            developer_id,
            "seed-queued",
            "ok",
            format!("queued {accepted_urls} manual seed urls"),
            None,
            None,
        );
        let response = SeedFrontierResponse {
            accepted_urls,
            frontier_depth: state
                .frontier
                .iter()
                .filter(|job| job.owner_developer_id == developer_id)
                .count(),
            known_urls: state
                .known_urls
                .iter()
                .filter(|key| key.starts_with(&format!("{developer_id}:")))
                .count(),
        };
        self.persist_locked(&state).await?;
        Ok(response)
    }

    pub async fn claim_jobs(
        &self,
        crawler_id: &str,
        auth_header: Option<&str>,
        request: ClaimJobsRequest,
    ) -> Result<ClaimJobsResponse, ApiError> {
        let token_hash = bearer_hash(auth_header)?;
        let max_jobs = request.max_jobs.clamp(1, 100);
        let mut state = self.inner.write().await;
        let now = Utc::now();
        let (crawler_id_owned, owner_developer_id, crawler_name) = {
            let crawler = state
                .crawlers
                .get_mut(crawler_id)
                .ok_or_else(|| ApiError::Unauthorized("unknown crawler id".to_string()))?;
            validate_crawler(crawler, &token_hash)?;
            crawler.last_seen_at = Some(now);
            crawler.last_claimed_at = Some(now);
            (
                crawler.id.clone(),
                crawler.owner_developer_id.clone(),
                crawler.name.clone(),
            )
        };

        let changed = apply_due_rules(&mut state, Some(&owner_developer_id));
        if changed {
            self.persist_locked(&state).await?;
        }

        let mut jobs = Vec::new();
        for _ in 0..max_jobs {
            let Some(job) = take_frontier_job(&mut state, &owner_developer_id) else {
                break;
            };
            state.in_flight.insert(
                job.job_id.clone(),
                InFlightRecord {
                    crawler_id: crawler_id_owned.clone(),
                    job: job.clone(),
                    claimed_at: now,
                },
            );
            jobs.push(CrawlJob {
                job_id: job.job_id,
                url: job.url,
                source: job.source,
                depth: job.depth,
                max_depth: job.max_depth,
                discovered_at: job.discovered_at,
            });
        }

        if let Some(crawler) = state.crawlers.get_mut(crawler_id) {
            crawler.jobs_claimed += jobs.len() as u64;
        }

        if !jobs.is_empty() {
            push_event(
                &mut state,
                &owner_developer_id,
                "jobs-claimed",
                "ok",
                format!("crawler {crawler_name} claimed {} jobs", jobs.len()),
                jobs.first().map(|job| job.url.clone()),
                Some(crawler_id_owned.clone()),
            );
        }

        let response = ClaimJobsResponse {
            crawler_id: crawler_id_owned,
            frontier_depth: state
                .frontier
                .iter()
                .filter(|job| job.owner_developer_id == owner_developer_id)
                .count(),
            jobs,
        };
        self.persist_locked(&state).await?;
        Ok(response)
    }

    pub async fn submit_report(
        &self,
        crawler_id: &str,
        auth_header: Option<&str>,
        request: SubmitCrawlReportRequest,
        search_index: &SearchIndex,
    ) -> Result<SubmitCrawlReportResponse, ApiError> {
        let token_hash = bearer_hash(auth_header)?;
        let mut state = self.inner.write().await;
        let now = Utc::now();
        let owner_developer_id = {
            let crawler = state
                .crawlers
                .get_mut(crawler_id)
                .ok_or_else(|| ApiError::Unauthorized("unknown crawler id".to_string()))?;
            validate_crawler(crawler, &token_hash)?;
            crawler.last_seen_at = Some(now);
            crawler.owner_developer_id.clone()
        };

        let mut documents = Vec::new();
        let mut discovered_urls = 0usize;
        let mut reported = 0u64;

        for result in request.results {
            let Some(in_flight) = state.in_flight.remove(&result.job_id) else {
                continue;
            };

            if in_flight.crawler_id != crawler_id || in_flight.job.url != result.url {
                return Err(ApiError::BadRequest(
                    "crawl report contained a job not assigned to this crawler".to_string(),
                ));
            }

            reported += 1;
            let document = build_document(&result);
            let allowed_discovery = in_flight.job.depth < in_flight.job.max_depth;
            if allowed_discovery {
                discovered_urls += enqueue_urls(
                    &mut state,
                    &owner_developer_id,
                    result.discovered_urls.clone(),
                    &result.url,
                    in_flight.job.depth.saturating_add(1),
                    in_flight.job.max_depth,
                    Some(owner_developer_id.clone()),
                    in_flight.job.rule_id.clone(),
                    false,
                );
            }

            push_event(
                &mut state,
                &owner_developer_id,
                "job-reported",
                if (200..300).contains(&result.status_code) {
                    "ok"
                } else {
                    "error"
                },
                format!("fetched {} with status {}", result.url, result.status_code),
                Some(result.url.clone()),
                Some(crawler_id.to_string()),
            );

            if let Some(document) = document {
                documents.push(document);
            }
        }

        if let Some(crawler) = state.crawlers.get_mut(crawler_id) {
            crawler.jobs_reported += reported;
        }
        let frontier_depth = state
            .frontier
            .iter()
            .filter(|job| job.owner_developer_id == owner_developer_id)
            .count();
        self.persist_locked(&state).await?;
        drop(state);

        let accepted_documents = search_index.upsert_documents(documents).await?;
        Ok(SubmitCrawlReportResponse {
            accepted_documents,
            discovered_urls,
            frontier_depth,
            indexed_documents: search_index.total_documents(),
        })
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
        let mut state = self.inner.write().await;
        push_event(
            &mut state,
            developer_id,
            kind,
            status,
            message,
            url,
            crawler_id,
        );
        self.persist_locked(&state).await?;
        Ok(())
    }

    async fn persist_locked(&self, state: &CrawlerStoreState) -> Result<(), ApiError> {
        let raw =
            serde_json::to_string_pretty(state).map_err(|error| ApiError::Internal(error.into()))?;
        fs::write(&self.path, raw).await?;
        Ok(())
    }
}

fn apply_due_rules(state: &mut CrawlerStoreState, developer_id: Option<&str>) -> bool {
    let now = Utc::now();
    let mut changed = false;
    let due_rules = state
        .rules
        .values_mut()
        .filter(|rule| developer_id.is_none_or(|filter| rule.owner_developer_id == filter))
        .filter(|rule| rule.enabled)
        .filter_map(|rule| {
            let is_due = rule
                .last_enqueued_at
                .map(|last| now.signed_duration_since(last).num_minutes() >= rule.interval_minutes as i64)
                .unwrap_or(true);
            if !is_due {
                return None;
            }
            rule.last_enqueued_at = Some(now);
            Some((
                rule.owner_developer_id.clone(),
                rule.id.clone(),
                rule.name.clone(),
                rule.seed_url.clone(),
                rule.max_depth,
            ))
        })
        .collect::<Vec<_>>();

    for (owner_developer_id, rule_id, rule_name, seed_url, max_depth) in due_rules {
        let accepted = enqueue_urls(
            state,
            &owner_developer_id,
            vec![seed_url.clone()],
            &format!("rule:{rule_name}"),
            0,
            max_depth,
            Some(owner_developer_id.clone()),
            Some(rule_id),
            true,
        );
        if accepted > 0 {
            changed = true;
            push_event(
                state,
                &owner_developer_id,
                "rule-enqueued",
                "ok",
                format!("rule {rule_name} queued {accepted} urls"),
                Some(seed_url),
                None,
            );
        }
    }

    changed
}

fn take_frontier_job(
    state: &mut CrawlerStoreState,
    developer_id: &str,
) -> Option<FrontierRecord> {
    let position = state
        .frontier
        .iter()
        .position(|job| job.owner_developer_id == developer_id)?;
    state.frontier.remove(position)
}

fn enqueue_urls(
    state: &mut CrawlerStoreState,
    developer_id: &str,
    urls: Vec<String>,
    source: &str,
    depth: u32,
    max_depth: u32,
    submitted_by: Option<String>,
    rule_id: Option<String>,
    allow_revisit: bool,
) -> usize {
    let mut accepted = 0usize;
    for url in urls {
        let Some(normalized) = normalize_url(&url) else {
            continue;
        };

        let known_key = format!("{developer_id}:{normalized}");
        let already_pending = state.frontier.iter().any(|job| {
            job.owner_developer_id == developer_id && job.url == normalized
        }) || state.in_flight.values().any(|job| {
            job.job.owner_developer_id == developer_id && job.job.url == normalized
        });

        if already_pending {
            continue;
        }

        let is_new = state.known_urls.insert(known_key);
        if !is_new && !allow_revisit {
            continue;
        }

        accepted += 1;
        state.frontier.push_back(FrontierRecord {
            job_id: Uuid::now_v7().to_string(),
            owner_developer_id: developer_id.to_string(),
            url: normalized,
            source: source.to_string(),
            depth,
            max_depth,
            discovered_at: Utc::now(),
            submitted_by: submitted_by.clone(),
            rule_id: rule_id.clone(),
        });
    }
    accepted
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

    Some(IndexedDocument {
        id: stable_document_id(&result.url),
        title,
        url: result.url.clone(),
        display_url: display_url(&result.url),
        snippet: snippet.chars().take(220).collect(),
        body: body.chars().take(4_000).collect(),
        language: result
            .language
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        last_crawled_at: result.fetched_at,
        suggest_terms,
        site_authority: result.site_authority.unwrap_or(0.5),
    })
}

fn to_crawler_metadata(crawler: &StoredCrawler) -> CrawlerMetadata {
    CrawlerMetadata {
        id: crawler.id.clone(),
        name: crawler.name.clone(),
        preview: crawler.preview.clone(),
        created_at: crawler.created_at,
        revoked_at: crawler.revoked_at,
        last_seen_at: crawler.last_seen_at,
        last_claimed_at: crawler.last_claimed_at,
        jobs_claimed: crawler.jobs_claimed,
        jobs_reported: crawler.jobs_reported,
    }
}

fn to_crawl_rule(rule: &StoredCrawlRule) -> CrawlRule {
    CrawlRule {
        id: rule.id.clone(),
        name: rule.name.clone(),
        seed_url: rule.seed_url.clone(),
        interval_minutes: rule.interval_minutes,
        max_depth: rule.max_depth,
        enabled: rule.enabled,
        created_at: rule.created_at,
        updated_at: rule.updated_at,
        last_enqueued_at: rule.last_enqueued_at,
    }
}

fn to_crawl_event(event: &StoredCrawlEvent) -> CrawlEvent {
    CrawlEvent {
        id: event.id.clone(),
        kind: event.kind.clone(),
        status: event.status.clone(),
        message: event.message.clone(),
        url: event.url.clone(),
        crawler_id: event.crawler_id.clone(),
        created_at: event.created_at,
    }
}

fn push_event(
    state: &mut CrawlerStoreState,
    developer_id: &str,
    kind: &str,
    status: &str,
    message: String,
    url: Option<String>,
    crawler_id: Option<String>,
) {
    state.events.push_back(StoredCrawlEvent {
        id: Uuid::now_v7().to_string(),
        owner_developer_id: developer_id.to_string(),
        kind: kind.to_string(),
        status: status.to_string(),
        message,
        url,
        crawler_id,
        created_at: Utc::now(),
    });

    while state.events.len() > 400 {
        state.events.pop_front();
    }
}

fn validate_crawler(crawler: &StoredCrawler, token_hash: &str) -> Result<(), ApiError> {
    if crawler.revoked_at.is_some() {
        return Err(ApiError::Unauthorized("crawler key is revoked".to_string()));
    }
    if crawler.key_hash != token_hash {
        return Err(ApiError::Unauthorized("invalid crawler key".to_string()));
    }
    Ok(())
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

fn default_owner_developer_id() -> String {
    "local:admin".to_string()
}

fn default_rule_depth() -> u32 {
    2
}

fn default_true() -> bool {
    true
}

fn normalize_url(input: &str) -> Option<String> {
    let mut url = Url::parse(input).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    url.set_fragment(None);
    Some(url.to_string())
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
    let secret = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(40)
        .map(char::from)
        .collect::<String>();
    format!("{prefix}_{secret}")
}

async fn ensure_file_with_fallbacks(
    path: &PathBuf,
    default_contents: &str,
    fallbacks: &[PathBuf],
) -> anyhow::Result<()> {
    if fs::metadata(path).await.is_ok() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    for fallback in fallbacks {
        if fallback != path && fs::metadata(fallback).await.is_ok() {
            fs::copy(fallback, path).await?;
            return Ok(());
        }
    }

    fs::write(path, default_contents).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::{
        CrawlerStore, CrawlerStoreState, enqueue_urls, normalize_url, push_event, take_frontier_job,
    };
    use tokio::fs;
    use uuid::Uuid;

    #[test]
    fn normalize_url_rejects_non_http() {
        assert!(normalize_url("ftp://example.com/file").is_none());
        assert_eq!(
            normalize_url("https://example.com/a#fragment"),
            Some("https://example.com/a".to_string())
        );
    }

    #[test]
    fn enqueue_urls_deduplicates_known_urls() {
        let mut state = CrawlerStoreState::default();
        let accepted = enqueue_urls(
            &mut state,
            "local:admin",
            vec![
                "https://example.com".to_string(),
                "https://example.com".to_string(),
            ],
            "seed",
            0,
            2,
            Some("local:admin".to_string()),
            None,
            false,
        );

        assert_eq!(accepted, 1);
        assert_eq!(state.frontier.len(), 1);
        assert_eq!(state.known_urls.len(), 1);
    }

    #[test]
    fn take_frontier_job_is_owner_scoped() {
        let mut state = CrawlerStoreState::default();
        enqueue_urls(
            &mut state,
            "local:admin",
            vec!["https://example.com".to_string()],
            "seed",
            0,
            1,
            None,
            None,
            false,
        );
        enqueue_urls(
            &mut state,
            "local:other",
            vec!["https://example.org".to_string()],
            "seed",
            0,
            1,
            None,
            None,
            false,
        );

        let job = take_frontier_job(&mut state, "local:other").expect("missing job");
        assert_eq!(job.url, "https://example.org/");
    }

    #[test]
    fn event_log_is_capped() {
        let mut state = CrawlerStoreState::default();
        for index in 0..405 {
            push_event(
                &mut state,
                "local:admin",
                "test",
                "ok",
                format!("event {index}"),
                None,
                None,
            );
        }

        assert_eq!(state.events.len(), 400);
    }

    #[tokio::test]
    async fn load_accepts_legacy_store_shape() {
        let temp_path = env::temp_dir().join(format!("findverse-crawler-{}.json", Uuid::now_v7()));
        fs::write(
            &temp_path,
            r#"{
  "crawlers": {},
  "frontier": [],
  "known_urls": [],
  "in_flight": {}
}"#,
        )
        .await
        .expect("failed to write legacy store");

        let store = CrawlerStore::load(temp_path.clone())
            .await
            .expect("legacy crawler store should load");
        let overview = store
            .overview("local:admin", 0)
            .await
            .expect("overview should succeed");

        assert_eq!(overview.rules.len(), 0);
        assert_eq!(overview.recent_events.len(), 0);

        let _ = fs::remove_file(temp_path).await;
    }
}

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
        ClaimJobsRequest, ClaimJobsResponse, CrawlJob, CrawlOverviewResponse, CrawlResultInput,
        CrawlerMetadata, CreateCrawlerRequest, CreatedCrawlerResponse, IndexedDocument,
        SeedFrontierRequest, SeedFrontierResponse, SubmitCrawlReportRequest,
        SubmitCrawlReportResponse,
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
    crawlers: HashMap<String, StoredCrawler>,
    frontier: VecDeque<FrontierRecord>,
    known_urls: HashSet<String>,
    in_flight: HashMap<String, InFlightRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredCrawler {
    id: String,
    owner_developer_id: String,
    name: String,
    preview: String,
    key_hash: String,
    created_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
    last_seen_at: Option<DateTime<Utc>>,
    last_claimed_at: Option<DateTime<Utc>>,
    jobs_claimed: u64,
    jobs_reported: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FrontierRecord {
    job_id: String,
    url: String,
    source: String,
    depth: u32,
    discovered_at: DateTime<Utc>,
    submitted_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InFlightRecord {
    crawler_id: String,
    job: FrontierRecord,
    claimed_at: DateTime<Utc>,
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

        let key = generate_token();
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

    pub async fn overview(
        &self,
        developer_id: &str,
        indexed_documents: usize,
    ) -> Result<CrawlOverviewResponse, ApiError> {
        let state = self.inner.read().await;
        let mut crawlers = state
            .crawlers
            .values()
            .filter(|crawler| crawler.owner_developer_id == developer_id)
            .map(to_crawler_metadata)
            .collect::<Vec<_>>();
        crawlers.sort_by(|left, right| right.created_at.cmp(&left.created_at));

        Ok(CrawlOverviewResponse {
            developer_id: developer_id.to_string(),
            frontier_depth: state.frontier.len(),
            known_urls: state.known_urls.len(),
            in_flight_jobs: state.in_flight.len(),
            indexed_documents,
            crawlers,
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
            .unwrap_or_else(|| format!("developer:{developer_id}"));
        let mut state = self.inner.write().await;
        let accepted_urls = enqueue_urls(
            &mut state,
            request.urls,
            &source,
            0,
            Some(developer_id.to_string()),
        );
        let response = SeedFrontierResponse {
            accepted_urls,
            frontier_depth: state.frontier.len(),
            known_urls: state.known_urls.len(),
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
        let crawler_id_owned = {
            let crawler = state
                .crawlers
                .get_mut(crawler_id)
                .ok_or_else(|| ApiError::Unauthorized("unknown crawler id".to_string()))?;
            validate_crawler(crawler, &token_hash)?;
            crawler.last_seen_at = Some(now);
            crawler.last_claimed_at = Some(now);
            crawler.id.clone()
        };

        let mut jobs = Vec::new();
        for _ in 0..max_jobs {
            let Some(job) = state.frontier.pop_front() else {
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
                discovered_at: job.discovered_at,
            });
        }

        if let Some(crawler) = state.crawlers.get_mut(crawler_id) {
            crawler.jobs_claimed += jobs.len() as u64;
        }

        let response = ClaimJobsResponse {
            crawler_id: crawler_id_owned,
            frontier_depth: state.frontier.len(),
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
            discovered_urls += enqueue_urls(
                &mut state,
                result.discovered_urls.clone(),
                &result.url,
                in_flight.job.depth.saturating_add(1),
                Some(owner_developer_id.clone()),
            );

            if let Some(document) = document {
                documents.push(document);
            }
        }

        if let Some(crawler) = state.crawlers.get_mut(crawler_id) {
            crawler.jobs_reported += reported;
        }
        let frontier_depth = state.frontier.len();
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

    async fn persist_locked(&self, state: &CrawlerStoreState) -> Result<(), ApiError> {
        let raw =
            serde_json::to_string_pretty(state).map_err(|error| ApiError::Internal(error.into()))?;
        fs::write(&self.path, raw).await?;
        Ok(())
    }
}

fn enqueue_urls(
    state: &mut CrawlerStoreState,
    urls: Vec<String>,
    source: &str,
    depth: u32,
    submitted_by: Option<String>,
) -> usize {
    let mut accepted = 0usize;
    for url in urls {
        let Some(normalized) = normalize_url(&url) else {
            continue;
        };
        if state.known_urls.insert(normalized.clone()) {
            accepted += 1;
            state.frontier.push_back(FrontierRecord {
                job_id: Uuid::now_v7().to_string(),
                url: normalized,
                source: source.to_string(),
                depth,
                discovered_at: Utc::now(),
                submitted_by: submitted_by.clone(),
            });
        }
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

fn validate_crawler(crawler: &StoredCrawler, token_hash: &str) -> Result<(), ApiError> {
    if crawler.revoked_at.is_some() {
        return Err(ApiError::Unauthorized("crawler key is revoked".to_string()));
    }
    if crawler.key_hash != token_hash {
        return Err(ApiError::Unauthorized("invalid crawler key".to_string()));
    }
    Ok(())
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

fn generate_token() -> String {
    let secret = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(40)
        .map(char::from)
        .collect::<String>();
    format!("fvc_{secret}")
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
    use super::{CrawlerStoreState, enqueue_urls, normalize_url};

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
            vec![
                "https://example.com".to_string(),
                "https://example.com".to_string(),
            ],
            "seed",
            0,
            Some("developer".to_string()),
        );

        assert_eq!(accepted, 1);
        assert_eq!(state.frontier.len(), 1);
        assert_eq!(state.known_urls.len(), 1);
    }
}

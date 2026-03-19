use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedDocument {
    pub id: String,
    pub title: String,
    pub url: String,
    pub display_url: String,
    pub snippet: String,
    pub body: String,
    pub language: String,
    pub last_crawled_at: DateTime<Utc>,
    #[serde(default)]
    pub suggest_terms: Vec<String>,
    #[serde(default = "default_site_authority")]
    pub site_authority: f32,
}

fn default_site_authority() -> f32 {
    0.5
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchParams {
    pub q: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
    pub lang: Option<String>,
    pub site: Option<String>,
    #[serde(default)]
    pub freshness: Freshness,
}

fn default_limit() -> usize {
    10
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub enum Freshness {
    #[serde(rename = "24h")]
    Day,
    #[serde(rename = "7d")]
    Week,
    #[serde(rename = "30d")]
    Month,
    #[default]
    #[serde(rename = "all")]
    All,
}

impl Freshness {
    pub fn max_age(self) -> Option<Duration> {
        match self {
            Self::Day => Some(Duration::hours(24)),
            Self::Week => Some(Duration::days(7)),
            Self::Month => Some(Duration::days(30)),
            Self::All => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub url: String,
    pub display_url: String,
    pub snippet: String,
    pub language: String,
    pub last_crawled_at: DateTime<Utc>,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResponse {
    pub query: String,
    pub took_ms: u128,
    pub total_estimate: usize,
    pub next_offset: Option<usize>,
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SuggestResponse {
    pub query: String,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateKeyRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreatedKeyResponse {
    pub id: String,
    pub name: String,
    pub preview: String,
    pub token: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiKeyMetadata {
    pub id: String,
    pub name: String,
    pub preview: String,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeveloperUsageResponse {
    pub developer_id: String,
    pub qps_limit: u32,
    pub daily_limit: u32,
    pub used_today: u32,
    pub keys: Vec<ApiKeyMetadata>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateCrawlerRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreatedCrawlerResponse {
    pub id: String,
    pub name: String,
    pub preview: String,
    pub key: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CrawlerMetadata {
    pub id: String,
    pub name: String,
    pub preview: String,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub last_claimed_at: Option<DateTime<Utc>>,
    pub jobs_claimed: u64,
    pub jobs_reported: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CrawlOverviewResponse {
    pub developer_id: String,
    pub frontier_depth: usize,
    pub known_urls: usize,
    pub in_flight_jobs: usize,
    pub indexed_documents: usize,
    pub crawlers: Vec<CrawlerMetadata>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SeedFrontierRequest {
    pub urls: Vec<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SeedFrontierResponse {
    pub accepted_urls: usize,
    pub frontier_depth: usize,
    pub known_urls: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClaimJobsRequest {
    #[serde(default = "default_claim_max_jobs")]
    pub max_jobs: usize,
}

fn default_claim_max_jobs() -> usize {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlJob {
    pub job_id: String,
    pub url: String,
    pub source: String,
    pub depth: u32,
    pub discovered_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClaimJobsResponse {
    pub crawler_id: String,
    pub frontier_depth: usize,
    pub jobs: Vec<CrawlJob>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubmitCrawlReportRequest {
    pub results: Vec<CrawlResultInput>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CrawlResultInput {
    pub job_id: String,
    pub url: String,
    pub status_code: u16,
    pub fetched_at: DateTime<Utc>,
    pub title: Option<String>,
    pub snippet: Option<String>,
    pub body: Option<String>,
    pub language: Option<String>,
    #[serde(default)]
    pub discovered_urls: Vec<String>,
    pub site_authority: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubmitCrawlReportResponse {
    pub accepted_documents: usize,
    pub discovered_urls: usize,
    pub frontier_depth: usize,
    pub indexed_documents: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub documents: usize,
}

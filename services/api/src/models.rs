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

#[derive(Debug, Clone, Deserialize)]
pub struct DocumentListParams {
    pub query: Option<String>,
    pub site: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
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
pub struct AdminLoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AdminSessionResponse {
    pub developer_id: String,
    pub username: String,
    pub token: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlRule {
    pub id: String,
    pub name: String,
    pub seed_url: String,
    pub interval_minutes: u64,
    pub max_depth: u32,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_enqueued_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateCrawlRuleRequest {
    pub name: String,
    pub seed_url: String,
    #[serde(default = "default_interval_minutes")]
    pub interval_minutes: u64,
    #[serde(default = "default_rule_depth")]
    pub max_depth: u32,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateCrawlRuleRequest {
    pub name: Option<String>,
    pub seed_url: Option<String>,
    pub interval_minutes: Option<u64>,
    pub max_depth: Option<u32>,
    pub enabled: Option<bool>,
}

fn default_interval_minutes() -> u64 {
    60
}

fn default_rule_depth() -> u32 {
    2
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlEvent {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub message: String,
    pub url: Option<String>,
    pub crawler_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CrawlOverviewResponse {
    pub developer_id: String,
    pub frontier_depth: usize,
    pub known_urls: usize,
    pub in_flight_jobs: usize,
    pub indexed_documents: usize,
    pub crawlers: Vec<CrawlerMetadata>,
    pub rules: Vec<CrawlRule>,
    pub recent_events: Vec<CrawlEvent>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SeedFrontierRequest {
    pub urls: Vec<String>,
    pub source: Option<String>,
    #[serde(default = "default_rule_depth")]
    pub max_depth: u32,
    #[serde(default)]
    pub allow_revisit: bool,
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
    pub max_depth: u32,
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
pub struct DocumentSummary {
    pub id: String,
    pub title: String,
    pub url: String,
    pub display_url: String,
    pub snippet: String,
    pub language: String,
    pub last_crawled_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocumentListResponse {
    pub total_estimate: usize,
    pub next_offset: Option<usize>,
    pub documents: Vec<DocumentSummary>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PurgeSiteRequest {
    pub site: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PurgeSiteResponse {
    pub deleted_documents: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub documents: usize,
}

// Developer self-service auth
#[derive(Debug, Clone, Deserialize)]
pub struct DevRegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DevLoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DevSessionResponse {
    pub user_id: String,
    pub username: String,
    pub token: String,
}

// Admin developer management
#[derive(Debug, Clone, Serialize)]
pub struct AdminDeveloperRecord {
    pub user_id: String,
    pub username: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub qps_limit: u32,
    pub daily_limit: u32,
    pub used_today: u32,
    pub key_count: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateDeveloperRequest {
    pub qps_limit: Option<u32>,
    pub daily_limit: Option<u32>,
    pub enabled: Option<bool>,
}

// Crawler auto-registration
#[derive(Debug, Clone, Deserialize)]
pub struct HelloCrawlerRequest {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HelloCrawlerResponse {
    pub crawler_id: String,
    pub name: String,
}

// Crawler join key
#[derive(Debug, Clone, Deserialize)]
pub struct JoinCrawlerRequest {
    pub join_key: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JoinCrawlerResponse {
    pub crawler_id: String,
    pub crawler_key: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlerJoinKeyResponse {
    pub join_key: Option<String>,
}

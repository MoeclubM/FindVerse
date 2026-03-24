use chrono::{DateTime, Duration, Utc};
use findverse_common::{CURRENT_INDEX_VERSION, CURRENT_PARSER_VERSION, CURRENT_SCHEMA_VERSION};
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
    pub canonical_url: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub content_hash: Option<String>,
    #[serde(default)]
    pub suggest_terms: Vec<String>,
    #[serde(default = "default_site_authority")]
    pub site_authority: f32,
    #[serde(default = "default_content_type")]
    pub content_type: String,
    #[serde(default)]
    pub word_count: u32,
    #[serde(default)]
    pub source_job_id: Option<String>,
    #[serde(default = "default_parser_version")]
    pub parser_version: i32,
    #[serde(default = "default_schema_version")]
    pub schema_version: i32,
    #[serde(default = "default_index_version")]
    pub index_version: i32,
    #[serde(default)]
    pub duplicate_of: Option<String>,
}

fn default_site_authority() -> f32 {
    0.5
}

fn default_content_type() -> String {
    "text/html".to_string()
}

fn default_parser_version() -> i32 {
    CURRENT_PARSER_VERSION
}

fn default_schema_version() -> i32 {
    CURRENT_SCHEMA_VERSION
}

fn default_index_version() -> i32 {
    CURRENT_INDEX_VERSION
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub query: String,
    pub took_ms: u128,
    pub total_estimate: usize,
    pub next_offset: Option<usize>,
    pub results: Vec<SearchResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub did_you_mean: Option<String>,
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
    pub user_id: String,
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
    pub daily_limit: u32,
    pub used_today: u32,
    pub keys: Vec<ApiKeyMetadata>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RenameCrawlerRequest {
    pub name: String,
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
    pub owner_id: String,
    pub frontier_depth: usize,
    pub known_urls: usize,
    pub in_flight_jobs: usize,
    pub indexed_documents: usize,
    pub duplicate_documents: usize,
    pub terminal_failures: usize,
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
    pub attempt_count: u32,
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
    pub final_url: Option<String>,
    pub content_type: Option<String>,
    pub title: Option<String>,
    pub snippet: Option<String>,
    pub body: Option<String>,
    pub language: Option<String>,
    #[serde(default)]
    pub discovered_urls: Vec<String>,
    pub site_authority: Option<f32>,
    pub retryable: Option<bool>,
    pub error_kind: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubmitCrawlReportResponse {
    pub accepted_documents: usize,
    pub duplicate_documents: usize,
    pub skipped_documents: usize,
    pub discovered_urls: usize,
    pub frontier_depth: usize,
    pub indexed_documents: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocumentSummary {
    pub id: String,
    pub title: String,
    pub url: String,
    pub canonical_url: String,
    pub host: String,
    pub display_url: String,
    pub snippet: String,
    pub language: String,
    pub last_crawled_at: DateTime<Utc>,
    pub content_type: String,
    pub word_count: u32,
    pub site_authority: f32,
    pub parser_version: i32,
    pub schema_version: i32,
    pub index_version: i32,
    pub source_job_id: Option<String>,
    pub duplicate_of: Option<String>,
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

#[derive(Debug, Clone, Serialize)]
pub struct ReadyResponse {
    pub status: &'static str,
    pub postgres: bool,
    pub redis: bool,
    pub opensearch: bool,
    pub frontier_depth: i32,
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
    pub daily_limit: u32,
    pub used_today: u32,
    pub key_count: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateDeveloperRequest {
    pub daily_limit: Option<u32>,
    pub enabled: Option<bool>,
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

// Crawl job management
#[derive(Debug, Clone, Serialize)]
pub struct CrawlJobDetail {
    pub id: String,
    pub url: String,
    pub final_url: Option<String>,
    pub status: String,
    pub depth: u32,
    pub max_depth: u32,
    pub attempt_count: u32,
    pub max_attempts: u32,
    pub source: String,
    pub rule_id: Option<String>,
    pub claimed_by: Option<String>,
    pub discovered_at: DateTime<Utc>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub content_type: Option<String>,
    pub http_status: Option<u16>,
    pub discovered_urls_count: usize,
    pub accepted_document_id: Option<String>,
    pub failure_kind: Option<String>,
    pub failure_message: Option<String>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CrawlJobListResponse {
    pub total: usize,
    pub next_offset: Option<usize>,
    pub jobs: Vec<CrawlJobDetail>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CrawlJobStats {
    pub queued: usize,
    pub claimed: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub blocked: usize,
    pub dead_letter: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CrawlJobListParams {
    pub status: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

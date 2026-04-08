use chrono::{DateTime, Duration, Utc};
use findverse_common::{
    CURRENT_INDEX_VERSION, CURRENT_PARSER_VERSION, CURRENT_SCHEMA_VERSION, DiscoveryScope,
};
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
    #[serde(default = "crate::models::default_network")]
    pub network: String,
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

pub fn default_network() -> String {
    "clearnet".to_string()
}

pub fn default_render_mode() -> String {
    "static".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlerCapabilities {
    #[serde(default)]
    pub js_render: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlerRuntimeSnapshot {
    pub version: String,
    pub platform: String,
    #[serde(default = "default_crawler_update_status")]
    pub update_status: String,
    #[serde(default)]
    pub update_message: Option<String>,
}

fn default_crawler_update_status() -> String {
    "idle".to_string()
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
    pub network: Option<String>,
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
pub struct DeveloperDomainInsightQuery {
    pub domain: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeveloperDomainSubmitRequest {
    pub domain: String,
    pub urls: Vec<String>,
    #[serde(default = "default_rule_depth")]
    pub max_depth: u32,
    #[serde(default = "default_max_pages")]
    pub max_pages: u32,
    #[serde(default = "default_same_origin_concurrency")]
    pub same_origin_concurrency: u32,
    #[serde(default)]
    pub allow_revisit: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeveloperDomainFacet {
    pub label: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeveloperDomainDocument {
    pub id: String,
    pub title: String,
    pub url: String,
    pub display_url: String,
    pub language: String,
    pub last_crawled_at: DateTime<Utc>,
    pub word_count: u32,
    pub content_type: String,
    pub duplicate_of: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeveloperDomainJob {
    pub id: String,
    pub url: String,
    pub status: String,
    pub http_status: Option<u16>,
    pub depth: u32,
    pub discovered_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub failure_kind: Option<String>,
    pub failure_message: Option<String>,
    pub accepted_document_id: Option<String>,
    pub render_mode: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeveloperDomainInsightResponse {
    pub domain: String,
    pub property_url: String,
    pub indexed_documents: usize,
    pub duplicate_documents: usize,
    pub pending_jobs: usize,
    pub successful_jobs: usize,
    pub filtered_jobs: usize,
    pub failed_jobs: usize,
    pub blocked_jobs: usize,
    pub last_indexed_at: Option<DateTime<Utc>>,
    pub last_crawled_at: Option<DateTime<Utc>>,
    pub top_languages: Vec<DeveloperDomainFacet>,
    pub top_content_types: Vec<DeveloperDomainFacet>,
    pub recent_documents: Vec<DeveloperDomainDocument>,
    pub recent_jobs: Vec<DeveloperDomainJob>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeveloperDomainSubmitResponse {
    pub accepted_urls: usize,
    pub queued_domain_jobs: usize,
    pub known_domain_urls: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateCrawlerRequest {
    pub name: Option<String>,
    pub worker_concurrency: Option<usize>,
    pub js_render_concurrency: Option<usize>,
    pub desired_version: Option<String>,
    #[serde(default)]
    pub sort_order: Option<Option<i32>>,
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
    pub online: bool,
    pub can_delete: bool,
    pub in_flight_jobs: u64,
    pub jobs_claimed: u64,
    pub jobs_reported: u64,
    pub supports_js_render: bool,
    pub worker_concurrency: usize,
    pub js_render_concurrency: usize,
    pub version: Option<String>,
    pub platform: Option<String>,
    pub desired_version: Option<String>,
    pub sort_order: Option<i32>,
    pub update_status: String,
    pub update_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlRule {
    pub id: String,
    pub name: String,
    pub seed_url: String,
    pub interval_minutes: u64,
    pub max_depth: u32,
    pub max_pages: u32,
    pub same_origin_concurrency: u32,
    pub discovery_scope: DiscoveryScope,
    pub max_discovered_urls_per_page: u32,
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
    #[serde(default = "default_max_pages")]
    pub max_pages: u32,
    #[serde(default = "default_same_origin_concurrency")]
    pub same_origin_concurrency: u32,
    #[serde(default)]
    pub discovery_scope: DiscoveryScope,
    #[serde(default = "default_max_discovered_urls_per_page")]
    pub max_discovered_urls_per_page: u32,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateCrawlRuleRequest {
    pub name: Option<String>,
    pub seed_url: Option<String>,
    pub interval_minutes: Option<u64>,
    pub max_depth: Option<u32>,
    pub max_pages: Option<u32>,
    pub same_origin_concurrency: Option<u32>,
    pub discovery_scope: Option<DiscoveryScope>,
    pub max_discovered_urls_per_page: Option<u32>,
    pub enabled: Option<bool>,
}

fn default_interval_minutes() -> u64 {
    60
}

fn default_rule_depth() -> u32 {
    2
}

fn default_max_pages() -> u32 {
    50
}

fn default_same_origin_concurrency() -> u32 {
    1
}

fn default_max_discovered_urls_per_page() -> u32 {
    50
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
    pub platform_version: String,
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
    #[serde(default = "default_max_pages")]
    pub max_pages: u32,
    #[serde(default = "default_same_origin_concurrency")]
    pub same_origin_concurrency: u32,
    #[serde(default)]
    pub discovery_scope: DiscoveryScope,
    #[serde(default = "default_max_discovered_urls_per_page")]
    pub max_discovered_urls_per_page: u32,
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
    pub origin_key: String,
    pub source: String,
    pub depth: u32,
    pub max_depth: u32,
    pub attempt_count: u32,
    pub discovered_at: DateTime<Utc>,
    #[serde(default = "crate::models::default_network")]
    pub network: String,
    #[serde(default)]
    pub etag: Option<String>,
    #[serde(default)]
    pub last_modified: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClaimJobsResponse {
    pub crawler_id: String,
    pub lease_id: Option<String>,
    pub frontier_depth: usize,
    pub jobs: Vec<CrawlJob>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CrawlerHeartbeatResponse {
    pub worker_concurrency: usize,
    pub js_render_concurrency: usize,
    pub desired_version: Option<String>,
    pub update_status: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubmitCrawlReportRequest {
    pub lease_id: String,
    pub results: Vec<CrawlResultInput>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CrawlResultInput {
    pub job_id: String,
    pub url: String,
    pub status_code: u16,
    pub fetched_at: DateTime<Utc>,
    pub final_url: Option<String>,
    #[serde(default)]
    pub redirect_chain: Vec<String>,
    pub content_type: Option<String>,
    pub title: Option<String>,
    pub snippet: Option<String>,
    pub body: Option<String>,
    #[serde(default)]
    pub canonical_hint: Option<String>,
    #[serde(default)]
    pub canonical_source: Option<String>,
    pub language: Option<String>,
    #[serde(default)]
    pub discovered_urls: Vec<String>,
    pub site_authority: Option<f32>,
    pub llm_should_index: Option<bool>,
    pub llm_should_discover: Option<bool>,
    pub llm_relevance_score: Option<f32>,
    pub llm_reason: Option<String>,
    pub retryable: Option<bool>,
    pub error_kind: Option<String>,
    pub error_message: Option<String>,
    #[serde(default = "crate::models::default_network")]
    pub network: String,
    #[serde(default)]
    pub http_etag: Option<String>,
    #[serde(default)]
    pub http_last_modified: Option<String>,
    #[serde(default)]
    pub applied_crawl_delay_secs: Option<u64>,
    #[serde(default)]
    pub retry_after_secs: Option<u64>,
    #[serde(default)]
    pub robots_status: Option<String>,
    #[serde(default)]
    pub robots_sitemaps: Vec<String>,
    #[serde(default = "crate::models::default_render_mode")]
    pub render_mode: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubmitCrawlReportResponse {
    pub lease_id: String,
    pub staged_results: usize,
    pub pending_results: usize,
    pub frontier_depth: usize,
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

#[derive(Debug, Clone, serde::Serialize)]
pub struct SystemConfigEntry {
    pub key: String,
    pub value: String,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SetSystemConfigRequest {
    pub value: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SystemConfigResponse {
    pub entries: Vec<SystemConfigEntry>,
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
    pub password: Option<String>,
}

// Crawl job management
#[derive(Debug, Clone, Serialize)]
pub struct CrawlJobDetail {
    pub id: String,
    pub url: String,
    pub origin_key: String,
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
    pub llm_decision: Option<String>,
    pub llm_reason: Option<String>,
    pub llm_relevance_score: Option<f32>,
    pub canonical_hint: Option<String>,
    pub canonical_source: Option<String>,
    pub failure_kind: Option<String>,
    pub failure_message: Option<String>,
    pub finished_at: Option<DateTime<Utc>>,
    pub render_mode: String,
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

#[derive(Debug, Clone, Serialize)]
pub struct CrawlOriginState {
    pub origin_key: String,
    pub robots_status: String,
    pub crawl_delay_secs: Option<u32>,
    pub next_allowed_at: DateTime<Utc>,
    pub in_flight_count: u32,
    pub last_fetch_status: Option<u16>,
    pub consecutive_failures: u32,
    pub robots_sitemaps: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CrawlJobListParams {
    pub status: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}

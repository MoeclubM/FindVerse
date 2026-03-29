use chrono::{DateTime, Utc};
use findverse_common::{CURRENT_INDEX_VERSION, CURRENT_PARSER_VERSION, CURRENT_SCHEMA_VERSION};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------
#[derive(Debug, clap::Parser)]
#[command(name = "findverse-crawler")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, clap::Subcommand)]
pub enum Command {
    Discover {
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = 50)]
        limit_per_seed: usize,
    },
    Fetch {
        #[arg(long)]
        frontier: PathBuf,
        #[arg(long)]
        output_dir: PathBuf,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    BuildIndex {
        #[arg(long)]
        input_dir: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Worker {
        #[arg(long)]
        server: String,
        /// Fixed crawler ID
        #[arg(long)]
        crawler_id: String,
        /// Fixed crawler key
        #[arg(long)]
        crawler_key: String,
        /// Claim batch size. Values below --concurrency are ignored.
        #[arg(long, default_value_t = 16)]
        max_jobs: usize,
        #[arg(long, default_value_t = 5)]
        poll_interval_secs: u64,
        #[arg(long, default_value_t = false)]
        once: bool,
        /// Number of concurrent page fetches
        #[arg(long, default_value_t = 16)]
        concurrency: usize,
        /// Comma-separated list of allowed domains (subdomains included)
        #[arg(long)]
        allowed_domains: Option<String>,
        /// HTTP proxy URL
        #[arg(long)]
        proxy: Option<String>,
        /// SOCKS5 proxy URL for Tor (.onion) crawling, e.g. socks5h://127.0.0.1:9050
        #[arg(long, default_value = "socks5h://127.0.0.1:9050")]
        tor_socks_url: String,
        /// OpenAI-compatible base URL, for example https://api.openai.com/v1
        #[arg(long)]
        llm_base_url: Option<String>,
        /// API key for the OpenAI-compatible endpoint
        #[arg(long)]
        llm_api_key: Option<String>,
        /// Model name used for page filtering
        #[arg(long)]
        llm_model: Option<String>,
        /// Minimum relevance score required for indexing
        #[arg(long, default_value_t = 0.45)]
        llm_min_score: f32,
        /// Number of body characters sent to the LLM
        #[arg(long, default_value_t = 6000)]
        llm_max_body_chars: usize,
        /// Deprecated compatibility flag. FindVerse now always uses the public crawler UA.
        #[arg(long, default_value_t = false)]
        stealth_ua: bool,
    },
}

// ---------------------------------------------------------------------------
// Data structs — offline commands
// ---------------------------------------------------------------------------
#[derive(Debug, Deserialize)]
pub struct SeedConfig {
    pub seeds: Vec<Seed>,
}

#[derive(Debug, Deserialize)]
pub struct Seed {
    pub name: String,
    pub url: String,
    pub sitemap: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FrontierEntry {
    pub url: String,
    pub source: String,
    pub discovered_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FetchManifestEntry {
    pub url: String,
    pub storage_path: String,
    pub fetched_at: DateTime<Utc>,
    pub content_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexedDocument {
    pub id: String,
    pub title: String,
    pub url: String,
    pub display_url: String,
    pub snippet: String,
    pub body: String,
    pub language: String,
    pub last_crawled_at: DateTime<Utc>,
    pub suggest_terms: Vec<String>,
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

// ---------------------------------------------------------------------------
// Data structs — worker API
// ---------------------------------------------------------------------------
#[derive(Debug, Serialize)]
pub struct ClaimJobsRequest {
    pub max_jobs: usize,
}

#[derive(Debug, Deserialize)]
pub struct ClaimJobsResponse {
    pub crawler_id: String,
    pub frontier_depth: usize,
    pub jobs: Vec<CrawlJob>,
}

#[derive(Debug, Deserialize, Clone)]
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

#[derive(Debug, Serialize)]
pub struct SubmitCrawlReportRequest {
    pub results: Vec<CrawlResultReport>,
}

#[derive(Debug, Serialize)]
pub struct CrawlResultReport {
    pub job_id: String,
    pub url: String,
    pub status_code: u16,
    pub fetched_at: DateTime<Utc>,
    pub final_url: Option<String>,
    pub redirect_chain: Vec<String>,
    pub content_type: Option<String>,
    pub title: Option<String>,
    pub snippet: Option<String>,
    pub body: Option<String>,
    pub canonical_hint: Option<String>,
    pub canonical_source: Option<String>,
    pub language: Option<String>,
    pub discovered_urls: Vec<String>,
    pub site_authority: Option<f32>,
    pub llm_should_index: Option<bool>,
    pub llm_should_discover: Option<bool>,
    pub llm_relevance_score: Option<f32>,
    pub llm_reason: Option<String>,
    pub retryable: Option<bool>,
    pub error_kind: Option<String>,
    pub error_message: Option<String>,
    pub network: String,
    pub http_etag: Option<String>,
    pub http_last_modified: Option<String>,
    pub applied_crawl_delay_secs: Option<u64>,
    pub retry_after_secs: Option<u64>,
    pub robots_status: Option<String>,
    pub robots_sitemaps: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubmitCrawlReportResponse {
    pub accepted_documents: usize,
    pub duplicate_documents: usize,
    pub skipped_documents: usize,
    pub discovered_urls: usize,
    pub frontier_depth: usize,
    pub indexed_documents: usize,
}

// ---------------------------------------------------------------------------
// Worker config
// ---------------------------------------------------------------------------
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    pub server: String,
    pub crawler_id: String,
    pub auth_token: String,
    pub max_jobs: usize,
    pub poll_interval_secs: u64,
    pub once: bool,
    pub concurrency: usize,
    pub allowed_domains: Vec<String>,
    pub tor_socks_url: Option<String>,
    pub llm_filter: Option<LlmFilterConfig>,
    #[allow(dead_code)]
    pub stealth_ua: bool,
}

#[derive(Debug, Clone)]
pub struct LlmFilterConfig {
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub min_score: f32,
    pub max_body_chars: usize,
}

// ---------------------------------------------------------------------------
// Parsed HTML result (used between extract and worker)
// ---------------------------------------------------------------------------
pub struct ParsedHtml {
    pub title: Option<String>,
    pub snippet: Option<String>,
    pub body: Option<String>,
    pub discovered_urls: Vec<String>,
    pub canonical_url: Option<String>,
    pub robots_directives: RobotsDirectives,
}

#[derive(Debug, Clone)]
pub struct LlmPageDecision {
    pub should_index: bool,
    pub should_discover: bool,
    pub relevance_score: f32,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RobotsDirectives {
    pub noindex: bool,
    pub nofollow: bool,
}

impl RobotsDirectives {
    pub fn merge(&mut self, other: Self) {
        self.noindex |= other.noindex;
        self.nofollow |= other.nofollow;
    }
}

impl LlmFilterConfig {
    pub fn is_enabled(&self) -> bool {
        !self.base_url.trim().is_empty() && !self.model.trim().is_empty()
    }
}

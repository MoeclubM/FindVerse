use std::{
    collections::BTreeSet,
    path::PathBuf,
    time::Duration,
};

use anyhow::Context;
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use reqwest::header::CONTENT_TYPE;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::time::sleep;
use tracing::{info, warn};
use url::Url;

#[derive(Debug, Parser)]
#[command(name = "findverse-crawler")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
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
        #[arg(long)]
        crawler_id: String,
        #[arg(long)]
        crawler_key: String,
        #[arg(long, default_value_t = 10)]
        max_jobs: usize,
        #[arg(long, default_value_t = 5)]
        poll_interval_secs: u64,
        #[arg(long, default_value_t = false)]
        once: bool,
    },
}

#[derive(Debug, Deserialize)]
struct SeedConfig {
    seeds: Vec<Seed>,
}

#[derive(Debug, Deserialize)]
struct Seed {
    name: String,
    url: String,
    sitemap: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FrontierEntry {
    url: String,
    source: String,
    discovered_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FetchManifestEntry {
    url: String,
    storage_path: String,
    fetched_at: DateTime<Utc>,
    content_type: String,
}

#[derive(Debug, Serialize)]
struct IndexedDocument {
    id: String,
    title: String,
    url: String,
    display_url: String,
    snippet: String,
    body: String,
    language: String,
    last_crawled_at: DateTime<Utc>,
    suggest_terms: Vec<String>,
    site_authority: f32,
}

#[derive(Debug, Serialize)]
struct ClaimJobsRequest {
    max_jobs: usize,
}

#[derive(Debug, Deserialize)]
struct ClaimJobsResponse {
    crawler_id: String,
    frontier_depth: usize,
    jobs: Vec<CrawlJob>,
}

#[derive(Debug, Deserialize, Clone)]
struct CrawlJob {
    job_id: String,
    url: String,
    source: String,
    depth: u32,
    discovered_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct SubmitCrawlReportRequest {
    results: Vec<CrawlResultReport>,
}

#[derive(Debug, Serialize)]
struct CrawlResultReport {
    job_id: String,
    url: String,
    status_code: u16,
    fetched_at: DateTime<Utc>,
    title: Option<String>,
    snippet: Option<String>,
    body: Option<String>,
    language: Option<String>,
    discovered_urls: Vec<String>,
    site_authority: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct SubmitCrawlReportResponse {
    accepted_documents: usize,
    discovered_urls: usize,
    frontier_depth: usize,
    indexed_documents: usize,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Discover {
            config,
            output,
            limit_per_seed,
        } => discover(config, output, limit_per_seed).await?,
        Command::Fetch {
            frontier,
            output_dir,
            limit,
        } => fetch(frontier, output_dir, limit).await?,
        Command::BuildIndex { input_dir, output } => build_index(input_dir, output).await?,
        Command::Worker {
            server,
            crawler_id,
            crawler_key,
            max_jobs,
            poll_interval_secs,
            once,
        } => {
            let config = WorkerConfig {
                server,
                crawler_id,
                crawler_key,
                max_jobs,
                poll_interval_secs,
                once,
            };
            run_worker(config).await?;
        }
    }

    Ok(())
}

async fn discover(
    config_path: PathBuf,
    output: PathBuf,
    limit_per_seed: usize,
) -> anyhow::Result<()> {
    let raw = tokio::fs::read_to_string(config_path).await?;
    let config: SeedConfig = serde_yaml::from_str(&raw)?;
    let client = reqwest::Client::builder()
        .user_agent("FindVerseBot/0.1 (+https://example.com/findverse)")
        .build()?;
    let mut discovered = Vec::new();
    let mut seen = BTreeSet::new();

    for seed in config.seeds {
        let mut seed_urls = Vec::new();
        seed_urls.push(seed.url.clone());

        if let Some(sitemap) = seed.sitemap.as_ref() {
            match client.get(sitemap).send().await {
                Ok(response) if response.status().is_success() => {
                    let body = response.text().await.unwrap_or_default();
                    seed_urls.extend(extract_sitemap_urls(&body).into_iter().take(limit_per_seed));
                }
                Ok(response) => {
                    warn!("sitemap fetch for {} returned {}", seed.name, response.status())
                }
                Err(error) => warn!("failed to fetch sitemap for {}: {error}", seed.name),
            }
        }

        match client.get(&seed.url).send().await {
            Ok(response) if response.status().is_success() => {
                let body = response.text().await.unwrap_or_default();
                seed_urls.extend(extract_links(&seed.url, &body).into_iter().take(limit_per_seed));
            }
            Ok(response) => warn!("seed page fetch for {} returned {}", seed.name, response.status()),
            Err(error) => warn!("failed to fetch seed page for {}: {error}", seed.name),
        }

        for url in seed_urls {
            if seen.insert(url.clone()) {
                discovered.push(FrontierEntry {
                    url,
                    source: seed.name.clone(),
                    discovered_at: Utc::now(),
                });
            }
        }
    }

    if let Some(parent) = output.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let raw = discovered
        .into_iter()
        .map(|entry| serde_json::to_string(&entry))
        .collect::<Result<Vec<_>, _>>()?
        .join("\n");
    tokio::fs::write(output, raw).await?;
    info!("wrote frontier entries");
    Ok(())
}

async fn fetch(frontier: PathBuf, output_dir: PathBuf, limit: usize) -> anyhow::Result<()> {
    let raw = tokio::fs::read_to_string(frontier).await?;
    tokio::fs::create_dir_all(&output_dir).await?;
    let client = reqwest::Client::builder()
        .user_agent("FindVerseBot/0.1 (+https://example.com/findverse)")
        .build()?;
    let mut manifest = Vec::new();

    for line in raw.lines().take(limit) {
        let entry: FrontierEntry = serde_json::from_str(line)?;
        let response = match client.get(&entry.url).send().await {
            Ok(response) => response,
            Err(error) => {
                warn!("failed to fetch {}: {error}", entry.url);
                continue;
            }
        };

        if !response.status().is_success() {
            warn!("{} returned {}", entry.url, response.status());
            continue;
        }

        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();
        if !content_type.contains("text/html") {
            continue;
        }

        let body = response.text().await.unwrap_or_default();
        let hash = hex_hash(&entry.url);
        let filename = format!("{hash}.html");
        let output_path = output_dir.join(&filename);
        tokio::fs::write(&output_path, body).await?;

        manifest.push(FetchManifestEntry {
            url: entry.url,
            storage_path: filename,
            fetched_at: Utc::now(),
            content_type,
        });
    }

    let manifest_path = output_dir.join("manifest.jsonl");
    let raw = manifest
        .into_iter()
        .map(|entry| serde_json::to_string(&entry))
        .collect::<Result<Vec<_>, _>>()?
        .join("\n");
    tokio::fs::write(manifest_path, raw).await?;
    Ok(())
}

async fn build_index(input_dir: PathBuf, output: PathBuf) -> anyhow::Result<()> {
    let manifest_path = input_dir.join("manifest.jsonl");
    let raw = tokio::fs::read_to_string(&manifest_path)
        .await
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let mut documents = Vec::new();

    for line in raw.lines() {
        let manifest_entry: FetchManifestEntry = serde_json::from_str(line)?;
        let html_path = input_dir.join(&manifest_entry.storage_path);
        let html = tokio::fs::read_to_string(&html_path).await?;
        if let Some(document) = build_document(&manifest_entry, &html) {
            documents.push(document);
        }
    }

    if let Some(parent) = output.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let serialized = serde_json::to_string_pretty(&documents)?;
    tokio::fs::write(output, serialized).await?;
    Ok(())
}

#[derive(Debug, Clone)]
struct WorkerConfig {
    server: String,
    crawler_id: String,
    crawler_key: String,
    max_jobs: usize,
    poll_interval_secs: u64,
    once: bool,
}

async fn run_worker(config: WorkerConfig) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .user_agent("FindVerseCrawlerWorker/0.1")
        .build()?;

    loop {
        let claim = claim_jobs(&client, &config).await?;
        if claim.jobs.is_empty() {
            info!(
                "crawler {} received no jobs, frontier depth {}",
                claim.crawler_id, claim.frontier_depth
            );
            if config.once {
                break;
            }
            sleep(Duration::from_secs(config.poll_interval_secs)).await;
            continue;
        }

        let mut results = Vec::new();
        for job in claim.jobs {
            info!(
                "crawler {} processing {} from {} depth {} discovered {}",
                config.crawler_id, job.url, job.source, job.depth, job.discovered_at
            );
            results.push(process_job(&client, &job).await);
        }

        let report = submit_report(&client, &config, results).await?;
        info!(
            "crawler {} accepted {} documents, discovered {} urls, frontier depth {}, indexed documents {}",
            config.crawler_id,
            report.accepted_documents,
            report.discovered_urls,
            report.frontier_depth,
            report.indexed_documents
        );

        if config.once {
            break;
        }
    }

    Ok(())
}

async fn claim_jobs(client: &reqwest::Client, config: &WorkerConfig) -> anyhow::Result<ClaimJobsResponse> {
    let response = client
        .post(format!("{}/internal/crawlers/claim", config.server.trim_end_matches('/')))
        .header("x-crawler-id", &config.crawler_id)
        .bearer_auth(&config.crawler_key)
        .json(&ClaimJobsRequest {
            max_jobs: config.max_jobs,
        })
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("claim failed with status {}", response.status());
    }

    Ok(response.json().await?)
}

async fn submit_report(
    client: &reqwest::Client,
    config: &WorkerConfig,
    results: Vec<CrawlResultReport>,
) -> anyhow::Result<SubmitCrawlReportResponse> {
    let response = client
        .post(format!("{}/internal/crawlers/report", config.server.trim_end_matches('/')))
        .header("x-crawler-id", &config.crawler_id)
        .bearer_auth(&config.crawler_key)
        .json(&SubmitCrawlReportRequest { results })
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("report failed with status {}", response.status());
    }

    Ok(response.json().await?)
}

async fn process_job(client: &reqwest::Client, job: &CrawlJob) -> CrawlResultReport {
    let fetched_at = Utc::now();
    let response = match client.get(&job.url).send().await {
        Ok(response) => response,
        Err(error) => {
            warn!("failed to fetch {}: {error}", job.url);
            return CrawlResultReport {
                job_id: job.job_id.clone(),
                url: job.url.clone(),
                status_code: 599,
                fetched_at,
                title: None,
                snippet: None,
                body: None,
                language: None,
                discovered_urls: Vec::new(),
                site_authority: None,
            };
        }
    };

    let status_code = response.status().as_u16();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    if !content_type.contains("text/html") {
        return CrawlResultReport {
            job_id: job.job_id.clone(),
            url: job.url.clone(),
            status_code,
            fetched_at,
            title: None,
            snippet: None,
            body: None,
            language: None,
            discovered_urls: Vec::new(),
            site_authority: None,
        };
    }

    let body = response.text().await.unwrap_or_default();
    let parsed = parse_html_document(&job.url, &body);
    CrawlResultReport {
        job_id: job.job_id.clone(),
        url: job.url.clone(),
        status_code,
        fetched_at,
        title: parsed.title,
        snippet: parsed.snippet,
        body: parsed.body,
        language: Some("unknown".to_string()),
        discovered_urls: parsed.discovered_urls,
        site_authority: Some(0.5),
    }
}

struct ParsedHtml {
    title: Option<String>,
    snippet: Option<String>,
    body: Option<String>,
    discovered_urls: Vec<String>,
}

fn parse_html_document(url: &str, html: &str) -> ParsedHtml {
    let document = Html::parse_document(html);
    let title_selector = Selector::parse("title").ok();
    let meta_selector = Selector::parse("meta[name='description']").ok();
    let body_selector = Selector::parse("body").ok();

    let title = title_selector
        .as_ref()
        .and_then(|selector| document.select(selector).next())
        .map(|node| node.text().collect::<String>())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let snippet = meta_selector
        .as_ref()
        .and_then(|selector| document.select(selector).next())
        .and_then(|node| node.value().attr("content"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);

    let body = body_selector
        .as_ref()
        .and_then(|selector| document.select(selector).next())
        .map(|node| node.text().collect::<Vec<_>>().join(" "))
        .map(|value| value.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(4_000).collect());

    ParsedHtml {
        title,
        snippet,
        body,
        discovered_urls: extract_links(url, html),
    }
}

fn build_document(entry: &FetchManifestEntry, html: &str) -> Option<IndexedDocument> {
    let parsed_url = Url::parse(&entry.url).ok()?;
    let document = Html::parse_document(html);
    let title_selector = Selector::parse("title").ok()?;
    let meta_selector = Selector::parse("meta[name='description']").ok()?;
    let body_selector = Selector::parse("body").ok()?;

    let title = document
        .select(&title_selector)
        .next()
        .map(|node| node.text().collect::<String>())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| parsed_url.as_str().to_string());

    let snippet = document
        .select(&meta_selector)
        .next()
        .and_then(|node| node.value().attr("content"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| title.clone());

    let body = document
        .select(&body_selector)
        .next()
        .map(|node| node.text().collect::<Vec<_>>().join(" "))
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if body.is_empty() {
        return None;
    }

    Some(IndexedDocument {
        id: hex_hash(&entry.url),
        title: title.trim().to_string(),
        url: entry.url.clone(),
        display_url: display_url(&entry.url),
        snippet: snippet.chars().take(220).collect(),
        body: body.chars().take(4_000).collect(),
        language: "unknown".to_string(),
        last_crawled_at: entry.fetched_at,
        suggest_terms: derive_terms(&title, &body),
        site_authority: 0.5,
    })
}

fn extract_sitemap_urls(xml: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let mut rest = xml;
    while let Some(start) = rest.find("<loc>") {
        let from = &rest[start + 5..];
        if let Some(end) = from.find("</loc>") {
            urls.push(from[..end].trim().to_string());
            rest = &from[end + 6..];
        } else {
            break;
        }
    }
    urls
}

fn extract_links(base: &str, html: &str) -> Vec<String> {
    let base_url = match Url::parse(base) {
        Ok(url) => url,
        Err(_) => return Vec::new(),
    };
    let selector = match Selector::parse("a[href]") {
        Ok(selector) => selector,
        Err(_) => return Vec::new(),
    };

    let mut links = BTreeSet::new();
    let document = Html::parse_document(html);
    for anchor in document.select(&selector) {
        let Some(raw_href) = anchor.value().attr("href") else {
            continue;
        };

        let Ok(resolved) = base_url.join(raw_href) else {
            continue;
        };

        if matches!(resolved.scheme(), "http" | "https") {
            resolved.fragment().map(|_| ());
            let mut normalized = resolved;
            normalized.set_fragment(None);
            links.insert(normalized.to_string());
        }
    }

    links.into_iter().collect()
}

fn derive_terms(title: &str, body: &str) -> Vec<String> {
    let mut terms = BTreeSet::new();
    for source in [title, body] {
        for token in source
            .split(|ch: char| !ch.is_alphanumeric())
            .map(str::trim)
            .filter(|token| token.len() >= 4)
        {
            terms.insert(token.to_lowercase());
            if terms.len() >= 12 {
                return terms.into_iter().collect();
            }
        }
    }
    terms.into_iter().collect()
}

fn display_url(input: &str) -> String {
    Url::parse(input)
        .ok()
        .and_then(|url| {
            let host = url.host_str()?.to_string();
            let path = url.path().trim_end_matches('/').to_string();
            Some(if path.is_empty() {
                host
            } else {
                format!("{host}{path}")
            })
        })
        .unwrap_or_else(|| input.to_string())
}

fn hex_hash(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{display_url, extract_links, extract_sitemap_urls, parse_html_document};

    #[test]
    fn sitemap_parser_collects_urls() {
        let xml = "<urlset><url><loc>https://example.com/a</loc></url><url><loc>https://example.com/b</loc></url></urlset>";
        assert_eq!(
            extract_sitemap_urls(xml),
            vec![
                "https://example.com/a".to_string(),
                "https://example.com/b".to_string()
            ]
        );
    }

    #[test]
    fn link_extractor_keeps_http_links() {
        let html = r#"<a href="/docs">Docs</a><a href="https://example.com/blog">Blog</a><a href="mailto:test@example.com">Skip</a>"#;
        assert_eq!(
            extract_links("https://example.com/", html),
            vec![
                "https://example.com/blog".to_string(),
                "https://example.com/docs".to_string()
            ]
        );
    }

    #[test]
    fn display_url_strips_scheme() {
        assert_eq!(
            display_url("https://example.com/a/b/"),
            "example.com/a/b".to_string()
        );
    }

    #[test]
    fn html_parser_extracts_fields() {
        let parsed = parse_html_document(
            "https://example.com",
            "<html><head><title>FindVerse</title><meta name='description' content='Search docs'></head><body>Hello crawler <a href='/docs'>Docs</a></body></html>",
        );

        assert_eq!(parsed.title, Some("FindVerse".to_string()));
        assert_eq!(parsed.snippet, Some("Search docs".to_string()));
        assert!(parsed.body.is_some());
        assert_eq!(parsed.discovered_urls, vec!["https://example.com/docs".to_string()]);
    }
}

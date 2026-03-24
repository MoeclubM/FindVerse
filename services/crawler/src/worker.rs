use std::{collections::BTreeSet, sync::Arc, time::Duration};

use chrono::Utc;
use futures::stream::{self, StreamExt};
use reqwest::header::CONTENT_TYPE;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::extract::{detect_language, filter_urls_by_domain, parse_html_document};
use crate::fetch::{
    WorkerState, check_robots_allowed, fetch_with_retry, random_user_agent, rate_limit_domain,
};
use crate::js_render::{needs_js_rendering, render_with_js};
use crate::models::{
    ClaimJobsRequest, ClaimJobsResponse, CrawlJob, CrawlResultReport, SubmitCrawlReportRequest,
    SubmitCrawlReportResponse, WorkerConfig,
};
use crate::sitemap::fetch_and_parse_sitemap;
use crate::url_normalize::normalize_url_advanced;

// ---------------------------------------------------------------------------
// Registration helpers
// ---------------------------------------------------------------------------
pub async fn crawler_join(
    client: &reqwest::Client,
    server: &str,
    join_key: &str,
) -> anyhow::Result<crate::models::JoinCrawlerResponse> {
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    let name = format!("worker-{hostname}");

    let response = client
        .post(format!(
            "{}/internal/crawlers/join",
            server.trim_end_matches('/')
        ))
        .json(&serde_json::json!({ "join_key": join_key, "name": name }))
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("join failed with status {}", response.status());
    }

    Ok(response.json().await?)
}

// ---------------------------------------------------------------------------
// Worker loop
// ---------------------------------------------------------------------------
pub async fn run_worker(config: WorkerConfig, proxy: Option<String>) -> anyhow::Result<()> {
    // Build the page-fetching client with cookie jar and rotating UA
    let mut fetch_client_builder = reqwest::Client::builder()
        .user_agent(random_user_agent())
        .cookie_store(true)
        .timeout(Duration::from_secs(30));

    if let Some(ref proxy_url) = proxy {
        fetch_client_builder = fetch_client_builder.proxy(reqwest::Proxy::all(proxy_url)?);
    }

    let fetch_client = fetch_client_builder.build()?;

    // Build the API client (for claim/report)
    let api_client = reqwest::Client::builder()
        .user_agent("FindVerseCrawlerWorker/0.1")
        .timeout(Duration::from_secs(30))
        .build()?;

    let state = Arc::new(Mutex::new(WorkerState::new()));

    loop {
        let claim = claim_jobs(&api_client, &config).await?;
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

        let results: Vec<CrawlResultReport> = stream::iter(claim.jobs)
            .map(|job| {
                let fetch_client = fetch_client.clone();
                let state = Arc::clone(&state);
                let allowed_domains = config.allowed_domains.clone();
                async move {
                    info!(
                        "processing {} from {} depth {}/{} attempt {} discovered {}",
                        job.url,
                        job.source,
                        job.depth,
                        job.max_depth,
                        job.attempt_count,
                        job.discovered_at
                    );
                    process_job(&fetch_client, &job, &state, &allowed_domains).await
                }
            })
            .buffer_unordered(config.concurrency)
            .collect()
            .await;

        let report = submit_report(&api_client, &config, results).await?;
        info!(
            "crawler {} accepted {} documents (duplicates {}, skipped {}), discovered {} urls, frontier depth {}, indexed documents {}",
            config.crawler_id,
            report.accepted_documents,
            report.duplicate_documents,
            report.skipped_documents,
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

async fn claim_jobs(
    client: &reqwest::Client,
    config: &WorkerConfig,
) -> anyhow::Result<ClaimJobsResponse> {
    let response = client
        .post(format!(
            "{}/internal/crawlers/claim",
            config.server.trim_end_matches('/')
        ))
        .header("x-crawler-id", &config.crawler_id)
        .bearer_auth(&config.auth_token)
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
        .post(format!(
            "{}/internal/crawlers/report",
            config.server.trim_end_matches('/')
        ))
        .header("x-crawler-id", &config.crawler_id)
        .bearer_auth(&config.auth_token)
        .json(&SubmitCrawlReportRequest { results })
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("report failed with status {}", response.status());
    }

    Ok(response.json().await?)
}

// ---------------------------------------------------------------------------
// Process a single crawl job
// ---------------------------------------------------------------------------
async fn process_job(
    client: &reqwest::Client,
    job: &CrawlJob,
    state: &Arc<Mutex<WorkerState>>,
    allowed_domains: &[String],
) -> CrawlResultReport {
    let fetched_at = Utc::now();

    // Check robots.txt
    if !check_robots_allowed(client, state, &job.url).await {
        warn!("robots.txt disallows {}", job.url);
        return CrawlResultReport {
            job_id: job.job_id.clone(),
            url: job.url.clone(),
            status_code: 599,
            fetched_at,
            final_url: Some(job.url.clone()),
            content_type: None,
            title: None,
            snippet: None,
            body: None,
            language: None,
            discovered_urls: Vec::new(),
            site_authority: None,
            retryable: Some(false),
            error_kind: Some("robots".to_string()),
            error_message: Some("blocked by robots.txt".to_string()),
        };
    }

    // Per-domain rate limiting
    rate_limit_domain(state, &job.url).await;

    let response = match fetch_with_retry(client, &job.url).await {
        Ok(response) => response,
        Err(error) => {
            warn!("failed to fetch {}: {error}", job.url);
            return CrawlResultReport {
                job_id: job.job_id.clone(),
                url: job.url.clone(),
                status_code: 599,
                fetched_at,
                final_url: None,
                content_type: None,
                title: None,
                snippet: None,
                body: None,
                language: None,
                discovered_urls: Vec::new(),
                site_authority: None,
                retryable: Some(true),
                error_kind: Some("network_error".to_string()),
                error_message: Some(error.to_string()),
            };
        }
    };

    let status_code = response.status().as_u16();
    let final_url = normalize_url_advanced(response.url().as_ref())
        .unwrap_or_else(|| response.url().to_string());
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    if !(200..300).contains(&status_code) {
        return CrawlResultReport {
            job_id: job.job_id.clone(),
            url: job.url.clone(),
            status_code,
            fetched_at,
            final_url: Some(final_url),
            content_type: Some(content_type),
            title: None,
            snippet: None,
            body: None,
            language: None,
            discovered_urls: Vec::new(),
            site_authority: None,
            retryable: Some(status_code == 429 || status_code >= 500),
            error_kind: Some(http_failure_kind(status_code)),
            error_message: Some(format!("fetch returned status {status_code}")),
        };
    }

    if !content_type.contains("text/html") {
        return CrawlResultReport {
            job_id: job.job_id.clone(),
            url: job.url.clone(),
            status_code,
            fetched_at,
            final_url: Some(final_url),
            content_type: Some(content_type.clone()),
            title: None,
            snippet: None,
            body: None,
            language: None,
            discovered_urls: Vec::new(),
            site_authority: None,
            retryable: Some(false),
            error_kind: Some("unsupported_content_type".to_string()),
            error_message: Some(format!("unsupported content type {content_type}")),
        };
    }

    let body = response.text().await.unwrap_or_default();
    let mut parsed = parse_html_document(&final_url, &body);

    // Check if JS rendering is needed
    if needs_js_rendering(&body, parsed.body.as_deref().unwrap_or("")) {
        if let Ok(rendered_html) = render_with_js(&final_url).await {
            info!("re-parsing {} with JS rendering", final_url);
            parsed = parse_html_document(&final_url, &rendered_html);
        }
    }

    // Try fetching sitemap.xml for root URLs
    let mut all_discovered = parsed.discovered_urls;
    if let Ok(url) = url::Url::parse(&final_url) {
        if url.path() == "/" || url.path().is_empty() {
            if let Some(domain) = url.domain() {
                let sitemap_url = format!("{}://{}/sitemap.xml", url.scheme(), domain);
                if let Ok(sitemap_urls) = fetch_and_parse_sitemap(client, &sitemap_url).await {
                    if !sitemap_urls.is_empty() {
                        info!(
                            "discovered {} URLs from sitemap at {}",
                            sitemap_urls.len(),
                            sitemap_url
                        );
                        all_discovered.extend(sitemap_urls);
                    }
                }
            }
        }
    }

    // Filter discovered URLs by allowed domains
    let normalized_discovered = normalize_discovered_urls(all_discovered);
    let discovered_urls = if allowed_domains.is_empty() {
        normalized_discovered
    } else {
        filter_urls_by_domain(normalized_discovered, allowed_domains)
    };

    // Detect language from body text
    let language = parsed
        .body
        .as_deref()
        .and_then(detect_language)
        .or(Some("unknown".to_string()));

    let has_body = parsed
        .body
        .as_deref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);

    if !has_body {
        return CrawlResultReport {
            job_id: job.job_id.clone(),
            url: job.url.clone(),
            status_code,
            fetched_at,
            final_url: Some(final_url),
            content_type: Some(content_type),
            title: parsed.title,
            snippet: parsed.snippet,
            body: None,
            language,
            discovered_urls,
            site_authority: Some(0.5),
            retryable: Some(false),
            error_kind: Some("empty_document".to_string()),
            error_message: Some("parsed page had no indexable body".to_string()),
        };
    }

    CrawlResultReport {
        job_id: job.job_id.clone(),
        url: job.url.clone(),
        status_code,
        fetched_at,
        final_url: Some(final_url),
        content_type: Some(content_type),
        title: parsed.title,
        snippet: parsed.snippet,
        body: parsed.body,
        language,
        discovered_urls,
        site_authority: Some(0.5),
        retryable: Some(false),
        error_kind: None,
        error_message: None,
    }
}

fn normalize_discovered_urls(urls: Vec<String>) -> Vec<String> {
    let mut normalized = BTreeSet::new();
    for url in urls {
        if let Some(url) = normalize_url_advanced(&url) {
            normalized.insert(url);
        }
    }
    normalized.into_iter().collect()
}

fn http_failure_kind(status_code: u16) -> String {
    match status_code {
        401 | 403 => "blocked".to_string(),
        404 => "http_404".to_string(),
        429 => "http_429".to_string(),
        500..=599 => "http_5xx".to_string(),
        _ => format!("http_{status_code}"),
    }
}

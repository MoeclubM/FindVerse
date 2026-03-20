use std::{sync::Arc, time::Duration};

use chrono::Utc;
use futures::stream::{self, StreamExt};
use reqwest::header::CONTENT_TYPE;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::extract::{detect_language, filter_urls_by_domain, parse_html_document};
use crate::fetch::{
    check_robots_allowed, fetch_with_retry, random_user_agent, rate_limit_domain, WorkerState,
};
use crate::models::{
    ClaimJobsRequest, ClaimJobsResponse, CrawlJob, CrawlResultReport, SubmitCrawlReportRequest,
    SubmitCrawlReportResponse, WorkerConfig,
};

// ---------------------------------------------------------------------------
// Registration helpers
// ---------------------------------------------------------------------------
pub async fn crawler_hello(
    client: &reqwest::Client,
    server: &str,
    api_key: &str,
) -> anyhow::Result<crate::models::HelloCrawlerResponse> {
    let response = client
        .post(format!(
            "{}/internal/crawlers/hello",
            server.trim_end_matches('/')
        ))
        .bearer_auth(api_key)
        .json(&serde_json::json!({ "name": null }))
        .send()
        .await?;

    if !response.status().is_success() {
        anyhow::bail!("hello failed with status {}", response.status());
    }

    Ok(response.json().await?)
}

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
                        "processing {} from {} depth {}",
                        job.url, job.source, job.depth
                    );
                    process_job(&fetch_client, &job, &state, &allowed_domains).await
                }
            })
            .buffer_unordered(config.concurrency)
            .collect()
            .await;

        let report = submit_report(&api_client, &config, results).await?;
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
            title: None,
            snippet: Some("blocked by robots.txt".to_string()),
            body: None,
            language: None,
            discovered_urls: Vec::new(),
            site_authority: None,
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

    // Filter discovered URLs by allowed domains
    let discovered_urls = if allowed_domains.is_empty() {
        parsed.discovered_urls
    } else {
        filter_urls_by_domain(parsed.discovered_urls, allowed_domains)
    };

    // Detect language from body text
    let language = parsed
        .body
        .as_deref()
        .and_then(detect_language)
        .or(Some("unknown".to_string()));

    CrawlResultReport {
        job_id: job.job_id.clone(),
        url: job.url.clone(),
        status_code,
        fetched_at,
        title: parsed.title,
        snippet: parsed.snippet,
        body: parsed.body,
        language,
        discovered_urls,
        site_authority: Some(0.5),
    }
}

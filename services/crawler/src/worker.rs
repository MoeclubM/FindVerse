use std::{collections::BTreeSet, sync::Arc, time::Duration};

use chrono::Utc;
use futures::stream::{self, StreamExt};
use reqwest::header::CONTENT_TYPE;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::extract::{
    detect_language, filter_urls_by_domain, parse_html_document, parse_x_robots_tag_values,
};
use crate::fetch::{
    FINDVERSE_UA, FetchWorkflowError, WorkerState, fetch_with_retry, inspect_robots,
};
use crate::js_render::{needs_js_rendering, render_with_js};
use crate::llm_filter::evaluate_page;
use crate::models::{
    ClaimJobsRequest, ClaimJobsResponse, CrawlJob, CrawlResultReport, LlmFilterConfig,
    SubmitCrawlReportRequest, SubmitCrawlReportResponse, WorkerConfig,
};
use crate::sitemap::fetch_and_parse_sitemap;
use crate::url_normalize::normalize_url_advanced;

#[derive(Clone)]
struct NetworkClients {
    page: reqwest::Client,
    meta: reqwest::Client,
}

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

fn build_fetch_client(
    proxy_url: Option<&str>,
    timeout: Duration,
    follow_redirects: bool,
) -> anyhow::Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .user_agent(FINDVERSE_UA)
        .timeout(timeout);

    if !follow_redirects {
        builder = builder.redirect(reqwest::redirect::Policy::none());
    }
    if let Some(proxy_url) = proxy_url {
        builder = builder.proxy(reqwest::Proxy::all(proxy_url)?);
    }

    Ok(builder.build()?)
}

// ---------------------------------------------------------------------------
// Worker loop
// ---------------------------------------------------------------------------
pub async fn run_worker(config: WorkerConfig, proxy: Option<String>) -> anyhow::Result<()> {
    if config.stealth_ua {
        warn!("--stealth-ua is ignored; FindVerse now always crawls with the public bot identity");
    }

    let clearnet_clients = NetworkClients {
        page: build_fetch_client(proxy.as_deref(), Duration::from_secs(30), false)?,
        meta: build_fetch_client(proxy.as_deref(), Duration::from_secs(30), true)?,
    };
    let tor_clients: Option<NetworkClients> = config
        .tor_socks_url
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|socks_url| -> anyhow::Result<NetworkClients> {
            Ok(NetworkClients {
                page: build_fetch_client(Some(socks_url), Duration::from_secs(60), false)?,
                meta: build_fetch_client(Some(socks_url), Duration::from_secs(60), true)?,
            })
        })
        .transpose()?;

    // Build the API client (for claim/report)
    let api_client = reqwest::Client::builder()
        .user_agent("FindVerseCrawlerWorker/0.1")
        .timeout(Duration::from_secs(30))
        .build()?;
    let llm_client = if config
        .llm_filter
        .as_ref()
        .map(LlmFilterConfig::is_enabled)
        .unwrap_or(false)
    {
        let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(45));
        if let Some(ref proxy_url) = proxy {
            builder = builder.proxy(reqwest::Proxy::all(proxy_url)?);
        }
        Some(builder.build()?)
    } else {
        None
    };

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
                let clearnet_clients = clearnet_clients.clone();
                let tor_clients = tor_clients.clone();
                let llm_client = llm_client.clone();
                let state = Arc::clone(&state);
                let allowed_domains = config.allowed_domains.clone();
                let llm_filter = config.llm_filter.clone();
                async move {
                    info!(
                        "processing {} ({}) from {} depth {}/{} attempt {} discovered {}",
                        job.url,
                        job.origin_key,
                        job.source,
                        job.depth,
                        job.max_depth,
                        job.attempt_count,
                        job.discovered_at
                    );
                    let network = if matches!(job.network.as_str(), "clearnet" | "tor") {
                        job.network.clone()
                    } else if is_onion_url(&job.url) {
                        "tor".to_string()
                    } else {
                        "clearnet".to_string()
                    };
                    let effective_clients = if network == "tor" {
                        tor_clients.as_ref().unwrap_or(&clearnet_clients)
                    } else {
                        &clearnet_clients
                    };
                    process_job(
                        &effective_clients.page,
                        &effective_clients.meta,
                        llm_client.as_ref(),
                        llm_filter.as_ref(),
                        &job,
                        &state,
                        &allowed_domains,
                        network,
                    )
                    .await
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

fn is_onion_url(url: &str) -> bool {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.ends_with(".onion")))
        .unwrap_or(false)
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
    page_client: &reqwest::Client,
    meta_client: &reqwest::Client,
    llm_client: Option<&reqwest::Client>,
    llm_filter: Option<&LlmFilterConfig>,
    job: &CrawlJob,
    state: &Arc<Mutex<WorkerState>>,
    allowed_domains: &[String],
    network: String,
) -> CrawlResultReport {
    let fetched_at = Utc::now();
    let initial_robots = inspect_robots(meta_client, state, &job.url).await;
    if !initial_robots.allowed {
        warn!("robots.txt blocks {} ({})", job.url, initial_robots.status);
        return CrawlResultReport {
            status_code: 599,
            final_url: Some(job.url.clone()),
            retryable: Some(false),
            error_kind: Some("robots".to_string()),
            error_message: Some(format!("blocked by robots ({})", initial_robots.status)),
            applied_crawl_delay_secs: initial_robots.crawl_delay_secs,
            robots_status: Some(initial_robots.status),
            robots_sitemaps: initial_robots.sitemap_urls,
            ..base_report(job, fetched_at, &network)
        };
    }

    let fetch = match fetch_with_retry(
        page_client,
        meta_client,
        state,
        &job.url,
        job.etag.as_deref(),
        job.last_modified.as_deref(),
    )
    .await
    {
        Ok(fetch) => fetch,
        Err(FetchWorkflowError::BlockedByRobots { url, status }) => {
            warn!("redirect chain hit robots block for {} ({})", url, status);
            return CrawlResultReport {
                status_code: 599,
                final_url: Some(url),
                retryable: Some(false),
                error_kind: Some("robots".to_string()),
                error_message: Some(format!("blocked by robots ({status})")),
                robots_status: Some(status),
                ..base_report(job, fetched_at, &network)
            };
        }
        Err(FetchWorkflowError::TooManyRedirects { chain }) => {
            warn!("too many redirects while fetching {}: {:?}", job.url, chain);
            return CrawlResultReport {
                status_code: 310,
                final_url: chain.last().cloned().or_else(|| Some(job.url.clone())),
                redirect_chain: chain,
                retryable: Some(false),
                error_kind: Some("redirect_loop".to_string()),
                error_message: Some("too many redirects".to_string()),
                ..base_report(job, fetched_at, &network)
            };
        }
        Err(FetchWorkflowError::Request(error)) => {
            warn!("failed to fetch {}: {error}", job.url);
            return CrawlResultReport {
                status_code: 599,
                retryable: Some(true),
                error_kind: Some("network_error".to_string()),
                error_message: Some(error.to_string()),
                ..base_report(job, fetched_at, &network)
            };
        }
    };

    let status_code = fetch.response.status().as_u16();
    let redirect_chain = fetch.redirect_chain;
    let retry_after_secs = fetch.retry_after_secs;
    let final_url = normalize_url_advanced(fetch.response.url().as_ref())
        .unwrap_or_else(|| fetch.response.url().to_string());
    let final_robots = inspect_robots(meta_client, state, &final_url).await;
    let etag = fetch
        .response
        .headers()
        .get("etag")
        .and_then(|value| value.to_str().ok())
        .map(String::from);
    let last_modified_header = fetch
        .response
        .headers()
        .get("last-modified")
        .and_then(|value| value.to_str().ok())
        .map(String::from);
    let content_type = fetch
        .response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    let x_robots_tag_values: Vec<String> = fetch
        .response
        .headers()
        .get_all("x-robots-tag")
        .iter()
        .filter_map(|value| value.to_str().ok().map(ToString::to_string))
        .collect();
    let header_robots_directives =
        parse_x_robots_tag_values(x_robots_tag_values.iter().map(String::as_str));

    if status_code == 304 {
        return CrawlResultReport {
            status_code: 304,
            final_url: Some(final_url),
            redirect_chain,
            retryable: Some(false),
            http_etag: job.etag.clone(),
            http_last_modified: job.last_modified.clone(),
            applied_crawl_delay_secs: final_robots.crawl_delay_secs,
            retry_after_secs,
            robots_status: Some(final_robots.status),
            robots_sitemaps: final_robots.sitemap_urls,
            ..base_report(job, fetched_at, &network)
        };
    }

    if !(200..300).contains(&status_code) {
        return CrawlResultReport {
            status_code,
            final_url: Some(final_url),
            redirect_chain,
            content_type: Some(content_type),
            retryable: Some(status_code == 429 || status_code >= 500),
            error_kind: Some(http_failure_kind(status_code)),
            error_message: Some(format!("fetch returned status {status_code}")),
            retry_after_secs,
            robots_status: Some(final_robots.status),
            robots_sitemaps: final_robots.sitemap_urls,
            ..base_report(job, fetched_at, &network)
        };
    }

    if !content_type.contains("text/html") {
        return CrawlResultReport {
            status_code,
            final_url: Some(final_url),
            redirect_chain,
            content_type: Some(content_type.clone()),
            retryable: Some(false),
            error_kind: Some("unsupported_content_type".to_string()),
            error_message: Some(format!("unsupported content type {content_type}")),
            applied_crawl_delay_secs: final_robots.crawl_delay_secs,
            retry_after_secs,
            robots_status: Some(final_robots.status),
            robots_sitemaps: final_robots.sitemap_urls,
            ..base_report(job, fetched_at, &network)
        };
    }

    let body = fetch.response.text().await.unwrap_or_default();
    let mut parsed = parse_html_document(&final_url, &body);
    if needs_js_rendering(&body, parsed.body.as_deref().unwrap_or("")) {
        match render_with_js(&final_url).await {
            Ok(rendered_html) => {
                info!("re-parsing {} with JS rendering", final_url);
                parsed = parse_html_document(&final_url, &rendered_html);
            }
            Err(error) => {
                warn!(
                    ?error,
                    url = %final_url,
                    "js rendering failed; falling back to static html fetch"
                );
            }
        }
    }

    let mut robots_directives = header_robots_directives;
    robots_directives.merge(parsed.robots_directives);

    let mut all_discovered = if robots_directives.nofollow {
        Vec::new()
    } else {
        parsed.discovered_urls.clone()
    };
    if !robots_directives.nofollow {
        if let Ok(url) = url::Url::parse(&final_url) {
            if url.path() == "/" || url.path().is_empty() {
                let sitemap_sources = if final_robots.sitemap_urls.is_empty() {
                    findverse_common::origin_key(&final_url)
                        .map(|origin| vec![format!("{origin}/sitemap.xml")])
                        .unwrap_or_default()
                } else {
                    final_robots.sitemap_urls.clone()
                };

                for sitemap_url in sitemap_sources {
                    if let Ok(sitemap_entries) =
                        fetch_and_parse_sitemap(meta_client, &sitemap_url).await
                    {
                        let sitemap_urls: Vec<String> = sitemap_entries
                            .iter()
                            .map(|entry| entry.url.clone())
                            .collect();
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
    }

    let normalized_discovered = normalize_discovered_urls(all_discovered);
    let discovered_urls = if allowed_domains.is_empty() {
        normalized_discovered
    } else {
        filter_urls_by_domain(normalized_discovered, allowed_domains)
    };
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
    let canonical_hint = parsed.canonical_url.clone();
    let canonical_source = canonical_hint.as_ref().map(|_| "rel_canonical".to_string());

    if robots_directives.noindex {
        return CrawlResultReport {
            status_code,
            final_url: Some(final_url),
            redirect_chain,
            content_type: Some(content_type),
            title: parsed.title,
            snippet: parsed.snippet,
            body: None,
            canonical_hint,
            canonical_source,
            language,
            discovered_urls,
            site_authority: Some(0.5),
            retryable: Some(false),
            error_kind: Some("page_noindex".to_string()),
            error_message: Some("page requested noindex via robots directives".to_string()),
            http_etag: etag.clone(),
            http_last_modified: last_modified_header.clone(),
            applied_crawl_delay_secs: final_robots.crawl_delay_secs,
            retry_after_secs,
            robots_status: Some(final_robots.status),
            robots_sitemaps: final_robots.sitemap_urls,
            ..base_report(job, fetched_at, &network)
        };
    }

    if !has_body {
        return CrawlResultReport {
            status_code,
            final_url: Some(final_url),
            redirect_chain,
            content_type: Some(content_type),
            title: parsed.title,
            snippet: parsed.snippet,
            body: None,
            canonical_hint,
            canonical_source,
            language,
            discovered_urls,
            site_authority: Some(0.5),
            retryable: Some(false),
            error_kind: Some("empty_document".to_string()),
            error_message: Some("parsed page had no indexable body".to_string()),
            http_etag: etag.clone(),
            http_last_modified: last_modified_header.clone(),
            applied_crawl_delay_secs: final_robots.crawl_delay_secs,
            retry_after_secs,
            robots_status: Some(final_robots.status),
            robots_sitemaps: final_robots.sitemap_urls,
            ..base_report(job, fetched_at, &network)
        };
    }

    let llm_decision = match (llm_client, llm_filter, parsed.body.as_deref()) {
        (Some(llm_client), Some(llm_filter), Some(body)) => match evaluate_page(
            llm_client,
            llm_filter,
            &final_url,
            parsed.title.as_deref(),
            parsed.snippet.as_deref(),
            body,
        )
        .await
        {
            Ok(decision) => Some(decision),
            Err(error) => {
                warn!(?error, url = %final_url, "llm page filter failed; falling back to default indexing");
                None
            }
        },
        _ => None,
    };

    let discovered_urls = match llm_decision.as_ref() {
        Some(decision) if !decision.should_discover => Vec::new(),
        _ => discovered_urls,
    };
    let body = match llm_decision.as_ref() {
        Some(decision) if !decision.should_index => None,
        _ => parsed.body,
    };

    CrawlResultReport {
        status_code,
        final_url: Some(final_url),
        redirect_chain,
        content_type: Some(content_type),
        title: parsed.title,
        snippet: parsed.snippet,
        body,
        canonical_hint,
        canonical_source,
        language,
        discovered_urls,
        site_authority: Some(0.5),
        llm_should_index: llm_decision.as_ref().map(|decision| decision.should_index),
        llm_should_discover: llm_decision
            .as_ref()
            .map(|decision| decision.should_discover),
        llm_relevance_score: llm_decision
            .as_ref()
            .map(|decision| decision.relevance_score),
        llm_reason: llm_decision
            .as_ref()
            .map(|decision| decision.reason.clone()),
        retryable: Some(false),
        http_etag: etag,
        http_last_modified: last_modified_header,
        applied_crawl_delay_secs: final_robots.crawl_delay_secs,
        retry_after_secs,
        robots_status: Some(final_robots.status),
        robots_sitemaps: final_robots.sitemap_urls,
        ..base_report(job, fetched_at, &network)
    }
}

fn base_report(
    job: &CrawlJob,
    fetched_at: chrono::DateTime<Utc>,
    network: &str,
) -> CrawlResultReport {
    CrawlResultReport {
        job_id: job.job_id.clone(),
        url: job.url.clone(),
        status_code: 0,
        fetched_at,
        final_url: None,
        redirect_chain: Vec::new(),
        content_type: None,
        title: None,
        snippet: None,
        body: None,
        canonical_hint: None,
        canonical_source: None,
        language: None,
        discovered_urls: Vec::new(),
        site_authority: None,
        llm_should_index: None,
        llm_should_discover: None,
        llm_relevance_score: None,
        llm_reason: None,
        retryable: None,
        error_kind: None,
        error_message: None,
        network: network.to_string(),
        http_etag: None,
        http_last_modified: None,
        applied_crawl_delay_secs: None,
        retry_after_secs: None,
        robots_status: None,
        robots_sitemaps: Vec::new(),
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

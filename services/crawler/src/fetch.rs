use std::{
    collections::{BTreeSet, HashMap},
    fmt,
    sync::Arc,
    time::Duration,
};

use findverse_common::origin_key;
use rand::RngExt;
use texting_robots::Robot;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::warn;
use url::Url;

pub const FINDVERSE_UA: &str = "FindVerseCrawler/0.1 (+https://findverse.org/bot)";

pub struct WorkerState {
    pub robots_cache: HashMap<String, CachedRobot>,
    pub origin_last_request: HashMap<String, tokio::time::Instant>,
}

pub enum RobotStatus {
    Fetched {
        robot: Robot,
        crawl_delay_secs: Option<u64>,
        sitemap_urls: Vec<String>,
    },
    Unrestricted {
        crawl_delay_secs: Option<u64>,
        sitemap_urls: Vec<String>,
    },
    TemporarilyUnavailable,
}

pub struct CachedRobot {
    pub status: RobotStatus,
    pub cached_at: tokio::time::Instant,
}

#[derive(Debug, Clone)]
pub struct RobotsSnapshot {
    pub allowed: bool,
    pub status: String,
    pub crawl_delay_secs: Option<u64>,
    pub sitemap_urls: Vec<String>,
}

pub struct FetchedResponse {
    pub response: reqwest::Response,
    pub redirect_chain: Vec<String>,
    pub retry_after_secs: Option<u64>,
}

#[derive(Debug)]
pub enum FetchWorkflowError {
    BlockedByRobots { url: String, status: String },
    TooManyRedirects { chain: Vec<String> },
    Request(reqwest::Error),
}

impl fmt::Display for FetchWorkflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BlockedByRobots { url, status } => {
                write!(f, "robots blocked {url} ({status})")
            }
            Self::TooManyRedirects { chain } => {
                write!(f, "too many redirects: {}", chain.join(" -> "))
            }
            Self::Request(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for FetchWorkflowError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Request(error) => Some(error),
            _ => None,
        }
    }
}

impl WorkerState {
    pub fn new() -> Self {
        Self {
            robots_cache: HashMap::new(),
            origin_last_request: HashMap::new(),
        }
    }
}

#[allow(dead_code)]
pub async fn check_robots_allowed(
    client: &reqwest::Client,
    state: &Arc<Mutex<WorkerState>>,
    url: &str,
) -> bool {
    inspect_robots(client, state, url).await.allowed
}

pub async fn inspect_robots(
    client: &reqwest::Client,
    state: &Arc<Mutex<WorkerState>>,
    url: &str,
) -> RobotsSnapshot {
    let Some(origin) = origin_key(url) else {
        return RobotsSnapshot {
            allowed: true,
            status: "unrestricted".to_string(),
            crawl_delay_secs: None,
            sitemap_urls: Vec::new(),
        };
    };

    let mut state_guard = state.lock().await;
    let now = tokio::time::Instant::now();
    let had_cached_rules = state_guard
        .robots_cache
        .get(&origin)
        .map(|cached| !matches!(cached.status, RobotStatus::TemporarilyUnavailable))
        .unwrap_or(false);
    let needs_refresh = state_guard
        .robots_cache
        .get(&origin)
        .map(|cached| {
            let ttl = match &cached.status {
                RobotStatus::Fetched { .. } | RobotStatus::Unrestricted { .. } => {
                    Duration::from_secs(3600)
                }
                RobotStatus::TemporarilyUnavailable => Duration::from_secs(300),
            };
            cached.cached_at.elapsed() > ttl
        })
        .unwrap_or(true);

    if needs_refresh {
        drop(state_guard);

        let robots_url = format!("{origin}/robots.txt");
        let fetched_status = match client.get(&robots_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let body = resp.bytes().await.unwrap_or_default();
                let crawl_delay_secs = extract_crawl_delay(&body);
                let sitemap_urls = extract_sitemap_urls_from_robots(&body);
                match Robot::new("FindVerseCrawler", &body) {
                    Ok(robot) => Some(RobotStatus::Fetched {
                        robot,
                        crawl_delay_secs,
                        sitemap_urls,
                    }),
                    Err(_) => Some(RobotStatus::Unrestricted {
                        crawl_delay_secs,
                        sitemap_urls,
                    }),
                }
            }
            Ok(resp) if resp.status().is_client_error() => Some(RobotStatus::Unrestricted {
                crawl_delay_secs: None,
                sitemap_urls: Vec::new(),
            }),
            Ok(resp) => {
                warn!(
                    status = %resp.status(),
                    robots_url = %robots_url,
                    "robots fetch did not succeed; defaulting to unrestricted when no cached rules exist"
                );
                fallback_robot_status(had_cached_rules)
            }
            Err(error) => {
                warn!(
                    ?error,
                    robots_url = %robots_url,
                    "robots fetch failed; defaulting to unrestricted when no cached rules exist"
                );
                fallback_robot_status(had_cached_rules)
            }
        };

        state_guard = state.lock().await;
        match fetched_status {
            Some(status) => {
                state_guard.robots_cache.insert(
                    origin.clone(),
                    CachedRobot {
                        status,
                        cached_at: now,
                    },
                );
            }
            None if had_cached_rules => {
                if let Some(cached) = state_guard.robots_cache.get_mut(&origin) {
                    cached.cached_at = now;
                }
            }
            None => {
                state_guard.robots_cache.insert(
                    origin.clone(),
                    CachedRobot {
                        status: RobotStatus::TemporarilyUnavailable,
                        cached_at: now,
                    },
                );
            }
        }
    }

    state_guard
        .robots_cache
        .get(&origin)
        .map(|cached| robots_snapshot(url, &cached.status))
        .unwrap_or(RobotsSnapshot {
            allowed: true,
            status: "unrestricted".to_string(),
            crawl_delay_secs: None,
            sitemap_urls: Vec::new(),
        })
}

fn fallback_robot_status(had_cached_rules: bool) -> Option<RobotStatus> {
    if had_cached_rules {
        None
    } else {
        Some(RobotStatus::Unrestricted {
            crawl_delay_secs: None,
            sitemap_urls: Vec::new(),
        })
    }
}

fn robots_snapshot(url: &str, status: &RobotStatus) -> RobotsSnapshot {
    match status {
        RobotStatus::Fetched {
            robot,
            crawl_delay_secs,
            sitemap_urls,
        } => RobotsSnapshot {
            allowed: robot.allowed(url),
            status: "fetched".to_string(),
            crawl_delay_secs: *crawl_delay_secs,
            sitemap_urls: sitemap_urls.clone(),
        },
        RobotStatus::Unrestricted {
            crawl_delay_secs,
            sitemap_urls,
        } => RobotsSnapshot {
            allowed: true,
            status: "unrestricted".to_string(),
            crawl_delay_secs: *crawl_delay_secs,
            sitemap_urls: sitemap_urls.clone(),
        },
        RobotStatus::TemporarilyUnavailable => RobotsSnapshot {
            allowed: false,
            status: "temporarily_unavailable".to_string(),
            crawl_delay_secs: None,
            sitemap_urls: Vec::new(),
        },
    }
}

pub fn extract_crawl_delay(robots_txt: &[u8]) -> Option<u64> {
    let text = String::from_utf8_lossy(robots_txt);

    let mut current_agents: Vec<String> = Vec::new();
    let mut in_agent_block = false;
    let mut best: Option<(u8, u64)> = None;
    let mut pending_delay: Option<u64> = None;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let lower = line.to_lowercase();
        if lower.starts_with("user-agent:") {
            if !in_agent_block {
                if let Some(delay) = pending_delay.take() {
                    let priority = section_priority(&current_agents);
                    if priority > 0
                        && best
                            .as_ref()
                            .is_none_or(|(best_priority, _)| priority > *best_priority)
                    {
                        best = Some((priority, delay));
                    }
                }
                current_agents.clear();
                in_agent_block = true;
            }
            if let Some(agent) = line.split(':').nth(1) {
                current_agents.push(agent.trim().to_lowercase());
            }
        } else {
            in_agent_block = false;
            if lower.starts_with("crawl-delay:")
                && let Some(value) = line.split(':').nth(1)
                && let Ok(delay) = value.trim().parse::<u64>()
            {
                pending_delay = Some(delay);
            }
        }
    }

    if let Some(delay) = pending_delay {
        let priority = section_priority(&current_agents);
        if priority > 0
            && best
                .as_ref()
                .is_none_or(|(best_priority, _)| priority > *best_priority)
        {
            best = Some((priority, delay));
        }
    }

    best.map(|(_, delay)| delay)
}

pub fn extract_sitemap_urls_from_robots(robots_txt: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(robots_txt);
    let mut sitemap_urls = BTreeSet::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if !lower.starts_with("sitemap:") {
            continue;
        }
        let Some((_, raw_url)) = trimmed.split_once(':') else {
            continue;
        };
        let sitemap_url = raw_url.trim();
        if Url::parse(sitemap_url).is_ok() {
            sitemap_urls.insert(sitemap_url.to_string());
        }
    }

    sitemap_urls.into_iter().collect()
}

fn section_priority(agents: &[String]) -> u8 {
    for agent in agents {
        if agent.contains("findversecrawler") || agent.contains("findversebot") {
            return 2;
        }
    }
    for agent in agents {
        if agent == "*" {
            return 1;
        }
    }
    0
}

pub async fn rate_limit_origin(state: &Arc<Mutex<WorkerState>>, url: &str) {
    let Some(origin) = origin_key(url) else {
        return;
    };

    let mut state_guard = state.lock().await;
    let crawl_delay_secs = state_guard
        .robots_cache
        .get(&origin)
        .and_then(|cached| match &cached.status {
            RobotStatus::Fetched {
                crawl_delay_secs, ..
            }
            | RobotStatus::Unrestricted {
                crawl_delay_secs, ..
            } => *crawl_delay_secs,
            RobotStatus::TemporarilyUnavailable => None,
        })
        .unwrap_or(1);

    if let Some(last) = state_guard.origin_last_request.get(&origin) {
        let elapsed = last.elapsed();
        let jitter_ms = rand::rng().random_range(800..1200);
        let min_interval = Duration::from_millis(crawl_delay_secs * 1000 + jitter_ms);
        if elapsed < min_interval {
            let wait = min_interval - elapsed;
            drop(state_guard);
            sleep(wait).await;
            let mut state_guard = state.lock().await;
            state_guard
                .origin_last_request
                .insert(origin, tokio::time::Instant::now());
            return;
        }
    }

    state_guard
        .origin_last_request
        .insert(origin, tokio::time::Instant::now());
}

pub async fn fetch_with_retry(
    page_client: &reqwest::Client,
    robots_client: &reqwest::Client,
    state: &Arc<Mutex<WorkerState>>,
    url: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
) -> Result<FetchedResponse, FetchWorkflowError> {
    let max_retries = 3u32;
    let mut attempt = 0u32;

    loop {
        match fetch_once(page_client, robots_client, state, url, etag, last_modified).await {
            Ok(outcome) => {
                let status = outcome.response.status().as_u16();
                if (status == 429 || status >= 500) && attempt < max_retries {
                    let backoff = outcome.retry_after_secs.unwrap_or(0).max(2u64.pow(attempt));
                    warn!(
                        "got {} for {}, retrying in {}s (attempt {}/{})",
                        status,
                        url,
                        backoff,
                        attempt + 1,
                        max_retries
                    );
                    sleep(Duration::from_secs(backoff)).await;
                    attempt += 1;
                    continue;
                }
                return Ok(outcome);
            }
            Err(FetchWorkflowError::Request(error)) if attempt < max_retries => {
                let backoff = 2u64.pow(attempt);
                warn!(
                    "request error for {}: {}, retrying in {}s (attempt {}/{})",
                    url,
                    error,
                    backoff,
                    attempt + 1,
                    max_retries
                );
                sleep(Duration::from_secs(backoff)).await;
                attempt += 1;
            }
            Err(error) => return Err(error),
        }
    }
}

async fn fetch_once(
    page_client: &reqwest::Client,
    robots_client: &reqwest::Client,
    state: &Arc<Mutex<WorkerState>>,
    url: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
) -> Result<FetchedResponse, FetchWorkflowError> {
    let mut current_url = url.to_string();
    let mut redirect_chain = Vec::new();
    let mut first_request = true;

    loop {
        let robots = inspect_robots(robots_client, state, &current_url).await;
        if !robots.allowed {
            return Err(FetchWorkflowError::BlockedByRobots {
                url: current_url,
                status: robots.status,
            });
        }

        rate_limit_origin(state, &current_url).await;

        let mut request = page_client.get(&current_url);
        if first_request {
            if let Some(etag) = etag {
                request = request.header("If-None-Match", etag);
            }
            if let Some(last_modified) = last_modified {
                request = request.header("If-Modified-Since", last_modified);
            }
        }

        let response = request.send().await.map_err(FetchWorkflowError::Request)?;
        let retry_after_secs = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .and_then(parse_retry_after);

        if response.status().is_redirection() {
            let Some(location) = response
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| resolve_redirect(&current_url, value))
            else {
                return Ok(FetchedResponse {
                    response,
                    redirect_chain,
                    retry_after_secs,
                });
            };

            if redirect_chain.len() >= 10 {
                redirect_chain.push(location);
                return Err(FetchWorkflowError::TooManyRedirects {
                    chain: redirect_chain,
                });
            }

            redirect_chain.push(location.clone());
            current_url = location;
            first_request = false;
            continue;
        }

        return Ok(FetchedResponse {
            response,
            redirect_chain,
            retry_after_secs,
        });
    }
}

fn parse_retry_after(value: &str) -> Option<u64> {
    value.trim().parse::<u64>().ok()
}

fn resolve_redirect(base: &str, location: &str) -> Option<String> {
    let base_url = Url::parse(base).ok()?;
    let next = base_url.join(location).ok()?;
    Some(next.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_crawl_delay_findverse_section() {
        let robots = b"\
User-agent: Googlebot
Crawl-delay: 10

User-agent: FindVerseCrawler
Crawl-delay: 5

User-agent: *
Crawl-delay: 2
";
        assert_eq!(extract_crawl_delay(robots), Some(5));
    }

    #[test]
    fn test_extract_crawl_delay_findversebot_case_insensitive() {
        let robots = b"\
User-agent: findverseBot
Crawl-delay: 7
";
        assert_eq!(extract_crawl_delay(robots), Some(7));
    }

    #[test]
    fn test_extract_crawl_delay_wildcard_fallback() {
        let robots = b"\
User-agent: Googlebot
Crawl-delay: 10

User-agent: *
Crawl-delay: 3
";
        assert_eq!(extract_crawl_delay(robots), Some(3));
    }

    #[test]
    fn test_extract_crawl_delay_multiple_agents_in_section() {
        let robots = b"\
User-agent: Googlebot
User-agent: FindVerseCrawler
Crawl-delay: 4
";
        assert_eq!(extract_crawl_delay(robots), Some(4));
    }

    #[test]
    fn test_extract_crawl_delay_no_match() {
        let robots = b"\
User-agent: Googlebot
Crawl-delay: 10

User-agent: Bingbot
Crawl-delay: 5
";
        assert_eq!(extract_crawl_delay(robots), None);
    }

    #[test]
    fn test_extract_crawl_delay_findverse_preferred_over_wildcard() {
        let robots = b"\
User-agent: *
Crawl-delay: 20

User-agent: FindVerseCrawler
Crawl-delay: 1
";
        assert_eq!(extract_crawl_delay(robots), Some(1));
    }

    #[test]
    fn test_extract_crawl_delay_empty() {
        assert_eq!(extract_crawl_delay(b""), None);
    }

    #[test]
    fn test_extract_crawl_delay_with_comments() {
        let robots = b"\
# robots.txt
User-agent: *
# Default delay
Crawl-delay: 8
";
        assert_eq!(extract_crawl_delay(robots), Some(8));
    }

    #[test]
    fn extract_sitemap_urls_reads_multiple_entries() {
        let robots = b"\
User-agent: *
Disallow:
Sitemap: https://example.com/sitemap.xml
Sitemap: https://example.com/news.xml
";
        assert_eq!(
            extract_sitemap_urls_from_robots(robots),
            vec![
                "https://example.com/news.xml".to_string(),
                "https://example.com/sitemap.xml".to_string()
            ]
        );
    }

    #[test]
    fn resolve_redirect_handles_relative_targets() {
        assert_eq!(
            resolve_redirect("https://example.com/docs/start", "../guide"),
            Some("https://example.com/guide".to_string())
        );
    }

    #[test]
    fn robots_fallback_defaults_to_unrestricted_without_cached_rules() {
        assert!(matches!(
            fallback_robot_status(false),
            Some(RobotStatus::Unrestricted { .. })
        ));
    }

    #[test]
    fn robots_fallback_preserves_cached_rules_when_refresh_fails() {
        assert!(fallback_robot_status(true).is_none());
    }
}

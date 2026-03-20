use std::{collections::HashMap, sync::Arc, time::Duration};

use rand::seq::IndexedRandom;
use texting_robots::Robot;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::warn;
use url::Url;

// ---------------------------------------------------------------------------
// User-agent pool for anti-bot resilience
// ---------------------------------------------------------------------------
pub const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_4_1) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4.1 Safari/605.1.15",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:125.0) Gecko/20100101 Firefox/125.0",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_4_1) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:125.0) Gecko/20100101 Firefox/125.0",
];

pub fn random_user_agent() -> &'static str {
    let mut rng = rand::rng();
    USER_AGENTS.choose(&mut rng).unwrap_or(&USER_AGENTS[0])
}

// ---------------------------------------------------------------------------
// Shared worker state for robots cache and rate limiting
// ---------------------------------------------------------------------------
pub struct WorkerState {
    pub robots_cache: HashMap<String, Option<Robot>>,
    pub domain_last_request: HashMap<String, tokio::time::Instant>,
}

impl WorkerState {
    pub fn new() -> Self {
        Self {
            robots_cache: HashMap::new(),
            domain_last_request: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// robots.txt compliance
// ---------------------------------------------------------------------------
pub async fn check_robots_allowed(
    client: &reqwest::Client,
    state: &Arc<Mutex<WorkerState>>,
    url: &str,
) -> bool {
    let parsed = match Url::parse(url) {
        Ok(u) => u,
        Err(_) => return true,
    };

    let host = match parsed.host_str() {
        Some(h) => h.to_string(),
        None => return true,
    };

    let origin = format!("{}://{}", parsed.scheme(), host);

    let mut state_guard = state.lock().await;

    if !state_guard.robots_cache.contains_key(&origin) {
        // Drop lock before network request
        drop(state_guard);

        let robots_url = format!("{}/robots.txt", origin);
        let robot = match client.get(&robots_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let body = resp.bytes().await.unwrap_or_default();
                Robot::new("FindVerseCrawler", &body).ok()
            }
            _ => None,
        };

        state_guard = state.lock().await;
        state_guard.robots_cache.insert(origin.clone(), robot);

        match state_guard.robots_cache.get(&origin) {
            Some(Some(robot)) => robot.allowed(url),
            _ => true,
        }
    } else {
        match state_guard.robots_cache.get(&origin) {
            Some(Some(robot)) => robot.allowed(url),
            _ => true,
        }
    }
}

// ---------------------------------------------------------------------------
// Per-domain rate limiting
// ---------------------------------------------------------------------------
pub async fn rate_limit_domain(state: &Arc<Mutex<WorkerState>>, url: &str) {
    let domain = Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()));

    let domain = match domain {
        Some(d) => d,
        None => return,
    };

    let mut state_guard = state.lock().await;
    if let Some(last) = state_guard.domain_last_request.get(&domain) {
        let elapsed = last.elapsed();
        if elapsed < Duration::from_secs(1) {
            let wait = Duration::from_secs(1) - elapsed;
            drop(state_guard);
            sleep(wait).await;
            let mut state_guard = state.lock().await;
            state_guard
                .domain_last_request
                .insert(domain, tokio::time::Instant::now());
        } else {
            state_guard
                .domain_last_request
                .insert(domain, tokio::time::Instant::now());
        }
    } else {
        state_guard
            .domain_last_request
            .insert(domain, tokio::time::Instant::now());
    }
}

// ---------------------------------------------------------------------------
// Fetch with retry + exponential backoff
// ---------------------------------------------------------------------------
pub async fn fetch_with_retry(
    client: &reqwest::Client,
    url: &str,
) -> Result<reqwest::Response, reqwest::Error> {
    let max_retries = 3u32;
    let mut attempt = 0u32;

    loop {
        // Use a different UA for each attempt
        let response = client
            .get(url)
            .header("User-Agent", random_user_agent())
            .send()
            .await;

        match response {
            Ok(resp) => {
                let status = resp.status().as_u16();
                if (status == 429 || status >= 500) && attempt < max_retries {
                    // Respect Retry-After header if present
                    let retry_after = resp
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or(0);

                    let backoff = std::cmp::max(retry_after, 2u64.pow(attempt));
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
                return Ok(resp);
            }
            Err(e) if attempt < max_retries => {
                let backoff = 2u64.pow(attempt);
                warn!(
                    "request error for {}: {}, retrying in {}s (attempt {}/{})",
                    url,
                    e,
                    backoff,
                    attempt + 1,
                    max_retries
                );
                sleep(Duration::from_secs(backoff)).await;
                attempt += 1;
                continue;
            }
            Err(e) => return Err(e),
        }
    }
}

use std::{collections::HashMap, sync::Arc, time::Duration};

use rand::Rng;
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
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:124.0) Gecko/20100101 Firefox/124.0",
    "Mozilla/5.0 (X11; Linux x86_64; rv:125.0) Gecko/20100101 Firefox/125.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_3_1) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.3.1 Safari/605.1.15",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_2_1) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 11.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Fedora; Linux x86_64; rv:125.0) Gecko/20100101 Firefox/125.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_4) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.3 Safari/605.1.15",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:123.0) Gecko/20100101 Firefox/123.0",
    "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:124.0) Gecko/20100101 Firefox/124.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_1) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64; rv:124.0) Gecko/20100101 Firefox/124.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Edge/124.0.0.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/123.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:122.0) Gecko/20100101 Firefox/122.0",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 13_6) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.6 Safari/605.1.15",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:123.0) Gecko/20100101 Firefox/123.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 13_5) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0",
    "Mozilla/5.0 (X11; Linux x86_64; rv:123.0) Gecko/20100101 Firefox/123.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 13_4) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.5 Safari/605.1.15",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/118.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:120.0) Gecko/20100101 Firefox/120.0",
    "Mozilla/5.0 (X11; Fedora; Linux x86_64; rv:124.0) Gecko/20100101 Firefox/124.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 13_3) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Edge/123.0.0.0",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 13_2) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.3 Safari/605.1.15",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:119.0) Gecko/20100101 Firefox/119.0",
    "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:122.0) Gecko/20100101 Firefox/122.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/117.0.0.0 Safari/537.36",
];

pub fn random_user_agent() -> &'static str {
    let mut rng = rand::rng();
    USER_AGENTS.choose(&mut rng).unwrap_or(&USER_AGENTS[0])
}

// ---------------------------------------------------------------------------
// Shared worker state for robots cache and rate limiting
// ---------------------------------------------------------------------------
pub struct WorkerState {
    pub robots_cache: HashMap<String, CachedRobot>,
    pub domain_last_request: HashMap<String, tokio::time::Instant>,
}

pub struct CachedRobot {
    pub robot: Option<Robot>,
    pub crawl_delay_secs: Option<u64>,
    pub cached_at: tokio::time::Instant,
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
// robots.txt compliance with crawl-delay
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

    // 检查缓存是否过期（1小时）
    let needs_refresh = state_guard
        .robots_cache
        .get(&origin)
        .map(|cached| cached.cached_at.elapsed() > Duration::from_secs(3600))
        .unwrap_or(true);

    if needs_refresh {
        drop(state_guard);

        let robots_url = format!("{}/robots.txt", origin);
        let (robot, crawl_delay) = match client.get(&robots_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                let body = resp.bytes().await.unwrap_or_default();
                let robot = Robot::new("FindVerseCrawler", &body).ok();
                let crawl_delay = extract_crawl_delay(&body);
                (robot, crawl_delay)
            }
            _ => (None, None),
        };

        state_guard = state.lock().await;
        state_guard.robots_cache.insert(
            origin.clone(),
            CachedRobot {
                robot,
                crawl_delay_secs: crawl_delay,
                cached_at: tokio::time::Instant::now(),
            },
        );
    }

    state_guard
        .robots_cache
        .get(&origin)
        .and_then(|cached| cached.robot.as_ref())
        .map(|robot| robot.allowed(url))
        .unwrap_or(true)
}

fn extract_crawl_delay(robots_txt: &[u8]) -> Option<u64> {
    let text = String::from_utf8_lossy(robots_txt);
    for line in text.lines() {
        if line.to_lowercase().starts_with("crawl-delay:") {
            if let Some(value) = line.split(':').nth(1) {
                if let Ok(delay) = value.trim().parse::<u64>() {
                    return Some(delay);
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Per-domain rate limiting with jitter and crawl-delay
// ---------------------------------------------------------------------------
pub async fn rate_limit_domain(state: &Arc<Mutex<WorkerState>>, url: &str) {
    let parsed = Url::parse(url).ok();
    let domain = parsed
        .as_ref()
        .and_then(|u| u.host_str().map(|h| h.to_string()));

    let domain = match domain {
        Some(d) => d,
        None => return,
    };

    let origin = parsed.map(|u| format!("{}://{}", u.scheme(), domain.clone()));

    let mut state_guard = state.lock().await;

    // 获取 crawl-delay（如果有）
    let crawl_delay = origin
        .as_ref()
        .and_then(|o| state_guard.robots_cache.get(o))
        .and_then(|cached| cached.crawl_delay_secs)
        .unwrap_or(1);

    if let Some(last) = state_guard.domain_last_request.get(&domain) {
        let elapsed = last.elapsed();
        let jitter = rand::rng().random_range(800..1200);
        let min_interval = Duration::from_millis(crawl_delay * 1000 + jitter);

        if elapsed < min_interval {
            let wait = min_interval - elapsed;
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

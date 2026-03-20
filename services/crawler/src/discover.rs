use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::Context;
use chrono::Utc;
use reqwest::header::CONTENT_TYPE;
use scraper::{Html, Selector};
use sha2::{Digest, Sha256};
use tracing::{info, warn};
use url::Url;

use crate::extract::{detect_language, extract_clean_body_text, extract_links, extract_sitemap_urls};
use crate::models::{FetchManifestEntry, FrontierEntry, IndexedDocument, SeedConfig};

// ---------------------------------------------------------------------------
// Discover
// ---------------------------------------------------------------------------
pub async fn discover(
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
                    warn!(
                        "sitemap fetch for {} returned {}",
                        seed.name,
                        response.status()
                    )
                }
                Err(error) => warn!("failed to fetch sitemap for {}: {error}", seed.name),
            }
        }

        match client.get(&seed.url).send().await {
            Ok(response) if response.status().is_success() => {
                let body = response.text().await.unwrap_or_default();
                seed_urls.extend(
                    extract_links(&seed.url, &body)
                        .into_iter()
                        .take(limit_per_seed),
                );
            }
            Ok(response) => warn!(
                "seed page fetch for {} returned {}",
                seed.name,
                response.status()
            ),
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

// ---------------------------------------------------------------------------
// Fetch
// ---------------------------------------------------------------------------
pub async fn fetch(frontier: PathBuf, output_dir: PathBuf, limit: usize) -> anyhow::Result<()> {
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

// ---------------------------------------------------------------------------
// BuildIndex
// ---------------------------------------------------------------------------
pub async fn build_index(input_dir: PathBuf, output: PathBuf) -> anyhow::Result<()> {
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

fn build_document(entry: &FetchManifestEntry, html: &str) -> Option<IndexedDocument> {
    let parsed_url = Url::parse(&entry.url).ok()?;
    let document = Html::parse_document(html);
    let title_selector = Selector::parse("title").ok()?;
    let meta_selector = Selector::parse("meta[name='description']").ok()?;

    let title = document
        .select(&title_selector)
        .next()
        .map(|node| node.text().collect::<String>())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| parsed_url.as_str().to_string());

    let body = extract_clean_body_text(&document);
    let body_text = body.unwrap_or_default();

    if body_text.is_empty() {
        return None;
    }

    let meta_snippet = document
        .select(&meta_selector)
        .next()
        .and_then(|node| node.value().attr("content"))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);

    let snippet = meta_snippet.unwrap_or_else(|| body_text.chars().take(200).collect());

    let language = detect_language(&body_text).unwrap_or_else(|| "unknown".to_string());

    Some(IndexedDocument {
        id: hex_hash(&entry.url),
        title: title.trim().to_string(),
        url: entry.url.clone(),
        display_url: display_url(&entry.url),
        snippet: snippet.chars().take(220).collect(),
        body: body_text.chars().take(4_000).collect(),
        language,
        last_crawled_at: entry.fetched_at,
        suggest_terms: derive_terms(&title, &body_text),
        site_authority: 0.5,
    })
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_url_strips_scheme() {
        assert_eq!(
            display_url("https://example.com/a/b/"),
            "example.com/a/b".to_string()
        );
    }
}

use std::{
    collections::{BTreeSet, HashMap},
    path::PathBuf,
    sync::{Arc, RwLock},
};

use anyhow::{Context, anyhow};
use chrono::{DateTime, NaiveDate, Utc};
use rand::{Rng, distr::Alphanumeric};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::{fs, sync::RwLock as AsyncRwLock};
use url::Url;
use uuid::Uuid;

use crate::{
    error::ApiError,
    models::{
        ApiKeyMetadata, CreateKeyRequest, CreatedKeyResponse, DeveloperUsageResponse,
        DocumentListParams, DocumentListResponse, DocumentSummary, Freshness, IndexedDocument,
        PurgeSiteResponse, SearchParams, SearchResponse, SearchResult, SuggestResponse,
    },
};

#[derive(Debug, Clone)]
pub struct SearchIndex {
    path: PathBuf,
    inner: Arc<RwLock<SearchState>>,
}

#[derive(Debug, Clone)]
struct SearchState {
    documents: Vec<IndexedDocument>,
    suggestions: Vec<String>,
}

impl SearchIndex {
    pub async fn load(path: PathBuf) -> anyhow::Result<Self> {
        ensure_file_with_fallbacks(
            &path,
            "[]",
            &[
                PathBuf::from("/opt/findverse/bootstrap_documents.json"),
                PathBuf::from("services/api/fixtures/bootstrap_documents.json"),
            ],
        )
        .await?;

        let raw = fs::read_to_string(&path)
            .await
            .context("failed to read bootstrap document file")?;
        let documents: Vec<IndexedDocument> =
            serde_json::from_str(&raw).context("failed to parse bootstrap document file")?;

        Ok(Self {
            path,
            inner: Arc::new(RwLock::new(SearchState {
                suggestions: rebuild_suggestions(&documents),
                documents,
            })),
        })
    }

    pub fn total_documents(&self) -> usize {
        self.inner.read().expect("search index poisoned").documents.len()
    }

    pub fn search(&self, params: SearchParams) -> SearchResponse {
        let started = std::time::Instant::now();
        let tokens = tokenize(&params.q);
        let freshness_limit = params.freshness.max_age();
        let now = Utc::now();
        let lang_filter = params.lang.as_deref().map(str::to_lowercase);
        let site_filter = params.site.as_deref().map(str::to_lowercase);
        let state = self.inner.read().expect("search index poisoned");

        let mut matches = state
            .documents
            .iter()
            .filter_map(|document| {
                if let Some(limit) = freshness_limit
                    && now.signed_duration_since(document.last_crawled_at) > limit
                {
                    return None;
                }

                if let Some(lang) = &lang_filter
                    && document.language.to_lowercase() != *lang
                {
                    return None;
                }

                if let Some(site) = &site_filter
                    && !document.url.to_lowercase().contains(site)
                {
                    return None;
                }

                let score = score_document(document, &tokens, params.freshness);
                if score <= 0.0 {
                    return None;
                }

                Some(SearchResult {
                    id: document.id.clone(),
                    title: document.title.clone(),
                    url: document.url.clone(),
                    display_url: document.display_url.clone(),
                    snippet: choose_snippet(document, &tokens),
                    language: document.language.clone(),
                    last_crawled_at: document.last_crawled_at,
                    score,
                })
            })
            .collect::<Vec<_>>();

        matches.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| right.last_crawled_at.cmp(&left.last_crawled_at))
        });

        let total = matches.len();
        let page = matches
            .into_iter()
            .skip(params.offset)
            .take(params.limit.min(20))
            .collect::<Vec<_>>();

        let next_offset = if params.offset + page.len() < total {
            Some(params.offset + page.len())
        } else {
            None
        };

        SearchResponse {
            query: params.q,
            took_ms: started.elapsed().as_millis(),
            total_estimate: total,
            next_offset,
            results: page,
        }
    }

    pub fn suggest(&self, query: &str) -> SuggestResponse {
        let query_normalized = query.trim().to_lowercase();
        let state = self.inner.read().expect("search index poisoned");
        let suggestions = state
            .suggestions
            .iter()
            .filter(|value| value.starts_with(&query_normalized))
            .take(8)
            .cloned()
            .collect::<Vec<_>>();

        SuggestResponse {
            query: query.to_string(),
            suggestions,
        }
    }

    pub async fn upsert_documents(
        &self,
        documents: Vec<IndexedDocument>,
    ) -> Result<usize, ApiError> {
        if documents.is_empty() {
            return Ok(0);
        }

        let serialized = {
            let mut state = self
                .inner
                .write()
                .map_err(|_| ApiError::Internal(anyhow!("search index poisoned")))?;

            let mut accepted = 0;
            for document in documents {
                accepted += 1;
                if let Some(existing) = state
                    .documents
                    .iter_mut()
                    .find(|existing| existing.url == document.url || existing.id == document.id)
                {
                    *existing = document;
                } else {
                    state.documents.push(document);
                }
            }

            state.suggestions = rebuild_suggestions(&state.documents);
            (
                accepted,
                serde_json::to_string_pretty(&state.documents)
                    .map_err(|error| ApiError::Internal(error.into()))?,
            )
        };

        fs::write(&self.path, serialized.1).await?;
        Ok(serialized.0)
    }

    pub fn list_documents(&self, params: DocumentListParams) -> DocumentListResponse {
        let limit = params.limit.clamp(1, 50);
        let offset = params.offset;
        let query_tokens = params
            .query
            .as_deref()
            .map(tokenize)
            .unwrap_or_default();
        let site_filter = params.site.as_deref().map(str::to_lowercase);
        let state = self.inner.read().expect("search index poisoned");

        let mut documents = state
            .documents
            .iter()
            .filter(|document| {
                if let Some(site) = &site_filter
                    && !document.url.to_lowercase().contains(site)
                {
                    return false;
                }

                if query_tokens.is_empty() {
                    return true;
                }

                let haystack = format!(
                    "{} {} {}",
                    document.title.to_lowercase(),
                    document.snippet.to_lowercase(),
                    document.url.to_lowercase()
                );
                query_tokens.iter().all(|token| haystack.contains(token))
            })
            .map(|document| DocumentSummary {
                id: document.id.clone(),
                title: document.title.clone(),
                url: document.url.clone(),
                display_url: document.display_url.clone(),
                snippet: document.snippet.clone(),
                language: document.language.clone(),
                last_crawled_at: document.last_crawled_at,
            })
            .collect::<Vec<_>>();

        documents.sort_by(|left, right| right.last_crawled_at.cmp(&left.last_crawled_at));
        let total_estimate = documents.len();
        let page = documents
            .into_iter()
            .skip(offset)
            .take(limit)
            .collect::<Vec<_>>();
        let next_offset = if offset + page.len() < total_estimate {
            Some(offset + page.len())
        } else {
            None
        };

        DocumentListResponse {
            total_estimate,
            next_offset,
            documents: page,
        }
    }

    pub async fn delete_document(&self, document_id: &str) -> Result<bool, ApiError> {
        let serialized = {
            let mut state = self
                .inner
                .write()
                .map_err(|_| ApiError::Internal(anyhow!("search index poisoned")))?;
            let original_len = state.documents.len();
            state.documents.retain(|document| document.id != document_id);
            if state.documents.len() == original_len {
                return Ok(false);
            }

            state.suggestions = rebuild_suggestions(&state.documents);
            serde_json::to_string_pretty(&state.documents)
                .map_err(|error| ApiError::Internal(error.into()))?
        };

        fs::write(&self.path, serialized).await?;
        Ok(true)
    }

    pub async fn purge_site(&self, site: &str) -> Result<PurgeSiteResponse, ApiError> {
        let normalized = site.trim().to_lowercase();
        if normalized.is_empty() {
            return Err(ApiError::BadRequest("site must not be empty".to_string()));
        }

        let serialized = {
            let mut state = self
                .inner
                .write()
                .map_err(|_| ApiError::Internal(anyhow!("search index poisoned")))?;
            let original_len = state.documents.len();
            state
                .documents
                .retain(|document| !document.url.to_lowercase().contains(&normalized));
            let deleted_documents = original_len - state.documents.len();
            state.suggestions = rebuild_suggestions(&state.documents);
            (
                deleted_documents,
                serde_json::to_string_pretty(&state.documents)
                    .map_err(|error| ApiError::Internal(error.into()))?,
            )
        };

        fs::write(&self.path, serialized.1).await?;
        Ok(PurgeSiteResponse {
            deleted_documents: serialized.0,
        })
    }
}

#[derive(Debug, Clone)]
pub struct DeveloperStore {
    path: PathBuf,
    inner: Arc<AsyncRwLock<DeveloperStoreState>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct DeveloperStoreState {
    #[serde(default)]
    records: HashMap<String, DeveloperRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeveloperRecord {
    developer_id: String,
    #[serde(default = "default_qps_limit")]
    qps_limit: u32,
    #[serde(default = "default_daily_limit")]
    daily_limit: u32,
    #[serde(default)]
    used_today: u32,
    #[serde(default = "default_usage_day")]
    usage_day: NaiveDate,
    #[serde(default)]
    keys: Vec<StoredKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredKey {
    id: String,
    name: String,
    preview: String,
    token_hash: String,
    created_at: DateTime<Utc>,
    #[serde(default)]
    revoked_at: Option<DateTime<Utc>>,
}

impl DeveloperStore {
    pub async fn load(path: PathBuf) -> anyhow::Result<Self> {
        let empty = serde_json::to_string_pretty(&DeveloperStoreState::default())?;
        ensure_file_with_fallbacks(
            &path,
            &empty,
            &[
                PathBuf::from("/opt/findverse/developer_store.json"),
                PathBuf::from("services/api/fixtures/developer_store.json"),
            ],
        )
        .await?;

        let raw = fs::read_to_string(&path)
            .await
            .context("failed to read developer store file")?;
        let state: DeveloperStoreState =
            serde_json::from_str(&raw).context("failed to parse developer store file")?;

        Ok(Self {
            path,
            inner: Arc::new(AsyncRwLock::new(state)),
        })
    }

    pub async fn create_key(
        &self,
        developer_id: &str,
        request: CreateKeyRequest,
    ) -> Result<CreatedKeyResponse, ApiError> {
        let clean_name = request.name.trim();
        if clean_name.len() < 2 {
            return Err(ApiError::BadRequest(
                "key name must contain at least 2 characters".to_string(),
            ));
        }

        let token = generate_token("fvk");
        let token_hash = hash_token(&token);
        let preview = format!("{}...{}", &token[..8], &token[token.len() - 4..]);
        let created_at = Utc::now();
        let key = StoredKey {
            id: Uuid::now_v7().to_string(),
            name: clean_name.to_string(),
            preview: preview.clone(),
            token_hash,
            created_at,
            revoked_at: None,
        };

        {
            let mut state = self.inner.write().await;
            let record = state
                .records
                .entry(developer_id.to_string())
                .or_insert_with(|| DeveloperRecord {
                    developer_id: developer_id.to_string(),
                    qps_limit: 5,
                    daily_limit: 10_000,
                    used_today: 0,
                    usage_day: Utc::now().date_naive(),
                    keys: Vec::new(),
                });

            record.keys.push(key.clone());
            self.persist_locked(&state).await?;
        }

        Ok(CreatedKeyResponse {
            id: key.id,
            name: key.name,
            preview,
            token,
            created_at,
        })
    }

    pub async fn revoke_key(&self, developer_id: &str, key_id: &str) -> Result<(), ApiError> {
        let mut state = self.inner.write().await;
        let record = state
            .records
            .get_mut(developer_id)
            .ok_or_else(|| ApiError::NotFound("developer record not found".to_string()))?;

        let key = record
            .keys
            .iter_mut()
            .find(|key| key.id == key_id)
            .ok_or_else(|| ApiError::NotFound("api key not found".to_string()))?;

        if key.revoked_at.is_some() {
            return Err(ApiError::Conflict("api key already revoked".to_string()));
        }

        key.revoked_at = Some(Utc::now());
        self.persist_locked(&state).await?;
        Ok(())
    }

    pub async fn usage(&self, developer_id: &str) -> Result<DeveloperUsageResponse, ApiError> {
        let mut state = self.inner.write().await;
        let record = state
            .records
            .entry(developer_id.to_string())
            .or_insert_with(|| DeveloperRecord {
                developer_id: developer_id.to_string(),
                qps_limit: 5,
                daily_limit: 10_000,
                used_today: 0,
                usage_day: Utc::now().date_naive(),
                keys: Vec::new(),
            });

        roll_usage_day(record);
        let response = DeveloperUsageResponse {
            developer_id: record.developer_id.clone(),
            qps_limit: record.qps_limit,
            daily_limit: record.daily_limit,
            used_today: record.used_today,
            keys: record
                .keys
                .iter()
                .map(|key| ApiKeyMetadata {
                    id: key.id.clone(),
                    name: key.name.clone(),
                    preview: key.preview.clone(),
                    created_at: key.created_at,
                    revoked_at: key.revoked_at,
                })
                .collect(),
        };

        self.persist_locked(&state).await?;
        Ok(response)
    }

    pub async fn validate_and_track(&self, auth_header: Option<&str>) -> Result<(), ApiError> {
        let Some(header) = auth_header else {
            return Err(ApiError::Unauthorized("api key required".to_string()));
        };

        let token_hash = bearer_hash(header)?;
        let today = Utc::now().date_naive();
        let mut state = self.inner.write().await;

        for record in state.records.values_mut() {
            roll_usage_day(record);

            if record
                .keys
                .iter()
                .any(|key| key.token_hash == token_hash && key.revoked_at.is_none())
            {
                if record.used_today >= record.daily_limit {
                    return Err(ApiError::Unauthorized("daily request quota exceeded".to_string()));
                }

                record.used_today += 1;
                record.usage_day = today;
                self.persist_locked(&state).await?;
                return Ok(());
            }
        }

        Err(ApiError::Unauthorized("invalid api key".to_string()))
    }

    async fn persist_locked(&self, state: &DeveloperStoreState) -> Result<(), ApiError> {
        let raw =
            serde_json::to_string_pretty(state).map_err(|error| ApiError::Internal(error.into()))?;
        fs::write(&self.path, raw).await?;
        Ok(())
    }
}

fn choose_snippet(document: &IndexedDocument, tokens: &[String]) -> String {
    if tokens.is_empty() {
        return document.snippet.clone();
    }

    let lower_body = document.body.to_lowercase();
    for token in tokens {
        if let Some(position) = lower_body.find(token) {
            let start = position.saturating_sub(70);
            let end = (position + 140).min(document.body.len());
            return document.body[start..end].trim().to_string();
        }
    }

    document.snippet.clone()
}

fn score_document(document: &IndexedDocument, tokens: &[String], freshness: Freshness) -> f32 {
    if tokens.is_empty() {
        return 0.0;
    }

    let title = document.title.to_lowercase();
    let snippet = document.snippet.to_lowercase();
    let body = document.body.to_lowercase();
    let url = document.url.to_lowercase();
    let authority_boost = 0.5 + document.site_authority;
    let freshness_bonus = freshness_score(document.last_crawled_at, freshness);

    let mut score = 0.0;
    for token in tokens {
        if title.contains(token) {
            score += 6.0;
        }
        if snippet.contains(token) {
            score += 2.5;
        }
        if body.contains(token) {
            score += 1.6;
        }
        if url.contains(token) {
            score += 1.0;
        }
    }

    score * authority_boost + freshness_bonus
}

fn freshness_score(last_crawled_at: DateTime<Utc>, freshness: Freshness) -> f32 {
    let age_hours = Utc::now()
        .signed_duration_since(last_crawled_at)
        .num_hours()
        .max(0) as f32;

    match freshness {
        Freshness::Day => (24.0 - age_hours).max(0.0) / 4.0,
        Freshness::Week => (168.0 - age_hours).max(0.0) / 24.0,
        Freshness::Month => (720.0 - age_hours).max(0.0) / 48.0,
        Freshness::All => 0.0,
    }
}

fn rebuild_suggestions(documents: &[IndexedDocument]) -> Vec<String> {
    let mut suggestions = BTreeSet::new();
    for document in documents {
        suggestions.extend(document.suggest_terms.iter().cloned());
        for phrase in [document.title.as_str(), document.display_url.as_str()] {
            for token in tokenize(phrase) {
                if token.len() >= 3 {
                    suggestions.insert(token);
                }
            }
        }
    }
    suggestions.into_iter().collect()
}

fn tokenize(input: &str) -> Vec<String> {
    let mut token = String::new();
    let mut tokens = Vec::new();

    for ch in input.chars() {
        if ch.is_alphanumeric() {
            token.extend(ch.to_lowercase());
        } else if !token.is_empty() {
            tokens.push(std::mem::take(&mut token));
        }
    }

    if !token.is_empty() {
        tokens.push(token);
    }

    tokens
}

pub fn derive_terms(title: &str, body: &str) -> Vec<String> {
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

pub fn display_url(input: &str) -> String {
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

pub fn stable_document_id(url: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn bearer_hash(header: &str) -> Result<String, ApiError> {
    let token = header
        .strip_prefix("Bearer ")
        .ok_or_else(|| ApiError::Unauthorized("invalid authorization scheme".to_string()))?
        .trim();

    if token.is_empty() {
        return Err(ApiError::Unauthorized("empty bearer token".to_string()));
    }

    Ok(hash_token(token))
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn generate_token(prefix: &str) -> String {
    let secret = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(40)
        .map(char::from)
        .collect::<String>();
    format!("{prefix}_{secret}")
}

fn roll_usage_day(record: &mut DeveloperRecord) {
    let today = Utc::now().date_naive();
    if record.usage_day != today {
        record.usage_day = today;
        record.used_today = 0;
    }
}

async fn ensure_file_with_fallbacks(
    path: &PathBuf,
    default_contents: &str,
    fallbacks: &[PathBuf],
) -> anyhow::Result<()> {
    if fs::metadata(path).await.is_ok() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }

    for fallback in fallbacks {
        if fallback != path && fs::metadata(fallback).await.is_ok() {
            fs::copy(fallback, path).await?;
            return Ok(());
        }
    }

    fs::write(path, default_contents).await?;
    Ok(())
}

fn default_qps_limit() -> u32 {
    5
}

fn default_daily_limit() -> u32 {
    10_000
}

fn default_usage_day() -> NaiveDate {
    Utc::now().date_naive()
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::{Arc, RwLock},
    };

    use chrono::{Duration, Utc};

    use super::{Freshness, IndexedDocument, SearchIndex, derive_terms, score_document};

    fn sample_document(title: &str, body: &str, age_hours: i64) -> IndexedDocument {
        IndexedDocument {
            id: title.to_string(),
            title: title.to_string(),
            url: format!("https://example.com/{}", title.replace(' ', "-").to_lowercase()),
            display_url: "example.com".to_string(),
            snippet: body.to_string(),
            body: body.to_string(),
            language: "en".to_string(),
            last_crawled_at: Utc::now() - Duration::hours(age_hours),
            suggest_terms: vec!["search".to_string()],
            site_authority: 0.5,
        }
    }

    #[test]
    fn freshness_helps_recent_docs() {
        let fresh = sample_document("Rust search", "latest rust search index", 1);
        let stale = sample_document("Rust search", "latest rust search index", 72);
        let tokens = vec!["rust".to_string(), "search".to_string()];

        assert!(
            score_document(&fresh, &tokens, Freshness::Week)
                > score_document(&stale, &tokens, Freshness::Week)
        );
    }

    #[test]
    fn suggestions_include_titles_and_terms() {
        let index = SearchIndex {
            path: PathBuf::from("ignored.json"),
            inner: Arc::new(RwLock::new(super::SearchState {
                documents: vec![sample_document("Search Ranking", "Ranking strategies", 1)],
                suggestions: vec!["search".to_string(), "ranking".to_string()],
            })),
        };

        let response = index.suggest("sea");
        assert_eq!(response.suggestions, vec!["search"]);
    }

    #[test]
    fn derive_terms_limits_output() {
        let terms = derive_terms(
            "FindVerse crawler integration",
            "Crawler integration should expose stable modular search interfaces",
        );
        assert!(!terms.is_empty());
        assert!(terms.len() <= 12);
    }
}

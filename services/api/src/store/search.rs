use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use anyhow::anyhow;
use chrono::{DateTime, Utc};

use crate::{
    error::ApiError,
    models::{
        DocumentListParams, DocumentListResponse, DocumentSummary, Freshness, IndexedDocument,
        PurgeSiteResponse, SearchParams, SearchResponse, SearchResult, SuggestResponse,
    },
};

use super::{atomic_write, ensure_file_with_fallbacks, tokenize};

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

        let raw = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| anyhow::anyhow!(e).context("failed to read bootstrap document file"))?;
        let documents: Vec<IndexedDocument> = serde_json::from_str(&raw)
            .map_err(|e| anyhow::anyhow!(e).context("failed to parse bootstrap document file"))?;

        Ok(Self {
            path,
            inner: Arc::new(RwLock::new(SearchState {
                suggestions: rebuild_suggestions(&documents),
                documents,
            })),
        })
    }

    pub fn total_documents(&self) -> usize {
        self.inner
            .read()
            .expect("search index poisoned")
            .documents
            .len()
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

        atomic_write(&self.path, &serialized.1).await?;
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
            state
                .documents
                .retain(|document| document.id != document_id);
            if state.documents.len() == original_len {
                return Ok(false);
            }

            state.suggestions = rebuild_suggestions(&state.documents);
            serde_json::to_string_pretty(&state.documents)
                .map_err(|error| ApiError::Internal(error.into()))?
        };

        atomic_write(&self.path, &serialized).await?;
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

        atomic_write(&self.path, &serialized.1).await?;
        Ok(PurgeSiteResponse {
            deleted_documents: serialized.0,
        })
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

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::{Arc, RwLock},
    };

    use chrono::{Duration, Utc};

    use crate::models::{Freshness, IndexedDocument};

    use super::{SearchIndex, SearchState, score_document};
    use crate::store::derive_terms;

    fn sample_document(title: &str, body: &str, age_hours: i64) -> IndexedDocument {
        IndexedDocument {
            id: title.to_string(),
            title: title.to_string(),
            url: format!(
                "https://example.com/{}",
                title.replace(' ', "-").to_lowercase()
            ),
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
            inner: Arc::new(RwLock::new(SearchState {
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

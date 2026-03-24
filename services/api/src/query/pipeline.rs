use chrono::Utc;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    models::{Freshness, SearchParams, SearchResponse, SearchResult, SuggestResponse},
    query::spell_check::suggest_correction,
};

pub struct PreparedSearch {
    pub query: String,
    pub limit: usize,
    pub offset: usize,
    pub did_you_mean: Option<String>,
    pub filters: Vec<Value>,
    pub cache_key: Option<String>,
}

impl PreparedSearch {
    pub fn from_params(params: &SearchParams) -> Self {
        let query = params.q.trim().to_string();
        let limit = params.limit.min(20);
        let offset = params.offset;
        let did_you_mean = suggest_correction(&query);

        let freshness_cutoff = params
            .freshness
            .max_age()
            .map(|duration| (Utc::now() - duration).to_rfc3339());

        let mut filters = Vec::new();
        if let Some(lang) = params
            .lang
            .as_deref()
            .map(|value| value.trim().to_lowercase())
        {
            if !lang.is_empty() {
                filters.push(json!({ "term": { "language": lang } }));
            }
        }
        if let Some(cutoff) = freshness_cutoff {
            filters.push(json!({ "range": { "fetched_at": { "gte": cutoff } } }));
        }
        if let Some(site_filter) = build_site_filter(params.site.as_deref()) {
            filters.push(site_filter);
        }

        let cache_key = if params.offset == 0
            && params.lang.is_none()
            && params.site.is_none()
            && matches!(params.freshness, Freshness::All)
        {
            Some(format!("search:{}:{}", query, limit))
        } else {
            None
        };

        Self {
            query,
            limit,
            offset,
            did_you_mean,
            filters,
            cache_key,
        }
    }

    pub fn request_body(&self) -> Value {
        json!({
            "from": self.offset,
            "size": self.limit,
            "_source": [
                "doc_id",
                "canonical_url",
                "display_url",
                "title",
                "snippet",
                "language",
                "fetched_at"
            ],
            "query": {
                "function_score": {
                    "query": {
                        "bool": {
                            "filter": self.filters.clone(),
                            "should": [
                                {
                                    "multi_match": {
                                        "query": self.query.as_str(),
                                        "fields": ["title^10", "snippet^3", "body"],
                                        "operator": "and",
                                        "fuzziness": "AUTO"
                                    }
                                },
                                {
                                    "multi_match": {
                                        "query": self.query.as_str(),
                                        "fields": ["title^12", "snippet^3"],
                                        "type": "phrase_prefix"
                                    }
                                },
                                {
                                    "match": {
                                        "display_url": {
                                            "query": self.query.as_str(),
                                            "boost": 0.5
                                        }
                                    }
                                }
                            ],
                            "minimum_should_match": 1
                        }
                    },
                    "functions": [
                        {
                            "field_value_factor": {
                                "field": "site_authority",
                                "factor": 2.0,
                                "missing": 0.5
                            }
                        },
                        {
                            "exp": {
                                "fetched_at": {
                                    "origin": "now",
                                    "scale": "60d",
                                    "offset": "7d",
                                    "decay": 0.3
                                }
                            }
                        }
                    ],
                    "score_mode": "multiply",
                    "boost_mode": "multiply"
                }
            },
            "sort": [
                { "_score": { "order": "desc" } },
                { "fetched_at": { "order": "desc" } }
            ]
        })
    }

    pub fn empty_response(&self) -> SearchResponse {
        SearchResponse {
            query: self.query.clone(),
            took_ms: 0,
            total_estimate: 0,
            next_offset: None,
            results: Vec::new(),
            did_you_mean: self.did_you_mean.clone(),
        }
    }

    pub fn map_response(&self, payload: OpenSearchSearchResponse) -> SearchResponse {
        let results: Vec<SearchResult> = payload
            .hits
            .hits
            .into_iter()
            .map(|hit| SearchResult {
                id: hit.source.doc_id.unwrap_or(hit.id),
                title: hit.source.title,
                url: hit.source.canonical_url,
                display_url: hit.source.display_url,
                snippet: hit.source.snippet,
                language: hit.source.language,
                last_crawled_at: hit.source.fetched_at,
                score: hit.score.unwrap_or_default(),
            })
            .collect();

        let total_estimate = payload.hits.total.value as usize;
        let next_offset = if self.offset + results.len() < total_estimate {
            Some(self.offset + results.len())
        } else {
            None
        };

        SearchResponse {
            query: self.query.clone(),
            took_ms: payload.took as u128,
            total_estimate,
            next_offset,
            results,
            did_you_mean: self.did_you_mean.clone(),
        }
    }
}

pub fn build_suggest_body(query: &str) -> Value {
    json!({
        "suggest": {
            "query-suggest": {
                "prefix": query.trim().to_lowercase(),
                "completion": {
                    "field": "suggest_input",
                    "size": 8,
                    "skip_duplicates": true
                }
            }
        }
    })
}

pub fn map_suggest_response(query: &str, body: OpenSearchSuggestResponse) -> SuggestResponse {
    let suggestions = body
        .suggest
        .query_suggest
        .into_iter()
        .flat_map(|entry| entry.options.into_iter().map(|option| option.text))
        .take(8)
        .collect();

    SuggestResponse {
        query: query.to_string(),
        suggestions,
    }
}

fn build_site_filter(site: Option<&str>) -> Option<Value> {
    let mut normalized = site?.trim().to_lowercase();
    if normalized.is_empty() {
        return None;
    }

    if let Some(stripped) = normalized.strip_prefix("http://") {
        normalized = stripped.to_string();
    } else if let Some(stripped) = normalized.strip_prefix("https://") {
        normalized = stripped.to_string();
    }
    normalized = normalized.trim_matches('/').to_string();

    Some(json!({
        "bool": {
            "should": [
                { "term": { "host": normalized } },
                { "wildcard": { "host": { "value": format!("*.{normalized}") } } },
                { "wildcard": { "canonical_url": { "value": format!("*{normalized}*") } } }
            ],
            "minimum_should_match": 1
        }
    }))
}

#[derive(Debug, Deserialize)]
pub struct OpenSearchSearchResponse {
    pub took: u64,
    pub hits: OpenSearchHits,
}

#[derive(Debug, Deserialize)]
pub struct OpenSearchHits {
    pub total: OpenSearchTotal,
    pub hits: Vec<OpenSearchHit>,
}

#[derive(Debug, Deserialize)]
pub struct OpenSearchTotal {
    pub value: u64,
}

#[derive(Debug, Deserialize)]
pub struct OpenSearchHit {
    #[serde(rename = "_id")]
    pub id: String,
    #[serde(rename = "_score")]
    pub score: Option<f32>,
    #[serde(rename = "_source")]
    pub source: SearchHitSource,
}

#[derive(Debug, Deserialize)]
pub struct SearchHitSource {
    pub doc_id: Option<String>,
    pub canonical_url: String,
    pub display_url: String,
    pub title: String,
    pub snippet: String,
    pub language: String,
    pub fetched_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct OpenSearchSuggestResponse {
    pub suggest: SuggestBody,
}

#[derive(Debug, Deserialize)]
pub struct SuggestBody {
    #[serde(rename = "query-suggest")]
    pub query_suggest: Vec<SuggestEntry>,
}

#[derive(Debug, Deserialize)]
pub struct SuggestEntry {
    pub options: Vec<SuggestOption>,
}

#[derive(Debug, Deserialize)]
pub struct SuggestOption {
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::PreparedSearch;
    use crate::models::{Freshness, SearchParams};

    #[test]
    fn prepared_search_builds_simple_cache_key() {
        let plan = PreparedSearch::from_params(&SearchParams {
            q: "ranking".to_string(),
            limit: 10,
            offset: 0,
            lang: None,
            site: None,
            freshness: Freshness::All,
        });

        assert_eq!(plan.cache_key.as_deref(), Some("search:ranking:10"));
    }
}

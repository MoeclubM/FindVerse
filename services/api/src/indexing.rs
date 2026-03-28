use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use findverse_common::{
    CURRENT_INDEX_VERSION, CURRENT_PARSER_VERSION, CURRENT_SCHEMA_VERSION, content_hash,
    derive_terms, display_url, extract_host, normalize_url, word_count,
};
use serde::Serialize;

use crate::models::IndexedDocument;

#[derive(Debug, Clone, Default)]
pub struct IngestBatchOutcome {
    pub accepted_documents: usize,
    pub duplicate_documents: usize,
    pub skipped_documents: usize,
}

#[derive(Debug, Clone)]
pub struct NormalizedDocument {
    pub id: String,
    pub title: String,
    pub url: String,
    pub canonical_url: String,
    pub host: String,
    pub display_url: String,
    pub snippet: String,
    pub body: String,
    pub language: String,
    pub last_crawled_at: DateTime<Utc>,
    pub content_hash: String,
    pub suggest_terms: Vec<String>,
    pub site_authority: f32,
    pub content_type: String,
    pub word_count: u32,
    pub network: String,
    pub source_job_id: Option<String>,
    pub parser_version: i32,
    pub schema_version: i32,
    pub index_version: i32,
    pub duplicate_of: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct IndexedDocumentPayload {
    pub doc_id: String,
    pub canonical_url: String,
    pub display_url: String,
    pub host: String,
    pub title: String,
    pub snippet: String,
    pub body: String,
    pub language: String,
    pub fetched_at: DateTime<Utc>,
    pub content_hash: String,
    pub site_authority: f32,
    pub content_type: String,
    pub word_count: u32,
    pub network: String,
    pub suggest_input: Vec<String>,
}

impl IndexedDocumentPayload {
    pub fn from_document(document: &NormalizedDocument) -> Self {
        Self {
            doc_id: document.id.clone(),
            canonical_url: document.canonical_url.clone(),
            display_url: document.display_url.clone(),
            host: document.host.clone(),
            title: document.title.clone(),
            snippet: document.snippet.clone(),
            body: document.body.clone(),
            language: document.language.clone(),
            fetched_at: document.last_crawled_at,
            content_hash: document.content_hash.clone(),
            site_authority: document.site_authority,
            content_type: document.content_type.clone(),
            word_count: document.word_count,
            network: document.network.clone(),
            suggest_input: build_suggest_inputs(document),
        }
    }
}

pub fn normalize_document(document: IndexedDocument) -> NormalizedDocument {
    let canonical_url = document
        .canonical_url
        .unwrap_or_else(|| normalize_url(&document.url).unwrap_or_else(|| document.url.clone()));
    let host = document
        .host
        .or_else(|| extract_host(&canonical_url))
        .unwrap_or_else(|| "unknown".to_string());
    let body = document.body.trim().chars().take(8_000).collect::<String>();
    let title = if document.title.trim().is_empty() {
        canonical_url.clone()
    } else {
        document.title.trim().to_string()
    };
    let snippet = if document.snippet.trim().is_empty() {
        body.chars().take(220).collect()
    } else {
        document.snippet.trim().chars().take(220).collect()
    };
    let mut suggest_term_set = BTreeSet::new();
    suggest_term_set.extend(
        document
            .suggest_terms
            .into_iter()
            .map(|term| term.trim().to_lowercase())
            .filter(|term| !term.is_empty()),
    );
    suggest_term_set.extend(derive_terms(&title, &body));
    let suggest_terms = suggest_term_set.into_iter().take(12).collect();
    let computed_word_count = if document.word_count == 0 {
        word_count(&body) as u32
    } else {
        document.word_count
    };

    NormalizedDocument {
        id: document.id,
        title,
        url: document.url,
        canonical_url: canonical_url.clone(),
        host,
        display_url: if document.display_url.trim().is_empty() {
            display_url(&canonical_url)
        } else {
            document.display_url
        },
        snippet,
        body: body.clone(),
        language: document.language.trim().to_lowercase(),
        last_crawled_at: document.last_crawled_at,
        content_hash: document.content_hash.unwrap_or_else(|| content_hash(&body)),
        suggest_terms,
        site_authority: document.site_authority.max(0.1),
        content_type: if document.content_type.trim().is_empty() {
            "text/html".to_string()
        } else {
            document.content_type.trim().to_lowercase()
        },
        word_count: computed_word_count,
        network: document.network,
        source_job_id: document.source_job_id,
        parser_version: if document.parser_version <= 0 {
            CURRENT_PARSER_VERSION
        } else {
            document.parser_version
        },
        schema_version: if document.schema_version <= 0 {
            CURRENT_SCHEMA_VERSION
        } else {
            document.schema_version
        },
        index_version: if document.index_version <= 0 {
            CURRENT_INDEX_VERSION
        } else {
            document.index_version
        },
        duplicate_of: document.duplicate_of,
    }
}

pub fn build_suggest_inputs(document: &NormalizedDocument) -> Vec<String> {
    let mut values = BTreeSet::new();
    values.insert(document.title.to_lowercase());
    values.insert(document.host.to_lowercase());
    values.extend(document.suggest_terms.iter().cloned());
    values.into_iter().take(12).collect()
}

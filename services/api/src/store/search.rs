use std::{path::PathBuf, time::Duration};

use anyhow::Context;
use chrono::Utc;
use redis::AsyncCommands;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use tracing::warn;

use crate::{
    error::ApiError,
    indexing::{
        IndexedDocumentPayload, IngestBatchOutcome, NormalizedDocument, normalize_document,
    },
    models::{
        DocumentListParams, DocumentListResponse, DocumentSummary, IndexedDocument,
        PurgeSiteResponse, ReadyResponse, SearchParams, SearchResponse, SuggestResponse,
    },
    query::pipeline::{
        OpenSearchSearchResponse, OpenSearchSuggestResponse, PreparedSearch, build_suggest_body,
        map_suggest_response,
    },
};

use super::ensure_file_with_fallbacks;

#[derive(Debug, Clone)]
pub struct SearchIndex {
    pg_pool: PgPool,
    http_client: Client,
    opensearch_url: String,
    opensearch_index: String,
    redis_client: redis::Client,
}

impl SearchIndex {
    pub async fn connect(
        pg_pool: PgPool,
        opensearch_url: String,
        opensearch_index: String,
        redis_client: redis::Client,
    ) -> anyhow::Result<Self> {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .context("failed to build opensearch client")?;

        let index = Self {
            pg_pool,
            http_client,
            opensearch_url: opensearch_url.trim_end_matches('/').to_string(),
            opensearch_index,
            redis_client,
        };

        index.ensure_index().await?;

        Ok(index)
    }

    pub async fn bootstrap_from_path(&self, path: PathBuf) -> anyhow::Result<()> {
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

        if documents.is_empty() {
            return Ok(());
        }

        let existing = sqlx::query_scalar::<_, i64>("select count(*) from documents")
            .fetch_one(&self.pg_pool)
            .await
            .unwrap_or(0);
        let should_import = if existing == 0 {
            true
        } else {
            let indexed = match self
                .http_client
                .get(self.index_endpoint("/_count"))
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => response
                    .json::<OpenSearchCountResponse>()
                    .await
                    .map(|payload| payload.count)
                    .unwrap_or(0),
                Ok(response) => {
                    warn!(
                        status = %response.status(),
                        "failed to inspect bootstrap opensearch count, reimporting bootstrap documents"
                    );
                    0
                }
                Err(error) => {
                    warn!(
                        ?error,
                        "failed to inspect bootstrap opensearch count, reimporting bootstrap documents"
                    );
                    0
                }
            };

            if indexed == 0 {
                warn!(
                    postgres_documents = existing,
                    "bootstrap documents exist in postgres but are missing from opensearch, reimporting"
                );
                true
            } else {
                false
            }
        };

        if should_import {
            if let Err(error) = self.upsert_documents(documents).await {
                warn!(?error, "failed to import bootstrap documents");
            }
        }

        Ok(())
    }

    pub async fn readiness(&self, postgres_ready: bool, redis_ready: bool) -> ReadyResponse {
        let opensearch_ready = self.ping().await;
        let frontier_depth =
            sqlx::query_scalar::<_, i32>("select count(*) from crawl_jobs where status = 'queued'")
                .fetch_one(&self.pg_pool)
                .await
                .unwrap_or(0);

        ReadyResponse {
            status: if postgres_ready && redis_ready && opensearch_ready {
                "ready"
            } else {
                "degraded"
            },
            postgres: postgres_ready,
            redis: redis_ready,
            opensearch: opensearch_ready,
            frontier_depth,
        }
    }

    pub async fn ping(&self) -> bool {
        self.http_client
            .get(self.index_endpoint(""))
            .send()
            .await
            .map(|response| {
                response.status().is_success() || response.status() == StatusCode::NOT_FOUND
            })
            .unwrap_or(false)
    }

    pub async fn total_documents(&self) -> usize {
        sqlx::query_scalar::<_, i64>("select count(*) from documents where duplicate_of is null")
            .fetch_one(&self.pg_pool)
            .await
            .unwrap_or(0) as usize
    }

    pub async fn duplicate_documents(&self) -> usize {
        sqlx::query_scalar::<_, i64>(
            "select count(*) from documents where duplicate_of is not null",
        )
        .fetch_one(&self.pg_pool)
        .await
        .unwrap_or(0) as usize
    }

    pub async fn search(&self, params: SearchParams) -> SearchResponse {
        let plan = PreparedSearch::from_params(&params);

        if let Some(cache_key) = plan.cache_key.as_deref() {
            if let Ok(cached) = self.get_cached_search(cache_key).await {
                return cached;
            }
        }

        let response = match self
            .http_client
            .post(self.index_endpoint("/_search"))
            .json(&plan.request_body())
            .send()
            .await
        {
            Ok(response) => response,
            Err(error) => {
                warn!(?error, query = %plan.query, "opensearch query failed");
                return plan.empty_response();
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!(status = %status, body = %body, query = %plan.query, "opensearch returned error");
            return plan.empty_response();
        }

        let payload: OpenSearchSearchResponse = match response.json().await {
            Ok(payload) => payload,
            Err(error) => {
                warn!(?error, query = %plan.query, "failed to decode opensearch search response");
                return plan.empty_response();
            }
        };

        let mapped = plan.map_response(payload);

        if let Some(cache_key) = plan.cache_key.as_deref() {
            self.set_cached_search(cache_key, &mapped).await;
        }

        mapped
    }

    async fn get_cached_search(&self, key: &str) -> Result<SearchResponse, ()> {
        let mut conn = self
            .redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|_| ())?;
        let cached: String = conn.get(key).await.map_err(|_| ())?;
        serde_json::from_str(&cached).map_err(|_| ())
    }

    async fn set_cached_search(&self, key: &str, response: &SearchResponse) {
        if let Ok(mut conn) = self.redis_client.get_multiplexed_async_connection().await {
            if let Ok(json) = serde_json::to_string(response) {
                let _: Result<(), _> = conn.set_ex(key, json, 60).await;
            }
        }
    }

    pub async fn suggest(&self, query: &str) -> SuggestResponse {
        let suggestions = match self
            .http_client
            .post(self.index_endpoint("/_search"))
            .json(&build_suggest_body(query))
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                match response.json::<OpenSearchSuggestResponse>().await {
                    Ok(body) => return map_suggest_response(query, body),
                    Err(error) => {
                        warn!(?error, query = %query, "failed to decode suggest response");
                        Vec::new()
                    }
                }
            }
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                warn!(status = %status, body = %body, query = %query, "suggest request failed");
                Vec::new()
            }
            Err(error) => {
                warn!(?error, query = %query, "suggest request failed");
                Vec::new()
            }
        };

        SuggestResponse {
            query: query.to_string(),
            suggestions,
        }
    }

    pub async fn upsert_documents(
        &self,
        documents: Vec<IndexedDocument>,
    ) -> Result<IngestBatchOutcome, ApiError> {
        let mut outcome = IngestBatchOutcome::default();
        if documents.is_empty() {
            return Ok(outcome);
        }

        for document in documents {
            let mut normalized = normalize_document(document);
            if normalized.title.trim().is_empty() || normalized.body.trim().is_empty() {
                outcome.skipped_documents += 1;
                continue;
            }

            normalized.duplicate_of = sqlx::query_scalar(
                "select id from documents where content_hash = $1 and id != $2 and duplicate_of is null limit 1",
            )
            .bind(&normalized.content_hash)
            .bind(&normalized.id)
            .fetch_optional(&self.pg_pool)
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

            let replaced_document_ids = self.persist_document(&normalized).await?;

            if normalized.duplicate_of.is_some() {
                self.delete_index_document(&normalized.id).await;
                outcome.duplicate_documents += 1;
            } else {
                self.index_document(&normalized).await?;
                for document_id in replaced_document_ids {
                    self.delete_index_document(&document_id).await;
                }
            }

            outcome.accepted_documents += 1;
        }

        Ok(outcome)
    }

    pub async fn list_documents(&self, params: DocumentListParams) -> DocumentListResponse {
        let limit = params.limit.clamp(1, 50) as i64;
        let offset = params.offset as i64;
        let query_pattern = params
            .query
            .as_deref()
            .map(|value| format!("%{}%", value.trim().to_lowercase()));
        let site_pattern = params
            .site
            .as_deref()
            .map(|value| format!("%{}%", value.trim().to_lowercase()));

        let total_estimate: i64 = sqlx::query_scalar(
            r#"
            select count(*) from documents
            where ($1::text is null or (
                lower(title) like $1
                or lower(snippet) like $1
                or lower(canonical_url) like $1
            ))
            and ($2::text is null or lower(host) like $2 or lower(canonical_url) like $2)
            "#,
        )
        .bind(query_pattern.as_deref())
        .bind(site_pattern.as_deref())
        .fetch_one(&self.pg_pool)
        .await
        .unwrap_or(0);

        let rows = sqlx::query_as::<_, DocumentSummaryRow>(
            r#"
            select
                id,
                title,
                url,
                canonical_url,
                host,
                display_url,
                snippet,
                language,
                last_crawled_at,
                content_type,
                word_count,
                site_authority,
                parser_version,
                schema_version,
                index_version,
                source_job_id,
                duplicate_of
            from documents
            where ($1::text is null or (
                lower(title) like $1
                or lower(snippet) like $1
                or lower(canonical_url) like $1
            ))
            and ($2::text is null or lower(host) like $2 or lower(canonical_url) like $2)
            order by (duplicate_of is not null), last_crawled_at desc
            limit $3 offset $4
            "#,
        )
        .bind(query_pattern.as_deref())
        .bind(site_pattern.as_deref())
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pg_pool)
        .await
        .unwrap_or_default();

        let documents = rows
            .into_iter()
            .map(|row| DocumentSummary {
                id: row.id,
                title: row.title,
                url: row.url,
                canonical_url: row.canonical_url,
                host: row.host,
                display_url: row.display_url,
                snippet: row.snippet,
                language: row.language,
                last_crawled_at: row.last_crawled_at,
                content_type: row.content_type,
                word_count: row.word_count as u32,
                site_authority: row.site_authority,
                parser_version: row.parser_version,
                schema_version: row.schema_version,
                index_version: row.index_version,
                source_job_id: row.source_job_id,
                duplicate_of: row.duplicate_of,
            })
            .collect::<Vec<_>>();

        let next_offset = if (offset as usize) + documents.len() < total_estimate as usize {
            Some((offset as usize) + documents.len())
        } else {
            None
        };

        DocumentListResponse {
            total_estimate: total_estimate as usize,
            next_offset,
            documents,
        }
    }

    pub async fn delete_document(&self, document_id: &str) -> Result<bool, ApiError> {
        let deleted = sqlx::query("delete from documents where id = $1")
            .bind(document_id)
            .execute(&self.pg_pool)
            .await
            .map_err(|error| ApiError::Internal(error.into()))?
            .rows_affected();

        if deleted > 0 {
            self.delete_index_document(document_id).await;
        }

        Ok(deleted > 0)
    }

    pub async fn purge_site(&self, site: &str) -> Result<PurgeSiteResponse, ApiError> {
        let normalized = site.trim().to_lowercase();
        if normalized.is_empty() {
            return Err(ApiError::BadRequest("site must not be empty".to_string()));
        }

        let pattern = format!("%{normalized}%");
        let document_ids: Vec<String> = sqlx::query_scalar(
            "select id from documents where lower(host) like $1 or lower(canonical_url) like $1",
        )
        .bind(&pattern)
        .fetch_all(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

        let deleted = sqlx::query(
            "delete from documents where lower(host) like $1 or lower(canonical_url) like $1",
        )
        .bind(&pattern)
        .execute(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?
        .rows_affected();

        for id in document_ids {
            self.delete_index_document(&id).await;
        }

        Ok(PurgeSiteResponse {
            deleted_documents: deleted as usize,
        })
    }

    async fn persist_document(
        &self,
        document: &NormalizedDocument,
    ) -> Result<Vec<String>, ApiError> {
        let suggest_terms = serde_json::to_value(&document.suggest_terms)
            .map_err(|error| ApiError::Internal(error.into()))?;

        sqlx::query(
            r#"
            insert into documents (
                id,
                url,
                canonical_url,
                host,
                content_hash,
                title,
                display_url,
                snippet,
                body,
                language,
                site_authority,
                suggest_terms,
                last_crawled_at,
                content_type,
                word_count,
                network,
                source_job_id,
                parser_version,
                schema_version,
                index_version,
                duplicate_of,
                search_vector
            )
            values (
                $1,
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                $8,
                $9,
                $10,
                $11,
                $12,
                $13,
                $14,
                $15,
                $16,
                $17,
                $18,
                $19,
                $20,
                $21,
                setweight(to_tsvector('simple', $6), 'A') ||
                setweight(to_tsvector('simple', $8), 'B') ||
                setweight(to_tsvector('simple', $9), 'C') ||
                setweight(to_tsvector('simple', $3), 'D')
            )
            on conflict (id) do update set
                url = excluded.url,
                canonical_url = excluded.canonical_url,
                host = excluded.host,
                content_hash = excluded.content_hash,
                title = excluded.title,
                display_url = excluded.display_url,
                snippet = excluded.snippet,
                body = excluded.body,
                language = excluded.language,
                site_authority = excluded.site_authority,
                suggest_terms = excluded.suggest_terms,
                last_crawled_at = excluded.last_crawled_at,
                content_type = excluded.content_type,
                word_count = excluded.word_count,
                network = excluded.network,
                source_job_id = excluded.source_job_id,
                parser_version = excluded.parser_version,
                schema_version = excluded.schema_version,
                index_version = excluded.index_version,
                duplicate_of = excluded.duplicate_of,
                search_vector = excluded.search_vector
            "#,
        )
        .bind(&document.id)
        .bind(&document.url)
        .bind(&document.canonical_url)
        .bind(&document.host)
        .bind(&document.content_hash)
        .bind(&document.title)
        .bind(&document.display_url)
        .bind(&document.snippet)
        .bind(&document.body)
        .bind(&document.language)
        .bind(document.site_authority)
        .bind(&suggest_terms)
        .bind(document.last_crawled_at)
        .bind(&document.content_type)
        .bind(document.word_count as i32)
        .bind(&document.network)
        .bind(document.source_job_id.as_deref())
        .bind(document.parser_version)
        .bind(document.schema_version)
        .bind(document.index_version)
        .bind(document.duplicate_of.as_deref())
        .execute(&self.pg_pool)
        .await
        .map_err(|error| ApiError::Internal(error.into()))?;

        let mut replaced_document_ids = Vec::new();
        if document.duplicate_of.is_none() {
            replaced_document_ids = sqlx::query_scalar(
                "delete from documents where canonical_url = $1 and id != $2 and duplicate_of is null returning id",
            )
                .bind(&document.canonical_url)
                .bind(&document.id)
                .fetch_all(&self.pg_pool)
                .await
                .map_err(|error| ApiError::Internal(error.into()))?;
        }

        Ok(replaced_document_ids)
    }

    async fn ensure_index(&self) -> anyhow::Result<()> {
        let response = self
            .http_client
            .put(self.index_endpoint(""))
            .json(&json!({
                "settings": {
                    "number_of_shards": 1,
                    "number_of_replicas": 0
                },
                "mappings": {
                    "properties": self.mapping_properties()
                }
            }))
            .send()
            .await
            .context("failed to create opensearch index")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if !(status == StatusCode::BAD_REQUEST
                && body.contains("resource_already_exists_exception"))
            {
                anyhow::bail!("opensearch index initialization failed: {status} {body}");
            }
        }

        self.ensure_mapping().await
    }

    async fn ensure_mapping(&self) -> anyhow::Result<()> {
        let response = self
            .http_client
            .put(self.index_endpoint("/_mapping"))
            .json(&json!({
                "properties": self.mapping_properties()
            }))
            .send()
            .await
            .context("failed to update opensearch mapping")?;

        if response.status().is_success() {
            return Ok(());
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("opensearch mapping update failed: {status} {body}");
    }

    fn mapping_properties(&self) -> serde_json::Value {
        json!({
            "doc_id": { "type": "keyword" },
            "canonical_url": { "type": "keyword" },
            "display_url": {
                "type": "text",
                "fields": {
                    "keyword": { "type": "keyword" }
                }
            },
            "host": { "type": "keyword" },
            "title": { "type": "text" },
            "snippet": { "type": "text" },
            "body": { "type": "text" },
            "language": { "type": "keyword" },
            "fetched_at": { "type": "date" },
            "content_hash": { "type": "keyword" },
            "site_authority": { "type": "float" },
            "content_type": { "type": "keyword" },
            "word_count": { "type": "integer" },
            "network": { "type": "keyword" },
            "suggest_input": { "type": "completion" }
        })
    }

    async fn index_document(&self, document: &NormalizedDocument) -> Result<(), ApiError> {
        let payload = IndexedDocumentPayload::from_document(document);
        let response = self
            .http_client
            .put(self.index_endpoint(&format!("/_doc/{}?refresh=wait_for", document.id)))
            .json(&payload)
            .send()
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

        if response.status().is_success() {
            return Ok(());
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(ApiError::Internal(anyhow::anyhow!(
            "opensearch index write failed: {status} {body}"
        )))
    }

    async fn delete_index_document(&self, document_id: &str) {
        match self
            .http_client
            .delete(self.index_endpoint(&format!("/_doc/{document_id}?refresh=wait_for")))
            .send()
            .await
        {
            Ok(response)
                if response.status().is_success() || response.status() == StatusCode::NOT_FOUND => {
            }
            Ok(response) => {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                warn!(%status, %body, document_id, "failed to delete opensearch document");
            }
            Err(error) => warn!(?error, document_id, "failed to delete opensearch document"),
        }
    }

    fn index_endpoint(&self, suffix: &str) -> String {
        format!(
            "{}/{}{}",
            self.opensearch_url.trim_end_matches('/'),
            self.opensearch_index,
            suffix
        )
    }
}

#[derive(sqlx::FromRow)]
struct DocumentSummaryRow {
    id: String,
    title: String,
    url: String,
    canonical_url: String,
    host: String,
    display_url: String,
    snippet: String,
    language: String,
    last_crawled_at: chrono::DateTime<Utc>,
    content_type: String,
    word_count: i32,
    site_authority: f32,
    parser_version: i32,
    schema_version: i32,
    index_version: i32,
    source_job_id: Option<String>,
    duplicate_of: Option<String>,
}

#[derive(Deserialize)]
struct OpenSearchCountResponse {
    count: i64,
}

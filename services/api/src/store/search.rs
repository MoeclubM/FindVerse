use std::{fmt::Write, path::PathBuf, time::Duration};

use anyhow::Context;
use chrono::Utc;
use redis::AsyncCommands;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use tracing::warn;

use crate::{
    blob_store::BlobStore,
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

#[derive(Debug, Clone)]
pub struct SearchIndex {
    pg_pool: PgPool,
    http_client: Client,
    opensearch_url: String,
    opensearch_index: String,
    opensearch_read_alias: String,
    opensearch_write_alias: String,
    blob_store: BlobStore,
    redis_client: redis::Client,
}

impl SearchIndex {
    pub async fn connect(
        pg_pool: PgPool,
        opensearch_url: String,
        opensearch_index: String,
        blob_store: BlobStore,
        redis_client: redis::Client,
    ) -> anyhow::Result<Self> {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .context("failed to build opensearch client")?;

        Ok(Self {
            pg_pool,
            http_client,
            opensearch_url: opensearch_url.trim_end_matches('/').to_string(),
            opensearch_read_alias: format!("{opensearch_index}-read"),
            opensearch_write_alias: format!("{opensearch_index}-write"),
            opensearch_index,
            blob_store,
            redis_client,
        })
    }

    pub async fn bootstrap_from_path(&self, path: PathBuf) -> anyhow::Result<()> {
        if tokio::fs::metadata(&path).await.is_err() {
            return Ok(());
        }

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
                .get(self.read_endpoint("/_count"))
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

    pub async fn bootstrap_storage(&self) -> anyhow::Result<()> {
        self.ensure_index().await?;
        self.ensure_alias(&self.opensearch_read_alias, false)
            .await?;
        self.ensure_alias(&self.opensearch_write_alias, true).await
    }

    pub async fn reindex_existing_documents(&self, batch_size: usize) -> anyhow::Result<usize> {
        let postgres_documents = sqlx::query_scalar::<_, i64>(
            "select count(*) from documents where duplicate_of is null",
        )
        .fetch_one(&self.pg_pool)
        .await?;
        if postgres_documents == 0 {
            return Ok(0);
        }

        let indexed_documents = self.opensearch_document_count().await;
        if indexed_documents >= postgres_documents {
            return Ok(0);
        }

        let mut reindexed = 0usize;
        let mut last_id: Option<String> = None;
        loop {
            let rows = sqlx::query_as::<_, ReindexDocumentRow>(
                r#"
                select
                    id,
                    title,
                    url,
                    canonical_url,
                    host,
                    display_url,
                    snippet,
                    text_blob_key,
                    language,
                    last_crawled_at,
                    content_hash,
                    suggest_terms,
                    site_authority,
                    content_type,
                    word_count,
                    network,
                    source_job_id,
                    parser_version,
                    schema_version,
                    index_version,
                    duplicate_of
                from documents
                where duplicate_of is null
                  and ($1::text is null or id > $1)
                order by id
                limit $2
                "#,
            )
            .bind(last_id.as_deref())
            .bind(batch_size.max(1) as i64)
            .fetch_all(&self.pg_pool)
            .await?;

            if rows.is_empty() {
                break;
            }

            let mut documents = Vec::with_capacity(rows.len());
            for row in rows {
                let blob_key = row.text_blob_key.as_deref().ok_or_else(|| {
                    anyhow::anyhow!(
                        "document {} is missing text_blob_key; run bootstrap migration first",
                        row.id
                    )
                })?;
                let body = self
                    .blob_store
                    .load_text_blob(blob_key)
                    .await
                    .map_err(anyhow::Error::from)?;
                let suggest_terms = serde_json::from_value::<Vec<String>>(row.suggest_terms)
                    .with_context(|| format!("failed to decode suggest_terms for {}", row.id))?;

                documents.push(normalize_document(IndexedDocument {
                    id: row.id.clone(),
                    title: row.title,
                    url: row.url,
                    display_url: row.display_url,
                    snippet: row.snippet,
                    body,
                    language: row.language,
                    last_crawled_at: row.last_crawled_at,
                    canonical_url: Some(row.canonical_url),
                    host: Some(row.host),
                    content_hash: Some(row.content_hash),
                    suggest_terms,
                    site_authority: row.site_authority,
                    content_type: row.content_type,
                    word_count: row.word_count as u32,
                    network: row.network,
                    source_job_id: row.source_job_id,
                    parser_version: row.parser_version,
                    schema_version: row.schema_version,
                    index_version: row.index_version,
                    duplicate_of: row.duplicate_of,
                }));
                last_id = Some(row.id);
            }

            self.bulk_index_documents(&documents)
                .await
                .map_err(anyhow::Error::from)?;
            reindexed += documents.len();
        }

        Ok(reindexed)
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
            .get(self.read_endpoint(""))
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
            .post(self.read_endpoint("/_search"))
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
            .post(self.read_endpoint("/_search"))
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

        let mut documents_to_index = Vec::new();
        let mut document_ids_to_delete = Vec::new();

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

            let replaced_document_rows = self.persist_document(&normalized).await?;

            if normalized.duplicate_of.is_some() {
                document_ids_to_delete.push(normalized.id.clone());
                outcome.duplicate_documents += 1;
            } else {
                documents_to_index.push(normalized);
            }

            for row in replaced_document_rows {
                document_ids_to_delete.push(row.id);
                if let Some(blob_key) = row.text_blob_key {
                    self.delete_document_blob(&blob_key).await;
                }
            }

            outcome.accepted_documents += 1;
        }

        if !documents_to_index.is_empty() {
            self.bulk_index_documents(&documents_to_index).await?;
        }
        if !document_ids_to_delete.is_empty() {
            self.bulk_delete_documents(&document_ids_to_delete).await;
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
        let blob_key: Option<String> =
            sqlx::query_scalar("select text_blob_key from documents where id = $1")
                .bind(document_id)
                .fetch_optional(&self.pg_pool)
                .await
                .map_err(|error| ApiError::Internal(error.into()))?
                .flatten();

        let deleted = sqlx::query("delete from documents where id = $1")
            .bind(document_id)
            .execute(&self.pg_pool)
            .await
            .map_err(|error| ApiError::Internal(error.into()))?
            .rows_affected();

        if deleted > 0 {
            self.delete_index_document(document_id).await;
            if let Some(blob_key) = blob_key {
                self.delete_document_blob(&blob_key).await;
            }
        }

        Ok(deleted > 0)
    }

    pub async fn purge_site(&self, site: &str) -> Result<PurgeSiteResponse, ApiError> {
        let normalized = site.trim().to_lowercase();
        if normalized.is_empty() {
            return Err(ApiError::BadRequest("site must not be empty".to_string()));
        }

        let pattern = format!("%{normalized}%");
        let document_rows = sqlx::query_as::<_, DeletedDocumentRow>(
            "select id, text_blob_key from documents where lower(host) like $1 or lower(canonical_url) like $1",
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

        if !document_rows.is_empty() {
            self.bulk_delete_documents(
                &document_rows
                    .iter()
                    .map(|row| row.id.clone())
                    .collect::<Vec<_>>(),
            )
            .await;
            for row in document_rows {
                if let Some(blob_key) = row.text_blob_key {
                    self.delete_document_blob(&blob_key).await;
                }
            }
        }

        Ok(PurgeSiteResponse {
            deleted_documents: deleted as usize,
        })
    }

    async fn persist_document(
        &self,
        document: &NormalizedDocument,
    ) -> Result<Vec<DeletedDocumentRow>, ApiError> {
        let suggest_terms = serde_json::to_value(&document.suggest_terms)
            .map_err(|error| ApiError::Internal(error.into()))?;
        let stored_body = document.body.chars().take(2_048).collect::<String>();
        let text_blob_key = format!("documents/{}.txt", document.id);
        self.blob_store
            .write_text_blob(&text_blob_key, &document.body)
            .await?;

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
                text_blob_key,
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
                $22,
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
                text_blob_key = excluded.text_blob_key,
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
        .bind(&stored_body)
        .bind(&text_blob_key)
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

        let mut replaced_document_rows = Vec::new();
        if document.duplicate_of.is_none() {
            replaced_document_rows = sqlx::query_as::<_, DeletedDocumentRow>(
                "delete from documents
                 where canonical_url = $1 and id != $2 and duplicate_of is null
                 returning id, text_blob_key",
            )
            .bind(&document.canonical_url)
            .bind(&document.id)
            .fetch_all(&self.pg_pool)
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;
        }

        Ok(replaced_document_rows)
    }

    async fn ensure_index(&self) -> anyhow::Result<()> {
        let versioned_index = self.versioned_index();
        let response = self
            .http_client
            .put(self.raw_index_endpoint(&versioned_index, ""))
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
        let versioned_index = self.versioned_index();
        let response = self
            .http_client
            .put(self.raw_index_endpoint(&versioned_index, "/_mapping"))
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

    async fn delete_index_document(&self, document_id: &str) {
        match self
            .http_client
            .delete(self.write_endpoint(&format!("/_doc/{document_id}")))
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

    async fn bulk_index_documents(&self, documents: &[NormalizedDocument]) -> Result<(), ApiError> {
        let mut body = String::new();
        for document in documents {
            writeln!(
                &mut body,
                "{}",
                serde_json::json!({ "index": { "_id": document.id } })
            )
            .expect("write to string");
            writeln!(
                &mut body,
                "{}",
                serde_json::to_string(&IndexedDocumentPayload::from_document(document))
                    .map_err(|error| ApiError::Internal(error.into()))?
            )
            .expect("write to string");
        }

        self.submit_bulk_request(body).await
    }

    async fn bulk_delete_documents(&self, document_ids: &[String]) {
        let mut body = String::new();
        for document_id in document_ids {
            writeln!(
                &mut body,
                "{}",
                serde_json::json!({ "delete": { "_id": document_id } })
            )
            .expect("write to string");
        }

        if let Err(error) = self.submit_bulk_request(body).await {
            warn!(
                ?error,
                count = document_ids.len(),
                "failed to delete opensearch documents in bulk"
            );
        }
    }

    async fn submit_bulk_request(&self, body: String) -> Result<(), ApiError> {
        let response = self
            .http_client
            .post(self.write_endpoint("/_bulk"))
            .header("content-type", "application/x-ndjson")
            .body(body)
            .send()
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ApiError::Internal(anyhow::anyhow!(
                "opensearch bulk request failed: {status} {body}"
            )));
        }

        let payload = response
            .json::<OpenSearchBulkResponse>()
            .await
            .map_err(|error| ApiError::Internal(error.into()))?;
        if payload.errors {
            return Err(ApiError::Internal(anyhow::anyhow!(
                "opensearch bulk request reported item failures"
            )));
        }

        Ok(())
    }

    fn read_endpoint(&self, suffix: &str) -> String {
        self.raw_index_endpoint(&self.opensearch_read_alias, suffix)
    }

    fn write_endpoint(&self, suffix: &str) -> String {
        self.raw_index_endpoint(&self.opensearch_write_alias, suffix)
    }

    fn raw_index_endpoint(&self, index_name: &str, suffix: &str) -> String {
        format!(
            "{}/{}{}",
            self.opensearch_url.trim_end_matches('/'),
            index_name,
            suffix
        )
    }

    fn versioned_index(&self) -> String {
        format!(
            "{}-v{}",
            self.opensearch_index.trim_end_matches('-'),
            crate::store::CURRENT_INDEX_VERSION
        )
    }

    async fn ensure_alias(&self, alias: &str, write: bool) -> anyhow::Result<()> {
        let versioned_index = self.versioned_index();
        let alias_response = self
            .http_client
            .get(format!(
                "{}/_alias/{}",
                self.opensearch_url.trim_end_matches('/'),
                alias
            ))
            .send()
            .await
            .context("failed to inspect opensearch aliases")?;

        let mut actions = Vec::new();
        if alias_response.status().is_success() {
            let payload = alias_response
                .json::<serde_json::Value>()
                .await
                .context("failed to decode opensearch alias response")?;
            if let Some(object) = payload.as_object() {
                for index_name in object.keys() {
                    if index_name != &versioned_index {
                        actions.push(json!({
                            "remove": {
                                "index": index_name,
                                "alias": alias
                            }
                        }));
                    }
                }
            }
        } else if alias_response.status() != StatusCode::NOT_FOUND {
            let status = alias_response.status();
            let body = alias_response.text().await.unwrap_or_default();
            anyhow::bail!("opensearch alias inspection failed: {status} {body}");
        }

        let add_action = if write {
            json!({
                "add": {
                    "index": versioned_index,
                    "alias": alias,
                    "is_write_index": true
                }
            })
        } else {
            json!({
                "add": {
                    "index": versioned_index,
                    "alias": alias
                }
            })
        };
        actions.push(add_action);

        let response = self
            .http_client
            .post(format!(
                "{}/_aliases",
                self.opensearch_url.trim_end_matches('/')
            ))
            .json(&json!({ "actions": actions }))
            .send()
            .await
            .context("failed to update opensearch aliases")?;

        if response.status().is_success() {
            return Ok(());
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("opensearch alias update failed: {status} {body}");
    }

    async fn delete_document_blob(&self, blob_key: &str) {
        self.blob_store.delete_blob(blob_key).await;
    }

    async fn opensearch_document_count(&self) -> i64 {
        match self
            .http_client
            .get(self.read_endpoint("/_count"))
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
                    "failed to inspect opensearch count"
                );
                0
            }
            Err(error) => {
                warn!(?error, "failed to inspect opensearch count");
                0
            }
        }
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

#[derive(sqlx::FromRow)]
struct DeletedDocumentRow {
    id: String,
    text_blob_key: Option<String>,
}

#[derive(sqlx::FromRow)]
struct ReindexDocumentRow {
    id: String,
    title: String,
    url: String,
    canonical_url: String,
    host: String,
    display_url: String,
    snippet: String,
    text_blob_key: Option<String>,
    language: String,
    last_crawled_at: chrono::DateTime<Utc>,
    content_hash: String,
    suggest_terms: serde_json::Value,
    site_authority: f32,
    content_type: String,
    word_count: i32,
    network: String,
    source_job_id: Option<String>,
    parser_version: i32,
    schema_version: i32,
    index_version: i32,
    duplicate_of: Option<String>,
}

#[derive(Deserialize)]
struct OpenSearchCountResponse {
    count: i64,
}

#[derive(Deserialize)]
struct OpenSearchBulkResponse {
    errors: bool,
}

use tracing::error;

use crate::{
    blob_store::BlobStore,
    crawl::ingest::{IngestService, PendingIngestItem},
    crawler::CrawlerStore,
    error::ApiError,
    store::SearchIndex,
};

#[derive(Debug, Clone)]
pub struct ProjectionRunner {
    ingest: IngestService,
    blob_store: BlobStore,
}

impl ProjectionRunner {
    pub fn new(ingest: IngestService, blob_store: BlobStore) -> Self {
        Self { ingest, blob_store }
    }

    pub async fn drain(
        &self,
        crawler_store: &CrawlerStore,
        search_index: &SearchIndex,
        limit: usize,
    ) -> Result<usize, ApiError> {
        let items = self.ingest.claim_pending_items(limit).await?;
        if items.is_empty() {
            return Ok(0);
        }

        let mut processed = 0usize;
        for item in items {
            match self.apply_item(crawler_store, search_index, &item).await {
                Ok(()) => {
                    self.ingest.mark_item_completed(&item).await?;
                    processed += 1;
                }
                Err(error) => {
                    let message = error.to_string();
                    error!(
                        lease_id = %item.lease_id,
                        crawl_job_id = %item.crawl_job_id,
                        ?error,
                        "crawl ingest projection failed"
                    );
                    crawler_store
                        .mark_projection_failure(&item, &message)
                        .await?;
                    self.ingest.mark_item_failed(&item, &message).await?;
                }
            }
        }

        Ok(processed)
    }

    async fn apply_item(
        &self,
        crawler_store: &CrawlerStore,
        search_index: &SearchIndex,
        item: &PendingIngestItem,
    ) -> Result<(), ApiError> {
        let result = self.blob_store.load_result(&item.blob_id).await?;
        crawler_store
            .apply_staged_result(search_index, item, result)
            .await
    }
}

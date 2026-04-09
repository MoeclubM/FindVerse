use tokio::task::JoinSet;
use tracing::error;

use crate::{
    blob_store::BlobStore,
    crawl::ingest::{IngestService, PendingIngestItem},
    crawler::CrawlerStore,
    error::ApiError,
    store::SearchIndex,
};

const PROJECTION_CONCURRENCY: usize = 8;

#[derive(Debug, Clone)]
pub struct ProjectionRunner {
    ingest: IngestService,
    blob_store: BlobStore,
}

impl ProjectionRunner {
    pub fn new(ingest: IngestService, blob_store: BlobStore) -> Self {
        Self { ingest, blob_store }
    }

    pub(crate) async fn drain(
        &self,
        crawler_store: &CrawlerStore,
        search_index: &SearchIndex,
        limit: usize,
    ) -> Result<usize, ApiError> {
        let items = self.ingest.claim_pending_items(limit).await?;
        if items.is_empty() {
            return Ok(0);
        }

        let mut set: JoinSet<(PendingIngestItem, Result<(), ApiError>)> = JoinSet::new();
        let mut pending = items.into_iter();
        let mut in_flight = 0usize;
        let mut processed = 0usize;

        loop {
            // Fill up to concurrency limit
            while in_flight < PROJECTION_CONCURRENCY {
                let Some(item) = pending.next() else { break };
                let runner = self.clone();
                let store = crawler_store.clone();
                let index = search_index.clone();
                set.spawn(async move {
                    let result = runner.apply_item(&store, &index, &item).await;
                    (item, result)
                });
                in_flight += 1;
            }

            let Some(join_result) = set.join_next().await else { break };
            in_flight -= 1;

            match join_result {
                Ok((item, Ok(()))) => {
                    self.ingest.mark_item_completed(&item).await?;
                    processed += 1;
                }
                Ok((item, Err(error))) => {
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
                Err(join_error) => {
                    error!(?join_error, "projection task panicked");
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

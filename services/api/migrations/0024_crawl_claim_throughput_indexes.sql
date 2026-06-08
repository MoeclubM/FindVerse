CREATE INDEX IF NOT EXISTS crawl_jobs_active_origin_idx
    ON crawl_jobs (owner_developer_id, origin_key, status)
    WHERE status IN ('claimed', 'ingesting');

CREATE INDEX IF NOT EXISTS crawl_jobs_claim_ready_idx
    ON crawl_jobs (
        owner_developer_id,
        status,
        requires_js,
        origin_key,
        priority DESC,
        discovered_at ASC,
        next_retry_at
    )
    WHERE status = 'queued';

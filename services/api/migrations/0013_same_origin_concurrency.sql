ALTER TABLE crawl_rules
    ADD COLUMN IF NOT EXISTS same_origin_concurrency INTEGER NOT NULL DEFAULT 1;

ALTER TABLE crawl_jobs
    ADD COLUMN IF NOT EXISTS same_origin_concurrency INTEGER NOT NULL DEFAULT 1;

CREATE INDEX IF NOT EXISTS crawl_jobs_origin_concurrency_idx
    ON crawl_jobs (owner_developer_id, origin_key, status, same_origin_concurrency)
    WHERE status IN ('queued', 'claimed');

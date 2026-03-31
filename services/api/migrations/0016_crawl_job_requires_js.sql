ALTER TABLE crawl_jobs
    ADD COLUMN IF NOT EXISTS requires_js boolean NOT NULL DEFAULT false;

CREATE INDEX IF NOT EXISTS crawl_jobs_requires_js_idx
    ON crawl_jobs (owner_developer_id, status, requires_js)
    WHERE requires_js = true AND status = 'queued';

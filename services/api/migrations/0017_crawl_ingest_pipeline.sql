ALTER TABLE crawl_jobs
    ADD COLUMN IF NOT EXISTS lease_id text,
    ADD COLUMN IF NOT EXISTS report_accepted_at timestamptz;

CREATE INDEX IF NOT EXISTS crawl_jobs_lease_idx
    ON crawl_jobs (owner_developer_id, lease_id)
    WHERE lease_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS crawl_ingest_batches (
    lease_id text PRIMARY KEY,
    owner_developer_id text NOT NULL,
    crawler_id text NOT NULL,
    status text NOT NULL DEFAULT 'pending',
    result_count integer NOT NULL DEFAULT 0,
    error_message text,
    created_at timestamptz NOT NULL DEFAULT now(),
    started_at timestamptz,
    finished_at timestamptz
);

CREATE INDEX IF NOT EXISTS crawl_ingest_batches_status_idx
    ON crawl_ingest_batches (status, created_at);

CREATE TABLE IF NOT EXISTS crawl_result_blobs (
    id text PRIMARY KEY,
    owner_developer_id text NOT NULL,
    crawler_id text NOT NULL,
    crawl_job_id text NOT NULL,
    lease_id text NOT NULL,
    payload jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (lease_id, crawl_job_id)
);

CREATE INDEX IF NOT EXISTS crawl_result_blobs_lease_idx
    ON crawl_result_blobs (lease_id, created_at);

CREATE TABLE IF NOT EXISTS crawl_ingest_items (
    id text PRIMARY KEY,
    lease_id text NOT NULL REFERENCES crawl_ingest_batches(lease_id) ON DELETE CASCADE,
    owner_developer_id text NOT NULL,
    crawler_id text NOT NULL,
    crawl_job_id text NOT NULL,
    blob_id text NOT NULL REFERENCES crawl_result_blobs(id) ON DELETE CASCADE,
    status text NOT NULL DEFAULT 'pending',
    error_message text,
    created_at timestamptz NOT NULL DEFAULT now(),
    started_at timestamptz,
    finished_at timestamptz,
    UNIQUE (lease_id, crawl_job_id)
);

CREATE INDEX IF NOT EXISTS crawl_ingest_items_status_idx
    ON crawl_ingest_items (status, created_at);

CREATE INDEX IF NOT EXISTS crawl_ingest_items_job_idx
    ON crawl_ingest_items (crawl_job_id, lease_id);

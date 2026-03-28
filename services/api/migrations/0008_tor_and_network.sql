ALTER TABLE crawler_config RENAME TO system_config;

ALTER TABLE crawl_jobs
    ADD COLUMN IF NOT EXISTS network text NOT NULL DEFAULT 'clearnet'
        CHECK (network IN ('clearnet', 'tor'));

ALTER TABLE documents
    ADD COLUMN IF NOT EXISTS network text NOT NULL DEFAULT 'clearnet'
        CHECK (network IN ('clearnet', 'tor'));

CREATE INDEX IF NOT EXISTS documents_network_idx ON documents (network);
CREATE INDEX IF NOT EXISTS crawl_jobs_network_idx
    ON crawl_jobs (owner_developer_id, status, network, discovered_at DESC);

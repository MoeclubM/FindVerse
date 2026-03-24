ALTER TABLE documents
    ADD COLUMN IF NOT EXISTS content_type text NOT NULL DEFAULT 'text/html',
    ADD COLUMN IF NOT EXISTS word_count integer NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS source_job_id text,
    ADD COLUMN IF NOT EXISTS parser_version integer NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS schema_version integer NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS index_version integer NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS duplicate_of text;

CREATE INDEX IF NOT EXISTS documents_duplicate_of_idx
    ON documents (duplicate_of)
    WHERE duplicate_of IS NOT NULL;

CREATE INDEX IF NOT EXISTS documents_host_crawled_idx
    ON documents (host, last_crawled_at DESC);

CREATE INDEX IF NOT EXISTS documents_source_job_idx
    ON documents (source_job_id)
    WHERE source_job_id IS NOT NULL;

ALTER TABLE crawl_jobs
    ADD COLUMN IF NOT EXISTS final_url text,
    ADD COLUMN IF NOT EXISTS content_type text,
    ADD COLUMN IF NOT EXISTS http_status integer,
    ADD COLUMN IF NOT EXISTS discovered_urls_count integer NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS accepted_document_id text;

CREATE INDEX IF NOT EXISTS crawl_jobs_status_http_idx
    ON crawl_jobs (status, http_status);

CREATE INDEX IF NOT EXISTS crawl_jobs_accepted_document_idx
    ON crawl_jobs (accepted_document_id)
    WHERE accepted_document_id IS NOT NULL;

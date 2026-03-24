ALTER TABLE documents ADD COLUMN IF NOT EXISTS canonical_url text;
ALTER TABLE documents ADD COLUMN IF NOT EXISTS host text;
ALTER TABLE documents ADD COLUMN IF NOT EXISTS content_hash text;

UPDATE documents
SET canonical_url = url
WHERE canonical_url IS NULL OR canonical_url = '';

UPDATE documents
SET host = lower(regexp_replace(canonical_url, '^https?://([^/]+).*$','\1'))
WHERE (host IS NULL OR host = '')
  AND canonical_url ~ '^https?://';

UPDATE documents
SET content_hash = md5(body)
WHERE content_hash IS NULL OR content_hash = '';

ALTER TABLE documents ALTER COLUMN canonical_url SET NOT NULL;
ALTER TABLE documents ALTER COLUMN host SET NOT NULL;
ALTER TABLE documents ALTER COLUMN content_hash SET NOT NULL;

CREATE INDEX IF NOT EXISTS documents_canonical_url_idx ON documents (canonical_url);
CREATE INDEX IF NOT EXISTS documents_host_idx ON documents (host);
CREATE INDEX IF NOT EXISTS documents_content_hash_idx ON documents (content_hash);

ALTER TABLE crawl_jobs ADD COLUMN IF NOT EXISTS attempt_count integer NOT NULL DEFAULT 0;
ALTER TABLE crawl_jobs ADD COLUMN IF NOT EXISTS max_attempts integer NOT NULL DEFAULT 3;
ALTER TABLE crawl_jobs ADD COLUMN IF NOT EXISTS next_retry_at timestamptz;
ALTER TABLE crawl_jobs ADD COLUMN IF NOT EXISTS failure_kind text;
ALTER TABLE crawl_jobs ADD COLUMN IF NOT EXISTS failure_message text;
ALTER TABLE crawl_jobs ADD COLUMN IF NOT EXISTS finished_at timestamptz;

UPDATE crawl_jobs
SET status = 'succeeded'
WHERE status = 'completed';

CREATE INDEX IF NOT EXISTS crawl_jobs_retry_idx
    ON crawl_jobs (status, next_retry_at, discovered_at)
    WHERE status = 'queued';

CREATE INDEX IF NOT EXISTS crawl_jobs_terminal_idx
    ON crawl_jobs (owner_developer_id, status, finished_at DESC)
    WHERE status IN ('succeeded', 'failed', 'blocked', 'dead_letter');

-- Priority field for intelligent crawl ordering
ALTER TABLE crawl_jobs ADD COLUMN IF NOT EXISTS priority INTEGER NOT NULL DEFAULT 50;

CREATE INDEX IF NOT EXISTS crawl_jobs_priority_idx
    ON crawl_jobs (owner_developer_id, status, priority DESC, discovered_at ASC)
    WHERE status = 'queued';

-- Inlink count for simplified PageRank
ALTER TABLE documents ADD COLUMN IF NOT EXISTS inlink_count INTEGER NOT NULL DEFAULT 0;

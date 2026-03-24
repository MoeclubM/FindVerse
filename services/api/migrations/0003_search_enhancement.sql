-- Search enhancement: hybrid FTS + trigram + URL search, and crawler job indexes.

-- Ensure pg_trgm extension exists (idempotent)
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Lower the default similarity threshold for broader fuzzy matching
-- (session-level SET is not persisted; we apply it in queries instead)

-- Add trigram index on snippet for fuzzy search
CREATE INDEX IF NOT EXISTS documents_snippet_trgm_idx
    ON documents USING gin (snippet gin_trgm_ops);

-- Add trigram index on display_url for URL fragment search
CREATE INDEX IF NOT EXISTS documents_display_url_trgm_idx
    ON documents USING gin (display_url gin_trgm_ops);

-- Partial index for queued crawl jobs (speeds up claim queries)
CREATE INDEX IF NOT EXISTS crawl_jobs_queued_idx
    ON crawl_jobs (owner_developer_id, discovered_at)
    WHERE status = 'queued';

-- Index on crawl_jobs by rule_id for rule-based re-enqueue
CREATE INDEX IF NOT EXISTS crawl_jobs_rule_idx
    ON crawl_jobs (rule_id, status)
    WHERE rule_id IS NOT NULL;

-- Index on crawl_events for efficient per-owner trimming
CREATE INDEX IF NOT EXISTS crawl_events_trim_idx
    ON crawl_events (owner_developer_id, created_at DESC);

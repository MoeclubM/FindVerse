ALTER TABLE crawl_rules
    ADD COLUMN IF NOT EXISTS discovery_scope text NOT NULL DEFAULT 'same_domain',
    ADD COLUMN IF NOT EXISTS max_discovered_urls_per_page integer NOT NULL DEFAULT 50;

ALTER TABLE crawl_jobs
    ADD COLUMN IF NOT EXISTS discovery_scope text NOT NULL DEFAULT 'same_domain',
    ADD COLUMN IF NOT EXISTS discovery_host text,
    ADD COLUMN IF NOT EXISTS max_discovered_urls_per_page integer NOT NULL DEFAULT 50,
    ADD COLUMN IF NOT EXISTS llm_decision text,
    ADD COLUMN IF NOT EXISTS llm_reason text,
    ADD COLUMN IF NOT EXISTS llm_relevance_score real;

UPDATE crawl_jobs
SET discovery_host = lower(regexp_replace(url, '^https?://([^/]+).*$','\1'))
WHERE discovery_host IS NULL
  AND url ~ '^https?://';

CREATE INDEX IF NOT EXISTS crawl_jobs_discovery_scope_idx
    ON crawl_jobs (owner_developer_id, status, discovery_scope, discovered_at DESC);

CREATE INDEX IF NOT EXISTS crawl_rules_scope_idx
    ON crawl_rules (owner_developer_id, enabled, discovery_scope);

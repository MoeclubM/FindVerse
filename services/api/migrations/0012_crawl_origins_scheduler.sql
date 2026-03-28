CREATE TABLE IF NOT EXISTS crawl_origins (
    owner_developer_id TEXT NOT NULL,
    origin_key TEXT NOT NULL,
    robots_status TEXT NOT NULL DEFAULT 'unknown',
    crawl_delay_secs INTEGER,
    next_allowed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    in_flight_count INTEGER NOT NULL DEFAULT 0,
    last_fetch_status INTEGER,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    robots_etag TEXT,
    robots_last_modified TEXT,
    robots_sitemaps JSONB NOT NULL DEFAULT '[]'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (owner_developer_id, origin_key)
);

ALTER TABLE crawl_jobs
    ADD COLUMN IF NOT EXISTS origin_key TEXT,
    ADD COLUMN IF NOT EXISTS canonical_hint TEXT,
    ADD COLUMN IF NOT EXISTS canonical_source TEXT,
    ADD COLUMN IF NOT EXISTS redirect_chain_json JSONB NOT NULL DEFAULT '[]'::jsonb;

UPDATE crawl_jobs
SET origin_key = lower(regexp_replace(url, '^(https?://[^/]+).*$','\1'))
WHERE origin_key IS NULL
  AND url ~ '^https?://';

UPDATE crawl_jobs
SET origin_key = lower(url)
WHERE origin_key IS NULL;

INSERT INTO crawl_origins (
    owner_developer_id,
    origin_key,
    robots_status,
    next_allowed_at,
    in_flight_count,
    updated_at
)
SELECT DISTINCT
    owner_developer_id,
    origin_key,
    'unknown',
    now(),
    0,
    now()
FROM crawl_jobs
WHERE origin_key IS NOT NULL
ON CONFLICT (owner_developer_id, origin_key) DO NOTHING;

ALTER TABLE crawl_jobs
    ALTER COLUMN origin_key SET NOT NULL;

CREATE INDEX IF NOT EXISTS crawl_jobs_origin_ready_idx
    ON crawl_jobs (owner_developer_id, status, origin_key, priority DESC, discovered_at ASC)
    WHERE status = 'queued';

CREATE INDEX IF NOT EXISTS crawl_origins_ready_idx
    ON crawl_origins (owner_developer_id, next_allowed_at, in_flight_count);

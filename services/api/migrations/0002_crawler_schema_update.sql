-- Crawler schema updates: add columns needed by the PostgreSQL-backed CrawlerStore.

-- crawlers: add fields from StoredCrawler
ALTER TABLE crawlers ADD COLUMN IF NOT EXISTS owner_developer_id text not null default 'local:admin';
ALTER TABLE crawlers ADD COLUMN IF NOT EXISTS preview text not null default '';
ALTER TABLE crawlers ADD COLUMN IF NOT EXISTS key_hash text not null default '';
ALTER TABLE crawlers ADD COLUMN IF NOT EXISTS revoked_at timestamptz;
ALTER TABLE crawlers ADD COLUMN IF NOT EXISTS last_claimed_at timestamptz;
ALTER TABLE crawlers ADD COLUMN IF NOT EXISTS jobs_claimed bigint not null default 0;
ALTER TABLE crawlers ADD COLUMN IF NOT EXISTS jobs_reported bigint not null default 0;

-- crawl_rules: add fields from StoredCrawlRule
ALTER TABLE crawl_rules ADD COLUMN IF NOT EXISTS owner_developer_id text not null default 'local:admin';
ALTER TABLE crawl_rules ADD COLUMN IF NOT EXISTS name text;
ALTER TABLE crawl_rules ADD COLUMN IF NOT EXISTS seed_url text;
ALTER TABLE crawl_rules ADD COLUMN IF NOT EXISTS interval_minutes bigint not null default 60;
ALTER TABLE crawl_rules ADD COLUMN IF NOT EXISTS enabled boolean not null default true;
ALTER TABLE crawl_rules ADD COLUMN IF NOT EXISTS last_enqueued_at timestamptz;

-- crawl_jobs: add fields from FrontierRecord / InFlightRecord
ALTER TABLE crawl_jobs ADD COLUMN IF NOT EXISTS owner_developer_id text not null default 'local:admin';
ALTER TABLE crawl_jobs ADD COLUMN IF NOT EXISTS source text not null default '';
ALTER TABLE crawl_jobs ADD COLUMN IF NOT EXISTS max_depth integer not null default 2;
ALTER TABLE crawl_jobs ADD COLUMN IF NOT EXISTS submitted_by text;
ALTER TABLE crawl_jobs ADD COLUMN IF NOT EXISTS rule_id text;

-- Replace uuid-based unique constraint with developer-id-based one
ALTER TABLE crawl_jobs DROP CONSTRAINT IF EXISTS crawl_jobs_owner_user_id_url_key;
CREATE UNIQUE INDEX IF NOT EXISTS crawl_jobs_owner_dev_url_idx ON crawl_jobs (owner_developer_id, url);

-- crawl_events: add fields from StoredCrawlEvent
ALTER TABLE crawl_events ADD COLUMN IF NOT EXISTS owner_developer_id text not null default 'local:admin';
ALTER TABLE crawl_events ADD COLUMN IF NOT EXISTS status text not null default 'ok';
ALTER TABLE crawl_events ADD COLUMN IF NOT EXISTS message text not null default '';
ALTER TABLE crawl_events ADD COLUMN IF NOT EXISTS url text;

-- Indexes for crawl_jobs claiming (queued jobs scoped to owner)
CREATE INDEX IF NOT EXISTS crawl_jobs_claim_owner_idx
    ON crawl_jobs (owner_developer_id, status, discovered_at)
    WHERE status = 'queued';

-- Index for stale job detection
CREATE INDEX IF NOT EXISTS crawl_jobs_stale_idx
    ON crawl_jobs (status, lease_expires_at)
    WHERE status = 'claimed';

-- Index for events by owner (recent first)
CREATE INDEX IF NOT EXISTS crawl_events_owner_idx
    ON crawl_events (owner_developer_id, created_at desc);

-- Simple key-value table for crawler configuration.
CREATE TABLE IF NOT EXISTS crawler_config (
    key text primary key,
    value text not null,
    updated_at timestamptz not null default now()
);

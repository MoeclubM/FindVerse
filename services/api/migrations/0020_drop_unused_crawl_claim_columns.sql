DROP INDEX IF EXISTS crawl_jobs_claim_idx;
DROP INDEX IF EXISTS crawl_jobs_stale_idx;

ALTER TABLE crawl_jobs
    DROP COLUMN IF EXISTS lease_expires_at,
    DROP COLUMN IF EXISTS report_accepted_at;

CREATE INDEX IF NOT EXISTS crawl_jobs_claimed_at_idx
    ON crawl_jobs (status, claimed_at)
    WHERE status = 'claimed';

ALTER TABLE crawl_rules
    ADD COLUMN IF NOT EXISTS max_pages integer NOT NULL DEFAULT 50;

ALTER TABLE crawl_jobs
    ADD COLUMN IF NOT EXISTS budget_id text,
    ADD COLUMN IF NOT EXISTS max_pages integer NOT NULL DEFAULT 50;

UPDATE crawl_jobs
SET budget_id = COALESCE(rule_id, NULLIF(source, ''), id)
WHERE budget_id IS NULL;

ALTER TABLE crawl_jobs
    ALTER COLUMN budget_id SET NOT NULL;

CREATE INDEX IF NOT EXISTS crawl_jobs_budget_idx
    ON crawl_jobs (owner_developer_id, budget_id);

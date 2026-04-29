ALTER TABLE crawl_jobs
    ADD COLUMN IF NOT EXISTS site_profile_id text;
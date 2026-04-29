ALTER TABLE crawlers DROP COLUMN IF EXISTS owner_user_id;
ALTER TABLE crawl_rules DROP COLUMN IF EXISTS owner_user_id;
ALTER TABLE crawl_jobs DROP COLUMN IF EXISTS owner_user_id;
ALTER TABLE crawl_events DROP COLUMN IF EXISTS owner_user_id;
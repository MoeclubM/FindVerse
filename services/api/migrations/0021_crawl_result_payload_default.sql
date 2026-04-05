ALTER TABLE crawl_result_blobs
    ALTER COLUMN payload SET DEFAULT '{}'::jsonb;

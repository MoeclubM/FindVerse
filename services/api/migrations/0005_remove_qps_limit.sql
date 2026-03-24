alter table users
drop column if exists qps_limit;

-- Query optimization: composite indexes for common access patterns
CREATE INDEX IF NOT EXISTS documents_lang_fetched_idx
    ON documents (language, last_crawled_at DESC)
    WHERE language IS NOT NULL;

CREATE INDEX IF NOT EXISTS documents_authority_fetched_idx
    ON documents (site_authority DESC, last_crawled_at DESC)
    WHERE site_authority IS NOT NULL;

CREATE INDEX IF NOT EXISTS api_keys_token_hash_idx
    ON api_keys (token_hash)
    WHERE revoked_at IS NULL;

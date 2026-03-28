-- Store HTTP caching headers for conditional requests
ALTER TABLE documents ADD COLUMN IF NOT EXISTS http_etag TEXT;
ALTER TABLE documents ADD COLUMN IF NOT EXISTS http_last_modified TEXT;

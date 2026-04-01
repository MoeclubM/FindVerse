ALTER TABLE crawl_result_blobs
    ADD COLUMN IF NOT EXISTS blob_key text,
    ADD COLUMN IF NOT EXISTS blob_size_bytes bigint,
    ADD COLUMN IF NOT EXISTS blob_content_type text;

CREATE INDEX IF NOT EXISTS crawl_result_blobs_blob_key_idx
    ON crawl_result_blobs (blob_key)
    WHERE blob_key IS NOT NULL;

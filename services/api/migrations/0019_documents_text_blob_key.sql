ALTER TABLE documents
    ADD COLUMN IF NOT EXISTS text_blob_key text;

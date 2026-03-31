ALTER TABLE crawl_jobs
    ADD COLUMN IF NOT EXISTS render_mode text NOT NULL DEFAULT 'static'
        CHECK (render_mode IN ('static', 'browser'));

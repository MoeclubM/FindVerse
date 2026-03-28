-- Persistent link graph for accurate authority calculation
CREATE TABLE IF NOT EXISTS link_edges (
    source_url TEXT NOT NULL,
    target_url TEXT NOT NULL,
    discovered_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (source_url, target_url)
);

CREATE INDEX IF NOT EXISTS idx_link_edges_target ON link_edges (target_url);

-- Domain-level crawl statistics
CREATE TABLE IF NOT EXISTS domain_crawl_stats (
    domain TEXT PRIMARY KEY,
    total_pages_indexed INTEGER NOT NULL DEFAULT 0,
    total_pages_failed INTEGER NOT NULL DEFAULT 0,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    last_success_at TIMESTAMPTZ,
    last_failure_at TIMESTAMPTZ,
    avg_change_frequency_hours REAL,
    content_changes INTEGER NOT NULL DEFAULT 0,
    content_checks INTEGER NOT NULL DEFAULT 0,
    health_status TEXT NOT NULL DEFAULT 'healthy' CHECK (health_status IN ('healthy', 'degraded', 'unhealthy')),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

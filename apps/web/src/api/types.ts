export type SearchResponse = {
  query: string;
  took_ms: number;
  total_estimate: number;
  next_offset: number | null;
  did_you_mean?: string | null;
  results: Array<{
    id: string;
    title: string;
    url: string;
    display_url: string;
    snippet: string;
    language: string;
    last_crawled_at: string;
    score: number;
  }>;
};

export type SuggestResponse = {
  query: string;
  suggestions: string[];
};

export type SystemConfigEntry = {
  key: string;
  value: string;
  updated_at: string;
};

export type DiscoveryScope = "same_host" | "same_domain" | "any";

export type UserRole = "admin" | "developer";

export type UserSession = {
  user_id: string;
  username: string;
  role: UserRole;
  token: string;
};

export type ApiKey = {
  id: string;
  name: string;
  preview: string;
  created_at: string;
  revoked_at: string | null;
};

export type DeveloperUsage = {
  developer_id: string;
  daily_limit: number;
  used_today: number;
  keys: ApiKey[];
};

export type DeveloperDomainInsight = {
  domain: string;
  property_url: string;
  indexed_documents: number;
  duplicate_documents: number;
  pending_jobs: number;
  successful_jobs: number;
  filtered_jobs: number;
  failed_jobs: number;
  blocked_jobs: number;
  last_indexed_at: string | null;
  last_crawled_at: string | null;
  top_languages: Array<{
    label: string;
    count: number;
  }>;
  top_content_types: Array<{
    label: string;
    count: number;
  }>;
  recent_documents: Array<{
    id: string;
    title: string;
    url: string;
    display_url: string;
    language: string;
    last_crawled_at: string;
    word_count: number;
    content_type: string;
    duplicate_of: string | null;
  }>;
  recent_jobs: Array<{
    id: string;
    url: string;
    status: string;
    http_status: number | null;
    depth: number;
    discovered_at: string;
    finished_at: string | null;
    failure_kind: string | null;
    failure_message: string | null;
    accepted_document_id: string | null;
    render_mode: string;
  }>;
};

export type DeveloperDomainSubmitResult = {
  accepted_urls: number;
  queued_domain_jobs: number;
  known_domain_urls: number;
};

export type AdminUserRecord = {
  user_id: string;
  username: string;
  role: UserRole;
  enabled: boolean;
  created_at: string;
  daily_limit: number;
  used_today: number;
  key_count: number;
};

export type CreatedApiKey = {
  id: string;
  name: string;
  preview: string;
  token: string;
  created_at: string;
};

export type CrawlRule = {
  id: string;
  name: string;
  seed_url: string;
  interval_minutes: number;
  max_depth: number;
  max_pages: number;
  same_origin_concurrency: number;
  discovery_scope: DiscoveryScope;
  max_discovered_urls_per_page: number;
  enabled: boolean;
  created_at: string;
  updated_at: string;
  last_enqueued_at: string | null;
};

export type CrawlEvent = {
  id: string;
  kind: string;
  status: string;
  message: string;
  url: string | null;
  crawler_id: string | null;
  created_at: string;
};

export type CrawlOverview = {
  owner_id: string;
  platform_version: string;
  frontier_depth: number;
  known_urls: number;
  in_flight_jobs: number;
  indexed_documents: number;
  duplicate_documents: number;
  terminal_failures: number;
  crawlers: Array<{
    id: string;
    name: string;
    preview: string;
    created_at: string;
    revoked_at: string | null;
    last_seen_at: string | null;
    last_claimed_at: string | null;
    online: boolean;
    can_delete: boolean;
    in_flight_jobs: number;
    jobs_claimed: number;
    jobs_reported: number;
    supports_js_render: boolean;
    worker_concurrency: number;
    js_render_concurrency: number;
    max_jobs: number;
    version: string | null;
    platform: string | null;
    desired_version: string | null;
    sort_order: number | null;
    update_status: string;
    update_message: string | null;
  }>;
  rules: CrawlRule[];
  recent_events: CrawlEvent[];
};

export type DocumentList = {
  total_estimate: number;
  next_offset: number | null;
  documents: Array<{
    id: string;
    title: string;
    url: string;
    canonical_url: string;
    host: string;
    display_url: string;
    snippet: string;
    language: string;
    last_crawled_at: string;
    content_type: string;
    word_count: number;
    site_authority: number;
    parser_version: number;
    schema_version: number;
    index_version: number;
    source_job_id: string | null;
    duplicate_of: string | null;
  }>;
};

export type CrawlJobDetail = {
  id: string;
  url: string;
  final_url: string | null;
  status: string;
  depth: number;
  max_depth: number;
  attempt_count: number;
  max_attempts: number;
  source: string;
  rule_id: string | null;
  site_profile_id: string | null;
  claimed_by: string | null;
  discovered_at: string;
  claimed_at: string | null;
  next_retry_at: string | null;
  content_type: string | null;
  http_status: number | null;
  discovered_urls_count: number;
  accepted_document_id: string | null;
  failure_kind: string | null;
  failure_message: string | null;
  finished_at: string | null;
  render_mode: string;
};

export type CrawlJobList = {
  total: number;
  next_offset: number | null;
  jobs: CrawlJobDetail[];
};

export type CrawlJobStats = {
  queued: number;
  claimed: number;
  succeeded: number;
  failed: number;
  blocked: number;
  dead_letter: number;
};

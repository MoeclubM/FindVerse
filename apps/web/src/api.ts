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

type RequestOptions = RequestInit & {
  token?: string | null;
};

async function request<T>(
  path: string,
  options: RequestOptions = {},
): Promise<T> {
  const { token, ...fetchOptions } = options;
  const response = await fetch(`/api${path}`, {
    ...fetchOptions,
    headers: {
      Accept: "application/json",
      ...(fetchOptions.body ? { "Content-Type": "application/json" } : {}),
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...(fetchOptions.headers ?? {}),
    },
  });

  if (!response.ok) {
    const text = await response.text();
    const error = new Error(text || `Request failed with ${response.status}`);
    (error as Error & { status: number }).status = response.status;
    throw error;
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return (await response.json()) as T;
}

export function search(query: string, apiKey?: string | null) {
  return request<SearchResponse>(`/v1/search?q=${encodeURIComponent(query)}`, {
    token: apiKey,
  });
}

export function suggestSearch(query: string) {
  return request<SuggestResponse>(`/v1/suggest?q=${encodeURIComponent(query)}`);
}

export function developerSearch(query: string, apiKey: string) {
  return request<SearchResponse>(
    `/v1/developer/search?q=${encodeURIComponent(query)}`,
    {
      token: apiKey,
    },
  );
}

export function registerUser(username: string, password: string) {
  return request<UserSession>("/v1/users/register", {
    method: "POST",
    body: JSON.stringify({ username, password }),
  });
}

export function loginUser(username: string, password: string) {
  return request<UserSession>("/v1/users/session/login", {
    method: "POST",
    body: JSON.stringify({ username, password }),
  });
}

export function getUserSession(token: string) {
  return request<UserSession>("/v1/users/session/me", {
    method: "GET",
    token,
  });
}

export function logoutUser(token: string) {
  return request<void>("/v1/users/session/logout", {
    method: "POST",
    token,
  });
}

export function getCrawlOverview(token: string) {
  return request<CrawlOverview>("/v1/admin/crawl/overview", {
    method: "GET",
    token,
  });
}

function updateCrawler(
  token: string,
  id: string,
  payload: {
    name?: string;
    worker_concurrency?: number;
    js_render_concurrency?: number;
    max_jobs?: number;
    desired_version?: string;
    sort_order?: number | null;
  },
) {
  return request<void>(`/v1/admin/crawlers/${id}`, {
    method: "PATCH",
    token,
    body: JSON.stringify(payload),
  });
}

export function renameCrawler(token: string, id: string, name: string) {
  return updateCrawler(token, id, { name });
}

export function updateCrawlerRuntime(
  token: string,
  id: string,
  workerConcurrency: number,
  jsRenderConcurrency: number,
  maxJobs: number,
) {
  return updateCrawler(token, id, {
    worker_concurrency: workerConcurrency,
    js_render_concurrency: jsRenderConcurrency,
    max_jobs: maxJobs,
  });
}

export function requestCrawlerUpdate(
  token: string,
  id: string,
  desiredVersion: string,
) {
  return updateCrawler(token, id, {
    desired_version: desiredVersion,
  });
}

export function updateCrawlerSortOrder(
  token: string,
  id: string,
  sortOrder: number | null,
) {
  return updateCrawler(token, id, {
    sort_order: sortOrder,
  });
}

export function deleteCrawler(token: string, id: string) {
  return request<void>(`/v1/admin/crawlers/${id}`, {
    method: "DELETE",
    token,
  });
}

export function seedFrontier(
  token: string,
  urls: string[],
  maxDepth: number,
  maxPages: number,
  sameOriginConcurrency: number,
  discoveryScope: DiscoveryScope,
  maxDiscoveredUrlsPerPage: number,
  allowRevisit: boolean,
) {
  return request<{
    accepted_urls: number;
    frontier_depth: number;
    known_urls: number;
  }>("/v1/admin/frontier/seed", {
    method: "POST",
    token,
    body: JSON.stringify({
      urls,
      source: "admin-panel",
      max_depth: maxDepth,
      max_pages: maxPages,
      same_origin_concurrency: sameOriginConcurrency,
      discovery_scope: discoveryScope,
      max_discovered_urls_per_page: maxDiscoveredUrlsPerPage,
      allow_revisit: allowRevisit,
    }),
  });
}

export function createRule(
  token: string,
  payload: {
    name: string;
    seed_url: string;
    interval_minutes: number;
    max_depth: number;
    max_pages: number;
    same_origin_concurrency: number;
    discovery_scope: DiscoveryScope;
    max_discovered_urls_per_page: number;
    enabled: boolean;
  },
) {
  return request<CrawlRule>("/v1/admin/crawl/rules", {
    method: "POST",
    token,
    body: JSON.stringify(payload),
  });
}

export function updateRule(
  token: string,
  id: string,
  payload: Partial<{
    name: string;
    seed_url: string;
    interval_minutes: number;
    max_depth: number;
    max_pages: number;
    same_origin_concurrency: number;
    discovery_scope: DiscoveryScope;
    max_discovered_urls_per_page: number;
    enabled: boolean;
  }>,
) {
  return request<CrawlRule>(`/v1/admin/crawl/rules/${id}`, {
    method: "PATCH",
    token,
    body: JSON.stringify(payload),
  });
}

export function deleteRule(token: string, id: string) {
  return request<void>(`/v1/admin/crawl/rules/${id}`, {
    method: "DELETE",
    token,
  });
}

export function listDocuments(
  token: string,
  params: { query?: string; site?: string; offset?: number } = {},
) {
  const search = new URLSearchParams();
  if (params.query) {
    search.set("query", params.query);
  }
  if (params.site) {
    search.set("site", params.site);
  }
  if (params.offset) {
    search.set("offset", String(params.offset));
  }
  search.set("limit", "20");

  return request<DocumentList>(`/v1/admin/documents?${search.toString()}`, {
    method: "GET",
    token,
  });
}

export function deleteDocument(token: string, id: string) {
  return request<void>(`/v1/admin/documents/${id}`, {
    method: "DELETE",
    token,
  });
}

export function purgeSite(token: string, site: string) {
  return request<{ deleted_documents: number }>(
    "/v1/admin/documents/purge-site",
    {
      method: "POST",
      token,
      body: JSON.stringify({ site }),
    },
  );
}

export function getUserKeys(token: string) {
  return request<DeveloperUsage>("/v1/users/keys", {
    method: "GET",
    token,
  });
}

export function getUserDomainInsight(token: string, domain: string) {
  return request<DeveloperDomainInsight>(
    `/v1/users/domains/inspect?domain=${encodeURIComponent(domain)}`,
    {
      method: "GET",
      token,
    },
  );
}

export function getAdminDomainInsight(token: string, domain: string) {
  return request<DeveloperDomainInsight>(
    `/v1/admin/domains/inspect?domain=${encodeURIComponent(domain)}`,
    {
      method: "GET",
      token,
    },
  );
}

export function submitUserDomain(
  token: string,
  payload: {
    domain: string;
    urls: string[];
    max_depth: number;
    max_pages: number;
    same_origin_concurrency: number;
    allow_revisit: boolean;
  },
) {
  return request<DeveloperDomainSubmitResult>("/v1/users/domains/submit", {
    method: "POST",
    token,
    body: JSON.stringify(payload),
  });
}

export function getAdminUserKeys(token: string, userId: string) {
  return request<DeveloperUsage>(`/v1/admin/users/${userId}/keys`, {
    method: "GET",
    token,
  });
}

export function createUserKey(token: string, name: string) {
  return request<CreatedApiKey>("/v1/users/keys", {
    method: "POST",
    token,
    body: JSON.stringify({ name }),
  });
}

export function createAdminUserKey(
  token: string,
  userId: string,
  name: string,
) {
  return request<CreatedApiKey>(`/v1/admin/users/${userId}/keys`, {
    method: "POST",
    token,
    body: JSON.stringify({ name }),
  });
}

export function revokeUserKey(token: string, id: string) {
  return request<void>(`/v1/users/keys/${id}`, {
    method: "DELETE",
    token,
  });
}

export function revokeAdminUserKey(
  token: string,
  userId: string,
  id: string,
) {
  return request<void>(`/v1/admin/users/${userId}/keys/${id}`, {
    method: "DELETE",
    token,
  });
}

export function listAdminUsers(token: string) {
  return request<AdminUserRecord[]>("/v1/admin/users", {
    method: "GET",
    token,
  });
}

export function createUser(
  token: string,
  payload: {
    username: string;
    password: string;
    role: UserRole;
  },
) {
  return request<AdminUserRecord>("/v1/admin/users", {
    method: "POST",
    token,
    body: JSON.stringify(payload),
  });
}

export function updateUser(
  token: string,
  userId: string,
  payload: {
    username?: string;
    role?: UserRole;
    daily_limit?: number;
    enabled?: boolean;
    password?: string;
  },
) {
  return request<void>(`/v1/admin/users/${userId}`, {
    method: "PATCH",
    token,
    body: JSON.stringify(payload),
  });
}

export function deleteUser(token: string, userId: string) {
  return request<void>(`/v1/admin/users/${userId}`, {
    method: "DELETE",
    token,
  });
}

export function getSystemConfig(
  token: string,
): Promise<{ entries: SystemConfigEntry[] }> {
  return request<{ entries: SystemConfigEntry[] }>("/v1/admin/system-config", {
    method: "GET",
    token,
  });
}

export function setSystemConfig(
  token: string,
  key: string,
  value: string | null,
): Promise<void> {
  return request<void>(`/v1/admin/system-config/${encodeURIComponent(key)}`, {
    method: "PUT",
    token,
    body: JSON.stringify({ value }),
  });
}

// Crawl job management types and API functions

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

export function listCrawlJobs(
  token: string,
  params: { status?: string; offset?: number; limit?: number } = {},
) {
  const search = new URLSearchParams();
  if (params.status) {
    search.set("status", params.status);
  }
  if (params.offset) {
    search.set("offset", String(params.offset));
  }
  search.set("limit", String(params.limit ?? 20));

  return request<CrawlJobList>(`/v1/admin/crawl/jobs?${search.toString()}`, {
    method: "GET",
    token,
  });
}

export function getCrawlJobStats(token: string) {
  return request<CrawlJobStats>("/v1/admin/crawl/jobs/stats", {
    method: "GET",
    token,
  });
}

export function retryFailedJobs(token: string) {
  return request<{ retried: number }>("/v1/admin/crawl/jobs/retry", {
    method: "POST",
    token,
  });
}

export function cleanupCompletedJobs(token: string) {
  return request<{ cleaned: number }>("/v1/admin/crawl/jobs/completed", {
    method: "DELETE",
    token,
  });
}

export function cleanupFailedJobs(token: string) {
  return request<{ cleaned: number }>("/v1/admin/crawl/jobs/failed", {
    method: "DELETE",
    token,
  });
}

export function stopAllCrawlJobs(token: string) {
  return request<{ disabled_rules: number; removed_jobs: number }>(
    "/v1/admin/crawl/jobs/stop",
    {
      method: "POST",
      token,
    },
  );
}

export function searchWithParams(
  query: string,
  params: {
    offset?: number;
    site?: string;
    lang?: string;
    freshness?: string;
    network?: "clearnet" | "tor";
  } = {},
  apiKey?: string | null,
) {
  const search = new URLSearchParams();
  search.set("q", query);
  if (params.offset) search.set("offset", String(params.offset));
  if (params.site) search.set("site", params.site);
  if (params.lang) search.set("lang", params.lang);
  if (params.freshness) search.set("freshness", params.freshness);
  if (params.network) search.set("network", params.network);

  const path = apiKey ? "/v1/developer/search" : "/v1/search";
  return request<SearchResponse>(`${path}?${search.toString()}`, {
    token: apiKey,
  });
}

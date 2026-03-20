export type SearchResponse = {
  query: string;
  took_ms: number;
  total_estimate: number;
  next_offset: number | null;
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

export type AdminSession = {
  developer_id: string;
  username: string;
  token: string;
};

export type DeveloperUsage = {
  developer_id: string;
  qps_limit: number;
  daily_limit: number;
  used_today: number;
  keys: Array<{
    id: string;
    name: string;
    preview: string;
    created_at: string;
    revoked_at: string | null;
  }>;
};

export type CrawlRule = {
  id: string;
  name: string;
  seed_url: string;
  interval_minutes: number;
  max_depth: number;
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
  developer_id: string;
  frontier_depth: number;
  known_urls: number;
  in_flight_jobs: number;
  indexed_documents: number;
  crawlers: Array<{
    id: string;
    name: string;
    preview: string;
    created_at: string;
    revoked_at: string | null;
    last_seen_at: string | null;
    last_claimed_at: string | null;
    jobs_claimed: number;
    jobs_reported: number;
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
    display_url: string;
    snippet: string;
    language: string;
    last_crawled_at: string;
  }>;
};

type RequestOptions = RequestInit & {
  token?: string | null;
};

async function request<T>(path: string, options: RequestOptions = {}): Promise<T> {
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
    throw new Error(text || `Request failed with ${response.status}`);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return (await response.json()) as T;
}

export function search(query: string) {
  return request<SearchResponse>(`/v1/search?q=${encodeURIComponent(query)}`);
}

export function login(username: string, password: string) {
  return request<AdminSession>("/v1/admin/session/login", {
    method: "POST",
    body: JSON.stringify({ username, password }),
  });
}

export function getSession(token: string) {
  return request<AdminSession>("/v1/admin/session/me", {
    method: "GET",
    token,
  });
}

export function logout(token: string) {
  return request<void>("/v1/admin/session/logout", {
    method: "POST",
    token,
  });
}

export function getUsage(token: string) {
  return request<DeveloperUsage>("/v1/admin/usage", {
    method: "GET",
    token,
  });
}

export function createApiKey(token: string, name: string) {
  return request<{
    id: string;
    name: string;
    preview: string;
    token: string;
    created_at: string;
  }>("/v1/admin/api-keys", {
    method: "POST",
    token,
    body: JSON.stringify({ name }),
  });
}

export function revokeApiKey(token: string, id: string) {
  return request<void>(`/v1/admin/api-keys/${id}`, {
    method: "DELETE",
    token,
  });
}

export function getCrawlOverview(token: string) {
  return request<CrawlOverview>("/v1/admin/crawl/overview", {
    method: "GET",
    token,
  });
}

export function createCrawler(token: string, name: string) {
  return request<{
    id: string;
    name: string;
    preview: string;
    key: string;
    created_at: string;
  }>("/v1/admin/crawlers", {
    method: "POST",
    token,
    body: JSON.stringify({ name }),
  });
}

export function seedFrontier(
  token: string,
  urls: string[],
  maxDepth: number,
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
  return request<{ deleted_documents: number }>("/v1/admin/documents/purge-site", {
    method: "POST",
    token,
    body: JSON.stringify({ site }),
  });
}

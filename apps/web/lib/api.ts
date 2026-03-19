import {
  CrawlOverviewSchema,
  CreatedApiKeySchema,
  CreatedCrawlerSchema,
  CreateApiKeyRequestSchema,
  CreateCrawlerRequestSchema,
  DeveloperUsageSchema,
  SeedFrontierRequestSchema,
  SeedFrontierResponseSchema,
  SearchQuerySchema,
  SearchResponseSchema,
  SuggestResponseSchema,
  type SearchQuery,
} from "@findverse/contracts";

const API_BASE_URL =
  process.env.NEXT_PUBLIC_FINDVERSE_API_URL ?? "http://localhost:8080";

async function apiFetch<T>(
  path: string,
  options: RequestInit,
  parser: (value: unknown) => T,
): Promise<T> {
  const response = await fetch(`${API_BASE_URL}${path}`, {
    ...options,
    headers: {
      Accept: "application/json",
      ...(options.headers ?? {}),
    },
    cache: "no-store",
  });

  if (!response.ok) {
    const body = await response.text();
    throw new Error(body || `FindVerse API failed with ${response.status}`);
  }

  return parser(await response.json());
}

export async function searchIndex(input: Partial<SearchQuery>) {
  const params = SearchQuerySchema.parse(input);
  const searchParams = new URLSearchParams({
    q: params.q,
    limit: String(params.limit),
    offset: String(params.offset),
    freshness: params.freshness,
  });

  if (params.lang) {
    searchParams.set("lang", params.lang);
  }
  if (params.site) {
    searchParams.set("site", params.site);
  }

  return apiFetch(
    `/v1/search?${searchParams.toString()}`,
    { method: "GET" },
    SearchResponseSchema.parse,
  );
}

export async function suggestQueries(query: string) {
  return apiFetch(
    `/v1/suggest?q=${encodeURIComponent(query)}`,
    { method: "GET" },
    SuggestResponseSchema.parse,
  );
}

export async function loadDeveloperUsage(developerId: string) {
  return apiFetch(
    "/v1/developer/usage",
    {
      method: "GET",
      headers: {
        "x-developer-id": developerId,
      },
    },
    DeveloperUsageSchema.parse,
  );
}

export async function createDeveloperKey(
  developerId: string,
  payload: { name: string },
) {
  const request = CreateApiKeyRequestSchema.parse(payload);
  return apiFetch(
    `/v1/developer/keys`,
    {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "x-developer-id": developerId,
      },
      body: JSON.stringify(request),
    },
    CreatedApiKeySchema.parse,
  );
}

export async function revokeDeveloperKey(developerId: string, keyId: string) {
  const response = await fetch(`${API_BASE_URL}/v1/developer/keys/${keyId}`, {
    method: "DELETE",
    headers: {
      "x-developer-id": developerId,
    },
    cache: "no-store",
  });

  if (!response.ok) {
    const body = await response.text();
    throw new Error(body || `Delete failed with ${response.status}`);
  }
}

export async function loadCrawlOverview(developerId: string) {
  return apiFetch(
    "/v1/developer/crawl/overview",
    {
      method: "GET",
      headers: {
        "x-developer-id": developerId,
      },
    },
    CrawlOverviewSchema.parse,
  );
}

export async function createCrawlerKey(
  developerId: string,
  payload: { name: string },
) {
  const request = CreateCrawlerRequestSchema.parse(payload);
  return apiFetch(
    "/v1/developer/crawlers",
    {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "x-developer-id": developerId,
      },
      body: JSON.stringify(request),
    },
    CreatedCrawlerSchema.parse,
  );
}

export async function seedCrawlerFrontier(
  developerId: string,
  payload: { urls: string[]; source?: string },
) {
  const request = SeedFrontierRequestSchema.parse(payload);
  return apiFetch(
    "/v1/developer/frontier/seed",
    {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "x-developer-id": developerId,
      },
      body: JSON.stringify(request),
    },
    SeedFrontierResponseSchema.parse,
  );
}

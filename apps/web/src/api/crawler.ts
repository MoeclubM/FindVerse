import { request } from "./client";
import type { CrawlOverview, CrawlRule, DiscoveryScope } from "./types";

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

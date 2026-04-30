import { request } from "./client";
import type { CrawlJobList, CrawlJobStats } from "./types";

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

import { request } from "./client";
import type {
  CreatedApiKey,
  DeveloperDomainInsight,
  DeveloperDomainSubmitResult,
  DeveloperUsage,
} from "./types";

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

export function createUserKey(token: string, name: string) {
  return request<CreatedApiKey>("/v1/users/keys", {
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

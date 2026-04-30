import { request } from "./client";
import type { SearchResponse, SuggestResponse } from "./types";

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

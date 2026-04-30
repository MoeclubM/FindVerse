import { request } from "./client";
import type { DocumentList } from "./types";

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

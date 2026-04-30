import { request } from "./client";
import type { SystemConfigEntry } from "./types";

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

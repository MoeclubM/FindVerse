import { request } from "./client";
import type {
  AdminUserRecord,
  CreatedApiKey,
  DeveloperDomainInsight,
  DeveloperUsage,
  UserRole,
} from "./types";

export function getAdminDomainInsight(token: string, domain: string) {
  return request<DeveloperDomainInsight>(
    `/v1/admin/domains/inspect?domain=${encodeURIComponent(domain)}`,
    {
      method: "GET",
      token,
    },
  );
}

export function getAdminUserKeys(token: string, userId: string) {
  return request<DeveloperUsage>(`/v1/admin/users/${userId}/keys`, {
    method: "GET",
    token,
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

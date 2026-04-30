import { request } from "./client";
import type { UserSession } from "./types";

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

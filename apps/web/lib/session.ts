import { createHmac, timingSafeEqual } from "node:crypto";

import { cookies } from "next/headers";
import { NextResponse } from "next/server";

export type DeveloperSession = {
  id: string;
  email: string;
  name: string;
};

const SESSION_COOKIE = "findverse_session";

function getAuthSecret() {
  return process.env.AUTH_SECRET;
}

function getLocalAdminUsername() {
  return process.env.FINDVERSE_LOCAL_ADMIN_USERNAME;
}

function getLocalAdminPassword() {
  return process.env.FINDVERSE_LOCAL_ADMIN_PASSWORD;
}

function encodeBase64Url(input: string) {
  return Buffer.from(input, "utf8").toString("base64url");
}

function decodeBase64Url(input: string) {
  return Buffer.from(input, "base64url").toString("utf8");
}

function signPayload(encodedPayload: string, secret: string) {
  return createHmac("sha256", secret).update(encodedPayload).digest("base64url");
}

export async function getSession(): Promise<DeveloperSession | null> {
  const cookieStore = await cookies();
  const raw = cookieStore.get(SESSION_COOKIE)?.value;
  const secret = getAuthSecret();

  if (!raw || !secret) {
    return null;
  }

  const [payload, signature] = raw.split(".");
  if (!payload || !signature || signPayload(payload, secret) !== signature) {
    return null;
  }

  try {
    return JSON.parse(decodeBase64Url(payload)) as DeveloperSession;
  } catch {
    return null;
  }
}

export function attachSessionCookie(
  response: NextResponse,
  session: DeveloperSession,
) {
  const secret = getAuthSecret();
  if (!secret) {
    throw new Error("AUTH_SECRET is required to create a session");
  }

  const payload = encodeBase64Url(JSON.stringify(session));
  const signed = `${payload}.${signPayload(payload, secret)}`;

  response.cookies.set(SESSION_COOKIE, signed, {
    httpOnly: true,
    sameSite: "lax",
    secure: process.env.NODE_ENV === "production",
    path: "/",
    maxAge: 60 * 60 * 24 * 30,
  });
}

export function clearSessionCookie(response: NextResponse) {
  response.cookies.set(SESSION_COOKIE, "", {
    httpOnly: true,
    sameSite: "lax",
    secure: process.env.NODE_ENV === "production",
    path: "/",
    maxAge: 0,
  });
}

export function isLocalAuthConfigured() {
  return Boolean(
    getAuthSecret() && getLocalAdminUsername() && getLocalAdminPassword(),
  );
}

function safeEqual(left: string, right: string) {
  const leftBuffer = Buffer.from(left);
  const rightBuffer = Buffer.from(right);
  if (leftBuffer.length !== rightBuffer.length) {
    return false;
  }
  return timingSafeEqual(leftBuffer, rightBuffer);
}

export function validateLocalCredentials(username: string, password: string) {
  const configuredUsername = getLocalAdminUsername();
  const configuredPassword = getLocalAdminPassword();

  if (!configuredUsername || !configuredPassword) {
    return null;
  }

  if (
    !safeEqual(username.trim(), configuredUsername) ||
    !safeEqual(password, configuredPassword)
  ) {
    return null;
  }

  return {
    id: `local:${configuredUsername}`,
    email: `${configuredUsername}@local.findverse`,
    name: configuredUsername,
  } satisfies DeveloperSession;
}

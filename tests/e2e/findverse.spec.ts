import { execFile } from "node:child_process";
import path from "node:path";
import { promisify } from "node:util";

import { expect, test } from "@playwright/test";

const execFileAsync = promisify(execFile);
const repoRoot = path.resolve(process.cwd());

test("developer self-service, admin management, crawler auto-registration, and search flow", async ({
  page,
}) => {
  const username = process.env.FINDVERSE_LOCAL_ADMIN_USERNAME ?? "admin";
  const password = process.env.FINDVERSE_LOCAL_ADMIN_PASSWORD ?? "change-me";
  const seedUrl = `https://example.com/?findverse-e2e=${Date.now()}`;
  const apiBaseUrl = process.env.PLAYWRIGHT_API_BASE_URL ?? "http://127.0.0.1:8080";
  const developerUsername = `dev-${Date.now()}`;
  const developerPassword = "dev-password-123";

  await page.goto("/?q=ranking");
  await expect(page.getByRole("link", { name: "Designing Ranking Systems for Search" }).first()).toBeVisible();
  await expect(page.getByText("Browser search")).toBeVisible();

  await page.goto("/dev");
  await page.getByRole("button", { name: "Register" }).click();
  await page.getByPlaceholder("Username").fill(developerUsername);
  await page.getByPlaceholder("Password").fill(developerPassword);
  await page.getByRole("button", { name: "Create account" }).click();

  await expect(page.getByRole("heading", { name: "Create API key" })).toBeVisible();
  await page.getByPlaceholder("Key name").fill("E2E key");
  await page.getByRole("button", { name: "Create key" }).click();

  const apiKeyEl = page.locator("pre").filter({ hasText: "fvk_" }).first();
  await expect(apiKeyEl).toBeVisible();
  const apiKey = (await apiKeyEl.textContent())?.trim();
  expect(apiKey).toBeTruthy();

  await page.locator(".key-reveal").getByRole("button", { name: "Use for search" }).click();
  await page.goto("/?q=ranking");
  await expect(page.getByText("Developer key active")).toBeVisible();
  await expect(page.getByRole("link", { name: "Designing Ranking Systems for Search" }).first()).toBeVisible();

  let workerApiKey: string | null = null;

  await page.goto("/console");
  await page.getByPlaceholder("Username").fill(username);
  await page.getByPlaceholder("Password").fill(password);
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page.getByRole("heading", { name: "Overview" })).toBeVisible();

  await page.getByRole("button", { name: "Settings" }).click();
  await page.getByPlaceholder("Key name").fill("Admin worker key");
  await page.getByRole("button", { name: "Create" }).first().click();
  const workerApiKeyEl = page.locator("pre").filter({ hasText: "fvk_" }).first();
  await expect(workerApiKeyEl).toBeVisible();
  workerApiKey = (await workerApiKeyEl.textContent())?.trim() ?? null;
  expect(workerApiKey).toBeTruthy();

  await page.getByRole("button", { name: "Users" }).click();
  await expect(page.getByText(developerUsername)).toBeVisible();

  const developerRow = page.locator(".table-row").filter({ hasText: developerUsername }).first();
  await developerRow.getByLabel(`QPS limit for ${developerUsername}`).fill("7");
  await developerRow.getByLabel(`Daily quota for ${developerUsername}`).fill("1234");
  await developerRow.getByRole("button", { name: "Save" }).click();
  await expect(page.getByText(/Refresh failed/i)).toHaveCount(0);

  await page.getByRole("button", { name: "Crawl Tasks" }).click();
  await page.getByPlaceholder("One URL per line").fill(seedUrl);
  await page.getByRole("button", { name: "Queue" }).click();
  await expect(page.getByText(/Queued 1 URLs/i)).toBeVisible();

  await execFileAsync(
    "cargo",
    [
      "run",
      "-p",
      "findverse-crawler",
      "--",
      "worker",
      "--server",
      apiBaseUrl,
      "--api-key",
      workerApiKey!,
      "--once",
      "--max-jobs",
      "1",
    ],
    {
      cwd: repoRoot,
      timeout: 120_000,
    },
  );

  await page.getByRole("button", { name: "Refresh" }).click();
  await page.getByRole("button", { name: "Workers" }).click();
  await expect(page.getByText(/Claimed/i).first()).toBeVisible();
  await expect(page.getByText(/Reported/i).first()).toBeVisible();

  await page.goto("/?q=ranking");
  await expect(page.getByRole("link", { name: "Designing Ranking Systems for Search" }).first()).toBeVisible();
});

test("crawler join key flow", async ({ request }) => {
  const apiBaseUrl = process.env.PLAYWRIGHT_API_BASE_URL ?? "http://127.0.0.1:8080";
  const username = process.env.FINDVERSE_LOCAL_ADMIN_USERNAME ?? "admin";
  const password = process.env.FINDVERSE_LOCAL_ADMIN_PASSWORD ?? "change-me";

  // Login as admin
  const loginRes = await request.post(`${apiBaseUrl}/v1/admin/session/login`, {
    data: { username, password },
  });
  expect(loginRes.ok()).toBeTruthy();
  const { token } = await loginRes.json();

  // Set join key via admin API
  const joinKey = `test-join-key-${Date.now()}`;
  const setRes = await request.put(`${apiBaseUrl}/v1/admin/crawler-join-key`, {
    headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
    data: { join_key: joinKey },
  });
  expect(setRes.status()).toBe(204);

  // Read it back
  const getRes = await request.get(`${apiBaseUrl}/v1/admin/crawler-join-key`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  expect(getRes.ok()).toBeTruthy();
  const { join_key } = await getRes.json();
  expect(join_key).toBe(joinKey);

  // Join with correct key
  const joinRes = await request.post(`${apiBaseUrl}/internal/crawlers/join`, {
    data: { join_key: joinKey, name: "e2e-join-crawler" },
  });
  expect(joinRes.status()).toBe(201);
  const joined = await joinRes.json();
  expect(joined.crawler_id).toBeTruthy();
  expect(joined.crawler_key).toBeTruthy();
  expect(joined.name).toBe("e2e-join-crawler");

  // Join with wrong key should fail
  const badRes = await request.post(`${apiBaseUrl}/internal/crawlers/join`, {
    data: { join_key: "wrong-key", name: "bad-crawler" },
  });
  expect(badRes.status()).toBe(401);
});

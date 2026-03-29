import { execFile } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { promisify } from "node:util";

import { expect, test, type APIRequestContext } from "@playwright/test";

const execFileAsync = promisify(execFile);
const repoRoot = path.resolve(process.cwd());
const controlApiBaseUrl = process.env.PLAYWRIGHT_API_BASE_URL ?? "http://127.0.0.1:8080";
const queryApiBaseUrl = process.env.PLAYWRIGHT_QUERY_API_BASE_URL ?? "http://127.0.0.1:8081";

test.describe.configure({ timeout: 300_000 });

async function waitForFindVerseReady(request: APIRequestContext) {
  await expect
    .poll(
      async () => {
        try {
          const [controlReady, queryReady] = await Promise.all([
            request.get(`${controlApiBaseUrl}/readyz`),
            request.get(`${queryApiBaseUrl}/readyz`),
          ]);

          if (!controlReady.ok() || !queryReady.ok()) {
            return "not-ready";
          }

          const searchResponse = await request.get(`${queryApiBaseUrl}/v1/search?q=ranking`);
          if (!searchResponse.ok()) {
            return "search-not-ready";
          }

          const payload = await searchResponse.json();
          return payload.total_estimate > 0 ? "ready" : "empty";
        } catch {
          return "not-ready";
        }
      },
      {
        timeout: 240_000,
        intervals: [1_000, 2_000, 3_000],
      },
    )
    .toBe("ready");
}

async function runCrawlerWorker(crawlerId: string, crawlerKey: string) {
  const workerArgs = [
    "worker",
    "--server",
    controlApiBaseUrl,
    "--crawler-id",
    crawlerId,
    "--crawler-key",
    crawlerKey,
    "--once",
    "--max-jobs",
    "1",
  ];
  const cargoArgs = ["run", "-p", "findverse-crawler", "--", ...workerArgs];
  const workerEnv = {
    ...process.env,
    HTTP_PROXY: "",
    HTTPS_PROXY: "",
    ALL_PROXY: "",
    http_proxy: "",
    https_proxy: "",
    all_proxy: "",
    NO_PROXY: "127.0.0.1,localhost,host.docker.internal,172.30.3.194",
    no_proxy: "127.0.0.1,localhost,host.docker.internal,172.30.3.194",
    RUSTFLAGS: `${process.env.RUSTFLAGS ?? ""} -Awarnings`.trim(),
  };
  const localBinary = path.join(repoRoot, "target", "debug", "findverse-crawler");

  if (existsSync(localBinary)) {
    await execFileAsync(localBinary, workerArgs, {
      cwd: repoRoot,
      timeout: 180_000,
      env: workerEnv,
    });
    return;
  }

  await execFileAsync("cargo", cargoArgs, {
    cwd: repoRoot,
    timeout: 180_000,
    env: workerEnv,
  });
}

test.beforeEach(async ({ request }) => {
  await waitForFindVerseReady(request);
});

test("developer self-service, admin management, crawler credential issuance, and search flow", async ({
  page,
  request,
}) => {
  const username = process.env.FINDVERSE_LOCAL_ADMIN_USERNAME ?? "admin";
  const password = process.env.FINDVERSE_LOCAL_ADMIN_PASSWORD ?? "change-me";
  const seedUrl = `https://example.com/?findverse-e2e=${Date.now()}`;
  const developerUsername = `dev-${Date.now()}`;
  const developerPassword = "dev-password-123";

  await waitForFindVerseReady(request);

  await page.goto("/?q=ranking");
  await expect(page.locator("main article a").first()).toBeVisible();
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
  await expect(page.getByText("Developer search")).toBeVisible();
  await expect(page.locator("main article a").first()).toBeVisible();

  await page.goto("/console");
  await page.getByPlaceholder("Username").fill(username);
  await page.getByPlaceholder("Password").fill(password);
  await page.getByRole("button", { name: "Login" }).click();
  await expect(page.getByRole("heading", { name: "System Overview" })).toBeVisible();

  const sessionResponse = await request.post(`${controlApiBaseUrl}/v1/admin/session/login`, {
    data: { username, password },
  });
  expect(sessionResponse.ok()).toBeTruthy();
  const { token } = await sessionResponse.json();
  const crawlerResponse = await request.post(`${controlApiBaseUrl}/v1/admin/crawlers`, {
    headers: { Authorization: `Bearer ${token}` },
    data: { name: `e2e-crawler-${Date.now()}` },
  });
  expect(crawlerResponse.ok()).toBeTruthy();
  const { crawler_id: crawlerId, crawler_key: crawlerKey } = await crawlerResponse.json();

  await page.getByRole("button", { name: "Users" }).click();
  const developerRow = page
    .locator("article.developer-user-card")
    .filter({ hasText: developerUsername })
    .first();
  await expect(developerRow).toBeVisible();
  await developerRow.getByLabel(`Daily quota for ${developerUsername}`).fill("1234");
  await developerRow.getByRole("button", { name: "Save" }).click();
  await expect(page.getByText(/Refresh failed/i)).toHaveCount(0);

  await page.getByRole("button", { name: "Crawl Tasks" }).click();
  await page.getByPlaceholder("One URL per line").fill(seedUrl);
  await page.getByRole("button", { name: "Queue URLs" }).click();
  await expect(page.getByText(/Queued 1 URLs/i)).toBeVisible();

  await runCrawlerWorker(crawlerId, crawlerKey);

  await expect
    .poll(
      async () => {
        const overviewResponse = await request.get(`${controlApiBaseUrl}/v1/admin/crawl/overview`, {
          headers: { Authorization: `Bearer ${token}` },
        });
        if (!overviewResponse.ok()) {
          return "not-ready";
        }

        const overview = await overviewResponse.json();
        const jobsClaimed = overview.crawlers.reduce(
          (sum: number, crawler: { jobs_claimed: number }) => sum + crawler.jobs_claimed,
          0,
        );
        const jobsReported = overview.crawlers.reduce(
          (sum: number, crawler: { jobs_reported: number }) => sum + crawler.jobs_reported,
          0,
        );

        return overview.crawlers.length > 0 && jobsClaimed > 0 && jobsReported > 0
          ? "ready"
          : "waiting";
      },
      {
        timeout: 30_000,
        intervals: [500, 1_000, 2_000],
      },
    )
    .toBe("ready");

  await page.goto("/console");
  await expect(page.getByRole("heading", { name: "System Overview" })).toBeVisible();
  await page.getByRole("button", { name: "Workers" }).click();
  await expect(page.getByText("Crawler Workers")).toBeVisible();
  await expect(page.locator(".worker-density-grid").getByText("Jobs claimed")).toBeVisible();
  await expect(page.locator(".worker-density-grid").getByText("Jobs reported")).toBeVisible();

  await page.goto("/?q=ranking");
  await expect(page.locator("main article a").first()).toBeVisible();
});

test("crawler credential flow", async ({ request }) => {
  const username = process.env.FINDVERSE_LOCAL_ADMIN_USERNAME ?? "admin";
  const password = process.env.FINDVERSE_LOCAL_ADMIN_PASSWORD ?? "change-me";

  // Login as admin
  const loginRes = await request.post(`${controlApiBaseUrl}/v1/admin/session/login`, {
    data: { username, password },
  });
  expect(loginRes.ok()).toBeTruthy();
  const { token } = await loginRes.json();

  const createRes = await request.post(`${controlApiBaseUrl}/v1/admin/crawlers`, {
    headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
    data: { name: "e2e-issued-crawler" },
  });
  expect(createRes.status()).toBe(201);
  const issued = await createRes.json();
  expect(issued.crawler_id).toBeTruthy();
  expect(issued.crawler_key).toBeTruthy();
  expect(issued.name).toBe("e2e-issued-crawler");

  const deleteRes = await request.delete(`${controlApiBaseUrl}/v1/admin/crawlers/${issued.crawler_id}`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  expect(deleteRes.status()).toBe(204);
});

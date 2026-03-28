import { execFile } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { promisify } from "node:util";

import { expect, test, type APIRequestContext } from "@playwright/test";

const execFileAsync = promisify(execFile);
const repoRoot = path.resolve(process.cwd());
const controlApiBaseUrl = process.env.PLAYWRIGHT_API_BASE_URL ?? "http://127.0.0.1:8080";
const queryApiBaseUrl = process.env.PLAYWRIGHT_QUERY_API_BASE_URL ?? "http://127.0.0.1:8081";
const wslDistro = process.env.PLAYWRIGHT_WSL_DISTRO ?? "Debian";
const wslUser = process.env.PLAYWRIGHT_WSL_USER ?? "root";

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

function toWslPath(value: string) {
  const normalized = value.replace(/\\/g, "/");
  if (/^[A-Za-z]:\//.test(normalized)) {
    return `/mnt/${normalized[0].toLowerCase()}${normalized.slice(2)}`;
  }
  return normalized;
}

async function runCrawlerWorker(joinKey: string) {
  const workerArgs = [
    "worker",
    "--server",
    controlApiBaseUrl,
    "--join-key",
    joinKey,
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
  const localBinary = path.join(
    repoRoot,
    "target",
    "debug",
    process.platform === "win32" ? "findverse-crawler.exe" : "findverse-crawler",
  );

  if (existsSync(localBinary)) {
    await execFileAsync(localBinary, workerArgs, {
      cwd: repoRoot,
      timeout: 180_000,
      env: workerEnv,
    });
    return;
  }

  if (process.platform === "win32") {
    try {
      await execFileAsync("cargo", cargoArgs, {
        cwd: repoRoot,
        timeout: 180_000,
        env: workerEnv,
      });
      return;
    } catch {
      const command = `cd ${JSON.stringify(toWslPath(repoRoot))} && cargo ${cargoArgs.join(" ")}`;
      await execFileAsync(
        "wsl.exe",
        ["-d", wslDistro, "-u", wslUser, "--", "bash", "-lc", command],
        {
          cwd: repoRoot,
          timeout: 180_000,
          env: workerEnv,
        },
      );
      return;
    }
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

test("developer self-service, admin management, crawler join registration, and search flow", async ({
  page,
  request,
}) => {
  const username = process.env.FINDVERSE_LOCAL_ADMIN_USERNAME ?? "admin";
  const password = process.env.FINDVERSE_LOCAL_ADMIN_PASSWORD ?? "change-me";
  const seedUrl = `https://example.com/?findverse-e2e=${Date.now()}`;
  const developerUsername = `dev-${Date.now()}`;
  const developerPassword = "dev-password-123";
  const joinKey = `e2e-join-key-${Date.now()}`;

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

  await page.getByRole("button", { name: "Settings" }).click();
  await expect(page.getByRole("heading", { name: "Crawler Join Key" })).toBeVisible();
  await page.getByPlaceholder("Join key for enrolling new workers").fill(joinKey);
  await page.getByRole("button", { name: "Save key" }).click();
  await expect(page.getByText("Join key updated")).toBeVisible();

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

  await runCrawlerWorker(joinKey);

  const sessionResponse = await request.post(`${controlApiBaseUrl}/v1/admin/session/login`, {
    data: { username, password },
  });
  expect(sessionResponse.ok()).toBeTruthy();
  const { token } = await sessionResponse.json();

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

test("crawler join key flow", async ({ request }) => {
  const username = process.env.FINDVERSE_LOCAL_ADMIN_USERNAME ?? "admin";
  const password = process.env.FINDVERSE_LOCAL_ADMIN_PASSWORD ?? "change-me";

  // Login as admin
  const loginRes = await request.post(`${controlApiBaseUrl}/v1/admin/session/login`, {
    data: { username, password },
  });
  expect(loginRes.ok()).toBeTruthy();
  const { token } = await loginRes.json();

  // Set join key via admin API
  const joinKey = `test-join-key-${Date.now()}`;
  const setRes = await request.put(`${controlApiBaseUrl}/v1/admin/crawler-join-key`, {
    headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
    data: { join_key: joinKey },
  });
  expect(setRes.status()).toBe(204);

  // Read it back
  const getRes = await request.get(`${controlApiBaseUrl}/v1/admin/crawler-join-key`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  expect(getRes.ok()).toBeTruthy();
  const { join_key } = await getRes.json();
  expect(join_key).toBe(joinKey);

  // Join with correct key
  const joinRes = await request.post(`${controlApiBaseUrl}/internal/crawlers/join`, {
    data: { join_key: joinKey, name: "e2e-join-crawler" },
  });
  expect(joinRes.status()).toBe(201);
  const joined = await joinRes.json();
  expect(joined.crawler_id).toBeTruthy();
  expect(joined.crawler_key).toBeTruthy();
  expect(joined.name).toBe("e2e-join-crawler");

  // Join with wrong key should fail
  const badRes = await request.post(`${controlApiBaseUrl}/internal/crawlers/join`, {
    data: { join_key: "wrong-key", name: "bad-crawler" },
  });
  expect(badRes.status()).toBe(401);
});

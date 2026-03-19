import { execFile } from "node:child_process";
import path from "node:path";
import { promisify } from "node:util";

import { expect, test } from "@playwright/test";

const execFileAsync = promisify(execFile);
const repoRoot = path.resolve(process.cwd());

test("developer login, crawler seed, worker ingestion, and search result flow", async ({
  page,
}) => {
  const username = process.env.FINDVERSE_LOCAL_ADMIN_USERNAME ?? "admin";
  const password = process.env.FINDVERSE_LOCAL_ADMIN_PASSWORD ?? "change-me";
  const seedUrl = `https://example.com/?findverse-e2e=${Date.now()}`;
  const apiBaseUrl = process.env.PLAYWRIGHT_API_BASE_URL ?? "http://127.0.0.1:8080";

  await page.goto("/developers");
  await page.getByPlaceholder("Username").fill(username);
  await page.getByPlaceholder("Password").fill(password);
  await page.getByRole("button", { name: "Sign in" }).click();

  await expect(
    page.getByRole("heading", {
      name: "Developer access for search tools and crawler workers",
    }),
  ).toBeVisible();

  await page.getByRole("button", { name: "Create API key" }).click();
  await expect(page.locator("pre").filter({ hasText: "fvk_" }).first()).toBeVisible();

  await page.getByPlaceholder("Crawler name").fill("e2e-worker");
  await page.getByRole("button", { name: "Create crawler" }).click();

  const crawlerBlock = page
    .locator(".developer-card pre")
    .filter({ hasText: "CRAWLER_ID=" })
    .first();
  await expect(crawlerBlock).toBeVisible();
  const crawlerText = await crawlerBlock.textContent();
  const crawlerId = crawlerText?.match(/CRAWLER_ID=(.+)/)?.[1]?.trim();
  const crawlerKey = crawlerText?.match(/CRAWLER_KEY=(.+)/)?.[1]?.trim();
  expect(crawlerId).toBeTruthy();
  expect(crawlerKey).toBeTruthy();

  await page.getByPlaceholder("One URL per line").fill(seedUrl);
  await page.getByRole("button", { name: "Queue seed URLs" }).click();
  await expect(page.getByText("Queued 1 seed URLs")).toBeVisible();

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
      "--crawler-id",
      crawlerId!,
      "--crawler-key",
      crawlerKey!,
      "--once",
      "--max-jobs",
      "1",
    ],
    {
      cwd: repoRoot,
      timeout: 120_000,
    },
  );

  await page.reload();
  await expect(page.getByText(/claimed 1 jobs, reported 1/i)).toBeVisible();

  await page.goto("/search?q=Example%20Domain");
  await expect(page.getByRole("heading", { name: "Example Domain" }).first()).toBeVisible();
});

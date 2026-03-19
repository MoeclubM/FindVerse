"use client";

import { startTransition, useEffect, useState, type FormEvent } from "react";

import type {
  CrawlOverview,
  CreatedApiKey,
  CreatedCrawler,
  DeveloperUsage,
} from "@findverse/contracts";

type Session = {
  id: string;
  email: string;
  name: string;
};

type DeveloperPortalProps = {
  session: Session;
  initialUsage: DeveloperUsage;
  initialOverview: CrawlOverview;
};

export function DeveloperPortal({
  session,
  initialUsage,
  initialOverview,
}: DeveloperPortalProps) {
  const [usage, setUsage] = useState(initialUsage);
  const [overview, setOverview] = useState(initialOverview);
  const [keyName, setKeyName] = useState("CLI key");
  const [crawlerName, setCrawlerName] = useState("worker-us-east-1");
  const [seedInput, setSeedInput] = useState("https://example.com/");
  const [isSaving, setIsSaving] = useState(false);
  const [flash, setFlash] = useState<string | null>(null);
  const [latestToken, setLatestToken] = useState<CreatedApiKey | null>(null);
  const [latestCrawler, setLatestCrawler] = useState<CreatedCrawler | null>(null);

  useEffect(() => {
    setUsage(initialUsage);
  }, [initialUsage]);

  useEffect(() => {
    setOverview(initialOverview);
  }, [initialOverview]);

  async function refreshUsage() {
    const response = await fetch("/api/developer/usage", {
      method: "GET",
      cache: "no-store",
    });

    if (!response.ok) {
      throw new Error("Failed to refresh usage");
    }

    const nextUsage = (await response.json()) as DeveloperUsage;
    setUsage(nextUsage);
  }

  async function refreshOverview() {
    const response = await fetch("/api/developer/crawl/overview", {
      method: "GET",
      cache: "no-store",
    });

    if (!response.ok) {
      throw new Error("Failed to refresh crawler overview");
    }

    const nextOverview = (await response.json()) as CrawlOverview;
    setOverview(nextOverview);
  }

  function refreshAll() {
    startTransition(() => {
      void refreshUsage();
      void refreshOverview();
    });
  }

  async function handleCreateKey(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSaving(true);
    setFlash(null);

    try {
      const response = await fetch("/api/developer/keys", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ name: keyName }),
      });

      if (!response.ok) {
        throw new Error(await response.text());
      }

      const created = (await response.json()) as CreatedApiKey;
      setLatestToken(created);
      setKeyName("CLI key");
      refreshAll();
    } catch (error) {
      setFlash(error instanceof Error ? error.message : "Failed to create key");
    } finally {
      setIsSaving(false);
    }
  }

  async function handleCreateCrawler(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSaving(true);
    setFlash(null);

    try {
      const response = await fetch("/api/developer/crawlers", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ name: crawlerName }),
      });

      if (!response.ok) {
        throw new Error(await response.text());
      }

      const created = (await response.json()) as CreatedCrawler;
      setLatestCrawler(created);
      setCrawlerName("worker-us-east-1");
      refreshOverview();
    } catch (error) {
      setFlash(
        error instanceof Error ? error.message : "Failed to create crawler key",
      );
    } finally {
      setIsSaving(false);
    }
  }

  async function handleSeedFrontier(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setIsSaving(true);
    setFlash(null);

    const urls = seedInput
      .split(/\r?\n/)
      .map((value) => value.trim())
      .filter(Boolean);

    try {
      const response = await fetch("/api/developer/frontier/seed", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          urls,
          source: "developer-portal",
        }),
      });

      if (!response.ok) {
        throw new Error(await response.text());
      }

      await response.json();
      refreshOverview();
      setFlash(`Queued ${urls.length} seed URLs`);
    } catch (error) {
      setFlash(error instanceof Error ? error.message : "Failed to seed frontier");
    } finally {
      setIsSaving(false);
    }
  }

  async function handleRevokeKey(id: string) {
    setFlash(null);

    try {
      const response = await fetch(`/api/developer/keys/${id}`, {
        method: "DELETE",
      });

      if (!response.ok) {
        throw new Error(await response.text());
      }

      refreshUsage();
    } catch (error) {
      setFlash(error instanceof Error ? error.message : "Failed to revoke key");
    }
  }

  return (
    <section className="developer-shell">
      <p>
        Signed in as {session.email}. Developer id <code>{session.id}</code>
      </p>
      <h1>Developer access for search tools and crawler workers</h1>
      <p>
        Public search uses the REST API. Dedicated crawler workers connect over
        internal endpoints with <code>x-crawler-id</code> plus bearer key, claim
        jobs, and push parsed pages back into the index.
      </p>

      {flash ? <p>{flash}</p> : null}
      {latestToken ? (
        <div className="developer-card">
          <h2>New search API key</h2>
          <p>
            Copy this now. The full token is only shown once for{" "}
            <strong>{latestToken.name}</strong>.
          </p>
          <pre>{latestToken.token}</pre>
        </div>
      ) : null}
      {latestCrawler ? (
        <div className="developer-card" style={{ marginTop: 20 }}>
          <h2>New crawler key</h2>
          <p>
            This worker credential is bound to crawler <strong>{latestCrawler.id}</strong>.
          </p>
          <pre>{`CRAWLER_ID=${latestCrawler.id}
CRAWLER_KEY=${latestCrawler.key}`}</pre>
        </div>
      ) : null}

      <div className="developer-grid">
        <div className="developer-card">
          <h2>Search API limits</h2>
          <p>{usage.qps_limit} QPS per key</p>
          <p>{usage.daily_limit.toLocaleString()} requests per day</p>
          <p>{usage.used_today.toLocaleString()} used today</p>
          <pre>{`curl -H "Authorization: Bearer YOUR_KEY" \\
  "http://localhost:8080/v1/search?q=search+ranking"`}</pre>
        </div>

        <div className="developer-card">
          <h2>Crawl overview</h2>
          <p>{overview.frontier_depth.toLocaleString()} queued frontier jobs</p>
          <p>{overview.in_flight_jobs.toLocaleString()} jobs in flight</p>
          <p>{overview.known_urls.toLocaleString()} known URLs</p>
          <p>{overview.indexed_documents.toLocaleString()} indexed documents</p>
        </div>
      </div>

      <div className="developer-grid" style={{ marginTop: 20 }}>
        <div className="developer-card">
          <h2>Create search API key</h2>
          <form onSubmit={handleCreateKey}>
            <input
              value={keyName}
              onChange={(event) => setKeyName(event.target.value)}
              placeholder="Key name"
            />
            <button disabled={isSaving} type="submit">
              {isSaving ? "Creating..." : "Create API key"}
            </button>
          </form>
        </div>

        <div className="developer-card">
          <h2>Create crawler worker credential</h2>
          <form onSubmit={handleCreateCrawler}>
            <input
              value={crawlerName}
              onChange={(event) => setCrawlerName(event.target.value)}
              placeholder="Crawler name"
            />
            <button disabled={isSaving} type="submit">
              {isSaving ? "Creating..." : "Create crawler"}
            </button>
          </form>
        </div>
      </div>

      <div className="developer-grid" style={{ marginTop: 20 }}>
        <div className="developer-card">
          <h2>Seed the frontier</h2>
          <form onSubmit={handleSeedFrontier}>
            <textarea
              className="developer-textarea"
              value={seedInput}
              onChange={(event) => setSeedInput(event.target.value)}
              placeholder="One URL per line"
            />
            <button disabled={isSaving} type="submit">
              {isSaving ? "Queueing..." : "Queue seed URLs"}
            </button>
          </form>
        </div>

        <div className="developer-card">
          <h2>Run a worker container</h2>
          <pre>{`docker run --rm \\
  -e RUST_LOG=info \\
  findverse-crawler:latest \\
  worker \\
  --server http://api:8080 \\
  --crawler-id YOUR_CRAWLER_ID \\
  --crawler-key YOUR_CRAWLER_KEY`}</pre>
        </div>
      </div>

      <div className="developer-card" style={{ marginTop: 20 }}>
        <h2>Issued search API keys</h2>
        {usage.keys.length === 0 ? <p>No keys yet.</p> : null}
        {usage.keys.map((key) => (
          <div key={key.id} className="key-row">
            <div>
              <strong>{key.name}</strong>
              <p>{key.preview}</p>
              <p>created {new Date(key.created_at).toLocaleString()}</p>
              {key.revoked_at ? (
                <p>revoked {new Date(key.revoked_at).toLocaleString()}</p>
              ) : null}
            </div>
            <button
              type="button"
              className="danger"
              disabled={Boolean(key.revoked_at)}
              onClick={() => void handleRevokeKey(key.id)}
            >
              {key.revoked_at ? "Revoked" : "Revoke"}
            </button>
          </div>
        ))}
      </div>

      <div className="developer-card" style={{ marginTop: 20 }}>
        <h2>Crawler workers</h2>
        {overview.crawlers.length === 0 ? <p>No crawler credentials yet.</p> : null}
        {overview.crawlers.map((crawler) => (
          <div key={crawler.id} className="key-row">
            <div>
              <strong>{crawler.name}</strong>
              <p>{crawler.id}</p>
              <p>{crawler.preview}</p>
              <p>
                claimed {crawler.jobs_claimed.toLocaleString()} jobs, reported{" "}
                {crawler.jobs_reported.toLocaleString()}
              </p>
              <p>created {new Date(crawler.created_at).toLocaleString()}</p>
              {crawler.last_seen_at ? (
                <p>last seen {new Date(crawler.last_seen_at).toLocaleString()}</p>
              ) : null}
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

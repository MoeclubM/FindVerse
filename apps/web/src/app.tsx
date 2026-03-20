import { FormEvent, useEffect, useState } from "react";

import {
  AdminSession,
  CrawlOverview,
  DeveloperUsage,
  createApiKey,
  createCrawler,
  createRule,
  deleteDocument,
  deleteRule,
  getCrawlOverview,
  getSession,
  getUsage,
  listDocuments,
  login,
  logout,
  purgeSite,
  revokeApiKey,
  search,
  seedFrontier,
  updateRule,
} from "./api";

const CONSOLE_TOKEN_KEY = "findverse_console_token";

export function App() {
  const [path, setPath] = useState(() => window.location.pathname);

  useEffect(() => {
    const onPopState = () => setPath(window.location.pathname);
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, []);

  if (path.startsWith("/console")) {
    return <ConsolePage onNavigateHome={() => navigate("/", setPath)} />;
  }

  return <SearchPage onNavigateConsole={() => navigate("/console", setPath)} />;
}

function SearchPage(props: { onNavigateConsole: () => void }) {
  const [query, setQuery] = useState(currentSearchQuery);
  const [submittedQuery, setSubmittedQuery] = useState(currentSearchQuery);
  const [results, setResults] = useState<Awaited<ReturnType<typeof search>> | null>(null);
  const [loading, setLoading] = useState(() => Boolean(currentSearchQuery()));
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!submittedQuery.trim()) {
      setResults(null);
      setLoading(false);
      setError(null);
      return;
    }

    let cancelled = false;
    setLoading(true);
    setError(null);
    search(submittedQuery)
      .then((response) => {
        if (!cancelled) {
          setResults(response);
        }
      })
      .catch((nextError: Error) => {
        if (!cancelled) {
          setError(nextError.message);
          setResults(null);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [submittedQuery]);

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const nextQuery = query.trim();
    const nextUrl = nextQuery ? `/?q=${encodeURIComponent(nextQuery)}` : "/";
    window.history.pushState({}, "", nextUrl);
    setSubmittedQuery(nextQuery);
  }

  const hasResults = Boolean(results);

  return (
    <div className="search-shell">
      <button className="console-link" type="button" onClick={props.onNavigateConsole}>
        Console
      </button>
      <main className={hasResults ? "search-page search-page-top" : "search-page"}>
        {!hasResults && <h1 className="search-brand">FindVerse</h1>}
        <form className="search-form" onSubmit={handleSubmit}>
          <input
            aria-label="Search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search"
          />
          <button type="submit">Search</button>
        </form>

        {loading ? <p className="search-meta">Searching...</p> : null}
        {error ? <p className="search-error">{error}</p> : null}
        {results ? (
          <section className="results-list">
            <p className="search-meta">
              {results.total_estimate} results in {results.took_ms}ms
            </p>
            {results.results.map((result) => (
              <article key={result.id} className="result-item">
                <a href={result.url} target="_blank" rel="noreferrer">
                  {result.title}
                </a>
                <div className="result-url">{result.display_url}</div>
                <p>{result.snippet}</p>
              </article>
            ))}
          </section>
        ) : null}
      </main>
    </div>
  );
}

function ConsolePage(props: { onNavigateHome: () => void }) {
  const [token, setToken] = useState<string | null>(() => localStorage.getItem(CONSOLE_TOKEN_KEY));
  const [session, setSession] = useState<AdminSession | null>(null);
  const [usage, setUsage] = useState<DeveloperUsage | null>(null);
  const [overview, setOverview] = useState<CrawlOverview | null>(null);
  const [documents, setDocuments] = useState<Awaited<ReturnType<typeof listDocuments>> | null>(null);
  const [authLoading, setAuthLoading] = useState(Boolean(token));
  const [busy, setBusy] = useState(false);
  const [flash, setFlash] = useState<string | null>(null);
  const [latestApiKey, setLatestApiKey] = useState<string | null>(null);
  const [latestCrawlerSecret, setLatestCrawlerSecret] = useState<string | null>(null);
  const [loginUsername, setLoginUsername] = useState("");
  const [loginPassword, setLoginPassword] = useState("");
  const [apiKeyName, setApiKeyName] = useState("CLI key");
  const [crawlerName, setCrawlerName] = useState("worker-local");
  const [seedUrls, setSeedUrls] = useState("");
  const [seedDepth, setSeedDepth] = useState("2");
  const [seedAllowRevisit, setSeedAllowRevisit] = useState(false);
  const [ruleName, setRuleName] = useState("");
  const [ruleUrl, setRuleUrl] = useState("");
  const [ruleInterval, setRuleInterval] = useState("60");
  const [ruleDepth, setRuleDepth] = useState("2");
  const [documentQuery, setDocumentQuery] = useState("");
  const [documentSite, setDocumentSite] = useState("");
  const [purgeSiteInput, setPurgeSiteInput] = useState("");
  const [activeTab, setActiveTab] = useState<"overview" | "tasks" | "workers" | "documents" | "settings">("overview");

  useEffect(() => {
    if (!token) {
      setAuthLoading(false);
      setSession(null);
      return;
    }

    let cancelled = false;
    setAuthLoading(true);
    getSession(token)
      .then((nextSession) => {
        if (!cancelled) {
          setSession(nextSession);
        }
      })
      .catch(() => {
        if (!cancelled) {
          localStorage.removeItem(CONSOLE_TOKEN_KEY);
          setToken(null);
          setSession(null);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setAuthLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [token]);

  useEffect(() => {
    if (!token || !session) {
      return;
    }
    void refreshAll(token, documentQuery, documentSite, {
      setUsage,
      setOverview,
      setDocuments,
      setFlash,
    });
  }, [token, session, documentQuery, documentSite]);

  async function handleLogin(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setBusy(true);
    setFlash(null);
    try {
      const nextSession = await login(loginUsername, loginPassword);
      localStorage.setItem(CONSOLE_TOKEN_KEY, nextSession.token);
      setToken(nextSession.token);
      setSession(nextSession);
    } catch (error) {
      setFlash(error instanceof Error ? error.message : "Login failed");
    } finally {
      setBusy(false);
    }
  }

  async function handleLogout() {
    if (!token) {
      return;
    }
    setBusy(true);
    setFlash(null);
    try {
      await logout(token);
    } catch {
      // Ignore logout failures and clear local state anyway.
    } finally {
      localStorage.removeItem(CONSOLE_TOKEN_KEY);
      setToken(null);
      setSession(null);
      setUsage(null);
      setOverview(null);
      setDocuments(null);
      setBusy(false);
    }
  }

  async function handleCreateApiKey(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!token) {
      return;
    }
    setBusy(true);
    setFlash(null);
    try {
      const created = await createApiKey(token, apiKeyName);
      setLatestApiKey(created.token);
      setApiKeyName("CLI key");
      await refreshAll(token, documentQuery, documentSite, {
        setUsage,
        setOverview,
        setDocuments,
        setFlash,
      });
    } catch (error) {
      setFlash(error instanceof Error ? error.message : "API key creation failed");
    } finally {
      setBusy(false);
    }
  }

  async function handleRevokeApiKey(id: string) {
    if (!token) {
      return;
    }
    setBusy(true);
    setFlash(null);
    try {
      await revokeApiKey(token, id);
      await refreshAll(token, documentQuery, documentSite, {
        setUsage,
        setOverview,
        setDocuments,
        setFlash,
      });
    } catch (error) {
      setFlash(error instanceof Error ? error.message : "API key revoke failed");
    } finally {
      setBusy(false);
    }
  }

  async function handleCreateCrawler(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!token) {
      return;
    }
    setBusy(true);
    setFlash(null);
    try {
      const created = await createCrawler(token, crawlerName);
      setLatestCrawlerSecret(`CRAWLER_ID=${created.id}\nCRAWLER_KEY=${created.key}`);
      setCrawlerName("worker-local");
      await refreshAll(token, documentQuery, documentSite, {
        setUsage,
        setOverview,
        setDocuments,
        setFlash,
      });
    } catch (error) {
      setFlash(error instanceof Error ? error.message : "Crawler creation failed");
    } finally {
      setBusy(false);
    }
  }

  async function handleSeedFrontier(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!token) {
      return;
    }
    const urls = seedUrls
      .split(/\r?\n/)
      .map((value) => value.trim())
      .filter(Boolean);

    setBusy(true);
    setFlash(null);
    try {
      const response = await seedFrontier(
        token,
        urls,
        Number(seedDepth) || 2,
        seedAllowRevisit,
      );
      setFlash(`Queued ${response.accepted_urls} URLs`);
      await refreshAll(token, documentQuery, documentSite, {
        setUsage,
        setOverview,
        setDocuments,
        setFlash,
      });
    } catch (error) {
      setFlash(error instanceof Error ? error.message : "Frontier seed failed");
    } finally {
      setBusy(false);
    }
  }

  async function handleCreateRule(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!token) {
      return;
    }
    setBusy(true);
    setFlash(null);
    try {
      await createRule(token, {
        name: ruleName,
        seed_url: ruleUrl,
        interval_minutes: Number(ruleInterval) || 60,
        max_depth: Number(ruleDepth) || 2,
        enabled: true,
      });
      setRuleName("");
      await refreshAll(token, documentQuery, documentSite, {
        setUsage,
        setOverview,
        setDocuments,
        setFlash,
      });
    } catch (error) {
      setFlash(error instanceof Error ? error.message : "Rule creation failed");
    } finally {
      setBusy(false);
    }
  }

  async function handleToggleRule(ruleId: string, enabled: boolean) {
    if (!token) {
      return;
    }
    setBusy(true);
    setFlash(null);
    try {
      await updateRule(token, ruleId, { enabled: !enabled });
      await refreshAll(token, documentQuery, documentSite, {
        setUsage,
        setOverview,
        setDocuments,
        setFlash,
      });
    } catch (error) {
      setFlash(error instanceof Error ? error.message : "Rule update failed");
    } finally {
      setBusy(false);
    }
  }

  async function handleDeleteRule(ruleId: string) {
    if (!token) {
      return;
    }
    setBusy(true);
    setFlash(null);
    try {
      await deleteRule(token, ruleId);
      await refreshAll(token, documentQuery, documentSite, {
        setUsage,
        setOverview,
        setDocuments,
        setFlash,
      });
    } catch (error) {
      setFlash(error instanceof Error ? error.message : "Rule delete failed");
    } finally {
      setBusy(false);
    }
  }

  async function handleDeleteDocument(documentId: string) {
    if (!token) {
      return;
    }
    setBusy(true);
    setFlash(null);
    try {
      await deleteDocument(token, documentId);
      await refreshAll(token, documentQuery, documentSite, {
        setUsage,
        setOverview,
        setDocuments,
        setFlash,
      });
    } catch (error) {
      setFlash(error instanceof Error ? error.message : "Document delete failed");
    } finally {
      setBusy(false);
    }
  }

  async function handlePurgeSite(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!token) {
      return;
    }
    setBusy(true);
    setFlash(null);
    try {
      const response = await purgeSite(token, purgeSiteInput);
      setFlash(`Deleted ${response.deleted_documents} documents`);
      await refreshAll(token, documentQuery, documentSite, {
        setUsage,
        setOverview,
        setDocuments,
        setFlash,
      });
    } catch (error) {
      setFlash(error instanceof Error ? error.message : "Site purge failed");
    } finally {
      setBusy(false);
    }
  }

  if (authLoading) {
    return <div className="console-loading">Checking session…</div>;
  }

  if (!session || !token) {
    return (
      <div className="console-page">
        <header className="console-topbar">
          <button type="button" className="plain-link" onClick={props.onNavigateHome}>
            Search
          </button>
        </header>
        <main className="console-login">
          <h1>Sign in</h1>
          <form onSubmit={handleLogin}>
            <input
              value={loginUsername}
              onChange={(event) => setLoginUsername(event.target.value)}
              placeholder="Username"
            />
            <input
              type="password"
              value={loginPassword}
              onChange={(event) => setLoginPassword(event.target.value)}
              placeholder="Password"
            />
            <button type="submit" disabled={busy}>
              {busy ? "Signing in…" : "Sign in"}
            </button>
          </form>
          {flash ? <p className="search-error">{flash}</p> : null}
        </main>
      </div>
    );
  }

  return (
    <div className="console-page">
      <header className="console-topbar">
        <div>
          <strong>FindVerse Console</strong>
          <span>{session.developer_id}</span>
        </div>
        <div className="topbar-actions">
          <button
            type="button"
            className="plain-link"
            onClick={() =>
              refreshAll(token, documentQuery, documentSite, {
                setUsage,
                setOverview,
                setDocuments,
                setFlash,
              })
            }
          >
            Refresh
          </button>
          <button type="button" className="plain-link" onClick={props.onNavigateHome}>
            Search
          </button>
          <button type="button" className="plain-link" onClick={() => void handleLogout()}>
            Sign out
          </button>
        </div>
      </header>

      {flash ? <div className="flash">{flash}</div> : null}

      <nav className="console-tabs">
        <button className={activeTab === "overview" ? "active" : ""} onClick={() => setActiveTab("overview")}>Overview</button>
        <button className={activeTab === "tasks" ? "active" : ""} onClick={() => setActiveTab("tasks")}>Crawl Tasks</button>
        <button className={activeTab === "workers" ? "active" : ""} onClick={() => setActiveTab("workers")}>Workers</button>
        <button className={activeTab === "documents" ? "active" : ""} onClick={() => setActiveTab("documents")}>Documents</button>
        <button className={activeTab === "settings" ? "active" : ""} onClick={() => setActiveTab("settings")}>Settings</button>
      </nav>

      <main className="console-grid">
        {activeTab === "overview" && (<>
          <section className="panel">
            <h2>Overview</h2>
            <div className="stats-grid">
              <div>
                <span>Queued</span>
                <strong>{overview?.frontier_depth ?? 0}</strong>
              </div>
              <div>
                <span>Known URLs</span>
                <strong>{overview?.known_urls ?? 0}</strong>
              </div>
              <div>
                <span>In flight</span>
                <strong>{overview?.in_flight_jobs ?? 0}</strong>
              </div>
              <div>
                <span>Indexed docs</span>
                <strong>{overview?.indexed_documents ?? 0}</strong>
              </div>
            </div>
            <div className="stats-grid">
              <div>
                <span>QPS</span>
                <strong>{usage?.qps_limit ?? 0}</strong>
              </div>
              <div>
                <span>Daily quota</span>
                <strong>{usage?.daily_limit ?? 0}</strong>
              </div>
              <div>
                <span>Used today</span>
                <strong>{usage?.used_today ?? 0}</strong>
              </div>
              <div>
                <span>Rules</span>
                <strong>{overview?.rules.length ?? 0}</strong>
              </div>
            </div>
          </section>

          <section className="panel panel-wide">
            <h2>Crawl records</h2>
            <div className="list">
              {overview?.recent_events.map((event) => (
                <div className="list-row stacked" key={event.id}>
                  <strong>{event.kind}</strong>
                  <div>{event.message}</div>
                  <div>
                    {event.status} · {event.created_at}
                  </div>
                  {event.url ? <div>{event.url}</div> : null}
                </div>
              ))}
            </div>
          </section>
        </>)}

        {activeTab === "tasks" && (<>
          <section className="panel">
            <h2>Manual crawl</h2>
            <form onSubmit={handleSeedFrontier}>
              <textarea
                value={seedUrls}
                onChange={(event) => setSeedUrls(event.target.value)}
                placeholder="One URL per line"
              />
              <div className="inline-form">
                <input
                  value={seedDepth}
                  onChange={(event) => setSeedDepth(event.target.value)}
                  placeholder="Max depth"
                />
                <label className="checkbox">
                  <input
                    type="checkbox"
                    checked={seedAllowRevisit}
                    onChange={(event) => setSeedAllowRevisit(event.target.checked)}
                  />
                  Allow revisit
                </label>
                <button type="submit" disabled={busy}>
                  Queue
                </button>
              </div>
            </form>
          </section>

          <section className="panel">
            <h2>Auto crawl rules</h2>
            <form onSubmit={handleCreateRule}>
              <input
                value={ruleName}
                onChange={(event) => setRuleName(event.target.value)}
                placeholder="Rule name"
              />
              <input
                value={ruleUrl}
                onChange={(event) => setRuleUrl(event.target.value)}
                placeholder="Seed URL"
              />
              <div className="inline-form">
                <input
                  value={ruleInterval}
                  onChange={(event) => setRuleInterval(event.target.value)}
                  placeholder="Interval minutes"
                />
                <input
                  value={ruleDepth}
                  onChange={(event) => setRuleDepth(event.target.value)}
                  placeholder="Max depth"
                />
                <button type="submit" disabled={busy}>
                  Save
                </button>
              </div>
            </form>
            <div className="list">
              {overview?.rules.map((rule) => (
                <div className="list-row stacked" key={rule.id}>
                  <strong>{rule.name}</strong>
                  <div>{rule.seed_url}</div>
                  <div>
                    every {rule.interval_minutes} min, depth {rule.max_depth}
                  </div>
                  <div className="topbar-actions">
                    <button
                      type="button"
                      className="plain-link"
                      disabled={busy}
                      onClick={() => void handleToggleRule(rule.id, rule.enabled)}
                    >
                      {rule.enabled ? "Disable" : "Enable"}
                    </button>
                    <button
                      type="button"
                      className="plain-link"
                      disabled={busy}
                      onClick={() => void handleDeleteRule(rule.id)}
                    >
                      Delete
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </section>
        </>)}

        {activeTab === "workers" && (
          <section className="panel panel-wide">
            <h2>Crawler workers</h2>
            <form className="inline-form" onSubmit={handleCreateCrawler}>
              <input
                value={crawlerName}
                onChange={(event) => setCrawlerName(event.target.value)}
                placeholder="Crawler name"
              />
              <button type="submit" disabled={busy}>
                Create
              </button>
            </form>
            {latestCrawlerSecret ? <pre>{latestCrawlerSecret}</pre> : null}
            <div className="list">
              {overview?.crawlers.map((crawler) => (
                <div className="list-row stacked" key={crawler.id}>
                  <strong>{crawler.name}</strong>
                  <div>{crawler.id}</div>
                  <div>
                    claimed {crawler.jobs_claimed}, reported {crawler.jobs_reported}
                  </div>
                  <div>last seen {crawler.last_seen_at ?? "-"}</div>
                </div>
              ))}
            </div>
          </section>
        )}

        {activeTab === "documents" && (
          <section className="panel panel-wide">
            <h2>Indexed documents</h2>
            <div className="inline-form">
              <input
                value={documentQuery}
                onChange={(event) => setDocumentQuery(event.target.value)}
                placeholder="Filter by title or URL"
              />
              <input
                value={documentSite}
                onChange={(event) => setDocumentSite(event.target.value)}
                placeholder="Filter by site"
              />
            </div>
            <form className="inline-form" onSubmit={handlePurgeSite}>
              <input
                value={purgeSiteInput}
                onChange={(event) => setPurgeSiteInput(event.target.value)}
                placeholder="Site to purge"
              />
              <button type="submit" disabled={busy}>
                Purge site
              </button>
            </form>
            <div className="list">
              {documents?.documents.map((document) => (
                <div className="list-row stacked" key={document.id}>
                  <strong>{document.title}</strong>
                  <div>{document.display_url}</div>
                  <div>{document.last_crawled_at}</div>
                  <div>{document.snippet}</div>
                  <button
                    type="button"
                    className="plain-link"
                    disabled={busy}
                    onClick={() => void handleDeleteDocument(document.id)}
                  >
                    Delete
                  </button>
                </div>
              ))}
            </div>
          </section>
        )}

        {activeTab === "settings" && (
          <section className="panel panel-wide">
            <h2>API keys</h2>
            <form className="inline-form" onSubmit={handleCreateApiKey}>
              <input
                value={apiKeyName}
                onChange={(event) => setApiKeyName(event.target.value)}
                placeholder="Key name"
              />
              <button type="submit" disabled={busy}>
                Create
              </button>
            </form>
            {latestApiKey ? <pre>{latestApiKey}</pre> : null}
            <div className="list">
              {usage?.keys.map((key) => (
                <div className="list-row" key={key.id}>
                  <div>
                    <strong>{key.name}</strong>
                    <div>{key.preview}</div>
                  </div>
                  <button
                    type="button"
                    className="plain-link"
                    disabled={busy || Boolean(key.revoked_at)}
                    onClick={() => void handleRevokeApiKey(key.id)}
                  >
                    {key.revoked_at ? "Revoked" : "Revoke"}
                  </button>
                </div>
              ))}
            </div>
          </section>
        )}
      </main>
    </div>
  );
}

async function refreshAll(
  token: string,
  documentQuery: string,
  documentSite: string,
  actions: {
    setUsage: (value: DeveloperUsage | null) => void;
    setOverview: (value: CrawlOverview | null) => void;
    setDocuments: (value: Awaited<ReturnType<typeof listDocuments>> | null) => void;
    setFlash: (value: string | null) => void;
  },
) {
  try {
    const [usage, overview, documents] = await Promise.all([
      getUsage(token),
      getCrawlOverview(token),
      listDocuments(token, {
        query: documentQuery.trim() || undefined,
        site: documentSite.trim() || undefined,
      }),
    ]);
    actions.setUsage(usage);
    actions.setOverview(overview);
    actions.setDocuments(documents);
  } catch (error) {
    actions.setFlash(error instanceof Error ? error.message : "Refresh failed");
  }
}

function navigate(path: string, setPath: (path: string) => void) {
  window.history.pushState({}, "", path);
  setPath(path);
}

function currentSearchQuery() {
  return new URLSearchParams(window.location.search).get("q") ?? "";
}

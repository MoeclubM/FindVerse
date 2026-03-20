import { FormEvent, useEffect, useState } from "react";

import {
  AdminDeveloperRecord,
  AdminSession,
  CrawlOverview,
  DeveloperUsage,
  createApiKey,
  createCrawler,
  createRule,
  deleteDocument,
  deleteRule,
  getCrawlOverview,
  getUsage,
  getSession,
  listDevelopers,
  listDocuments,
  login,
  logout,
  purgeSite,
  revokeApiKey,
  seedFrontier,
  updateDeveloper,
  updateRule,
} from "../api";
import { JoinKeyManager } from "./JoinKeyManager";

const CONSOLE_TOKEN_KEY = "findverse_console_token";

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

async function refreshAll(
  token: string,
  documentQuery: string,
  documentSite: string,
  actions: {
    setUsage: (value: DeveloperUsage | null) => void;
    setOverview: (value: CrawlOverview | null) => void;
    setDevelopers: (value: AdminDeveloperRecord[]) => void;
    setDocuments: (value: Awaited<ReturnType<typeof listDocuments>> | null) => void;
    setFlash: (value: string | null) => void;
  },
) {
  try {
    const [usage, overview, developers, documents] = await Promise.all([
      getUsage(token),
      getCrawlOverview(token),
      listDevelopers(token),
      listDocuments(token, {
        query: documentQuery.trim() || undefined,
        site: documentSite.trim() || undefined,
      }),
    ]);
    actions.setUsage(usage);
    actions.setOverview(overview);
    actions.setDevelopers(developers);
    actions.setDocuments(documents);
  } catch (error) {
    actions.setFlash(getErrorMessage(error, "Refresh failed"));
  }
}

export function ConsolePage(props: { onNavigateHome: () => void }) {
  const [token, setToken] = useState<string | null>(() => localStorage.getItem(CONSOLE_TOKEN_KEY));
  const [session, setSession] = useState<AdminSession | null>(null);
  const [usage, setUsage] = useState<DeveloperUsage | null>(null);
  const [overview, setOverview] = useState<CrawlOverview | null>(null);
  const [developers, setDevelopers] = useState<AdminDeveloperRecord[]>([]);
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
  const [developerDrafts, setDeveloperDrafts] = useState<Record<string, { qps_limit: string; daily_limit: string }>>({});
  const [activeTab, setActiveTab] = useState<
    "overview" | "users" | "tasks" | "workers" | "documents" | "settings"
  >("overview");

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
      setDevelopers,
      setDocuments,
      setFlash,
    });
  }, [token, session, documentQuery, documentSite]);

  useEffect(() => {
    setDeveloperDrafts((current) => {
      const next = { ...current };
      for (const developer of developers) {
        next[developer.user_id] ??= {
          qps_limit: String(developer.qps_limit),
          daily_limit: String(developer.daily_limit),
        };
      }
      return next;
    });
  }, [developers]);

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
      setFlash(getErrorMessage(error, "Login failed"));
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
      setDevelopers([]);
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
        setDevelopers,
        setDocuments,
        setFlash,
      });
    } catch (error) {
      setFlash(getErrorMessage(error, "API key creation failed"));
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
        setDevelopers,
        setDocuments,
        setFlash,
      });
    } catch (error) {
      setFlash(getErrorMessage(error, "API key revoke failed"));
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
        setDevelopers,
        setDocuments,
        setFlash,
      });
    } catch (error) {
      setFlash(getErrorMessage(error, "Crawler creation failed"));
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
        setDevelopers,
        setDocuments,
        setFlash,
      });
    } catch (error) {
      setFlash(getErrorMessage(error, "Frontier seed failed"));
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
      setRuleUrl("");
      await refreshAll(token, documentQuery, documentSite, {
        setUsage,
        setOverview,
        setDevelopers,
        setDocuments,
        setFlash,
      });
    } catch (error) {
      setFlash(getErrorMessage(error, "Rule creation failed"));
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
        setDevelopers,
        setDocuments,
        setFlash,
      });
    } catch (error) {
      setFlash(getErrorMessage(error, "Rule update failed"));
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
        setDevelopers,
        setDocuments,
        setFlash,
      });
    } catch (error) {
      setFlash(getErrorMessage(error, "Rule delete failed"));
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
        setDevelopers,
        setDocuments,
        setFlash,
      });
    } catch (error) {
      setFlash(getErrorMessage(error, "Document delete failed"));
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
        setDevelopers,
        setDocuments,
        setFlash,
      });
    } catch (error) {
      setFlash(getErrorMessage(error, "Site purge failed"));
    } finally {
      setBusy(false);
    }
  }

  async function handleToggleDeveloperEnabled(user: AdminDeveloperRecord) {
    if (!token) {
      return;
    }
    setBusy(true);
    setFlash(null);
    try {
      await updateDeveloper(token, user.user_id, { enabled: !user.enabled });
      await refreshAll(token, documentQuery, documentSite, {
        setUsage,
        setOverview,
        setDevelopers,
        setDocuments,
        setFlash,
      });
    } catch (error) {
      setFlash(getErrorMessage(error, "Developer update failed"));
    } finally {
      setBusy(false);
    }
  }

  async function handleSaveDeveloperQuota(userId: string) {
    if (!token) {
      return;
    }
    const draft = developerDrafts[userId];
    if (!draft) {
      return;
    }

    setBusy(true);
    setFlash(null);
    try {
      await updateDeveloper(token, userId, {
        qps_limit: Math.max(1, Number(draft.qps_limit) || 1),
        daily_limit: Math.max(1, Number(draft.daily_limit) || 1),
      });
      await refreshAll(token, documentQuery, documentSite, {
        setUsage,
        setOverview,
        setDevelopers,
        setDocuments,
        setFlash,
      });
    } catch (error) {
      setFlash(getErrorMessage(error, "Quota update failed"));
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
          <span>{session.username}</span>
        </div>
        <div className="topbar-actions">
          <button
            type="button"
            className="plain-link"
            onClick={() =>
              refreshAll(token, documentQuery, documentSite, {
                setUsage,
                setOverview,
                setDevelopers,
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
        <button className={activeTab === "users" ? "active" : ""} onClick={() => setActiveTab("users")}>Users</button>
        <button className={activeTab === "tasks" ? "active" : ""} onClick={() => setActiveTab("tasks")}>Crawl Tasks</button>
        <button className={activeTab === "workers" ? "active" : ""} onClick={() => setActiveTab("workers")}>Workers</button>
        <button className={activeTab === "documents" ? "active" : ""} onClick={() => setActiveTab("documents")}>Documents</button>
        <button className={activeTab === "settings" ? "active" : ""} onClick={() => setActiveTab("settings")}>Settings</button>
      </nav>

      <main className="console-grid">
        <section className="panel panel-wide compact-panel">
          <div className="summary-strip">
            <div>
              <span>User</span>
              <strong>{session.username}</strong>
            </div>
            <div>
              <span>Developer ID</span>
              <strong>{session.developer_id}</strong>
            </div>
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
              <span>Automation</span>
              <strong>{overview?.recent_events[0]?.kind ?? "idle"}</strong>
            </div>
          </div>
        </section>

        {activeTab === "overview" && (
          <>
            <section className="panel panel-wide compact-panel">
              <div className="section-header">
                <h2>Overview</h2>
                <span className="section-meta">{overview?.recent_events.length ?? 0} recent events</span>
              </div>
              <div className="dense-grid">
                <div className="metric-card">
                  <span>Queued</span>
                  <strong>{overview?.frontier_depth ?? 0}</strong>
                </div>
                <div className="metric-card">
                  <span>Known URLs</span>
                  <strong>{overview?.known_urls ?? 0}</strong>
                </div>
                <div className="metric-card">
                  <span>In flight</span>
                  <strong>{overview?.in_flight_jobs ?? 0}</strong>
                </div>
                <div className="metric-card">
                  <span>Indexed docs</span>
                  <strong>{overview?.indexed_documents ?? 0}</strong>
                </div>
                <div className="metric-card">
                  <span>Rules</span>
                  <strong>{overview?.rules.length ?? 0}</strong>
                </div>
                <div className="metric-card">
                  <span>Workers</span>
                  <strong>{overview?.crawlers.length ?? 0}</strong>
                </div>
                <div className="metric-card">
                  <span>Keys</span>
                  <strong>{usage?.keys.length ?? 0}</strong>
                </div>
                <div className="metric-card">
                  <span>Developer users</span>
                  <strong>{developers.length}</strong>
                </div>
              </div>
            </section>

            <section className="panel panel-wide compact-panel">
              <div className="section-header">
                <h2>Recent crawl events</h2>
                <span className="section-meta">Automation health and worker activity</span>
              </div>
              <div className="dense-list">
                {overview?.recent_events.length ? (
                  overview.recent_events.map((event) => (
                    <div className="compact-row event-row" key={event.id}>
                      <div className="row-primary">
                        <strong>{event.kind}</strong>
                        <span>{event.message}</span>
                      </div>
                      <div className="row-meta">
                        <span className={event.status === "ok" ? "status-pill" : "status-pill status-pill-muted"}>
                          {event.status}
                        </span>
                        <span>{event.created_at}</span>
                        {event.url ? <span>{event.url}</span> : null}
                        {event.crawler_id ? <span>{event.crawler_id}</span> : null}
                      </div>
                    </div>
                  ))
                ) : (
                  <div className="list-row">No crawl events yet.</div>
                )}
              </div>
            </section>
          </>
        )}

        {activeTab === "users" && (
          <section className="panel panel-wide compact-panel">
            <div className="section-header">
              <h2>Developer users</h2>
              <span className="section-meta">{developers.length} accounts</span>
            </div>
            <div className="table-head developer-table">
              <span>User</span>
              <span>Status</span>
              <span>QPS</span>
              <span>Daily</span>
              <span>Used</span>
              <span>Keys</span>
              <span>Created</span>
              <span>Actions</span>
            </div>
            <div className="dense-list">
              {developers.length ? (
                developers.map((developer) => {
                  const draft = developerDrafts[developer.user_id] ?? {
                    qps_limit: String(developer.qps_limit),
                    daily_limit: String(developer.daily_limit),
                  };
                  return (
                    <div className="table-row developer-table" key={developer.user_id}>
                      <div className="cell cell-primary">
                        <strong>{developer.username}</strong>
                        <span>{developer.user_id}</span>
                      </div>
                      <div className="cell">
                        <span className={developer.enabled ? "status-pill" : "status-pill status-pill-muted"}>
                          {developer.enabled ? "Enabled" : "Disabled"}
                        </span>
                      </div>
                      <div className="cell">
                        <input
                          aria-label={`QPS limit for ${developer.username}`}
                          value={draft.qps_limit}
                          onChange={(event) =>
                            setDeveloperDrafts((current) => ({
                              ...current,
                              [developer.user_id]: {
                                ...draft,
                                qps_limit: event.target.value,
                              },
                            }))
                          }
                          placeholder="QPS"
                        />
                      </div>
                      <div className="cell">
                        <input
                          aria-label={`Daily quota for ${developer.username}`}
                          value={draft.daily_limit}
                          onChange={(event) =>
                            setDeveloperDrafts((current) => ({
                              ...current,
                              [developer.user_id]: {
                                ...draft,
                                daily_limit: event.target.value,
                              },
                            }))
                          }
                          placeholder="Daily quota"
                        />
                      </div>
                      <div className="cell">
                        <strong>{developer.used_today}</strong>
                      </div>
                      <div className="cell">
                        <strong>{developer.key_count}</strong>
                      </div>
                      <div className="cell">
                        <span>{developer.created_at}</span>
                      </div>
                      <div className="cell cell-actions">
                        <button
                          type="button"
                          disabled={busy}
                          onClick={() => void handleSaveDeveloperQuota(developer.user_id)}
                        >
                          Save
                        </button>
                        <button
                          type="button"
                          className="plain-link"
                          disabled={busy}
                          onClick={() => void handleToggleDeveloperEnabled(developer)}
                        >
                          {developer.enabled ? "Disable" : "Enable"}
                        </button>
                      </div>
                    </div>
                  );
                })
              ) : (
                <div className="list-row">No developer accounts yet.</div>
              )}
            </div>
          </section>
        )}

        {activeTab === "tasks" && (
          <>
            <section className="panel compact-panel">
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

            <section className="panel compact-panel">
              <h2>New auto rule</h2>
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
            </section>

            <section className="panel panel-wide compact-panel">
              <div className="section-header">
                <h2>Auto crawl rules</h2>
                <span className="section-meta">{overview?.rules.length ?? 0} configured</span>
              </div>
              <div className="dense-list">
                {overview?.rules.length ? (
                  overview.rules.map((rule) => (
                    <div className="compact-row rule-row" key={rule.id}>
                      <div className="row-primary">
                        <strong>{rule.name}</strong>
                        <span>{rule.seed_url}</span>
                      </div>
                      <div className="metadata-grid compact-metadata">
                        <div>
                          <span>Interval</span>
                          <strong>{rule.interval_minutes} min</strong>
                        </div>
                        <div>
                          <span>Depth</span>
                          <strong>{rule.max_depth}</strong>
                        </div>
                        <div>
                          <span>Created</span>
                          <strong>{rule.created_at}</strong>
                        </div>
                        <div>
                          <span>Updated</span>
                          <strong>{rule.updated_at}</strong>
                        </div>
                        <div>
                          <span>Last enqueue</span>
                          <strong>{rule.last_enqueued_at ?? "never"}</strong>
                        </div>
                        <div>
                          <span>Status</span>
                          <strong>{rule.enabled ? "enabled" : "disabled"}</strong>
                        </div>
                      </div>
                      <div className="row-actions topbar-actions">
                        <span className={rule.enabled ? "status-pill" : "status-pill status-pill-muted"}>
                          {rule.enabled ? "Enabled" : "Disabled"}
                        </span>
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
                  ))
                ) : (
                  <div className="list-row">No crawl rules yet.</div>
                )}
              </div>
            </section>
          </>
        )}

        {activeTab === "workers" && (
          <section className="panel panel-wide compact-panel">
            <div className="section-header">
              <h2>Crawler workers</h2>
              <span className="section-meta">{overview?.crawlers.length ?? 0} registered</span>
            </div>
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
            <p className="dev-hint">Production workers can also start with a developer API key and auto-register.</p>
            <div className="dense-list">
              {overview?.crawlers.length ? (
                overview.crawlers.map((crawler) => (
                  <div className="compact-row worker-row" key={crawler.id}>
                    <div className="row-primary">
                      <strong>{crawler.name}</strong>
                      <span>{crawler.id}</span>
                    </div>
                    <div className="metadata-grid compact-metadata">
                      <div>
                        <span>Preview</span>
                        <strong>{crawler.preview}</strong>
                      </div>
                      <div>
                        <span>Created</span>
                        <strong>{crawler.created_at}</strong>
                      </div>
                      <div>
                        <span>Last seen</span>
                        <strong>{crawler.last_seen_at ?? "-"}</strong>
                      </div>
                      <div>
                        <span>Last claim</span>
                        <strong>{crawler.last_claimed_at ?? "-"}</strong>
                      </div>
                      <div>
                        <span>Claimed</span>
                        <strong>{crawler.jobs_claimed}</strong>
                      </div>
                      <div>
                        <span>Reported</span>
                        <strong>{crawler.jobs_reported}</strong>
                      </div>
                    </div>
                  </div>
                ))
              ) : (
                <div className="list-row">No crawler workers yet.</div>
              )}
            </div>
          </section>
        )}

        {activeTab === "documents" && (
          <section className="panel panel-wide compact-panel">
            <div className="section-header">
              <h2>Indexed documents</h2>
              <span className="section-meta">
                {documents?.total_estimate ?? 0} total · next {documents?.next_offset ?? "-"}
              </span>
            </div>
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
            <div className="dense-list">
              {documents?.documents.length ? (
                documents.documents.map((document) => (
                  <div className="compact-row document-row" key={document.id}>
                    <div className="row-primary">
                      <strong>{document.title}</strong>
                      <span>{document.display_url}</span>
                    </div>
                    <div className="row-meta">
                      <span>{document.language}</span>
                      <span>{document.last_crawled_at}</span>
                    </div>
                    <p>{document.snippet}</p>
                    <div className="row-actions topbar-actions">
                      <button
                        type="button"
                        className="plain-link"
                        disabled={busy}
                        onClick={() => void handleDeleteDocument(document.id)}
                      >
                        Delete
                      </button>
                    </div>
                  </div>
                ))
              ) : (
                <div className="list-row">No indexed documents match the current filters.</div>
              )}
            </div>
          </section>
        )}

        {activeTab === "settings" && (
          <>
            <section className="panel panel-wide compact-panel">
              <div className="section-header">
                <h2>Crawler join key</h2>
                <span className="section-meta">External crawlers can self-register using this key</span>
              </div>
              <JoinKeyManager token={token} />
            </section>
            <section className="panel panel-wide compact-panel">
              <div className="section-header">
                <h2>API keys</h2>
                <span className="section-meta">{usage?.keys.length ?? 0} keys</span>
              </div>
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
              <div className="dense-list">
                {usage?.keys.length ? (
                  usage.keys.map((key) => (
                    <div className="compact-row key-row" key={key.id}>
                      <div className="row-primary">
                        <strong>{key.name}</strong>
                        <span>{key.preview}</span>
                      </div>
                      <div className="metadata-grid compact-metadata">
                        <div>
                          <span>Created</span>
                          <strong>{key.created_at}</strong>
                        </div>
                        <div>
                          <span>Revoked</span>
                          <strong>{key.revoked_at ?? "active"}</strong>
                        </div>
                      </div>
                      <div className="row-actions topbar-actions">
                        <span className={key.revoked_at ? "status-pill status-pill-muted" : "status-pill"}>
                          {key.revoked_at ? "Revoked" : "Active"}
                        </span>
                        <button
                          type="button"
                          className="plain-link"
                          disabled={busy || Boolean(key.revoked_at)}
                          onClick={() => void handleRevokeApiKey(key.id)}
                        >
                          {key.revoked_at ? "Revoked" : "Revoke"}
                        </button>
                      </div>
                    </div>
                  ))
                ) : (
                  <div className="list-row">No API keys yet.</div>
                )}
              </div>
            </section>
          </>
        )}
      </main>
    </div>
  );
}

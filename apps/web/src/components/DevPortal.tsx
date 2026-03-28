import { ExitIcon, MagnifyingGlassIcon } from "@radix-ui/react-icons";
import { FormEvent, useEffect, useState } from "react";

import {
  CreatedApiKey,
  DevSession,
  DeveloperDomainInsight,
  DeveloperUsage,
  createDeveloperKey,
  getDeveloperDomainInsight,
  getDeveloperKeys,
  getDeveloperSession,
  loginDeveloper,
  logoutDeveloper,
  registerDeveloper,
  revokeDeveloperKey,
  submitDeveloperDomain,
} from "../api";
import { AppTopbar, TopbarActionButton, TopbarBadge } from "./common/AppTopbar";
import { FieldShell, SectionHeader, StatStrip } from "./common/PanelPrimitives";
import type { ThemeMode } from "./ThemeSwitcher";

const DEV_SESSION_KEY = "findverse_dev_session";
const SITE_NAME = (import.meta.env.VITE_FINDVERSE_SITE_NAME || "FindVerse").trim() || "FindVerse";

function persistDevSession(token: string | null, setToken: (value: string | null) => void) {
  if (token) {
    localStorage.setItem(DEV_SESSION_KEY, token);
  } else {
    localStorage.removeItem(DEV_SESSION_KEY);
  }
  setToken(token);
}

function tokenPreview(token: string) {
  return `${token.slice(0, 8)}...${token.slice(-4)}`;
}

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

function formatPortalTimestamp(value: string | null) {
  if (!value) {
    return "Not yet";
  }

  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? value : parsed.toLocaleString();
}

function buildSeedSuggestions(domain: string) {
  return `https://${domain}/\nhttps://${domain}/sitemap.xml`;
}

export function DevPortalPage(props: {
  devToken: string | null;
  theme: "light" | "dark";
  themeMode: ThemeMode;
  onThemeModeChange: (theme: ThemeMode) => void;
  onTokenChange: (token: string | null) => void;
  onNavigateSearch: () => void;
}) {
  const [sessionToken, setSessionToken] = useState<string | null>(() => localStorage.getItem(DEV_SESSION_KEY));
  const [session, setSession] = useState<DevSession | null>(null);
  const [usage, setUsage] = useState<DeveloperUsage | null>(null);
  const [mode, setMode] = useState<"login" | "register">("login");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [keyName, setKeyName] = useState("Search key");
  const [latestKey, setLatestKey] = useState<CreatedApiKey | null>(null);
  const [busy, setBusy] = useState(false);
  const [loadingSession, setLoadingSession] = useState(Boolean(sessionToken));
  const [flash, setFlash] = useState<string | null>(null);
  const [propertyQuery, setPropertyQuery] = useState("");
  const [propertyInsight, setPropertyInsight] = useState<DeveloperDomainInsight | null>(null);
  const [propertyLoading, setPropertyLoading] = useState(false);
  const [propertySubmitting, setPropertySubmitting] = useState(false);
  const [submitDomain, setSubmitDomain] = useState("");
  const [submitUrls, setSubmitUrls] = useState("");
  const [submitDepth, setSubmitDepth] = useState("2");
  const [submitMaxPages, setSubmitMaxPages] = useState("50");
  const [submitRevisit, setSubmitRevisit] = useState(true);

  useEffect(() => {
    if (!sessionToken) {
      setLoadingSession(false);
      setSession(null);
      setUsage(null);
      return;
    }

    let cancelled = false;
    setLoadingSession(true);
    Promise.all([getDeveloperSession(sessionToken), getDeveloperKeys(sessionToken)])
      .then(([nextSession, nextUsage]) => {
        if (!cancelled) {
          setSession(nextSession);
          setUsage(nextUsage);
        }
      })
      .catch(() => {
        if (!cancelled) {
          persistDevSession(null, setSessionToken);
          setSession(null);
          setUsage(null);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoadingSession(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [sessionToken]);

  useEffect(() => {
    if (!propertyInsight) {
      return;
    }
    if (submitDomain === propertyInsight.domain) {
      return;
    }
    setPropertyQuery(propertyInsight.domain);
    setSubmitDomain(propertyInsight.domain);
    setSubmitUrls(buildSeedSuggestions(propertyInsight.domain));
    setSubmitDepth("2");
    setSubmitMaxPages("50");
    setSubmitRevisit(true);
  }, [propertyInsight, submitDomain]);

  const activePreview = props.devToken ? tokenPreview(props.devToken) : null;

  async function refreshUsage(token: string) {
    const nextUsage = await getDeveloperKeys(token);
    setUsage(nextUsage);
  }

  async function loadPropertyInsight(token: string, domain: string) {
    const insight = await getDeveloperDomainInsight(token, domain);
    setPropertyInsight(insight);
    return insight;
  }

  async function handleAuthSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setBusy(true);
    setFlash(null);
    try {
      const nextSession =
        mode === "register"
          ? await registerDeveloper(username, password)
          : await loginDeveloper(username, password);
      persistDevSession(nextSession.token, setSessionToken);
      setSession(nextSession);
      setUsername("");
      setPassword("");
      setLatestKey(null);
      await refreshUsage(nextSession.token);
    } catch (error) {
      setFlash(getErrorMessage(error, mode === "register" ? "Register failed" : "Login failed"));
    } finally {
      setBusy(false);
    }
  }

  async function handleSignOut() {
    setBusy(true);
    setFlash(null);
    try {
      if (sessionToken) {
        await logoutDeveloper(sessionToken);
      }
    } catch {
      // Ignore logout failures and clear local state anyway.
    } finally {
      persistDevSession(null, setSessionToken);
      setSession(null);
      setUsage(null);
      setLatestKey(null);
      setPropertyInsight(null);
      setPropertyQuery("");
      setSubmitDomain("");
      setSubmitUrls("");
      props.onTokenChange(null);
      setBusy(false);
    }
  }

  async function handleCreateKey(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!sessionToken) {
      return;
    }

    setBusy(true);
    setFlash(null);
    try {
      const created = await createDeveloperKey(sessionToken, keyName);
      setLatestKey(created);
      setKeyName("Search key");
      await refreshUsage(sessionToken);
    } catch (error) {
      setFlash(getErrorMessage(error, "API key creation failed"));
    } finally {
      setBusy(false);
    }
  }

  async function handleRevokeKey(id: string, preview: string) {
    if (!sessionToken) {
      return;
    }

    setBusy(true);
    setFlash(null);
    try {
      await revokeDeveloperKey(sessionToken, id);
      if (activePreview === preview) {
        props.onTokenChange(null);
      }
      if (latestKey?.id === id) {
        setLatestKey(null);
      }
      await refreshUsage(sessionToken);
    } catch (error) {
      setFlash(getErrorMessage(error, "API key revoke failed"));
    } finally {
      setBusy(false);
    }
  }

  async function handleAnalyzeProperty(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!sessionToken || !propertyQuery.trim()) {
      return;
    }

    setPropertyLoading(true);
    setFlash(null);
    try {
      await loadPropertyInsight(sessionToken, propertyQuery.trim());
    } catch (error) {
      setFlash(getErrorMessage(error, "Property analysis failed"));
    } finally {
      setPropertyLoading(false);
    }
  }

  async function handleSubmitProperty(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!sessionToken) {
      return;
    }

    const domain = submitDomain.trim() || propertyInsight?.domain || propertyQuery.trim();
    const urls = submitUrls
      .split(/\r?\n/)
      .map((value) => value.trim())
      .filter(Boolean);

    if (!domain || !urls.length) {
      setFlash("Enter a property and at least one URL before submitting.");
      return;
    }

    setPropertySubmitting(true);
    setFlash(null);
    try {
      const response = await submitDeveloperDomain(sessionToken, {
        domain,
        urls,
        max_depth: Math.max(0, Number(submitDepth) || 0),
        max_pages: Math.max(1, Number(submitMaxPages) || 1),
        allow_revisit: submitRevisit,
      });
      const insight = await loadPropertyInsight(sessionToken, domain);
      setSubmitDomain(insight.domain);
      setFlash(
        `Queued ${response.accepted_urls} URL(s). Domain queue now has ${response.queued_domain_jobs} pending job(s) across ${response.known_domain_urls} known URL(s).`,
      );
    } catch (error) {
      setFlash(getErrorMessage(error, "Property submission failed"));
    } finally {
      setPropertySubmitting(false);
    }
  }

  function handleUseSearchKey(token: string) {
    props.onTokenChange(token);
    setFlash("Active search key updated");
  }

  if (loadingSession) {
    return <div className="portal-loading">Checking developer session...</div>;
  }

  if (!session || !sessionToken) {
    return (
      <div className="portal-page">
        <AppTopbar
          theme={props.theme}
          themeMode={props.themeMode}
          onThemeModeChange={props.onThemeModeChange}
          title={`${SITE_NAME} · Developer Portal`}
          onTitleClick={props.onNavigateSearch}
          beforeControls={
            props.devToken ? <TopbarBadge theme={props.theme}>Key</TopbarBadge> : null
          }
          afterControls={
            <TopbarActionButton
              theme={props.theme}
              leading={<MagnifyingGlassIcon className="size-4" />}
              onClick={props.onNavigateSearch}
            >
              Search
            </TopbarActionButton>
          }
        />
        <main className="portal-auth-shell">
          <section className="portal-auth-card">
            <div className="auth-mode-switch">
              <button
                type="button"
                className={mode === "login" ? "active" : ""}
                onClick={() => setMode("login")}
              >
                Sign in
              </button>
              <button
                type="button"
                className={mode === "register" ? "active" : ""}
                onClick={() => setMode("register")}
              >
                Register
              </button>
            </div>
            <h1>{mode === "register" ? "Create developer account" : "Developer sign in"}</h1>
            <form onSubmit={handleAuthSubmit}>
              <input
                value={username}
                onChange={(event) => setUsername(event.target.value)}
                placeholder="Username"
                autoComplete="username"
              />
              <input
                type="password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
                placeholder="Password"
                autoComplete={mode === "register" ? "new-password" : "current-password"}
              />
              <button type="submit" disabled={busy}>
                {busy ? "Submitting..." : mode === "register" ? "Create account" : "Sign in"}
              </button>
            </form>
            {flash ? <p className="search-error">{flash}</p> : null}
            <p className="dev-hint">
              Create an account, generate an <code>fvk_</code> key, inspect a domain property, and
              submit URLs for crawl without leaving the portal.
            </p>
          </section>
        </main>
      </div>
    );
  }

  return (
    <div className="portal-page">
      <AppTopbar
        theme={props.theme}
        themeMode={props.themeMode}
        onThemeModeChange={props.onThemeModeChange}
        title={`${SITE_NAME} · Developer Portal`}
        onTitleClick={props.onNavigateSearch}
        beforeControls={props.devToken ? <TopbarBadge theme={props.theme}>Key</TopbarBadge> : null}
        afterControls={
          <>
            <TopbarActionButton
              theme={props.theme}
              leading={<MagnifyingGlassIcon className="size-4" />}
              onClick={props.onNavigateSearch}
            >
              Search
            </TopbarActionButton>
            <TopbarActionButton
              theme={props.theme}
              leading={<ExitIcon className="size-4" />}
              disabled={busy}
              onClick={() => void handleSignOut()}
            >
              Sign out
            </TopbarActionButton>
          </>
        }
      />

      {flash ? <div className="portal-flash">{flash}</div> : null}

      <main className="portal-main">
        <section className="panel panel-wide property-panel">
          <SectionHeader
            title="Site Console"
            meta="Inspect a property, review crawl coverage, then submit URLs with the shared crawler."
            actions={propertyInsight ? <span className="status-pill status-pill-muted">{propertyInsight.domain}</span> : null}
          />

          <form className="property-toolbar" onSubmit={handleAnalyzeProperty}>
            <FieldShell className="field-group-wide property-toolbar-field" label="Domain or URL">
              <input
                id="property-query"
                value={propertyQuery}
                onChange={(event) => setPropertyQuery(event.target.value)}
                placeholder="example.com or https://example.com/docs"
              />
            </FieldShell>
            <button type="submit" disabled={propertyLoading || !propertyQuery.trim()}>
              {propertyLoading ? "Analyzing..." : "Analyze property"}
            </button>
          </form>

          {propertyInsight ? (
            <>
              <StatStrip
                className="property-summary-strip"
                items={[
                  { label: "Indexed docs", value: propertyInsight.indexed_documents },
                  { label: "Duplicates", value: propertyInsight.duplicate_documents },
                  { label: "Pending crawl", value: propertyInsight.pending_jobs },
                  { label: "Indexed jobs", value: propertyInsight.successful_jobs },
                  { label: "Filtered jobs", value: propertyInsight.filtered_jobs },
                  { label: "Failures", value: propertyInsight.failed_jobs + propertyInsight.blocked_jobs },
                  { label: "Last indexed", value: formatPortalTimestamp(propertyInsight.last_indexed_at) },
                  { label: "Last crawl activity", value: formatPortalTimestamp(propertyInsight.last_crawled_at) },
                ]}
              />

              <div className="property-grid">
                <div className="property-stack">
                  <section className="property-card">
                    <SectionHeader
                      className="property-card-header"
                      heading="h3"
                      title="Recent indexed pages"
                      meta={`${propertyInsight.recent_documents.length} rows`}
                    />
                    <div className="dense-list">
                      {propertyInsight.recent_documents.length ? (
                        propertyInsight.recent_documents.map((document) => (
                          <div className="list-row stacked property-row" key={document.id}>
                            <div className="property-row-head">
                              <a href={document.url} target="_blank" rel="noreferrer">
                                {document.title}
                              </a>
                              {document.duplicate_of ? (
                                <span className="status-pill status-pill-muted">Duplicate</span>
                              ) : null}
                            </div>
                            <div className="property-row-meta">
                              <span>{document.display_url}</span>
                              <span>{document.language || "unknown"}</span>
                              <span>{document.content_type}</span>
                              <span>{document.word_count} words</span>
                              <span>{formatPortalTimestamp(document.last_crawled_at)}</span>
                            </div>
                          </div>
                        ))
                      ) : (
                        <div className="list-row">No indexed pages for this property yet.</div>
                      )}
                    </div>
                  </section>

                  <section className="property-card">
                    <SectionHeader
                      className="property-card-header"
                      heading="h3"
                      title="Coverage facets"
                      meta="Top distributions from indexed docs"
                    />
                    <div className="property-facet-grid">
                      <div className="property-facet-panel">
                        <h4>Languages</h4>
                        <div className="dense-list">
                          {propertyInsight.top_languages.length ? (
                            propertyInsight.top_languages.map((facet) => (
                              <div className="list-row property-facet-row" key={`lang-${facet.label}`}>
                                <span>{facet.label}</span>
                                <strong>{facet.count}</strong>
                              </div>
                            ))
                          ) : (
                            <div className="list-row">No language data yet.</div>
                          )}
                        </div>
                      </div>
                      <div className="property-facet-panel">
                        <h4>Content types</h4>
                        <div className="dense-list">
                          {propertyInsight.top_content_types.length ? (
                            propertyInsight.top_content_types.map((facet) => (
                              <div className="list-row property-facet-row" key={`type-${facet.label}`}>
                                <span>{facet.label}</span>
                                <strong>{facet.count}</strong>
                              </div>
                            ))
                          ) : (
                            <div className="list-row">No content type data yet.</div>
                          )}
                        </div>
                      </div>
                    </div>
                  </section>

                  <section className="property-card">
                    <SectionHeader
                      className="property-card-header"
                      heading="h3"
                      title="Recent crawl activity"
                      meta={`${propertyInsight.recent_jobs.length} rows`}
                    />
                    <div className="dense-list">
                      {propertyInsight.recent_jobs.length ? (
                        propertyInsight.recent_jobs.map((job) => (
                          <div className="list-row stacked property-row" key={job.id}>
                            <div className="property-row-head">
                              <a href={job.url} target="_blank" rel="noreferrer">
                                {job.url}
                              </a>
                              <span
                                className={
                                  job.status === "succeeded" && job.accepted_document_id
                                    ? "status-pill"
                                    : "status-pill status-pill-muted"
                                }
                              >
                                {job.status}
                              </span>
                            </div>
                            <div className="property-row-meta">
                              <span>depth {job.depth}</span>
                              <span>{job.http_status ? `HTTP ${job.http_status}` : "no status"}</span>
                              <span>{formatPortalTimestamp(job.finished_at ?? job.discovered_at)}</span>
                              {job.failure_kind ? <span>{job.failure_kind}</span> : null}
                              {job.failure_message ? <span>{job.failure_message}</span> : null}
                            </div>
                          </div>
                        ))
                      ) : (
                        <div className="list-row">No crawl history for this property yet.</div>
                      )}
                    </div>
                  </section>
                </div>

                <section className="property-submit-panel">
                  <SectionHeader
                    className="property-card-header"
                    heading="h3"
                    title="Submit URLs"
                    meta="Queue homepage, sitemap, or a focused URL set for this property."
                  />
                  <form onSubmit={handleSubmitProperty}>
                    <FieldShell label="Property">
                      <input
                        id="submit-domain"
                        value={submitDomain}
                        onChange={(event) => setSubmitDomain(event.target.value)}
                        placeholder="example.com"
                      />
                    </FieldShell>
                    <div className="topbar-actions property-quick-actions">
                      <button
                        type="button"
                        onClick={() => {
                          const domain = submitDomain || propertyInsight.domain;
                          setSubmitDomain(domain);
                          setSubmitUrls(`https://${domain}/`);
                        }}
                      >
                        Homepage only
                      </button>
                      <button
                        type="button"
                        onClick={() => {
                          const domain = submitDomain || propertyInsight.domain;
                          setSubmitDomain(domain);
                          setSubmitUrls(buildSeedSuggestions(domain));
                        }}
                      >
                        Homepage + sitemap
                      </button>
                    </div>
                    <FieldShell label="URL list">
                      <textarea
                        id="submit-urls"
                        rows={8}
                        value={submitUrls}
                        onChange={(event) => setSubmitUrls(event.target.value)}
                        placeholder="One URL per line"
                      />
                    </FieldShell>
                    <div className="property-submit-grid">
                      <FieldShell label="Crawl depth">
                        <input
                          id="submit-depth"
                          type="number"
                          min={0}
                          max={10}
                          value={submitDepth}
                          onChange={(event) => setSubmitDepth(event.target.value)}
                        />
                      </FieldShell>
                      <FieldShell label="Page budget">
                        <input
                          id="submit-pages"
                          type="number"
                          min={1}
                          max={10000}
                          value={submitMaxPages}
                          onChange={(event) => setSubmitMaxPages(event.target.value)}
                        />
                      </FieldShell>
                    </div>
                    <label className="checkbox">
                      <input
                        type="checkbox"
                        checked={submitRevisit}
                        onChange={(event) => setSubmitRevisit(event.target.checked)}
                      />
                      Allow revisit for URLs already seen before
                    </label>
                    <button type="submit" disabled={propertySubmitting || !submitUrls.trim()}>
                      {propertySubmitting ? "Submitting..." : "Queue property crawl"}
                    </button>
                  </form>
                  <p className="dev-hint">
                    This portal uses the shared crawler owner. It is designed as a lightweight
                    search-console workflow, not a fully isolated multi-tenant crawl pipeline.
                  </p>
                </section>
              </div>
            </>
          ) : (
            <div className="property-empty">
              Enter a domain or URL above to inspect indexed pages, crawl coverage, and submission
              controls for that property.
            </div>
          )}
        </section>

        <section className="panel">
          <h2>Account</h2>
          <div className="stats-grid single-column-stats">
            <div>
              <span>User</span>
              <strong>{session.username}</strong>
            </div>
            <div>
              <span>Daily quota</span>
              <strong>{usage?.daily_limit ?? 0}</strong>
            </div>
            <div>
              <span>Used today</span>
              <strong>{usage?.used_today ?? 0}</strong>
            </div>
          </div>
        </section>

        <section className="panel">
          <h2>Create API key</h2>
          <form onSubmit={handleCreateKey}>
            <input
              value={keyName}
              onChange={(event) => setKeyName(event.target.value)}
              placeholder="Key name"
            />
            <button type="submit" disabled={busy}>
              {busy ? "Creating..." : "Create key"}
            </button>
          </form>
          <p className="dev-hint">Raw keys are only shown once. Save them before leaving this page.</p>
          {latestKey ? (
            <div className="key-reveal">
              <pre>{latestKey.token}</pre>
              <div className="topbar-actions">
                <button type="button" onClick={() => handleUseSearchKey(latestKey.token)}>
                  Use for search
                </button>
              </div>
            </div>
          ) : null}
        </section>

        <section className="panel panel-wide">
          <h2>API keys</h2>
          <div className="list">
            {usage?.keys.length ? (
              usage.keys.map((key) => (
                <div className="list-row stacked" key={key.id}>
                  <div className="user-row-header">
                    <strong>{key.name}</strong>
                    <div className="topbar-actions">
                      {activePreview === key.preview ? <span className="status-pill">Active</span> : null}
                      {latestKey?.id === key.id ? (
                        <button type="button" onClick={() => handleUseSearchKey(latestKey.token)}>
                          Use for search
                        </button>
                      ) : null}
                      <button
                        type="button"
                        className="plain-link"
                        disabled={busy || Boolean(key.revoked_at)}
                        onClick={() => void handleRevokeKey(key.id, key.preview)}
                      >
                        {key.revoked_at ? "Revoked" : "Revoke"}
                      </button>
                    </div>
                  </div>
                  <div>{key.preview}</div>
                  <div>{key.revoked_at ? `revoked ${key.revoked_at}` : `created ${key.created_at}`}</div>
                </div>
              ))
            ) : (
              <div className="list-row">No API keys yet.</div>
            )}
          </div>
        </section>
      </main>
    </div>
  );
}

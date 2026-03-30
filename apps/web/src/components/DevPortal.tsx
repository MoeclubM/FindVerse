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
import { Alert, AlertDescription } from "./ui/alert";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "./ui/card";
import { Checkbox } from "./ui/checkbox";
import { Input } from "./ui/input";
import { Textarea } from "./ui/textarea";

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
  const [submitSameOriginConcurrency, setSubmitSameOriginConcurrency] = useState("1");
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
    setSubmitSameOriginConcurrency("1");
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
        same_origin_concurrency: Math.max(1, Number(submitSameOriginConcurrency) || 1),
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
    return <div className="grid min-h-screen place-items-center bg-background text-foreground">Checking developer session...</div>;
  }

  if (!session || !sessionToken) {
    return (
      <div className="min-h-screen bg-background text-foreground">
        <AppTopbar
          theme={props.theme}
          themeMode={props.themeMode}
          onThemeModeChange={props.onThemeModeChange}
          title={`${SITE_NAME} · Developer Portal`}
          onTitleClick={props.onNavigateSearch}
          beforeControls={
            props.devToken ? <TopbarBadge>Key</TopbarBadge> : null
          }
          afterControls={
            <TopbarActionButton
              leading={<MagnifyingGlassIcon className="size-4" />}
              onClick={props.onNavigateSearch}
            >
              Search
            </TopbarActionButton>
          }
        />
        <main className="mx-auto flex min-h-[calc(100vh-73px)] w-full max-w-md items-center px-4 py-10">
          <Card className="w-full rounded-3xl">
            <CardHeader className="gap-4 pb-4">
              <div className="flex flex-wrap gap-2">
                <Button type="button" variant={mode === "login" ? "default" : "outline"} onClick={() => setMode("login")}>
                  Sign in
                </Button>
                <Button type="button" variant={mode === "register" ? "default" : "outline"} onClick={() => setMode("register")}>
                  Register
                </Button>
              </div>
              <div className="space-y-1">
                <CardTitle>{mode === "register" ? "Create developer account" : "Developer sign in"}</CardTitle>
                <CardDescription>
                  Create an account, generate an <code>fvk_</code> key, inspect a domain property, and submit URLs for crawl without leaving the portal.
                </CardDescription>
              </div>
            </CardHeader>
            <CardContent className="space-y-4">
              <form className="grid gap-3" onSubmit={handleAuthSubmit}>
                <Input
                  value={username}
                  onChange={(event) => setUsername(event.target.value)}
                  placeholder="Username"
                  autoComplete="username"
                />
                <Input
                  type="password"
                  value={password}
                  onChange={(event) => setPassword(event.target.value)}
                  placeholder="Password"
                  autoComplete={mode === "register" ? "new-password" : "current-password"}
                />
                <Button type="submit" disabled={busy}>
                  {busy ? "Submitting..." : mode === "register" ? "Create account" : "Sign in"}
                </Button>
              </form>
              {flash ? (
                <Alert>
                  <AlertDescription>{flash}</AlertDescription>
                </Alert>
              ) : null}
            </CardContent>
          </Card>
        </main>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-background text-foreground">
      <AppTopbar
        theme={props.theme}
        themeMode={props.themeMode}
        onThemeModeChange={props.onThemeModeChange}
        title={`${SITE_NAME} · Developer Portal`}
        onTitleClick={props.onNavigateSearch}
        beforeControls={props.devToken ? <TopbarBadge>Key</TopbarBadge> : null}
        afterControls={
          <>
            <TopbarActionButton
              leading={<MagnifyingGlassIcon className="size-4" />}
              onClick={props.onNavigateSearch}
            >
              Search
            </TopbarActionButton>
            <TopbarActionButton
              leading={<ExitIcon className="size-4" />}
              disabled={busy}
              onClick={() => void handleSignOut()}
            >
              Sign out
            </TopbarActionButton>
          </>
        }
      />

      {flash ? (
        <div className="mx-auto mt-4 w-full max-w-7xl px-4 sm:px-6 lg:px-8">
          <Alert>
            <AlertDescription>{flash}</AlertDescription>
          </Alert>
        </div>
      ) : null}

      <main className="mx-auto grid w-full max-w-7xl gap-4 px-4 py-5 sm:px-6 lg:grid-cols-2 lg:px-8">
        <Card className="rounded-3xl lg:col-span-2">
          <CardHeader className="gap-4 pb-4">
            <SectionHeader
              title="Site Console"
              meta="Inspect a property, review crawl coverage, then submit URLs with the shared crawler."
              actions={propertyInsight ? <Badge variant="outline">{propertyInsight.domain}</Badge> : null}
            />
            <form className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_auto]" onSubmit={handleAnalyzeProperty}>
              <FieldShell label="Domain or URL">
                <Input
                  id="property-query"
                  value={propertyQuery}
                  onChange={(event) => setPropertyQuery(event.target.value)}
                  placeholder="example.com or https://example.com/docs"
                />
              </FieldShell>
              <Button className="lg:self-end" type="submit" disabled={propertyLoading || !propertyQuery.trim()}>
                {propertyLoading ? "Analyzing..." : "Analyze property"}
              </Button>
            </form>
          </CardHeader>
          <CardContent className="space-y-5">
            {propertyInsight ? (
              <>
                <StatStrip
                  className="xl:grid-cols-4"
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

                <div className="grid gap-4 xl:grid-cols-[minmax(0,1.7fr)_minmax(320px,0.9fr)]">
                  <div className="grid gap-4">
                    <Card className="rounded-2xl shadow-none">
                      <CardHeader className="pb-4">
                        <SectionHeader
                          heading="h3"
                          title="Recent indexed pages"
                          meta={`${propertyInsight.recent_documents.length} rows`}
                        />
                      </CardHeader>
                      <CardContent className="grid gap-3">
                        {propertyInsight.recent_documents.length ? (
                          propertyInsight.recent_documents.map((document) => (
                            <Card key={document.id} className="rounded-2xl bg-muted/30 shadow-none">
                              <CardContent className="grid gap-3 p-4">
                                <div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
                                  <a
                                    href={document.url}
                                    target="_blank"
                                    rel="noreferrer"
                                    className="text-sm font-semibold text-foreground underline-offset-4 hover:underline"
                                  >
                                    {document.title}
                                  </a>
                                  {document.duplicate_of ? <Badge variant="outline">Duplicate</Badge> : null}
                                </div>
                                <div className="flex flex-wrap gap-x-4 gap-y-2 text-sm text-muted-foreground">
                                  <span>{document.display_url}</span>
                                  <span>{document.language || "unknown"}</span>
                                  <span>{document.content_type}</span>
                                  <span>{document.word_count} words</span>
                                  <span>{formatPortalTimestamp(document.last_crawled_at)}</span>
                                </div>
                              </CardContent>
                            </Card>
                          ))
                        ) : (
                          <div className="rounded-2xl border border-dashed border-border bg-muted/40 px-4 py-8 text-center text-sm text-muted-foreground">
                            No indexed pages for this property yet.
                          </div>
                        )}
                      </CardContent>
                    </Card>

                    <Card className="rounded-2xl shadow-none">
                      <CardHeader className="pb-4">
                        <SectionHeader
                          heading="h3"
                          title="Coverage facets"
                          meta="Top distributions from indexed docs"
                        />
                      </CardHeader>
                      <CardContent className="grid gap-4 md:grid-cols-2">
                        <Card className="rounded-2xl bg-muted/30 shadow-none">
                          <CardHeader className="pb-4">
                            <CardTitle className="text-base">Languages</CardTitle>
                          </CardHeader>
                          <CardContent className="grid gap-3">
                            {propertyInsight.top_languages.length ? (
                              propertyInsight.top_languages.map((facet) => (
                                <div key={`lang-${facet.label}`} className="flex items-center justify-between gap-3 rounded-xl border border-border bg-card px-4 py-3">
                                  <span className="text-sm text-foreground">{facet.label}</span>
                                  <strong className="text-sm font-semibold text-foreground">{facet.count}</strong>
                                </div>
                              ))
                            ) : (
                              <div className="rounded-xl border border-dashed border-border bg-card px-4 py-6 text-center text-sm text-muted-foreground">
                                No language data yet.
                              </div>
                            )}
                          </CardContent>
                        </Card>
                        <Card className="rounded-2xl bg-muted/30 shadow-none">
                          <CardHeader className="pb-4">
                            <CardTitle className="text-base">Content types</CardTitle>
                          </CardHeader>
                          <CardContent className="grid gap-3">
                            {propertyInsight.top_content_types.length ? (
                              propertyInsight.top_content_types.map((facet) => (
                                <div key={`type-${facet.label}`} className="flex items-center justify-between gap-3 rounded-xl border border-border bg-card px-4 py-3">
                                  <span className="text-sm text-foreground">{facet.label}</span>
                                  <strong className="text-sm font-semibold text-foreground">{facet.count}</strong>
                                </div>
                              ))
                            ) : (
                              <div className="rounded-xl border border-dashed border-border bg-card px-4 py-6 text-center text-sm text-muted-foreground">
                                No content type data yet.
                              </div>
                            )}
                          </CardContent>
                        </Card>
                      </CardContent>
                    </Card>

                    <Card className="rounded-2xl shadow-none">
                      <CardHeader className="pb-4">
                        <SectionHeader
                          heading="h3"
                          title="Recent crawl activity"
                          meta={`${propertyInsight.recent_jobs.length} rows`}
                        />
                      </CardHeader>
                      <CardContent className="grid gap-3">
                        {propertyInsight.recent_jobs.length ? (
                          propertyInsight.recent_jobs.map((job) => (
                            <Card key={job.id} className="rounded-2xl bg-muted/30 shadow-none">
                              <CardContent className="grid gap-3 p-4">
                                <div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
                                  <a
                                    href={job.url}
                                    target="_blank"
                                    rel="noreferrer"
                                    className="break-all text-sm font-semibold text-foreground underline-offset-4 hover:underline"
                                  >
                                    {job.url}
                                  </a>
                                  <Badge
                                    variant={
                                      job.status === "succeeded" && job.accepted_document_id
                                        ? "success"
                                        : "outline"
                                    }
                                  >
                                    {job.status}
                                  </Badge>
                                </div>
                                <div className="flex flex-wrap gap-x-4 gap-y-2 text-sm text-muted-foreground">
                                  <span>depth {job.depth}</span>
                                  <span>{job.http_status ? `HTTP ${job.http_status}` : "no status"}</span>
                                  <span>{formatPortalTimestamp(job.finished_at ?? job.discovered_at)}</span>
                                  {job.failure_kind ? <span>{job.failure_kind}</span> : null}
                                  {job.failure_message ? <span>{job.failure_message}</span> : null}
                                </div>
                              </CardContent>
                            </Card>
                          ))
                        ) : (
                          <div className="rounded-2xl border border-dashed border-border bg-muted/40 px-4 py-8 text-center text-sm text-muted-foreground">
                            No crawl history for this property yet.
                          </div>
                        )}
                      </CardContent>
                    </Card>
                  </div>

                  <Card className="rounded-2xl shadow-none">
                    <CardHeader className="pb-4">
                      <SectionHeader
                        heading="h3"
                        title="Submit URLs"
                        meta="Queue homepage, sitemap, or a focused URL set for this property."
                      />
                    </CardHeader>
                    <CardContent className="space-y-4">
                      <form className="grid gap-4" onSubmit={handleSubmitProperty}>
                        <FieldShell label="Property">
                          <Input
                            id="submit-domain"
                            value={submitDomain}
                            onChange={(event) => setSubmitDomain(event.target.value)}
                            placeholder="example.com"
                          />
                        </FieldShell>
                        <div className="flex flex-wrap gap-2">
                          <Button
                            type="button"
                            variant="outline"
                            onClick={() => {
                              const domain = submitDomain || propertyInsight.domain;
                              setSubmitDomain(domain);
                              setSubmitUrls(`https://${domain}/`);
                            }}
                          >
                            Homepage only
                          </Button>
                          <Button
                            type="button"
                            variant="outline"
                            onClick={() => {
                              const domain = submitDomain || propertyInsight.domain;
                              setSubmitDomain(domain);
                              setSubmitUrls(buildSeedSuggestions(domain));
                            }}
                          >
                            Homepage + sitemap
                          </Button>
                        </div>
                        <FieldShell label="URL list">
                          <Textarea
                            id="submit-urls"
                            rows={8}
                            value={submitUrls}
                            onChange={(event) => setSubmitUrls(event.target.value)}
                            placeholder="One URL per line"
                          />
                        </FieldShell>
                        <div className="grid gap-4 sm:grid-cols-2">
                          <FieldShell label="Crawl depth">
                            <Input
                              id="submit-depth"
                              type="number"
                              min={0}
                              max={10}
                              value={submitDepth}
                              onChange={(event) => setSubmitDepth(event.target.value)}
                            />
                          </FieldShell>
                          <FieldShell label="Page budget">
                            <Input
                              id="submit-pages"
                              type="number"
                              min={1}
                              max={10000}
                              value={submitMaxPages}
                              onChange={(event) => setSubmitMaxPages(event.target.value)}
                            />
                          </FieldShell>
                          <FieldShell label="Same-origin concurrency">
                            <Input
                              id="submit-origin-concurrency"
                              type="number"
                              min={1}
                              max={32}
                              value={submitSameOriginConcurrency}
                              onChange={(event) => setSubmitSameOriginConcurrency(event.target.value)}
                            />
                          </FieldShell>
                        </div>
                        <label className="flex items-center gap-3 rounded-xl border border-border bg-muted/40 px-4 py-3 text-sm text-foreground">
                          <Checkbox
                            checked={submitRevisit}
                            onCheckedChange={(checked) => setSubmitRevisit(checked === true)}
                          />
                          Allow revisit for URLs already seen before
                        </label>
                        <Button type="submit" disabled={propertySubmitting || !submitUrls.trim()}>
                          {propertySubmitting ? "Submitting..." : "Queue property crawl"}
                        </Button>
                      </form>
                      <p className="text-sm text-muted-foreground">
                        This portal uses the shared crawler owner. It is designed as a lightweight
                        search-console workflow, not a fully isolated multi-tenant crawl pipeline.
                      </p>
                    </CardContent>
                  </Card>
                </div>
              </>
            ) : (
              <div className="rounded-2xl border border-dashed border-border bg-muted/40 px-4 py-8 text-center text-sm text-muted-foreground">
                Enter a domain or URL above to inspect indexed pages, crawl coverage, and
                submission controls for that property.
              </div>
            )}
          </CardContent>
        </Card>

        <Card className="rounded-3xl">
          <CardHeader className="pb-4">
            <CardTitle>Account</CardTitle>
            <CardDescription>Current developer session and daily usage.</CardDescription>
          </CardHeader>
          <CardContent>
            <StatStrip
              className="grid-cols-1"
              items={[
                { label: "User", value: session.username },
                { label: "Daily quota", value: usage?.daily_limit ?? 0 },
                { label: "Used today", value: usage?.used_today ?? 0 },
              ]}
            />
          </CardContent>
        </Card>

        <Card className="rounded-3xl">
          <CardHeader className="pb-4">
            <CardTitle>Create API key</CardTitle>
            <CardDescription>Raw keys are only shown once. Save them before leaving this page.</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <form className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto]" onSubmit={handleCreateKey}>
              <Input
                value={keyName}
                onChange={(event) => setKeyName(event.target.value)}
                placeholder="Key name"
              />
              <Button type="submit" disabled={busy}>
                {busy ? "Creating..." : "Create key"}
              </Button>
            </form>
            {latestKey ? (
              <div className="space-y-3 rounded-2xl border border-border bg-muted/40 p-4">
                <pre>{latestKey.token}</pre>
                <Button type="button" variant="outline" onClick={() => handleUseSearchKey(latestKey.token)}>
                  Use for search
                </Button>
              </div>
            ) : null}
          </CardContent>
        </Card>

        <Card className="rounded-3xl lg:col-span-2">
          <CardHeader className="pb-4">
            <CardTitle>API keys</CardTitle>
            <CardDescription>Manage search keys for this developer account.</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="grid gap-3">
              {usage?.keys.length ? (
                usage.keys.map((key) => (
                  <Card key={key.id} className="rounded-2xl bg-muted/30 shadow-none">
                    <CardContent className="grid gap-3 p-4">
                      <div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
                        <div className="grid gap-1">
                          <strong className="text-sm font-semibold text-foreground">{key.name}</strong>
                          <span className="text-sm text-muted-foreground">{key.preview}</span>
                        </div>
                        <div className="flex flex-wrap items-center gap-2">
                          {activePreview === key.preview ? <Badge>Active</Badge> : null}
                          {latestKey?.id === key.id ? (
                            <Button type="button" variant="outline" size="sm" onClick={() => handleUseSearchKey(latestKey.token)}>
                              Use for search
                            </Button>
                          ) : null}
                          <Button
                            type="button"
                            variant={key.revoked_at ? "outline" : "ghost"}
                            size="sm"
                            disabled={busy || Boolean(key.revoked_at)}
                            onClick={() => void handleRevokeKey(key.id, key.preview)}
                          >
                            {key.revoked_at ? "Revoked" : "Revoke"}
                          </Button>
                        </div>
                      </div>
                      <div className="text-sm text-muted-foreground">
                        {key.revoked_at ? `revoked ${key.revoked_at}` : `created ${key.created_at}`}
                      </div>
                    </CardContent>
                  </Card>
                ))
              ) : (
                <div className="rounded-2xl border border-dashed border-border bg-muted/40 px-4 py-8 text-center text-sm text-muted-foreground">
                  No API keys yet.
                </div>
              )}
            </div>
          </CardContent>
        </Card>
      </main>
    </div>
  );
}

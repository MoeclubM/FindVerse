import { ExitIcon, MagnifyingGlassIcon } from "@radix-ui/react-icons";
import { FormEvent, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

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

function formatPortalTimestamp(value: string | null, emptyLabel: string) {
  if (!value) {
    return emptyLabel;
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
  const { t } = useTranslation();
  const [sessionToken, setSessionToken] = useState<string | null>(() => localStorage.getItem(DEV_SESSION_KEY));
  const [session, setSession] = useState<DevSession | null>(null);
  const [usage, setUsage] = useState<DeveloperUsage | null>(null);
  const [mode, setMode] = useState<"login" | "register">("login");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [keyName, setKeyName] = useState(() => t("dev_portal.create_api_key.default_name"));
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
  const defaultKeyName = t("dev_portal.create_api_key.default_name");
  const portalTitle = `${SITE_NAME} · ${t("dev_portal.title")}`;

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
      setFlash(
        getErrorMessage(
          error,
          mode === "register"
            ? t("dev_portal.flash.register_failed")
            : t("dev_portal.flash.login_failed"),
        ),
      );
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
      setKeyName(defaultKeyName);
      await refreshUsage(sessionToken);
    } catch (error) {
      setFlash(getErrorMessage(error, t("dev_portal.flash.api_key_creation_failed")));
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
      setFlash(getErrorMessage(error, t("dev_portal.flash.api_key_revoke_failed")));
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
      setFlash(getErrorMessage(error, t("dev_portal.flash.property_analysis_failed")));
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
      setFlash(t("dev_portal.flash.missing_submission_fields"));
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
        t("dev_portal.flash.queue_summary", {
          accepted: response.accepted_urls,
          queued: response.queued_domain_jobs,
          known: response.known_domain_urls,
        }),
      );
    } catch (error) {
      setFlash(getErrorMessage(error, t("dev_portal.flash.property_submission_failed")));
    } finally {
      setPropertySubmitting(false);
    }
  }

  function handleUseSearchKey(token: string) {
    props.onTokenChange(token);
    setFlash(t("dev_portal.flash.active_search_key_updated"));
  }

  if (loadingSession) {
    return <div className="grid min-h-screen place-items-center bg-background text-foreground">{t("dev_portal.checking")}</div>;
  }

  if (!session || !sessionToken) {
    return (
      <div className="min-h-screen bg-background text-foreground">
        <AppTopbar
          theme={props.theme}
          themeMode={props.themeMode}
          onThemeModeChange={props.onThemeModeChange}
          title={portalTitle}
          onTitleClick={props.onNavigateSearch}
          beforeControls={
            props.devToken ? <TopbarBadge>{t("dev_portal.key_badge")}</TopbarBadge> : null
          }
          afterControls={
            <TopbarActionButton
              leading={<MagnifyingGlassIcon className="size-4" />}
              onClick={props.onNavigateSearch}
            >
              {t("dev_portal.search")}
            </TopbarActionButton>
          }
        />
        <main className="mx-auto flex min-h-[calc(100vh-73px)] w-full max-w-md items-center px-4 py-10">
          <Card className="w-full rounded-3xl">
            <CardHeader className="gap-4 pb-4">
              <div className="flex flex-wrap gap-2">
                <Button type="button" variant={mode === "login" ? "default" : "outline"} onClick={() => setMode("login")}>
                  {t("dev_portal.auth.sign_in")}
                </Button>
                <Button type="button" variant={mode === "register" ? "default" : "outline"} onClick={() => setMode("register")}>
                  {t("dev_portal.auth.register")}
                </Button>
              </div>
              <div className="space-y-1">
                <CardTitle>
                  {mode === "register"
                    ? t("dev_portal.auth.create_account_title")
                    : t("dev_portal.auth.sign_in_title")}
                </CardTitle>
                <CardDescription>
                  {t("dev_portal.auth.description")} <code>fvk_</code> {t("dev_portal.auth.description_suffix")}
                </CardDescription>
              </div>
            </CardHeader>
            <CardContent className="space-y-4">
              <form className="grid gap-3" onSubmit={handleAuthSubmit}>
                <Input
                  value={username}
                  onChange={(event) => setUsername(event.target.value)}
                  placeholder={t("dev_portal.auth.username")}
                  autoComplete="username"
                />
                <Input
                  type="password"
                  value={password}
                  onChange={(event) => setPassword(event.target.value)}
                  placeholder={t("dev_portal.auth.password")}
                  autoComplete={mode === "register" ? "new-password" : "current-password"}
                />
                <Button type="submit" disabled={busy}>
                  {busy
                    ? t("dev_portal.auth.submitting")
                    : mode === "register"
                      ? t("dev_portal.auth.create_account")
                      : t("dev_portal.auth.sign_in")}
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
        title={portalTitle}
        onTitleClick={props.onNavigateSearch}
        beforeControls={props.devToken ? <TopbarBadge>{t("dev_portal.key_badge")}</TopbarBadge> : null}
        afterControls={
          <>
            <TopbarActionButton
              leading={<MagnifyingGlassIcon className="size-4" />}
              onClick={props.onNavigateSearch}
            >
              {t("dev_portal.search")}
            </TopbarActionButton>
            <TopbarActionButton
              leading={<ExitIcon className="size-4" />}
              disabled={busy}
              onClick={() => void handleSignOut()}
            >
              {t("dev_portal.sign_out")}
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
              title={t("dev_portal.site_console.title")}
              meta={t("dev_portal.site_console.meta")}
              actions={propertyInsight ? <Badge variant="outline">{propertyInsight.domain}</Badge> : null}
            />
            <form className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_auto]" onSubmit={handleAnalyzeProperty}>
              <FieldShell label={t("dev_portal.site_console.domain_or_url")}>
                <Input
                  id="property-query"
                  value={propertyQuery}
                  onChange={(event) => setPropertyQuery(event.target.value)}
                  placeholder={t("dev_portal.site_console.domain_placeholder")}
                />
              </FieldShell>
              <Button className="lg:self-end" type="submit" disabled={propertyLoading || !propertyQuery.trim()}>
                {propertyLoading ? t("dev_portal.site_console.analyzing") : t("dev_portal.site_console.analyze")}
              </Button>
            </form>
          </CardHeader>
          <CardContent className="space-y-5">
            {propertyInsight ? (
              <>
                <StatStrip
                  className="xl:grid-cols-4"
                  items={[
                    { label: t("dev_portal.site_console.stats.indexed_docs"), value: propertyInsight.indexed_documents },
                    { label: t("dev_portal.site_console.stats.duplicates"), value: propertyInsight.duplicate_documents },
                    { label: t("dev_portal.site_console.stats.pending_crawl"), value: propertyInsight.pending_jobs },
                    { label: t("dev_portal.site_console.stats.indexed_jobs"), value: propertyInsight.successful_jobs },
                    { label: t("dev_portal.site_console.stats.filtered_jobs"), value: propertyInsight.filtered_jobs },
                    { label: t("dev_portal.site_console.stats.failures"), value: propertyInsight.failed_jobs + propertyInsight.blocked_jobs },
                    { label: t("dev_portal.site_console.stats.last_indexed"), value: formatPortalTimestamp(propertyInsight.last_indexed_at, t("dev_portal.common.not_yet")) },
                    { label: t("dev_portal.site_console.stats.last_crawl_activity"), value: formatPortalTimestamp(propertyInsight.last_crawled_at, t("dev_portal.common.not_yet")) },
                  ]}
                />

                <div className="grid gap-4 xl:grid-cols-[minmax(0,1.7fr)_minmax(320px,0.9fr)]">
                  <div className="grid gap-4">
                    <Card className="rounded-2xl shadow-none">
                      <CardHeader className="pb-4">
                        <SectionHeader
                          heading="h3"
                          title={t("dev_portal.site_console.recent_indexed_pages.title")}
                          meta={t("dev_portal.common.rows", { count: propertyInsight.recent_documents.length })}
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
                                  {document.duplicate_of ? <Badge variant="outline">{t("dev_portal.common.duplicate")}</Badge> : null}
                                </div>
                                <div className="flex flex-wrap gap-x-4 gap-y-2 text-sm text-muted-foreground">
                                  <span>{document.display_url}</span>
                                  <span>{document.language || t("dev_portal.common.unknown")}</span>
                                  <span>{document.content_type}</span>
                                  <span>{t("dev_portal.common.words", { count: document.word_count })}</span>
                                  <span>{formatPortalTimestamp(document.last_crawled_at, t("dev_portal.common.not_yet"))}</span>
                                </div>
                              </CardContent>
                            </Card>
                          ))
                        ) : (
                          <div className="rounded-2xl border border-dashed border-border bg-muted/40 px-4 py-8 text-center text-sm text-muted-foreground">
                            {t("dev_portal.site_console.recent_indexed_pages.empty")}
                          </div>
                        )}
                      </CardContent>
                    </Card>

                    <Card className="rounded-2xl shadow-none">
                      <CardHeader className="pb-4">
                        <SectionHeader
                          heading="h3"
                          title={t("dev_portal.site_console.coverage_facets.title")}
                          meta={t("dev_portal.site_console.coverage_facets.meta")}
                        />
                      </CardHeader>
                      <CardContent className="grid gap-4 md:grid-cols-2">
                        <Card className="rounded-2xl bg-muted/30 shadow-none">
                          <CardHeader className="pb-4">
                            <CardTitle className="text-base">{t("dev_portal.site_console.coverage_facets.languages")}</CardTitle>
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
                                {t("dev_portal.site_console.coverage_facets.no_language_data")}
                              </div>
                            )}
                          </CardContent>
                        </Card>
                        <Card className="rounded-2xl bg-muted/30 shadow-none">
                          <CardHeader className="pb-4">
                            <CardTitle className="text-base">{t("dev_portal.site_console.coverage_facets.content_types")}</CardTitle>
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
                                {t("dev_portal.site_console.coverage_facets.no_content_type_data")}
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
                          title={t("dev_portal.site_console.recent_crawl_activity.title")}
                          meta={t("dev_portal.common.rows", { count: propertyInsight.recent_jobs.length })}
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
                                  <span>{t("dev_portal.site_console.recent_crawl_activity.depth", { depth: job.depth })}</span>
                                  <span>{job.http_status ? `HTTP ${job.http_status}` : t("dev_portal.site_console.recent_crawl_activity.no_status")}</span>
                                  <span>{formatPortalTimestamp(job.finished_at ?? job.discovered_at, t("dev_portal.common.not_yet"))}</span>
                                  {job.failure_kind ? <span>{job.failure_kind}</span> : null}
                                  {job.failure_message ? <span>{job.failure_message}</span> : null}
                                </div>
                              </CardContent>
                            </Card>
                          ))
                        ) : (
                          <div className="rounded-2xl border border-dashed border-border bg-muted/40 px-4 py-8 text-center text-sm text-muted-foreground">
                            {t("dev_portal.site_console.recent_crawl_activity.empty")}
                          </div>
                        )}
                      </CardContent>
                    </Card>
                  </div>

                  <Card className="rounded-2xl shadow-none">
                    <CardHeader className="pb-4">
                      <SectionHeader
                        heading="h3"
                        title={t("dev_portal.site_console.submit_urls.title")}
                        meta={t("dev_portal.site_console.submit_urls.meta")}
                      />
                    </CardHeader>
                    <CardContent className="space-y-4">
                      <form className="grid gap-4" onSubmit={handleSubmitProperty}>
                        <FieldShell label={t("dev_portal.site_console.submit_urls.property")}>
                          <Input
                            id="submit-domain"
                            value={submitDomain}
                            onChange={(event) => setSubmitDomain(event.target.value)}
                            placeholder={t("dev_portal.site_console.submit_urls.property_placeholder")}
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
                            {t("dev_portal.site_console.submit_urls.homepage_only")}
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
                            {t("dev_portal.site_console.submit_urls.homepage_and_sitemap")}
                          </Button>
                        </div>
                        <FieldShell label={t("dev_portal.site_console.submit_urls.url_list")}>
                          <Textarea
                            id="submit-urls"
                            rows={8}
                            value={submitUrls}
                            onChange={(event) => setSubmitUrls(event.target.value)}
                            placeholder={t("dev_portal.site_console.submit_urls.url_list_placeholder")}
                          />
                        </FieldShell>
                        <div className="grid gap-4 sm:grid-cols-2">
                          <FieldShell label={t("dev_portal.site_console.submit_urls.crawl_depth")}>
                            <Input
                              id="submit-depth"
                              type="number"
                              min={0}
                              max={10}
                              value={submitDepth}
                              onChange={(event) => setSubmitDepth(event.target.value)}
                            />
                          </FieldShell>
                          <FieldShell label={t("dev_portal.site_console.submit_urls.page_budget")}>
                            <Input
                              id="submit-pages"
                              type="number"
                              min={1}
                              max={10000}
                              value={submitMaxPages}
                              onChange={(event) => setSubmitMaxPages(event.target.value)}
                            />
                          </FieldShell>
                          <FieldShell label={t("dev_portal.site_console.submit_urls.same_origin_concurrency")}>
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
                          {t("dev_portal.site_console.submit_urls.allow_revisit")}
                        </label>
                        <Button type="submit" disabled={propertySubmitting || !submitUrls.trim()}>
                          {propertySubmitting ? t("dev_portal.site_console.submit_urls.submitting") : t("dev_portal.site_console.submit_urls.submit")}
                        </Button>
                      </form>
                      <p className="text-sm text-muted-foreground">
                        {t("dev_portal.site_console.submit_urls.hint")}
                      </p>
                    </CardContent>
                  </Card>
                </div>
              </>
            ) : (
              <div className="rounded-2xl border border-dashed border-border bg-muted/40 px-4 py-8 text-center text-sm text-muted-foreground">
                {t("dev_portal.site_console.empty")}
              </div>
            )}
          </CardContent>
        </Card>

        <Card className="rounded-3xl">
          <CardHeader className="pb-4">
            <CardTitle>{t("dev_portal.account.title")}</CardTitle>
            <CardDescription>{t("dev_portal.account.description")}</CardDescription>
          </CardHeader>
          <CardContent>
            <StatStrip
              className="grid-cols-1"
              items={[
                { label: t("dev_portal.account.user"), value: session.username },
                { label: t("dev_portal.account.daily_quota"), value: usage?.daily_limit ?? 0 },
                { label: t("dev_portal.account.used_today"), value: usage?.used_today ?? 0 },
              ]}
            />
          </CardContent>
        </Card>

        <Card className="rounded-3xl">
          <CardHeader className="pb-4">
            <CardTitle>{t("dev_portal.create_api_key.title")}</CardTitle>
            <CardDescription>{t("dev_portal.create_api_key.description")}</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <form className="grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto]" onSubmit={handleCreateKey}>
              <Input
                value={keyName}
                onChange={(event) => setKeyName(event.target.value)}
                placeholder={t("dev_portal.create_api_key.placeholder")}
              />
              <Button type="submit" disabled={busy}>
                {busy ? t("dev_portal.create_api_key.creating") : t("dev_portal.create_api_key.submit")}
              </Button>
            </form>
            {latestKey ? (
              <div className="space-y-3 rounded-2xl border border-border bg-muted/40 p-4">
                <pre>{latestKey.token}</pre>
                <Button type="button" variant="outline" onClick={() => handleUseSearchKey(latestKey.token)}>
                  {t("dev_portal.create_api_key.use_for_search")}
                </Button>
              </div>
            ) : null}
          </CardContent>
        </Card>

        <Card className="rounded-3xl lg:col-span-2">
          <CardHeader className="pb-4">
            <CardTitle>{t("dev_portal.api_keys.title")}</CardTitle>
            <CardDescription>{t("dev_portal.api_keys.description")}</CardDescription>
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
                          {activePreview === key.preview ? <Badge>{t("dev_portal.api_keys.active")}</Badge> : null}
                          {latestKey?.id === key.id ? (
                            <Button type="button" variant="outline" size="sm" onClick={() => handleUseSearchKey(latestKey.token)}>
                              {t("dev_portal.create_api_key.use_for_search")}
                            </Button>
                          ) : null}
                          <Button
                            type="button"
                            variant={key.revoked_at ? "outline" : "ghost"}
                            size="sm"
                            disabled={busy || Boolean(key.revoked_at)}
                            onClick={() => void handleRevokeKey(key.id, key.preview)}
                          >
                            {key.revoked_at ? t("dev_portal.api_keys.revoked") : t("dev_portal.api_keys.revoke")}
                          </Button>
                        </div>
                      </div>
                      <div className="text-sm text-muted-foreground">
                        {key.revoked_at
                          ? t("dev_portal.api_keys.revoked_at", { time: key.revoked_at })
                          : t("dev_portal.api_keys.created_at", { time: key.created_at })}
                      </div>
                    </CardContent>
                  </Card>
                ))
              ) : (
                <div className="rounded-2xl border border-dashed border-border bg-muted/40 px-4 py-8 text-center text-sm text-muted-foreground">
                  {t("dev_portal.api_keys.empty")}
                </div>
              )}
            </div>
          </CardContent>
        </Card>
      </main>
    </div>
  );
}

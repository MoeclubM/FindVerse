import { FormEvent, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  getAdminDomainInsight,
  getSystemConfig,
  seedFrontier,
  setSystemConfig,
  type DeveloperDomainInsight,
  type DiscoveryScope,
} from "../../api";
import { FieldShell, PanelSection, StatStrip } from "../common/PanelPrimitives";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "../ui/card";
import { Checkbox } from "../ui/checkbox";
import { Input } from "../ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../ui/select";
import { Textarea } from "../ui/textarea";
import { useConsole } from "./ConsoleContext";

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

function formatTimestamp(value: string | null, emptyLabel: string) {
  if (!value) {
    return emptyLabel;
  }

  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? value : parsed.toLocaleString();
}

function buildSeedSuggestions(domain: string) {
  return `https://${domain}/\nhttps://${domain}/sitemap.xml`;
}

function normalizeDomainInput(value: string) {
  const trimmed = value.trim();
  if (!trimmed) {
    return "";
  }

  try {
    return new URL(trimmed).hostname.toLowerCase();
  } catch {
    try {
      return new URL(`https://${trimmed}`).hostname.toLowerCase();
    } catch {
      return trimmed.toLowerCase();
    }
  }
}

function parseDomainList(value: string) {
  return Array.from(
    new Set(
      value
        .split(/[\s,;]+/)
        .map((item) => normalizeDomainInput(item))
        .filter(Boolean),
    ),
  ).sort();
}

export function ConsoleDomains() {
  const { token, busy, setBusy, setFlash, refreshAll, refreshDocumentList } = useConsole();
  const { t } = useTranslation();
  const [domainQuery, setDomainQuery] = useState("");
  const [insight, setInsight] = useState<DeveloperDomainInsight | null>(null);
  const [loading, setLoading] = useState(false);
  const [submitDomain, setSubmitDomain] = useState("");
  const [submitUrls, setSubmitUrls] = useState("");
  const [submitDepth, setSubmitDepth] = useState("2");
  const [submitMaxPages, setSubmitMaxPages] = useState("50");
  const [submitSameOriginConcurrency, setSubmitSameOriginConcurrency] = useState("1");
  const [submitScope, setSubmitScope] = useState<DiscoveryScope>("same_domain");
  const [submitMaxDiscovered, setSubmitMaxDiscovered] = useState("50");
  const [submitRevisit, setSubmitRevisit] = useState(true);
  const [savedBlacklist, setSavedBlacklist] = useState("");
  const [blacklistDraft, setBlacklistDraft] = useState("");
  const [savingBlacklist, setSavingBlacklist] = useState(false);

  useEffect(() => {
    if (!insight || submitDomain === insight.domain) {
      return;
    }

    setSubmitDomain(insight.domain);
    setSubmitUrls(buildSeedSuggestions(insight.domain));
    setSubmitDepth("2");
    setSubmitMaxPages("50");
    setSubmitSameOriginConcurrency("1");
    setSubmitScope("same_domain");
    setSubmitMaxDiscovered("50");
    setSubmitRevisit(true);
  }, [insight, submitDomain]);

  useEffect(() => {
    let cancelled = false;

    getSystemConfig(token)
      .then((response) => {
        if (cancelled) {
          return;
        }
        const blacklist =
          response.entries.find((entry) => entry.key === "crawler.domain_blacklist")?.value ?? "";
        setSavedBlacklist(blacklist);
        setBlacklistDraft(blacklist);
      })
      .catch((error) => {
        if (!cancelled) {
          setFlash(getErrorMessage(error, t("console.domains.blacklist_load_failed")));
        }
      });

    return () => {
      cancelled = true;
    };
  }, [token, setFlash, t]);

  async function loadInsight(domain: string) {
    const nextInsight = await getAdminDomainInsight(token, domain);
    setInsight(nextInsight);
    return nextInsight;
  }

  async function handleAnalyze(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!domainQuery.trim()) {
      return;
    }

    setLoading(true);
    setFlash(null);
    try {
      await loadInsight(domainQuery.trim());
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.domains.load_failed")));
    } finally {
      setLoading(false);
    }
  }

  async function handleSeed(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const urls = submitUrls
      .split(/\r?\n/)
      .map((value) => value.trim())
      .filter(Boolean);
    const domain = submitDomain.trim() || insight?.domain || domainQuery.trim();

    if (!domain || !urls.length) {
      setFlash(t("console.domains.seed_missing"));
      return;
    }

    setBusy(true);
    setFlash(null);
    try {
      const response = await seedFrontier(
        token,
        urls,
        Math.max(0, Number(submitDepth) || 0),
        Math.max(1, Number(submitMaxPages) || 1),
        Math.max(1, Number(submitSameOriginConcurrency) || 1),
        submitScope,
        Math.max(1, Number(submitMaxDiscovered) || 1),
        submitRevisit,
      );
      await Promise.all([refreshAll(), refreshDocumentList(), loadInsight(domain)]);
      setFlash(
        t("console.domains.seed_success", {
          accepted: response.accepted_urls,
          queued: response.frontier_depth,
          known: response.known_urls,
        }),
      );
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.domains.seed_failed")));
    } finally {
      setBusy(false);
    }
  }

  async function handleSaveBlacklist() {
    const normalized = parseDomainList(blacklistDraft).join("\n");
    setSavingBlacklist(true);
    setFlash(null);
    try {
      await setSystemConfig(token, "crawler.domain_blacklist", normalized || null);
      setSavedBlacklist(normalized);
      setBlacklistDraft(normalized);
      await Promise.all([
        refreshAll(),
        refreshDocumentList(),
        insight ? loadInsight(insight.domain) : Promise.resolve(null),
      ]);
      setFlash(t("console.domains.blacklist_save_success"));
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.domains.blacklist_save_failed")));
    } finally {
      setSavingBlacklist(false);
    }
  }

  const blacklistDomains = parseDomainList(blacklistDraft);
  const savedBlacklistDomains = parseDomainList(savedBlacklist);
  const currentDomain = normalizeDomainInput(insight?.domain || submitDomain || domainQuery);
  const currentDomainBlacklisted = currentDomain
    ? blacklistDomains.some(
        (domain) => currentDomain === domain || currentDomain.endsWith(`.${domain}`),
      )
    : false;
  const blacklistDirty = blacklistDomains.join("\n") !== savedBlacklistDomains.join("\n");

  return (
    <div className="space-y-4">
      <PanelSection
        title={t("console.domains.title")}
        meta={t("console.domains.meta")}
        contentClassName="space-y-5"
      >
        <form className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_auto]" onSubmit={handleAnalyze}>
          <FieldShell label={t("console.domains.query_label")}>
            <Input
              value={domainQuery}
              onChange={(event) => setDomainQuery(event.target.value)}
              placeholder={t("console.domains.query_placeholder")}
            />
          </FieldShell>
          <Button className="lg:self-end" type="submit" disabled={loading || !domainQuery.trim()}>
            {loading ? t("console.domains.loading") : t("console.domains.inspect")}
          </Button>
        </form>

        {insight ? (
          <>
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant="outline">{insight.domain}</Badge>
              <Badge variant="outline">{insight.property_url}</Badge>
              {currentDomainBlacklisted ? (
                <Badge variant="destructive">{t("console.domains.blacklisted_badge")}</Badge>
              ) : null}
            </div>

            <StatStrip
              className="xl:grid-cols-4"
              items={[
                { label: t("dev_portal.site_console.stats.indexed_docs"), value: insight.indexed_documents },
                { label: t("dev_portal.site_console.stats.duplicates"), value: insight.duplicate_documents },
                { label: t("dev_portal.site_console.stats.pending_crawl"), value: insight.pending_jobs },
                { label: t("dev_portal.site_console.stats.indexed_jobs"), value: insight.successful_jobs },
                { label: t("dev_portal.site_console.stats.filtered_jobs"), value: insight.filtered_jobs },
                { label: t("dev_portal.site_console.stats.failures"), value: insight.failed_jobs + insight.blocked_jobs },
                {
                  label: t("dev_portal.site_console.stats.last_indexed"),
                  value: formatTimestamp(insight.last_indexed_at, t("dev_portal.common.not_yet")),
                },
                {
                  label: t("dev_portal.site_console.stats.last_crawl_activity"),
                  value: formatTimestamp(insight.last_crawled_at, t("dev_portal.common.not_yet")),
                },
              ]}
            />

            <div className="grid gap-4 xl:grid-cols-[minmax(0,1.6fr)_minmax(320px,1fr)]">
              <div className="grid gap-4">
                <Card className="rounded-2xl shadow-none">
                  <CardHeader className="pb-4">
                    <CardTitle className="text-base">{t("dev_portal.site_console.recent_indexed_pages.title")}</CardTitle>
                  </CardHeader>
                  <CardContent className="grid gap-3">
                    {insight.recent_documents.length ? (
                      insight.recent_documents.map((document) => (
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
                              <span>{formatTimestamp(document.last_crawled_at, t("dev_portal.common.not_yet"))}</span>
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
                    <CardTitle className="text-base">{t("dev_portal.site_console.recent_crawl_activity.title")}</CardTitle>
                  </CardHeader>
                  <CardContent className="grid gap-3">
                    {insight.recent_jobs.length ? (
                      insight.recent_jobs.map((job) => (
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
                              <div className="flex flex-wrap items-center gap-2">
                                <Badge
                                  variant={
                                    job.status === "succeeded" && job.accepted_document_id
                                      ? "success"
                                      : "outline"
                                  }
                                >
                                  {job.status}
                                </Badge>
                                <Badge variant={job.render_mode === "browser" ? "warning" : "outline"}>
                                  {job.render_mode === "browser"
                                    ? t("console.jobs.browser_rendered")
                                    : t("console.jobs.static_rendered")}
                                </Badge>
                              </div>
                            </div>
                            <div className="flex flex-wrap gap-x-4 gap-y-2 text-sm text-muted-foreground">
                              <span>{t("dev_portal.site_console.recent_crawl_activity.depth", { depth: job.depth })}</span>
                              <span>{job.http_status ? `HTTP ${job.http_status}` : t("dev_portal.site_console.recent_crawl_activity.no_status")}</span>
                              <span>{formatTimestamp(job.finished_at ?? job.discovered_at, t("dev_portal.common.not_yet"))}</span>
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

              <div className="grid gap-4">
                <Card className="rounded-2xl shadow-none">
                  <CardHeader className="pb-4">
                    <CardTitle className="text-base">{t("console.domains.blacklist_title")}</CardTitle>
                  </CardHeader>
                  <CardContent className="space-y-4">
                    <div className="rounded-xl border border-border bg-muted/40 px-4 py-3 text-sm text-muted-foreground">
                      {t("console.domains.blacklist_meta")}
                    </div>
                    <div className="flex flex-wrap gap-2">
                      <Button
                        type="button"
                        variant="outline"
                        disabled={!currentDomain || currentDomainBlacklisted}
                        onClick={() => {
                          if (!currentDomain) {
                            return;
                          }
                          const next = Array.from(new Set([...blacklistDomains, currentDomain])).join("\n");
                          setBlacklistDraft(next);
                        }}
                      >
                        {t("console.domains.blacklist_add_current")}
                      </Button>
                    </div>
                    <FieldShell label={t("console.domains.blacklist_label")}>
                      <Textarea
                        rows={8}
                        value={blacklistDraft}
                        onChange={(event) => setBlacklistDraft(event.target.value)}
                        placeholder={t("console.domains.blacklist_placeholder")}
                      />
                    </FieldShell>
                    <Button type="button" disabled={savingBlacklist || !blacklistDirty} onClick={() => void handleSaveBlacklist()}>
                      {t("console.settings.save")}
                    </Button>
                  </CardContent>
                </Card>

                <Card className="rounded-2xl shadow-none">
                  <CardHeader className="pb-4">
                    <CardTitle className="text-base">{t("console.domains.seed_title")}</CardTitle>
                  </CardHeader>
                  <CardContent className="space-y-4">
                    <form className="grid gap-4" onSubmit={handleSeed}>
                      <FieldShell label={t("dev_portal.site_console.submit_urls.property")}>
                        <Input
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
                            const domain = submitDomain || insight.domain;
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
                            const domain = submitDomain || insight.domain;
                            setSubmitDomain(domain);
                            setSubmitUrls(buildSeedSuggestions(domain));
                          }}
                        >
                          {t("dev_portal.site_console.submit_urls.homepage_and_sitemap")}
                        </Button>
                      </div>
                      <FieldShell label={t("dev_portal.site_console.submit_urls.url_list")}>
                        <Textarea
                          rows={8}
                          value={submitUrls}
                          onChange={(event) => setSubmitUrls(event.target.value)}
                          placeholder={t("dev_portal.site_console.submit_urls.url_list_placeholder")}
                        />
                      </FieldShell>
                      <div className="grid gap-4 sm:grid-cols-2">
                        <FieldShell label={t("dev_portal.site_console.submit_urls.crawl_depth")}>
                          <Input
                            type="number"
                            min={0}
                            max={10}
                            value={submitDepth}
                            onChange={(event) => setSubmitDepth(event.target.value)}
                          />
                        </FieldShell>
                        <FieldShell label={t("dev_portal.site_console.submit_urls.page_budget")}>
                          <Input
                            type="number"
                            min={1}
                            max={10000}
                            value={submitMaxPages}
                            onChange={(event) => setSubmitMaxPages(event.target.value)}
                          />
                        </FieldShell>
                        <FieldShell label={t("dev_portal.site_console.submit_urls.same_origin_concurrency")}>
                          <Input
                            type="number"
                            min={1}
                            max={32}
                            value={submitSameOriginConcurrency}
                            onChange={(event) => setSubmitSameOriginConcurrency(event.target.value)}
                          />
                        </FieldShell>
                        <FieldShell label={t("console.tasks.scope_label")}>
                          <Select value={submitScope} onValueChange={(value) => setSubmitScope(value as DiscoveryScope)}>
                            <SelectTrigger>
                              <SelectValue />
                            </SelectTrigger>
                            <SelectContent>
                              <SelectItem value="same_host">{t("console.tasks.scope_same_host")}</SelectItem>
                              <SelectItem value="same_domain">{t("console.tasks.scope_same_domain")}</SelectItem>
                              <SelectItem value="any">{t("console.tasks.scope_any")}</SelectItem>
                            </SelectContent>
                          </Select>
                        </FieldShell>
                        <FieldShell label={t("console.tasks.max_links_label")}>
                          <Input
                            type="number"
                            min={1}
                            max={200}
                            value={submitMaxDiscovered}
                            onChange={(event) => setSubmitMaxDiscovered(event.target.value)}
                          />
                        </FieldShell>
                      </div>
                      <label className="flex items-center gap-3 rounded-xl border border-border bg-muted/40 px-4 py-3 text-sm text-foreground">
                        <Checkbox checked={submitRevisit} onCheckedChange={(checked) => setSubmitRevisit(checked === true)} />
                        <span>{t("dev_portal.site_console.submit_urls.allow_revisit")}</span>
                      </label>
                      <Button type="submit" disabled={busy || !submitUrls.trim()}>
                        {t("dev_portal.site_console.submit_urls.submit")}
                      </Button>
                    </form>
                  </CardContent>
                </Card>
              </div>
            </div>
          </>
        ) : (
          <div className="rounded-2xl border border-dashed border-border bg-muted/40 px-4 py-8 text-center text-sm text-muted-foreground">
            {t("console.domains.empty")}
          </div>
        )}
      </PanelSection>
    </div>
  );
}

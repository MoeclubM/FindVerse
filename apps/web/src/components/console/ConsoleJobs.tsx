import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  cleanupFailedJobs,
  cleanupCompletedJobs,
  getCrawlJobStats,
  listCrawlJobs,
  retryFailedJobs,
  stopAllCrawlJobs,
  type CrawlJobList,
  type CrawlJobStats,
} from "../../api";
import { DetailDialog, PanelSection, StatStrip } from "../common/PanelPrimitives";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "../ui/alert-dialog";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";
import { Card, CardContent } from "../ui/card";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../ui/select";
import { Skeleton } from "../ui/skeleton";
import { useConsole } from "./ConsoleContext";
import { getConsoleJobStatusLabel, getConsoleValueLabel } from "./consoleLabels";

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

function formatTimestamp(value: string | null) {
  return value ? value.replace("T", " ").replace("Z", "").slice(0, 16) : "-";
}

const PAGE_SIZE = 20;

export function ConsoleJobs() {
  const { token, busy, setBusy, setFlash, refreshAll } = useConsole();
  const { t } = useTranslation();

  const [statusFilter, setStatusFilter] = useState("");
  const [offset, setOffset] = useState(0);
  const [jobs, setJobs] = useState<CrawlJobList | null>(null);
  const [stats, setStats] = useState<CrawlJobStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [selectedJobId, setSelectedJobId] = useState<string | null>(null);
  const [cleanupFailedOpen, setCleanupFailedOpen] = useState(false);
  const [stopAllOpen, setStopAllOpen] = useState(false);

  const selectedJob = useMemo(
    () => jobs?.jobs.find((job) => job.id === selectedJobId) ?? null,
    [jobs?.jobs, selectedJobId],
  );

  const refreshJobs = useCallback(
    async (nextOffset: number, silent = false) => {
      const [nextStats, nextJobs] = await Promise.all([
        getCrawlJobStats(token),
        listCrawlJobs(token, {
          status: statusFilter || undefined,
          offset: nextOffset,
          limit: PAGE_SIZE,
        }),
      ]);
      setStats(nextStats);
      setJobs(nextJobs);
      setSelectedJobId((current) =>
        current && nextJobs.jobs.some((job) => job.id === current) ? current : null,
      );
      if (!silent) {
        setLoading(false);
      }
    },
    [token, statusFilter],
  );

  useEffect(() => {
    let cancelled = false;
    setLoading(true);

    refreshJobs(offset)
      .catch((error) => {
        if (!cancelled) {
          setFlash(getErrorMessage(error, t("console.jobs.load_failed")));
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [offset, refreshJobs, setFlash, t]);

  useEffect(() => {
    const timer = window.setInterval(() => {
      void refreshJobs(offset, true).catch(() => undefined);
    }, 1000);
    return () => window.clearInterval(timer);
  }, [offset, refreshJobs]);

  async function handleRetryFailed() {
    setBusy(true);
    setFlash(null);
    try {
      const response = await retryFailedJobs(token);
      setFlash(t("console.jobs.retry_success", { count: response.retried }));
      await refreshAll();
      setOffset(0);
      await refreshJobs(0, true);
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.jobs.retry_failed_error")));
    } finally {
      setBusy(false);
    }
  }

  async function handleCleanupSucceeded() {
    setBusy(true);
    setFlash(null);
    try {
      const response = await cleanupCompletedJobs(token);
      setFlash(t("console.jobs.cleanup_success", { count: response.cleaned }));
      await refreshAll();
      setOffset(0);
      await refreshJobs(0, true);
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.jobs.cleanup_failed_error")));
    } finally {
      setBusy(false);
    }
  }

  async function handleCleanupFailed() {
    setCleanupFailedOpen(false);
    setBusy(true);
    setFlash(null);
    try {
      const response = await cleanupFailedJobs(token);
      setFlash(t("console.jobs.cleanup_failed_success", { count: response.cleaned }));
      await refreshAll();
      setOffset(0);
      await refreshJobs(0, true);
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.jobs.cleanup_failed_error_action")));
    } finally {
      setBusy(false);
    }
  }

  async function handleStopAll() {
    setStopAllOpen(false);
    setBusy(true);
    setFlash(null);
    try {
      const response = await stopAllCrawlJobs(token);
      setFlash(
        t("console.jobs.stop_all_success", {
          rules: response.disabled_rules,
          jobs: response.removed_jobs,
        }),
      );
      await refreshAll();
      setOffset(0);
      await refreshJobs(0, true);
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.jobs.stop_all_failed")));
    } finally {
      setBusy(false);
    }
  }

  async function handleCopy(value: string) {
    try {
      await navigator.clipboard.writeText(value);
      setFlash(t("console.settings.save_success"));
    } catch {
      setFlash(value);
    }
  }

  return (
    <>
      <PanelSection
          title={t("console.jobs.status_title")}
          meta={t("console.jobs.visible_jobs", { count: jobs?.total ?? 0 })}
          contentClassName="space-y-5"
      >
        <StatStrip
          items={[
            { label: t("console.jobs.stats.queued"), value: stats?.queued ?? 0 },
            { label: t("console.jobs.stats.claimed"), value: stats?.claimed ?? 0 },
            { label: t("console.jobs.stats.succeeded"), value: stats?.succeeded ?? 0 },
            { label: t("console.jobs.stats.failed"), value: stats?.failed ?? 0 },
            { label: t("console.jobs.stats.blocked"), value: stats?.blocked ?? 0 },
            { label: t("console.jobs.stats.dead_letter"), value: stats?.dead_letter ?? 0 },
          ]}
          className="xl:grid-cols-6"
        />
      </PanelSection>

      <PanelSection title={t("console.jobs.queue_title")} meta={t("console.jobs.queue_meta")} contentClassName="space-y-5">

        <div className="flex flex-wrap gap-3">
          <Select
            value={statusFilter || "all"}
            onValueChange={(value) => {
              setStatusFilter(value === "all" ? "" : value);
              setOffset(0);
            }}
          >
            <SelectTrigger className="w-[210px]">
              <SelectValue placeholder={t("console.jobs.all_statuses")} />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">{t("console.jobs.all_statuses")}</SelectItem>
              <SelectItem value="queued">{t("console.jobs.stats.queued")}</SelectItem>
              <SelectItem value="claimed">{t("console.jobs.stats.claimed")}</SelectItem>
              <SelectItem value="succeeded">{t("console.jobs.stats.succeeded")}</SelectItem>
              <SelectItem value="failed">{t("console.jobs.stats.failed")}</SelectItem>
              <SelectItem value="blocked">{t("console.jobs.stats.blocked")}</SelectItem>
              <SelectItem value="dead_letter">{t("console.jobs.stats.dead_letter")}</SelectItem>
            </SelectContent>
          </Select>
          <Button type="button" variant="outline" disabled={busy} onClick={() => void handleRetryFailed()}>
            {t("console.jobs.retry_failed")}
          </Button>
          <Button type="button" variant="outline" disabled={busy} onClick={() => setCleanupFailedOpen(true)}>
            {t("console.jobs.cleanup_failed")}
          </Button>
          <Button type="button" variant="outline" disabled={busy} onClick={() => void handleCleanupSucceeded()}>
            {t("console.jobs.cleanup_succeeded")}
          </Button>
          <Button type="button" variant="destructive" disabled={busy} onClick={() => setStopAllOpen(true)}>
            {t("console.jobs.stop_all")}
          </Button>
        </div>

        <div className="grid gap-3">
          {loading ? (
            Array.from({ length: 3 }).map((_, index) => (
              <Card key={index} className="rounded-2xl">
                <CardContent className="grid gap-4 p-4">
                  <div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
                    <div className="grid min-w-0 flex-1 gap-2">
                      <Skeleton className="h-4 w-full max-w-2xl" />
                      <Skeleton className="h-4 w-40" />
                    </div>
                    <Skeleton className="h-8 w-20 rounded-lg" />
                  </div>
                  <div className="flex flex-wrap items-center gap-2">
                    <Skeleton className="h-6 w-24 rounded-full" />
                    <Skeleton className="h-4 w-20" />
                    <Skeleton className="h-4 w-24" />
                    <Skeleton className="h-4 w-28" />
                  </div>
                  <div className="grid gap-3 sm:grid-cols-3">
                    <Skeleton className="h-20 rounded-xl" />
                    <Skeleton className="h-20 rounded-xl" />
                    <Skeleton className="h-20 rounded-xl" />
                  </div>
                </CardContent>
              </Card>
            ))
          ) : jobs?.jobs.length ? (
            jobs.jobs.map((job) => (
              <Card key={job.id} className="rounded-2xl">
                <CardContent className="grid min-w-0 gap-4 p-4">
                <div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
                  <div className="grid min-w-0 flex-1 gap-1">
                    <strong
                      className="truncate text-sm font-semibold text-foreground"
                      title={job.final_url ?? job.url}
                    >
                      {job.final_url ?? job.url}
                    </strong>
                    <span className="text-sm text-muted-foreground">{job.source}</span>
                  </div>
                  <Button type="button" variant="ghost" size="sm" onClick={() => setSelectedJobId(job.id)}>
                    {t("console.actions.details")}
                  </Button>
                </div>
                <div className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
                  <Badge variant={job.status === "succeeded" ? "success" : "outline"}>
                    {getConsoleJobStatusLabel(t, job.status)}
                  </Badge>
                  <Badge variant={job.render_mode === "browser" ? "warning" : "outline"}>
                    {job.render_mode === "browser"
                      ? t("console.jobs.browser_rendered")
                      : t("console.jobs.static_rendered")}
                  </Badge>
                  {job.http_status != null ? <span>HTTP {job.http_status}</span> : null}
                  <span>{t("console.jobs.attempt_progress", { current: job.attempt_count, max: job.max_attempts })}</span>
                  <span>{t("console.jobs.depth_progress", { current: job.depth, max: job.max_depth })}</span>
                  {job.claimed_by ? <span>{t("console.jobs.worker_id", { id: job.claimed_by })}</span> : null}
                </div>
                <div className="grid gap-3 sm:grid-cols-3">
                  <div className="rounded-xl border border-border bg-muted/40 px-3 py-2">
                    <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.discovered")}</span>
                    <strong className="mt-2 block text-lg font-semibold text-foreground">{job.discovered_urls_count}</strong>
                  </div>
                  <div className="rounded-xl border border-border bg-muted/40 px-3 py-2">
                    <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.finished")}</span>
                    <strong className="mt-2 block text-sm font-semibold text-foreground">{formatTimestamp(job.finished_at ?? job.claimed_at)}</strong>
                  </div>
                  <div className="rounded-xl border border-border bg-muted/40 px-3 py-2">
                    <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.status")}</span>
                    <strong className="mt-2 block text-sm font-semibold text-foreground">{getConsoleValueLabel(job.failure_kind ?? job.llm_decision)}</strong>
                  </div>
                </div>
                </CardContent>
              </Card>
            ))
          ) : (
            <div className="rounded-2xl border border-dashed border-border bg-muted/40 px-4 py-8 text-center text-sm text-muted-foreground">{t("console.jobs.no_jobs_match")}</div>
          )}
        </div>

        <div className="flex items-center gap-3">
          <Button
            type="button"
            variant="outline"
            disabled={offset === 0}
            onClick={() => setOffset((current) => Math.max(0, current - PAGE_SIZE))}
          >
            {t("search.previous")}
          </Button>
          <span className="text-sm text-muted-foreground">{t("console.jobs.offset", { offset })}</span>
          <Button
            type="button"
            variant="outline"
            disabled={jobs?.next_offset == null}
            onClick={() => setOffset(jobs?.next_offset ?? offset)}
          >
            {t("search.next")}
          </Button>
        </div>
      </PanelSection>

      <AlertDialog open={cleanupFailedOpen} onOpenChange={setCleanupFailedOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("console.jobs.cleanup_failed")}</AlertDialogTitle>
            <AlertDialogDescription>{t("console.jobs.cleanup_failed_confirm")}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={busy}>{t("console.workers.cancel")}</AlertDialogCancel>
            <AlertDialogAction variant="destructive" disabled={busy} onClick={() => void handleCleanupFailed()}>
              {t("console.jobs.cleanup_failed")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <AlertDialog open={stopAllOpen} onOpenChange={setStopAllOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("console.jobs.stop_all")}</AlertDialogTitle>
            <AlertDialogDescription>{t("console.jobs.stop_all_confirm")}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={busy}>{t("console.workers.cancel")}</AlertDialogCancel>
            <AlertDialogAction variant="destructive" disabled={busy} onClick={() => void handleStopAll()}>
              {t("console.jobs.stop_all")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <DetailDialog
        open={Boolean(selectedJob)}
        title={
          <span className="block max-w-full truncate">
            {selectedJob?.final_url ?? selectedJob?.url ?? t("console.jobs.title")}
          </span>
        }
        meta={selectedJob?.source}
        closeLabel={t("console.actions.close")}
        onClose={() => setSelectedJobId(null)}
      >
        {selectedJob ? (
            <div className="grid gap-4">
              <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
                <div className="rounded-xl border border-border bg-muted/40 p-4">
                  <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.status")}</span>
                <strong className="mt-2 block text-sm font-semibold text-foreground">{getConsoleJobStatusLabel(t, selectedJob.status)}</strong>
                </div>
              <div className="rounded-xl border border-border bg-muted/40 p-4">
                <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.http_status")}</span>
                <strong className="mt-2 block text-sm font-semibold text-foreground">{selectedJob.http_status ?? "-"}</strong>
              </div>
              <div className="rounded-xl border border-border bg-muted/40 p-4">
                <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.attempts")}</span>
                <strong className="mt-2 block text-sm font-semibold text-foreground">{selectedJob.attempt_count} / {selectedJob.max_attempts}</strong>
              </div>
              <div className="rounded-xl border border-border bg-muted/40 p-4">
                <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.depth")}</span>
                <strong className="mt-2 block text-sm font-semibold text-foreground">{selectedJob.depth} / {selectedJob.max_depth}</strong>
              </div>
              <div className="rounded-xl border border-border bg-muted/40 p-4">
                <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.worker")}</span>
                <strong className="mt-2 block text-sm font-semibold text-foreground">{selectedJob.claimed_by ?? "-"}</strong>
              </div>
              <div className="rounded-xl border border-border bg-muted/40 p-4">
                <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.discovered")}</span>
                <strong className="mt-2 block text-sm font-semibold text-foreground">{selectedJob.discovered_urls_count}</strong>
              </div>
              <div className="rounded-xl border border-border bg-muted/40 p-4">
                <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.retry_after")}</span>
                <strong className="mt-2 block text-sm font-semibold text-foreground">{formatTimestamp(selectedJob.next_retry_at)}</strong>
              </div>
              <div className="rounded-xl border border-border bg-muted/40 p-4">
                <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.finished")}</span>
                <strong className="mt-2 block text-sm font-semibold text-foreground">{formatTimestamp(selectedJob.finished_at)}</strong>
              </div>
            </div>
            <div className="grid gap-3 rounded-xl border border-border bg-muted/30 p-4">
              <div className="flex items-center justify-between gap-3">
                <span className="text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.url")}</span>
                <Button type="button" variant="outline" size="sm" onClick={() => void handleCopy(selectedJob.url)}>
                  {t("console.actions.copy")}
                </Button>
              </div>
              <code className="max-w-full break-all text-xs">{selectedJob.url}</code>
            </div>
            {selectedJob.final_url ? (
              <div className="grid gap-3 rounded-xl border border-border bg-muted/30 p-4">
                <div className="flex items-center justify-between gap-3">
                  <span className="text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.final_url")}</span>
                  <Button type="button" variant="outline" size="sm" onClick={() => void handleCopy(selectedJob.final_url!)}>
                    {t("console.actions.copy")}
                  </Button>
                </div>
                <code className="max-w-full break-all text-xs">{selectedJob.final_url}</code>
              </div>
            ) : null}
            <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
              <div className="rounded-xl border border-border bg-muted/40 p-4">
                <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.content_type")}</span>
                <strong className="mt-2 block text-sm font-semibold text-foreground">{selectedJob.content_type ?? "-"}</strong>
              </div>
              <div className="rounded-xl border border-border bg-muted/40 p-4">
                <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.document_id")}</span>
                <strong className="mt-2 block text-sm font-semibold text-foreground">{selectedJob.accepted_document_id ?? "-"}</strong>
              </div>
              <div className="rounded-xl border border-border bg-muted/40 p-4">
                <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.llm_decision")}</span>
                <strong className="mt-2 block text-sm font-semibold text-foreground">{getConsoleValueLabel(selectedJob.llm_decision)}</strong>
              </div>
              <div className="rounded-xl border border-border bg-muted/40 p-4">
                <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.score")}</span>
                <strong className="mt-2 block text-sm font-semibold text-foreground">
                  {selectedJob.llm_relevance_score != null ? selectedJob.llm_relevance_score.toFixed(2) : "-"}
                </strong>
              </div>
              <div className="rounded-xl border border-border bg-muted/40 p-4">
                <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.render_mode")}</span>
                <strong className="mt-2 block text-sm font-semibold text-foreground">
                  {selectedJob.render_mode === "browser"
                    ? t("console.jobs.browser_rendered")
                    : t("console.jobs.static_rendered")}
                </strong>
              </div>
            </div>
            {selectedJob.llm_reason ? (
              <div className="grid gap-2 rounded-xl border border-border bg-muted/30 p-4">
                <span className="text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.llm_decision")}</span>
                <p className="text-sm leading-6 text-muted-foreground whitespace-pre-wrap">{selectedJob.llm_reason}</p>
              </div>
            ) : null}
            {selectedJob.failure_kind || selectedJob.failure_message ? (
              <div className="grid gap-2 rounded-xl border border-border bg-muted/30 p-4">
                <span className="text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.failure")}</span>
                <p className="text-sm leading-6 text-muted-foreground whitespace-pre-wrap">
                  {[selectedJob.failure_kind, selectedJob.failure_message].filter(Boolean).join(" · ")}
                </p>
              </div>
            ) : null}
          </div>
        ) : null}
      </DetailDialog>
    </>
  );
}

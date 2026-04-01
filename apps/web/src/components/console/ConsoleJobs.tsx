import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Eye, RotateCcw, StopCircle, Trash2 } from "lucide-react";

import {
  cleanupCompletedJobs,
  cleanupFailedJobs,
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
import { Alert, AlertDescription, AlertTitle } from "../ui/alert";
import { Badge, type BadgeProps } from "../ui/badge";
import { Button } from "../ui/button";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../ui/select";
import { Skeleton } from "../ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "../ui/table";
import { useConsole } from "./ConsoleContext";
import { getConsoleJobStatusLabel, getConsoleValueLabel } from "./consoleLabels";

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

function formatTimestamp(value: string | null) {
  return value ? value.replace("T", " ").replace("Z", "").slice(0, 16) : "-";
}

function getStatusVariant(status: string): BadgeProps["variant"] {
  switch (status) {
    case "succeeded":
      return "success";
    case "failed":
    case "dead_letter":
      return "destructive";
    case "blocked":
      return "warning";
    default:
      return "outline";
  }
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

  const cleanupFailedCount = (stats?.failed ?? 0) + (stats?.blocked ?? 0) + (stats?.dead_letter ?? 0);
  const cleanupSucceededCount = stats?.succeeded ?? 0;

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

    refreshJobs(offset).catch((error) => {
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
        meta={`${t("console.jobs.visible_jobs", { count: jobs?.total ?? 0 })} · ${t("console.live_refresh")}`}
        contentClassName="space-y-4"
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
          className="md:grid-cols-3 xl:grid-cols-6"
        />
      </PanelSection>

      <PanelSection
        title={t("console.jobs.queue_title")}
        meta={t("console.jobs.queue_meta")}
        actions={
          <div className="flex flex-wrap items-center gap-2">
            <Select
              value={statusFilter || "all"}
              onValueChange={(value) => {
                setStatusFilter(value === "all" ? "" : value);
                setOffset(0);
              }}
            >
              <SelectTrigger size="sm" className="w-[180px]">
                <SelectValue placeholder={t("console.jobs.all_statuses")} />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  <SelectItem value="all">{t("console.jobs.all_statuses")}</SelectItem>
                  <SelectItem value="queued">{t("console.jobs.stats.queued")}</SelectItem>
                  <SelectItem value="claimed">{t("console.jobs.stats.claimed")}</SelectItem>
                  <SelectItem value="succeeded">{t("console.jobs.stats.succeeded")}</SelectItem>
                  <SelectItem value="failed">{t("console.jobs.stats.failed")}</SelectItem>
                  <SelectItem value="blocked">{t("console.jobs.stats.blocked")}</SelectItem>
                  <SelectItem value="dead_letter">{t("console.jobs.stats.dead_letter")}</SelectItem>
                </SelectGroup>
              </SelectContent>
            </Select>
            <Button type="button" size="sm" variant="outline" disabled={busy} onClick={() => void handleRetryFailed()}>
              <RotateCcw data-icon="inline-start" />
              {t("console.jobs.retry_failed")}
            </Button>
            <Button type="button" size="sm" variant="destructive" disabled={busy} onClick={() => setCleanupFailedOpen(true)}>
              <Trash2 data-icon="inline-start" />
              {t("console.jobs.cleanup_failed")} ({cleanupFailedCount})
            </Button>
            <Button type="button" size="sm" variant="outline" disabled={busy} onClick={() => void handleCleanupSucceeded()}>
              <Trash2 data-icon="inline-start" />
              {t("console.jobs.cleanup_succeeded")} ({cleanupSucceededCount})
            </Button>
            <Button type="button" size="sm" variant="destructive" disabled={busy} onClick={() => setStopAllOpen(true)}>
              <StopCircle data-icon="inline-start" />
              {t("console.jobs.stop_all")}
            </Button>
          </div>
        }
        contentClassName="space-y-4"
      >
        <div className="overflow-hidden rounded-xl border border-border bg-card">
          <Table className="table-fixed">
            <TableHeader>
              <TableRow className="hover:bg-transparent">
                <TableHead className="w-[38%]">{t("console.jobs.url")}</TableHead>
                <TableHead className="w-[28%]">{t("console.jobs.status")}</TableHead>
                <TableHead className="hidden w-[20%] lg:table-cell">{t("console.jobs.source")}</TableHead>
                <TableHead className="hidden w-[14%] xl:table-cell">{t("console.jobs.finished")}</TableHead>
                <TableHead className="w-24 text-right">{t("console.actions.details")}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {loading
                ? Array.from({ length: 6 }).map((_, index) => (
                    <TableRow key={index}>
                      <TableCell className="whitespace-normal">
                        <div className="flex min-w-0 flex-col gap-2">
                          <Skeleton className="h-4 w-full max-w-xl" />
                          <Skeleton className="h-3 w-40" />
                        </div>
                      </TableCell>
                      <TableCell className="whitespace-normal">
                        <div className="flex flex-col gap-2">
                          <div className="flex flex-wrap items-center gap-2">
                            <Skeleton className="h-6 w-24 rounded-full" />
                            <Skeleton className="h-6 w-24 rounded-full" />
                          </div>
                          <Skeleton className="h-3 w-48" />
                        </div>
                      </TableCell>
                      <TableCell className="hidden whitespace-normal lg:table-cell">
                        <div className="flex flex-col gap-2">
                          <Skeleton className="h-4 w-36" />
                          <Skeleton className="h-3 w-28" />
                        </div>
                      </TableCell>
                      <TableCell className="hidden whitespace-normal xl:table-cell">
                        <div className="flex flex-col gap-2">
                          <Skeleton className="h-4 w-28" />
                          <Skeleton className="h-3 w-24" />
                        </div>
                      </TableCell>
                      <TableCell className="text-right">
                        <Skeleton className="ml-auto h-7 w-16 rounded-lg" />
                      </TableCell>
                    </TableRow>
                  ))
                : jobs?.jobs.length
                  ? jobs.jobs.map((job) => (
                      <TableRow key={job.id} data-state={selectedJobId === job.id ? "selected" : undefined}>
                        <TableCell className="max-w-0 whitespace-normal">
                          <div className="flex min-w-0 flex-col gap-1">
                            <div className="flex min-w-0 items-center gap-2">
                              <span className="truncate text-sm font-semibold text-foreground" title={job.final_url ?? job.url}>
                                {job.final_url ?? job.url}
                              </span>
                              {job.final_url && job.final_url !== job.url ? <Badge variant="outline">{t("console.jobs.final_url")}</Badge> : null}
                            </div>
                            <div className="flex min-w-0 flex-col gap-1 text-xs text-muted-foreground">
                              {job.final_url && job.final_url !== job.url ? (
                                <span className="truncate" title={job.url}>
                                  {job.url}
                                </span>
                              ) : null}
                              <span className="truncate lg:hidden" title={job.source}>
                                {job.source}
                              </span>
                            </div>
                          </div>
                        </TableCell>
                        <TableCell className="whitespace-normal">
                          <div className="flex flex-col gap-2">
                            <div className="flex flex-wrap items-center gap-2">
                              <Badge variant={getStatusVariant(job.status)}>{getConsoleJobStatusLabel(t, job.status)}</Badge>
                              <Badge variant={job.render_mode === "browser" ? "warning" : "outline"}>
                                {job.render_mode === "browser" ? t("console.jobs.browser_rendered") : t("console.jobs.static_rendered")}
                              </Badge>
                              {job.http_status != null ? <Badge variant="outline">HTTP {job.http_status}</Badge> : null}
                            </div>
                            <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                              <span>{t("console.jobs.attempt_progress", { current: job.attempt_count, max: job.max_attempts })}</span>
                              <span>{t("console.jobs.depth_progress", { current: job.depth, max: job.max_depth })}</span>
                              <span>{t("console.jobs.discovered_count", { count: job.discovered_urls_count })}</span>
                            </div>
                          </div>
                        </TableCell>
                        <TableCell className="hidden whitespace-normal lg:table-cell">
                          <div className="flex flex-col gap-1">
                            <span className="truncate text-sm font-medium text-foreground" title={job.source}>
                              {job.source}
                            </span>
                            <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                              {job.claimed_by ? <span>{t("console.jobs.worker_id", { id: job.claimed_by })}</span> : null}
                              <span>{getConsoleValueLabel(job.failure_kind ?? job.llm_decision)}</span>
                            </div>
                          </div>
                        </TableCell>
                        <TableCell className="hidden whitespace-normal xl:table-cell">
                          <div className="flex flex-col gap-1 text-sm">
                            <span className="font-medium text-foreground">{formatTimestamp(job.finished_at)}</span>
                            <span className="text-xs text-muted-foreground">
                              {t("console.jobs.retry_after")}: {formatTimestamp(job.next_retry_at)}
                            </span>
                          </div>
                        </TableCell>
                        <TableCell className="text-right">
                          <Button type="button" size="sm" variant="ghost" onClick={() => setSelectedJobId(job.id)}>
                            <Eye data-icon="inline-start" />
                            {t("console.actions.details")}
                          </Button>
                        </TableCell>
                      </TableRow>
                    ))
                  : null}
            </TableBody>
          </Table>
        </div>

        {!loading && !jobs?.jobs.length ? (
          <Alert>
            <AlertTitle>{t("console.jobs.no_jobs_match")}</AlertTitle>
            <AlertDescription>{t("console.jobs.queue_meta")}</AlertDescription>
          </Alert>
        ) : null}

        <div className="flex flex-wrap items-center justify-between gap-3">
          <span className="text-sm text-muted-foreground">{t("console.jobs.offset", { offset })}</span>
          <div className="flex items-center gap-2">
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={offset === 0}
              onClick={() => setOffset((current) => Math.max(0, current - PAGE_SIZE))}
            >
              {t("search.previous")}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={jobs?.next_offset == null}
              onClick={() => setOffset(jobs?.next_offset ?? offset)}
            >
              {t("search.next")}
            </Button>
          </div>
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
                <strong className="mt-2 block text-sm font-semibold text-foreground">
                  {getConsoleJobStatusLabel(t, selectedJob.status)}
                </strong>
              </div>
              <div className="rounded-xl border border-border bg-muted/40 p-4">
                <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.http_status")}</span>
                <strong className="mt-2 block text-sm font-semibold text-foreground">{selectedJob.http_status ?? "-"}</strong>
              </div>
              <div className="rounded-xl border border-border bg-muted/40 p-4">
                <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.attempts")}</span>
                <strong className="mt-2 block text-sm font-semibold text-foreground">
                  {selectedJob.attempt_count} / {selectedJob.max_attempts}
                </strong>
              </div>
              <div className="rounded-xl border border-border bg-muted/40 p-4">
                <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.depth")}</span>
                <strong className="mt-2 block text-sm font-semibold text-foreground">
                  {selectedJob.depth} / {selectedJob.max_depth}
                </strong>
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
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    onClick={() => void handleCopy(selectedJob.final_url ?? selectedJob.url)}
                  >
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
                <strong className="mt-2 block text-sm font-semibold text-foreground">
                  {getConsoleValueLabel(selectedJob.llm_decision)}
                </strong>
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
                <p className="whitespace-pre-wrap text-sm leading-6 text-muted-foreground">{selectedJob.llm_reason}</p>
              </div>
            ) : null}
            {selectedJob.failure_kind || selectedJob.failure_message ? (
              <div className="grid gap-2 rounded-xl border border-border bg-muted/30 p-4">
                <span className="text-xs font-medium uppercase tracking-[0.14em] text-muted-foreground">{t("console.jobs.failure")}</span>
                <p className="whitespace-pre-wrap text-sm leading-6 text-muted-foreground">
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

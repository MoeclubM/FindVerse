import { useState } from "react";
import { useTranslation } from "react-i18next";
import { HardDriveDownload, Trash2 } from "lucide-react";

import {
  deleteCrawler,
  renameCrawler,
  requestCrawlerUpdate,
  updateCrawlerSortOrder,
  updateCrawlerRuntime,
} from "../../api";
import {
  DetailDialog,
  FieldShell,
  PanelSection,
  StatStrip,
} from "../common/PanelPrimitives";
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
import { Input } from "../ui/input";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "../ui/table";
import { useConsole } from "./ConsoleContext";

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

function formatTimestamp(value: string | null) {
  return value ? value.replace("T", " ").replace("Z", "").slice(0, 16) : "-";
}

function formatCrawlerVersion(value: string | null) {
  return value ?? "-";
}

function getCrawlerUpdateVariant(status: string) {
  switch (status) {
    case "failed":
      return "destructive" as const;
    case "downloading":
    case "restarting":
      return "warning" as const;
    case "pending":
      return "default" as const;
    default:
      return "outline" as const;
  }
}

export function ConsoleWorkers() {
  const { token, busy, setBusy, setFlash, refreshAll, overview } = useConsole();
  const { t } = useTranslation();
  const platformVersion = overview?.platform_version ?? "-";
  const crawlers = overview?.crawlers ?? [];
  const onlineWorkers = crawlers.filter((crawler) => crawler.online).length;
  const totalClaimed = crawlers.reduce(
    (sum, crawler) => sum + crawler.jobs_claimed,
    0,
  );
  const totalReported = crawlers.reduce(
    (sum, crawler) => sum + crawler.jobs_reported,
    0,
  );
  const totalInFlight = crawlers.reduce(
    (sum, crawler) => sum + crawler.in_flight_jobs,
    0,
  );

  const [editingId, setEditingId] = useState<string | null>(null);
  const [editName, setEditName] = useState("");
  const [selectedCrawlerId, setSelectedCrawlerId] = useState<string | null>(
    null,
  );
  const [deleteCrawlerId, setDeleteCrawlerId] = useState<string | null>(null);
  const [runtimeWorkerConcurrency, setRuntimeWorkerConcurrency] = useState("");
  const [runtimeJsRenderConcurrency, setRuntimeJsRenderConcurrency] =
    useState("");
  const [runtimeMaxJobs, setRuntimeMaxJobs] = useState("");
  const [sortOrder, setSortOrder] = useState("");
  const selectedCrawler =
    crawlers.find((crawler) => crawler.id === selectedCrawlerId) ?? null;
  const deleteCrawlerTarget =
    crawlers.find((crawler) => crawler.id === deleteCrawlerId) ?? null;
  const nextRuntimeWorkerConcurrency = String(
    Math.max(
      1,
      Number(runtimeWorkerConcurrency) ||
        selectedCrawler?.worker_concurrency ||
        1,
    ),
  );
  const nextRuntimeJsRenderConcurrency = String(
    Math.max(
      1,
      Number(runtimeJsRenderConcurrency) ||
        selectedCrawler?.js_render_concurrency ||
        1,
    ),
  );
  const nextRuntimeMaxJobs = String(
    Math.max(1, Number(runtimeMaxJobs) || selectedCrawler?.max_jobs || 1),
  );
  const runtimeDirty = selectedCrawler
    ? runtimeWorkerConcurrency !== String(selectedCrawler.worker_concurrency) ||
      runtimeJsRenderConcurrency !==
        String(selectedCrawler.js_render_concurrency) ||
      runtimeMaxJobs !== String(selectedCrawler.max_jobs)
    : false;
  const nextSortOrder =
    sortOrder.trim() === "" ? null : Number(sortOrder.trim());
  const sortOrderDirty = selectedCrawler
    ? (selectedCrawler.sort_order ?? null) !== nextSortOrder
    : false;
  const selectedCrawlerUpdateQueued =
    selectedCrawler?.desired_version === platformVersion &&
    selectedCrawler?.version !== platformVersion;
  const selectedCrawlerUpToDate =
    selectedCrawler?.version === platformVersion && platformVersion !== "-";

  function formatCrawlerUpdateStatus(status: string) {
    switch (status) {
      case "pending":
        return t("console.workers.update_status_pending");
      case "downloading":
        return t("console.workers.update_status_downloading");
      case "restarting":
        return t("console.workers.update_status_restarting");
      case "failed":
        return t("console.workers.update_status_failed");
      default:
        return t("console.workers.update_status_idle");
    }
  }

  function startEditing(crawlerId: string, currentName: string) {
    setEditingId(crawlerId);
    setEditName(currentName);
  }

  async function handleSaveName(crawlerId: string) {
    if (!editName.trim() || editName.trim().length < 2) {
      setFlash(t("console.workers.name_too_short"));
      return;
    }
    setBusy(true);
    setFlash(null);
    try {
      await renameCrawler(token, crawlerId, editName.trim());
      setEditingId(null);
      await refreshAll();
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.workers.rename_failed")));
    } finally {
      setBusy(false);
    }
  }

  async function handleDeleteCrawler(crawlerId: string) {
    setDeleteCrawlerId(null);
    setBusy(true);
    setFlash(null);
    try {
      await deleteCrawler(token, crawlerId);
      setEditingId(null);
      setEditName("");
      setSelectedCrawlerId(null);
      await refreshAll();
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.workers.delete_failed")));
    } finally {
      setBusy(false);
    }
  }

  async function handleSaveRuntime(crawlerId: string) {
    setBusy(true);
    setFlash(null);
    try {
      await updateCrawlerRuntime(
        token,
        crawlerId,
        Number(nextRuntimeWorkerConcurrency),
        Number(nextRuntimeJsRenderConcurrency),
        Number(nextRuntimeMaxJobs),
      );
      setRuntimeWorkerConcurrency(nextRuntimeWorkerConcurrency);
      setRuntimeJsRenderConcurrency(nextRuntimeJsRenderConcurrency);
      setRuntimeMaxJobs(nextRuntimeMaxJobs);
      await refreshAll();
      setFlash(t("console.workers.runtime_saved"));
    } catch (error) {
      setFlash(
        getErrorMessage(error, t("console.workers.runtime_save_failed")),
      );
    } finally {
      setBusy(false);
    }
  }

  async function handleRequestUpdate(crawlerId: string) {
    if (platformVersion === "-") {
      return;
    }
    setBusy(true);
    setFlash(null);
    try {
      await requestCrawlerUpdate(token, crawlerId, platformVersion);
      await refreshAll();
      setFlash(
        t("console.workers.update_requested", { version: platformVersion }),
      );
    } catch (error) {
      setFlash(
        getErrorMessage(error, t("console.workers.update_request_failed")),
      );
    } finally {
      setBusy(false);
    }
  }

  async function handleSaveSortOrder(crawlerId: string) {
    if (nextSortOrder !== null && !Number.isInteger(nextSortOrder)) {
      setFlash(t("console.workers.sort_order_invalid"));
      return;
    }
    setBusy(true);
    setFlash(null);
    try {
      await updateCrawlerSortOrder(token, crawlerId, nextSortOrder);
      setSortOrder(nextSortOrder === null ? "" : String(nextSortOrder));
      await refreshAll();
      setFlash(t("console.workers.sort_order_saved"));
    } catch (error) {
      setFlash(
        getErrorMessage(error, t("console.workers.sort_order_save_failed")),
      );
    } finally {
      setBusy(false);
    }
  }

  function openCrawlerDetails(crawlerId: string) {
    const crawler = crawlers.find((entry) => entry.id === crawlerId);
    if (!crawler) {
      return;
    }

    setEditingId(null);
    setEditName(crawler.name);
    setSelectedCrawlerId(crawler.id);
    setRuntimeWorkerConcurrency(String(crawler.worker_concurrency));
    setRuntimeJsRenderConcurrency(String(crawler.js_render_concurrency));
    setRuntimeMaxJobs(String(crawler.max_jobs));
    setSortOrder(crawler.sort_order === null ? "" : String(crawler.sort_order));
  }

  return (
    <PanelSection
      title={t("console.workers.title")}
      meta={t("console.workers.registered", { count: crawlers.length })}
      contentClassName="space-y-4"
    >
      <div className="rounded-lg border border-border bg-muted/40 px-3 py-2 text-xs text-muted-foreground">
        {t("console.workers.platform_version_hint", {
          version: platformVersion,
        })}
      </div>
      <StatStrip
        className="xl:grid-cols-5"
        items={[
          { label: t("console.overview.workers"), value: crawlers.length },
          { label: t("console.workers.online_count"), value: onlineWorkers },
          { label: t("console.workers.in_flight_jobs"), value: totalInFlight },
          { label: t("console.workers.jobs_claimed"), value: totalClaimed },
          { label: t("console.workers.jobs_reported"), value: totalReported },
        ]}
      />

      {crawlers.length ? (
        <div className="overflow-hidden rounded-lg border border-border bg-card">
          <Table>
            <TableHeader className="bg-muted/30">
              <TableRow>
                <TableHead>{t("console.workers.title")}</TableHead>
                <TableHead>{t("console.workers.status")}</TableHead>
                <TableHead>{t("console.workers.current_version")}</TableHead>
                <TableHead>{t("console.workers.runtime_title")}</TableHead>
                <TableHead>{t("console.workers.in_flight_jobs")}</TableHead>
                <TableHead>{t("console.workers.jobs_claimed")}</TableHead>
                <TableHead>{t("console.workers.jobs_reported")}</TableHead>
                <TableHead>{t("console.workers.last_seen")}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {crawlers.map((crawler) => (
                <TableRow
                  key={crawler.id}
                  role="button"
                  tabIndex={0}
                  onClick={() => openCrawlerDetails(crawler.id)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault();
                      openCrawlerDetails(crawler.id);
                    }
                  }}
                  className={selectedCrawlerId === crawler.id ? "cursor-pointer bg-muted/30" : "cursor-pointer"}
                >
                  <TableCell className="min-w-65 whitespace-normal align-top">
                    <div className="grid gap-1">
                      <div className="flex flex-wrap items-center gap-1.5">
                        <strong className="text-sm font-semibold text-foreground">{crawler.name}</strong>
                        <Badge variant="outline">{crawler.id.slice(0, 8)}</Badge>
                        {crawler.sort_order !== null ? (
                          <Badge variant="outline">
                            {t("console.workers.sort_order_badge", {
                              value: crawler.sort_order,
                            })}
                          </Badge>
                        ) : null}
                      </div>
                      <span className="text-xs text-muted-foreground">{crawler.preview}</span>
                      <span className="text-[11px] text-muted-foreground">
                        {t("console.workers.platform_label")}: {crawler.platform ?? t("console.workers.platform_unknown")}
                      </span>
                    </div>
                  </TableCell>
                  <TableCell className="whitespace-normal align-top">
                    <div className="flex flex-wrap gap-1.5">
                      <Badge variant={crawler.online ? "success" : "outline"}>
                        {crawler.online ? t("console.workers.online") : t("console.workers.offline")}
                      </Badge>
                      <Badge variant={crawler.supports_js_render ? "warning" : "outline"}>
                        {crawler.supports_js_render ? t("console.workers.chromium_enabled") : t("console.workers.chromium_disabled")}
                      </Badge>
                      <Badge variant={getCrawlerUpdateVariant(crawler.update_status)}>
                        {formatCrawlerUpdateStatus(crawler.update_status)}
                      </Badge>
                    </div>
                    {crawler.desired_version ? (
                      <div className="mt-1 text-[11px] text-muted-foreground">
                        {t("console.workers.target_version")}: {crawler.desired_version}
                      </div>
                    ) : null}
                  </TableCell>
                  <TableCell className="whitespace-normal align-top">
                    <div className="grid gap-1 text-xs text-muted-foreground">
                      <span className="font-medium text-foreground">{formatCrawlerVersion(crawler.version)}</span>
                      <span>{t("console.workers.available_version")}: {platformVersion}</span>
                    </div>
                  </TableCell>
                  <TableCell className="whitespace-normal align-top">
                    <div className="grid gap-1 text-xs text-muted-foreground">
                      <span>W {crawler.worker_concurrency}</span>
                      <span>JS {crawler.js_render_concurrency}</span>
                      <span>Max {crawler.max_jobs}</span>
                    </div>
                  </TableCell>
                  <TableCell className="align-top text-sm font-medium text-foreground">{crawler.in_flight_jobs}</TableCell>
                  <TableCell className="align-top text-sm font-medium text-foreground">{crawler.jobs_claimed}</TableCell>
                  <TableCell className="align-top text-sm font-medium text-foreground">{crawler.jobs_reported}</TableCell>
                  <TableCell className="whitespace-normal align-top">
                    <div className="grid gap-1 text-xs text-muted-foreground">
                      <span className="font-medium text-foreground">{formatTimestamp(crawler.last_seen_at)}</span>
                      <span>{t("console.workers.last_claimed")}: {formatTimestamp(crawler.last_claimed_at)}</span>
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      ) : (
        <div className="rounded-lg border border-dashed border-border bg-muted/40 px-4 py-6 text-center text-sm text-muted-foreground">
          {t("console.workers.no_workers")}
        </div>
      )}

      <DetailDialog
        open={Boolean(selectedCrawler)}
        title={selectedCrawler?.name ?? t("console.workers.title")}
        meta={selectedCrawler?.id}
        closeLabel={t("console.actions.close")}
        onClose={() => {
          setEditingId(null);
          setEditName("");
          setDeleteCrawlerId(null);
          setSelectedCrawlerId(null);
          setRuntimeWorkerConcurrency("");
          setRuntimeJsRenderConcurrency("");
          setRuntimeMaxJobs("");
          setSortOrder("");
        }}
        actions={
          selectedCrawler ? (
            <>
              <Button
                variant="ghost"
                size="sm"
                onClick={() =>
                  startEditing(selectedCrawler.id, selectedCrawler.name)
                }
              >
                {t("console.workers.rename")}
              </Button>
              <Button
                variant="destructive"
                size="sm"
                disabled={busy || !selectedCrawler.can_delete}
                onClick={() => setDeleteCrawlerId(selectedCrawler.id)}
              >
                <Trash2 data-icon="inline-start" />
                {t("console.workers.delete")}
              </Button>
            </>
          ) : null
        }
      >
        {selectedCrawler ? (
          <div className="space-y-4">
            {editingId === selectedCrawler.id ? (
              <div className="grid gap-3 rounded-lg border border-border bg-muted/40 p-3 sm:grid-cols-[minmax(0,1fr)_auto_auto] sm:items-end">
                <div className="grid gap-2">
                  <span className="text-sm font-medium text-foreground">
                    {t("console.workers.name_placeholder")}
                  </span>
                  <Input
                    value={editName}
                    onChange={(event) => setEditName(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter")
                        void handleSaveName(selectedCrawler.id);
                      if (event.key === "Escape") setEditingId(null);
                    }}
                    autoFocus
                  />
                </div>
                <Button
                  size="sm"
                  disabled={busy}
                  onClick={() => void handleSaveName(selectedCrawler.id)}
                >
                  {t("console.workers.save")}
                </Button>
                <Button variant="outline" size="sm" onClick={() => setEditingId(null)}>
                  {t("console.workers.cancel")}
                </Button>
              </div>
            ) : null}

            <div className="grid gap-3 rounded-lg border border-border bg-muted/30 p-3 md:grid-cols-2 xl:grid-cols-4">
              <div className="space-y-1">
                <span className="text-[11px] uppercase tracking-[0.12em] text-muted-foreground">{t("console.workers.preview")}</span>
                <p className="break-all text-sm font-medium text-foreground">{selectedCrawler.preview}</p>
              </div>
              <div className="space-y-1">
                <span className="text-[11px] uppercase tracking-[0.12em] text-muted-foreground">{t("console.workers.status")}</span>
                <div className="flex flex-wrap gap-1.5">
                  <Badge variant={selectedCrawler.online ? "success" : "outline"}>
                    {selectedCrawler.online ? t("console.workers.online") : t("console.workers.offline")}
                  </Badge>
                  <Badge variant={selectedCrawler.supports_js_render ? "warning" : "outline"}>
                    {selectedCrawler.supports_js_render ? t("console.workers.chromium_enabled") : t("console.workers.chromium_disabled")}
                  </Badge>
                </div>
              </div>
              <div className="space-y-1">
                <span className="text-[11px] uppercase tracking-[0.12em] text-muted-foreground">{t("console.workers.last_seen")}</span>
                <p className="text-sm font-medium text-foreground">{formatTimestamp(selectedCrawler.last_seen_at)}</p>
              </div>
              <div className="space-y-1">
                <span className="text-[11px] uppercase tracking-[0.12em] text-muted-foreground">{t("console.workers.last_claimed")}</span>
                <p className="text-sm font-medium text-foreground">{formatTimestamp(selectedCrawler.last_claimed_at)}</p>
              </div>
            </div>

            <StatStrip
              compact
              className="xl:grid-cols-6 2xl:grid-cols-7"
              items={[
                { label: t("console.workers.in_flight_jobs"), value: selectedCrawler.in_flight_jobs },
                { label: t("console.workers.jobs_claimed"), value: selectedCrawler.jobs_claimed },
                { label: t("console.workers.jobs_reported"), value: selectedCrawler.jobs_reported },
                { label: t("console.workers.worker_concurrency_label"), value: selectedCrawler.worker_concurrency },
                { label: t("console.workers.js_render_concurrency_label"), value: selectedCrawler.js_render_concurrency },
                { label: t("console.workers.max_jobs_label"), value: selectedCrawler.max_jobs },
                { label: t("console.workers.created"), value: formatTimestamp(selectedCrawler.created_at) },
              ]}
            />

            <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-5">
              <div className="rounded-lg border border-border bg-card p-3 shadow-none">
                <span className="text-[11px] uppercase tracking-[0.12em] text-muted-foreground">{t("console.workers.current_version")}</span>
                <div className="mt-1.5 text-sm font-semibold text-foreground">
                  {selectedCrawler.version ??
                    t("console.workers.version_unknown")}
                </div>
              </div>
              <div className="rounded-lg border border-border bg-card p-3 shadow-none">
                <span className="text-[11px] uppercase tracking-[0.12em] text-muted-foreground">{t("console.workers.target_version")}</span>
                <div className="mt-1.5 text-sm font-semibold text-foreground">
                  {selectedCrawler.desired_version ?? "-"}
                </div>
              </div>
              <div className="rounded-lg border border-border bg-card p-3 shadow-none">
                <span className="text-[11px] uppercase tracking-[0.12em] text-muted-foreground">{t("console.workers.platform_label")}</span>
                <div className="mt-1.5 text-sm font-semibold text-foreground">
                  {selectedCrawler.platform ??
                    t("console.workers.platform_unknown")}
                </div>
              </div>
              <div className="rounded-lg border border-border bg-card p-3 shadow-none">
                <span className="text-[11px] uppercase tracking-[0.12em] text-muted-foreground">{t("console.workers.update_status_label")}</span>
                <div className="mt-1.5">
                  <Badge
                    variant={getCrawlerUpdateVariant(
                      selectedCrawler.update_status,
                    )}
                  >
                    {formatCrawlerUpdateStatus(selectedCrawler.update_status)}
                  </Badge>
                </div>
              </div>
              <div className="rounded-lg border border-border bg-card p-3 shadow-none">
                <span className="text-[11px] uppercase tracking-[0.12em] text-muted-foreground">{t("console.workers.available_version")}</span>
                <div className="mt-1.5 text-sm font-semibold text-foreground">
                  {platformVersion}
                </div>
              </div>
            </div>

            <div className="grid gap-3 rounded-lg border border-border bg-card p-3 shadow-none">
              <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
                <div>
                  <div className="text-sm font-semibold text-foreground">
                    {t("console.workers.remote_update_title")}
                  </div>
                </div>
                <Button
                  size="sm"
                  disabled={
                    busy ||
                    platformVersion === "-" ||
                    selectedCrawlerUpdateQueued ||
                    selectedCrawlerUpToDate
                  }
                  onClick={() => void handleRequestUpdate(selectedCrawler.id)}
                >
                  <HardDriveDownload data-icon="inline-start" />
                  {selectedCrawlerUpToDate
                    ? t("console.workers.up_to_date")
                    : selectedCrawlerUpdateQueued
                      ? t("console.workers.update_queued")
                      : t("console.workers.update_to", {
                          version: platformVersion,
                        })}
                </Button>
              </div>
              {selectedCrawler.update_message ? (
                <div className="rounded-md border border-border bg-muted/40 px-3 py-2 text-sm text-muted-foreground">
                  {selectedCrawler.update_message}
                </div>
              ) : null}
            </div>

            <div className="grid gap-3 rounded-lg border border-border bg-card p-3 shadow-none">
              <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
                <div>
                  <div className="text-sm font-semibold text-foreground">
                    {t("console.workers.sort_order_title")}
                  </div>
                </div>
                <Badge variant="outline">
                  {selectedCrawler.sort_order === null
                    ? t("console.workers.sort_order_default")
                    : t("console.workers.sort_order_value", {
                        value: selectedCrawler.sort_order,
                      })}
                </Badge>
              </div>
              <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_auto] md:items-end">
                <FieldShell
                  label={t("console.workers.sort_order_label")}
                  hint={t("console.workers.sort_order_field_hint")}
                >
                  <Input
                    type="number"
                    step={1}
                    value={sortOrder}
                    placeholder={t("console.workers.sort_order_placeholder")}
                    onChange={(event) => setSortOrder(event.target.value)}
                  />
                </FieldShell>
                <Button
                  size="sm"
                  disabled={busy || !sortOrderDirty}
                  onClick={() => void handleSaveSortOrder(selectedCrawler.id)}
                >
                  {t("console.workers.save")}
                </Button>
              </div>
            </div>

            <div className="grid gap-3 rounded-lg border border-border bg-card p-3 shadow-none">
              <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
                <div>
                  <div className="text-sm font-semibold text-foreground">
                    {t("console.workers.runtime_title")}
                  </div>
                </div>
                <Badge
                  variant={
                    selectedCrawler.supports_js_render ? "warning" : "outline"
                  }
                >
                  {selectedCrawler.supports_js_render
                    ? t("console.workers.chromium_enabled")
                    : t("console.workers.chromium_disabled")}
                </Badge>
              </div>
              <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_minmax(0,1fr)_auto] md:items-end">
                <FieldShell
                  label={t("console.workers.worker_concurrency_label")}
                >
                  <Input
                    type="number"
                    min={1}
                    value={runtimeWorkerConcurrency}
                    onChange={(event) =>
                      setRuntimeWorkerConcurrency(event.target.value)
                    }
                  />
                </FieldShell>
                <FieldShell
                  label={t("console.workers.js_render_concurrency_label")}
                >
                  <Input
                    type="number"
                    min={1}
                    value={runtimeJsRenderConcurrency}
                    onChange={(event) =>
                      setRuntimeJsRenderConcurrency(event.target.value)
                    }
                  />
                </FieldShell>
                <FieldShell label={t("console.workers.max_jobs_label")}>
                  <Input
                    type="number"
                    min={1}
                    value={runtimeMaxJobs}
                    onChange={(event) => setRuntimeMaxJobs(event.target.value)}
                  />
                </FieldShell>
                <Button
                  size="sm"
                  disabled={busy || !runtimeDirty}
                  onClick={() => void handleSaveRuntime(selectedCrawler.id)}
                >
                  {t("console.workers.save")}
                </Button>
              </div>
            </div>
          </div>
        ) : null}
      </DetailDialog>
      <AlertDialog
        open={Boolean(deleteCrawlerTarget)}
        onOpenChange={(open) => {
          if (!open) {
            setDeleteCrawlerId(null);
          }
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("console.workers.delete")}</AlertDialogTitle>
            <AlertDialogDescription>
              {deleteCrawlerTarget
                ? t("console.workers.delete_confirm", {
                    name: deleteCrawlerTarget.name,
                  })
                : ""}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={busy}>
              {t("console.workers.cancel")}
            </AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              disabled={busy || !deleteCrawlerTarget}
              onClick={() =>
                deleteCrawlerTarget &&
                void handleDeleteCrawler(deleteCrawlerTarget.id)
              }
            >
              {t("console.workers.delete")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </PanelSection>
  );
}

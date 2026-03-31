import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Activity, Bot, Clock3, Trash2 } from "lucide-react";

import { deleteCrawler, renameCrawler, updateCrawlerRuntime } from "../../api";
import { DetailDialog, FieldShell, PanelSection, StatStrip } from "../common/PanelPrimitives";
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
import { useConsole } from "./ConsoleContext";

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

const ONLINE_THRESHOLD_MS = 90 * 1000;

function isWorkerOnline(lastSeenAt: string | null): boolean {
  if (!lastSeenAt) return false;
  return Date.now() - new Date(lastSeenAt).getTime() < ONLINE_THRESHOLD_MS;
}

function formatTimestamp(value: string | null) {
  return value ? value.replace("T", " ").replace("Z", "").slice(0, 16) : "-";
}

export function ConsoleWorkers() {
  const { token, busy, setBusy, setFlash, refreshAll, overview } = useConsole();
  const { t } = useTranslation();
  const crawlers = useMemo(
    () =>
      [...(overview?.crawlers ?? [])].sort((left, right) => {
        const leftOnline = isWorkerOnline(left.last_seen_at);
        const rightOnline = isWorkerOnline(right.last_seen_at);
        if (leftOnline !== rightOnline) return Number(rightOnline) - Number(leftOnline);
        const leftSeen = left.last_seen_at ? new Date(left.last_seen_at).getTime() : 0;
        const rightSeen = right.last_seen_at ? new Date(right.last_seen_at).getTime() : 0;
        if (leftSeen !== rightSeen) return rightSeen - leftSeen;
        return new Date(right.created_at).getTime() - new Date(left.created_at).getTime();
      }),
    [overview?.crawlers],
  );
  const onlineWorkers = crawlers.filter((crawler) => isWorkerOnline(crawler.last_seen_at)).length;
  const totalClaimed = crawlers.reduce((sum, crawler) => sum + crawler.jobs_claimed, 0);
  const totalReported = crawlers.reduce((sum, crawler) => sum + crawler.jobs_reported, 0);

  const [editingId, setEditingId] = useState<string | null>(null);
  const [editName, setEditName] = useState("");
  const [selectedCrawlerId, setSelectedCrawlerId] = useState<string | null>(null);
  const [deleteCrawlerId, setDeleteCrawlerId] = useState<string | null>(null);
  const [runtimeWorkerConcurrency, setRuntimeWorkerConcurrency] = useState("");
  const [runtimeJsRenderConcurrency, setRuntimeJsRenderConcurrency] = useState("");
  const selectedCrawler = crawlers.find((crawler) => crawler.id === selectedCrawlerId) ?? null;
  const deleteCrawlerTarget = crawlers.find((crawler) => crawler.id === deleteCrawlerId) ?? null;
  const nextRuntimeWorkerConcurrency = String(
    Math.max(1, Number(runtimeWorkerConcurrency) || selectedCrawler?.worker_concurrency || 1),
  );
  const nextRuntimeJsRenderConcurrency = String(
    Math.max(1, Number(runtimeJsRenderConcurrency) || selectedCrawler?.js_render_concurrency || 1),
  );
  const runtimeDirty = selectedCrawler
    ? runtimeWorkerConcurrency !== String(selectedCrawler.worker_concurrency) ||
      runtimeJsRenderConcurrency !== String(selectedCrawler.js_render_concurrency)
    : false;

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
      );
      setRuntimeWorkerConcurrency(nextRuntimeWorkerConcurrency);
      setRuntimeJsRenderConcurrency(nextRuntimeJsRenderConcurrency);
      await refreshAll();
      setFlash(t("console.workers.runtime_saved"));
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.workers.runtime_save_failed")));
    } finally {
      setBusy(false);
    }
  }

  return (
    <PanelSection
      title={t("console.workers.title")}
      meta={t("console.workers.registered", { count: crawlers.length })}
      contentClassName="space-y-5"
    >
      <p className="text-sm text-muted-foreground">{t("console.workers.setup_hint")}</p>
      <StatStrip
        className="xl:grid-cols-5"
        items={[
          { label: t("console.overview.workers"), value: crawlers.length },
          { label: t("console.workers.online_count"), value: onlineWorkers },
          { label: t("console.workers.jobs_claimed"), value: totalClaimed },
          { label: t("console.workers.jobs_reported"), value: totalReported },
          { label: t("console.overview.in_flight"), value: overview?.in_flight_jobs ?? 0 },
        ]}
      />

      <div className="grid gap-3">
        {crawlers.length ? (
          crawlers.map((crawler) => {
            const online = isWorkerOnline(crawler.last_seen_at);
            return (
              <button
                key={crawler.id}
                type="button"
                onClick={() => {
                  setEditingId(null);
                  setEditName(crawler.name);
                  setSelectedCrawlerId(crawler.id);
                  setRuntimeWorkerConcurrency(String(crawler.worker_concurrency));
                  setRuntimeJsRenderConcurrency(String(crawler.js_render_concurrency));
                }}
                className="grid w-full gap-4 rounded-2xl border border-border bg-card p-4 text-left shadow-sm transition-colors hover:bg-muted/40"
              >
                <div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
                  <div className="space-y-2">
                    <div className="flex flex-wrap items-center gap-2">
                      <h3 className="text-base font-semibold text-foreground">{crawler.name}</h3>
                      <Badge variant={online ? "success" : "outline"}>
                        {online ? t("console.workers.online") : t("console.workers.offline")}
                      </Badge>
                      <Badge variant={crawler.supports_js_render ? "warning" : "outline"}>
                        {crawler.supports_js_render
                          ? t("console.workers.chromium_enabled")
                          : t("console.workers.chromium_disabled")}
                      </Badge>
                      <Badge variant="outline">{crawler.id.slice(0, 8)}</Badge>
                    </div>
                    <p className="text-sm text-muted-foreground">{crawler.preview}</p>
                  </div>
                  <div className="grid gap-1 text-sm text-muted-foreground md:text-right">
                    <span>{t("console.workers.last_seen")}</span>
                    <strong className="text-foreground">{formatTimestamp(crawler.last_seen_at)}</strong>
                  </div>
                </div>
                <div className="grid gap-3 sm:grid-cols-5">
                  <div className="rounded-xl border border-border bg-muted/40 px-3 py-2">
                    <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.workers.jobs_claimed")}</span>
                    <div className="mt-2 text-lg font-semibold text-foreground">{crawler.jobs_claimed}</div>
                  </div>
                  <div className="rounded-xl border border-border bg-muted/40 px-3 py-2">
                    <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.workers.jobs_reported")}</span>
                    <div className="mt-2 text-lg font-semibold text-foreground">{crawler.jobs_reported}</div>
                  </div>
                  <div className="rounded-xl border border-border bg-muted/40 px-3 py-2">
                    <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.workers.worker_concurrency_label")}</span>
                    <div className="mt-2 text-lg font-semibold text-foreground">{crawler.worker_concurrency}</div>
                  </div>
                  <div className="rounded-xl border border-border bg-muted/40 px-3 py-2">
                    <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.workers.js_render_concurrency_label")}</span>
                    <div className="mt-2 text-lg font-semibold text-foreground">{crawler.js_render_concurrency}</div>
                  </div>
                  <div className="rounded-xl border border-border bg-muted/40 px-3 py-2">
                    <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.workers.created")}</span>
                    <div className="mt-2 text-lg font-semibold text-foreground">{formatTimestamp(crawler.created_at)}</div>
                  </div>
                </div>
              </button>
            );
          })
        ) : (
          <div className="rounded-2xl border border-dashed border-border bg-muted/40 px-4 py-8 text-center text-sm text-muted-foreground">
            {t("console.workers.no_workers")}
          </div>
        )}
      </div>

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
        }}
        actions={
          selectedCrawler ? (
            <>
              <Button variant="ghost" onClick={() => startEditing(selectedCrawler.id, selectedCrawler.name)}>
                {t("console.workers.rename")}
              </Button>
              {!isWorkerOnline(selectedCrawler.last_seen_at) ? (
                <Button
                  variant="destructive"
                  disabled={busy}
                  onClick={() => setDeleteCrawlerId(selectedCrawler.id)}
                >
                  <Trash2 data-icon="inline-start" />
                  {t("console.workers.delete")}
                </Button>
              ) : null}
            </>
          ) : null
        }
      >
        {selectedCrawler ? (
          <div className="space-y-5">
            {editingId === selectedCrawler.id ? (
              <div className="grid gap-3 rounded-2xl border border-border bg-muted/40 p-4 sm:grid-cols-[1fr_auto_auto] sm:items-end">
                <div className="grid gap-2">
                  <span className="text-sm font-medium text-foreground">{t("console.workers.name_placeholder")}</span>
                  <Input
                    value={editName}
                    onChange={(event) => setEditName(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") void handleSaveName(selectedCrawler.id);
                      if (event.key === "Escape") setEditingId(null);
                    }}
                    autoFocus
                  />
                </div>
                <Button disabled={busy} onClick={() => void handleSaveName(selectedCrawler.id)}>
                  {t("console.workers.save")}
                </Button>
                <Button variant="outline" onClick={() => setEditingId(null)}>
                  {t("console.workers.cancel")}
                </Button>
              </div>
            ) : null}

            <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
              <div className="rounded-xl border border-border bg-muted/40 p-4">
                <div className="flex items-center gap-2 text-muted-foreground"><Bot className="size-4" />{t("console.workers.preview")}</div>
                <div className="mt-2 break-all font-medium text-foreground">{selectedCrawler.preview}</div>
              </div>
              <div className="rounded-xl border border-border bg-muted/40 p-4">
                <div className="flex items-center gap-2 text-muted-foreground"><Activity className="size-4" />{t("console.workers.status")}</div>
                <div className="mt-2">
                  <Badge variant={isWorkerOnline(selectedCrawler.last_seen_at) ? "success" : "outline"}>
                    {isWorkerOnline(selectedCrawler.last_seen_at)
                      ? t("console.workers.online")
                      : t("console.workers.offline")}
                  </Badge>
                </div>
              </div>
              <div className="rounded-xl border border-border bg-muted/40 p-4">
                <div className="flex items-center gap-2 text-muted-foreground"><Clock3 className="size-4" />{t("console.workers.last_seen")}</div>
                <div className="mt-2 font-medium text-foreground">{formatTimestamp(selectedCrawler.last_seen_at)}</div>
              </div>
              <div className="rounded-xl border border-border bg-muted/40 p-4">
                <div className="flex items-center gap-2 text-muted-foreground"><Clock3 className="size-4" />{t("console.workers.last_claimed")}</div>
                <div className="mt-2 font-medium text-foreground">{formatTimestamp(selectedCrawler.last_claimed_at)}</div>
              </div>
            </div>

            <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
              <div className="rounded-xl border border-border bg-card p-4 shadow-sm">
                <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.workers.jobs_claimed")}</span>
                <div className="mt-2 text-2xl font-semibold text-foreground">{selectedCrawler.jobs_claimed}</div>
              </div>
              <div className="rounded-xl border border-border bg-card p-4 shadow-sm">
                <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.workers.jobs_reported")}</span>
                <div className="mt-2 text-2xl font-semibold text-foreground">{selectedCrawler.jobs_reported}</div>
              </div>
              <div className="rounded-xl border border-border bg-card p-4 shadow-sm">
                <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.workers.created")}</span>
                <div className="mt-2 text-lg font-semibold text-foreground">{formatTimestamp(selectedCrawler.created_at)}</div>
              </div>
              <div className="rounded-xl border border-border bg-card p-4 shadow-sm">
                <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.workers.id")}</span>
                <div className="mt-2 break-all text-sm font-medium text-foreground">{selectedCrawler.id}</div>
              </div>
            </div>

            <div className="grid gap-4 rounded-2xl border border-border bg-card p-4 shadow-sm">
              <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
                <div>
                  <div className="text-sm font-semibold text-foreground">{t("console.workers.runtime_title")}</div>
                  <p className="mt-1 text-sm text-muted-foreground">{t("console.workers.runtime_hint")}</p>
                </div>
                <Badge variant={selectedCrawler.supports_js_render ? "warning" : "outline"}>
                  {selectedCrawler.supports_js_render
                    ? t("console.workers.chromium_enabled")
                    : t("console.workers.chromium_disabled")}
                </Badge>
              </div>
              <div className="grid gap-3 md:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] md:items-end">
                <FieldShell label={t("console.workers.worker_concurrency_label")}>
                  <Input
                    type="number"
                    min={1}
                    value={runtimeWorkerConcurrency}
                    onChange={(event) => setRuntimeWorkerConcurrency(event.target.value)}
                  />
                </FieldShell>
                <FieldShell label={t("console.workers.js_render_concurrency_label")}>
                  <Input
                    type="number"
                    min={1}
                    value={runtimeJsRenderConcurrency}
                    onChange={(event) => setRuntimeJsRenderConcurrency(event.target.value)}
                  />
                </FieldShell>
                <Button
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
              {deleteCrawlerTarget ? t("console.workers.delete_confirm", { name: deleteCrawlerTarget.name }) : ""}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={busy}>{t("console.workers.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              disabled={busy || !deleteCrawlerTarget}
              onClick={() => deleteCrawlerTarget && void handleDeleteCrawler(deleteCrawlerTarget.id)}
            >
              {t("console.workers.delete")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </PanelSection>
  );
}

import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { deleteCrawler, renameCrawler } from "../../api";
import { DetailDialog, SectionHeader, StatStrip } from "../common/PanelPrimitives";
import { useConsole } from "./ConsoleContext";

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

const ONLINE_THRESHOLD_MS = 5 * 60 * 1000;

function isWorkerOnline(lastSeenAt: string | null): boolean {
  if (!lastSeenAt) return false;
  const lastSeen = new Date(lastSeenAt).getTime();
  return Date.now() - lastSeen < ONLINE_THRESHOLD_MS;
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
        if (leftOnline !== rightOnline) {
          return Number(rightOnline) - Number(leftOnline);
        }
        const leftSeen = left.last_seen_at ? new Date(left.last_seen_at).getTime() : 0;
        const rightSeen = right.last_seen_at ? new Date(right.last_seen_at).getTime() : 0;
        if (leftSeen !== rightSeen) {
          return rightSeen - leftSeen;
        }
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

  const selectedCrawler = crawlers.find((crawler) => crawler.id === selectedCrawlerId) ?? null;

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

  async function handleDeleteCrawler(crawlerId: string, crawlerName: string) {
    if (!window.confirm(t("console.workers.delete_confirm", { name: crawlerName }))) {
      return;
    }
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

  return (
    <section className="panel panel-wide compact-panel">
      <SectionHeader title={t("console.workers.title")} meta={t("console.workers.registered", { count: crawlers.length })} />
      <p className="dev-hint">{t("console.workers.setup_hint")}</p>
      <StatStrip
        className="worker-density-grid"
        items={[
          { label: t("console.overview.workers"), value: crawlers.length },
          { label: t("console.workers.online_count"), value: onlineWorkers },
          { label: t("console.workers.jobs_claimed"), value: totalClaimed },
          { label: t("console.workers.jobs_reported"), value: totalReported },
          { label: t("console.overview.in_flight"), value: overview?.in_flight_jobs ?? 0 },
        ]}
      />
      <div className="dense-list">
        {crawlers.length ? (
          crawlers.map((crawler) => {
            const online = isWorkerOnline(crawler.last_seen_at);
            return (
              <div className="compact-row worker-row worker-card" key={crawler.id}>
                <div className="worker-card-head">
                  <div className="row-primary">
                    <div className="row-meta row-meta-tight">
                      <strong>{crawler.name}</strong>
                      <span className={online ? "status-pill" : "status-pill status-pill-muted"}>
                        {online ? t("console.workers.online") : t("console.workers.offline")}
                      </span>
                      <code>{crawler.id.slice(0, 8)}</code>
                    </div>
                    <span>{crawler.preview}</span>
                  </div>
                  <button
                    type="button"
                    className="plain-link"
                    onClick={() => {
                      setEditingId(null);
                      setEditName(crawler.name);
                      setSelectedCrawlerId(crawler.id);
                    }}
                  >
                    {t("console.actions.details")}
                  </button>
                </div>
                <div className="worker-card-stats">
                  <div>
                    <span>{t("console.workers.last_seen")}</span>
                    <strong>{formatTimestamp(crawler.last_seen_at)}</strong>
                  </div>
                  <div>
                    <span>{t("console.workers.jobs_claimed")}</span>
                    <strong>{crawler.jobs_claimed}</strong>
                  </div>
                  <div>
                    <span>{t("console.workers.jobs_reported")}</span>
                    <strong>{crawler.jobs_reported}</strong>
                  </div>
                </div>
              </div>
            );
          })
        ) : (
          <div className="list-row">{t("console.workers.no_workers")}</div>
        )}
      </div>

      <DetailDialog
        open={Boolean(selectedCrawler)}
        title={selectedCrawler?.name ?? t("console.workers.title")}
        meta={selectedCrawler ? t("console.workers.id") : undefined}
        closeLabel={t("console.actions.close")}
        onClose={() => {
          setEditingId(null);
          setEditName("");
          setSelectedCrawlerId(null);
        }}
        actions={
          selectedCrawler ? (
            <button
              type="button"
              className="plain-link"
              disabled={busy}
              onClick={() => startEditing(selectedCrawler.id, selectedCrawler.name)}
            >
              {t("console.workers.rename")}
            </button>
          ) : null
        }
      >
        {selectedCrawler ? (
          <div className="detail-stack">
            {editingId === selectedCrawler.id ? (
              <div className="inline-form form-fields">
                <label className="field-group compact-field field-group-wide">
                  <span className="field-label">{t("console.workers.name_placeholder")}</span>
                  <input
                    value={editName}
                    onChange={(event) => setEditName(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") void handleSaveName(selectedCrawler.id);
                      if (event.key === "Escape") setEditingId(null);
                    }}
                    autoFocus
                  />
                </label>
                <button type="button" disabled={busy} onClick={() => void handleSaveName(selectedCrawler.id)}>
                  {t("console.workers.save")}
                </button>
                <button type="button" className="plain-link" onClick={() => setEditingId(null)}>
                  {t("console.workers.cancel")}
                </button>
              </div>
            ) : null}
            <div className="metadata-grid compact-metadata-wide detail-grid">
              <div>
                <span>{t("console.workers.id")}</span>
                <strong>{selectedCrawler.id}</strong>
              </div>
              <div>
                <span>{t("console.workers.preview")}</span>
                <strong>{selectedCrawler.preview}</strong>
              </div>
              <div>
                <span>{t("console.workers.created")}</span>
                <strong>{formatTimestamp(selectedCrawler.created_at)}</strong>
              </div>
              <div>
                <span>{t("console.workers.last_seen")}</span>
                <strong>{formatTimestamp(selectedCrawler.last_seen_at)}</strong>
              </div>
              <div>
                <span>{t("console.workers.last_claimed")}</span>
                <strong>{formatTimestamp(selectedCrawler.last_claimed_at)}</strong>
              </div>
              <div>
                <span>{t("console.workers.jobs_claimed")}</span>
                <strong>{selectedCrawler.jobs_claimed}</strong>
              </div>
              <div>
                <span>{t("console.workers.jobs_reported")}</span>
                <strong>{selectedCrawler.jobs_reported}</strong>
              </div>
              <div>
                <span>{t("console.workers.status")}</span>
                <strong>
                  {isWorkerOnline(selectedCrawler.last_seen_at)
                    ? t("console.workers.online")
                    : t("console.workers.offline")}
                </strong>
              </div>
            </div>
            {!isWorkerOnline(selectedCrawler.last_seen_at) ? (
              <div className="detail-actions">
                <button
                  type="button"
                  className="danger-button"
                  disabled={busy}
                  onClick={() => void handleDeleteCrawler(selectedCrawler.id, selectedCrawler.name)}
                >
                  {t("console.workers.delete")}
                </button>
              </div>
            ) : null}
          </div>
        ) : null}
      </DetailDialog>
    </section>
  );
}

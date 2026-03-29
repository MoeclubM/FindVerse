import { useState } from "react";
import { useTranslation } from "react-i18next";

import { deleteCrawler, renameCrawler } from "../../api";
import { SectionHeader, StatStrip } from "../common/PanelPrimitives";
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

export function ConsoleWorkers() {
  const { token, busy, setBusy, setFlash, refreshAll, overview } = useConsole();
  const { t } = useTranslation();
  const crawlers = [...(overview?.crawlers ?? [])]
    .sort((left, right) => {
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
    });
  const onlineWorkers = crawlers.filter((crawler) => isWorkerOnline(crawler.last_seen_at)).length;
  const totalClaimed = crawlers.reduce((sum, crawler) => sum + crawler.jobs_claimed, 0);
  const totalReported = crawlers.reduce((sum, crawler) => sum + crawler.jobs_reported, 0);

  const [editingId, setEditingId] = useState<string | null>(null);
  const [editName, setEditName] = useState("");

  function startEditing(crawlerId: string, currentName: string) {
    setEditingId(crawlerId);
    setEditName(currentName);
  }

  function cancelEditing() {
    setEditingId(null);
    setEditName("");
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
      setEditName("");
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
      <p className="dev-hint">
        {t("console.workers.setup_hint")}
      </p>
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
            const isEditing = editingId === crawler.id;
            return (
              <div className="compact-row worker-row" key={crawler.id}>
                <div className="row-primary">
                  <div className="row-meta">
                    {isEditing ? (
                      <div className="inline-form">
                        <input
                          value={editName}
                          onChange={(event) => setEditName(event.target.value)}
                          placeholder={t("console.workers.name_placeholder")}
                          onKeyDown={(event) => {
                            if (event.key === "Enter") void handleSaveName(crawler.id);
                            if (event.key === "Escape") cancelEditing();
                          }}
                          autoFocus
                        />
                        <button
                          type="button"
                          disabled={busy}
                          onClick={() => void handleSaveName(crawler.id)}
                        >
                          {t("console.workers.save")}
                        </button>
                        <button
                          type="button"
                          className="plain-link"
                          onClick={cancelEditing}
                        >
                          {t("console.workers.cancel")}
                        </button>
                      </div>
                    ) : (
                      <>
                        <strong>{crawler.name}</strong>
                        <button
                          type="button"
                          className="plain-link"
                          disabled={busy}
                          onClick={() => startEditing(crawler.id, crawler.name)}
                        >
                          {t("console.workers.rename")}
                        </button>
                        {!online ? (
                          <button
                            type="button"
                            className="plain-link"
                            disabled={busy}
                            onClick={() => void handleDeleteCrawler(crawler.id, crawler.name)}
                          >
                            {t("console.workers.delete")}
                          </button>
                        ) : null}
                      </>
                    )}
                    <span className={online ? "status-pill" : "status-pill status-pill-muted"}>
                      {online ? t("console.workers.online") : t("console.workers.offline")}
                    </span>
                  </div>
                  <code>{crawler.id}</code>
                </div>
                <div className="metadata-grid compact-metadata compact-metadata-wide">
                  <div>
                    <span>{t("console.workers.preview")}</span>
                    <strong>{crawler.preview}</strong>
                  </div>
                  <div>
                    <span>{t("console.workers.created")}</span>
                    <strong>{crawler.created_at}</strong>
                  </div>
                  <div>
                    <span>{t("console.workers.last_seen")}</span>
                    <strong>{crawler.last_seen_at ?? "-"}</strong>
                  </div>
                  <div>
                    <span>{t("console.workers.last_claimed")}</span>
                    <strong>{crawler.last_claimed_at ?? "-"}</strong>
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
    </section>
  );
}

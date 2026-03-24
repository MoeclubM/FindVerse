import { useState } from "react";

import { renameCrawler } from "../../api";
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
      setFlash("Crawler name must be at least 2 characters");
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
      setFlash(getErrorMessage(error, "Rename failed"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel panel-wide compact-panel">
      <div className="section-header">
        <h2>Crawler workers</h2>
        <span className="section-meta">{overview?.crawlers.length ?? 0} registered</span>
      </div>
      <p className="dev-hint">
        Workers join with the manual join key from Settings, then continue using their own crawler credentials.
      </p>
      <div className="dense-list">
        {overview?.crawlers.length ? (
          overview.crawlers.map((crawler) => {
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
                          placeholder="Crawler name"
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
                          Save
                        </button>
                        <button
                          type="button"
                          className="plain-link"
                          onClick={cancelEditing}
                        >
                          Cancel
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
                          Rename
                        </button>
                      </>
                    )}
                    <span className={online ? "status-pill" : "status-pill status-pill-muted"}>
                      {online ? "Online" : "Offline"}
                    </span>
                  </div>
                  <span>{crawler.id}</span>
                </div>
                <div className="metadata-grid compact-metadata">
                  <div>
                    <span>Preview</span>
                    <strong>{crawler.preview}</strong>
                  </div>
                  <div>
                    <span>Created</span>
                    <strong>{crawler.created_at}</strong>
                  </div>
                  <div>
                    <span>Last seen</span>
                    <strong>{crawler.last_seen_at ?? "-"}</strong>
                  </div>
                  <div>
                    <span>Last claim</span>
                    <strong>{crawler.last_claimed_at ?? "-"}</strong>
                  </div>
                  <div>
                    <span>Claimed</span>
                    <strong>{crawler.jobs_claimed}</strong>
                  </div>
                  <div>
                    <span>Reported</span>
                    <strong>{crawler.jobs_reported}</strong>
                  </div>
                </div>
              </div>
            );
          })
        ) : (
          <div className="list-row">No crawler workers yet. Set a join key in Settings, then start a worker to enroll it.</div>
        )}
      </div>
    </section>
  );
}

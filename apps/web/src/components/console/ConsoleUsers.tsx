import { FormEvent, useEffect, useState } from "react";

import {
  AdminDeveloperRecord,
  CreatedApiKey,
  DeveloperUsage,
  createAdminDeveloperKey,
  getAdminDeveloperKeys,
  revokeAdminDeveloperKey,
  updateDeveloper,
} from "../../api";
import { useConsole } from "./ConsoleContext";

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

type KeyPanelState = {
  loading: boolean;
  usage: DeveloperUsage | null;
  keyName: string;
  latestKey: CreatedApiKey | null;
};

const DEFAULT_KEY_NAME = "Search key";

export function ConsoleUsers() {
  const { token, busy, setBusy, setFlash, refreshAll, developers } = useConsole();

  const [developerDrafts, setDeveloperDrafts] = useState<Record<string, { daily_limit: string }>>({});
  const [expandedUserId, setExpandedUserId] = useState<string | null>(null);
  const [keyPanels, setKeyPanels] = useState<Record<string, KeyPanelState>>({});

  useEffect(() => {
    setDeveloperDrafts((current) => {
      const next = { ...current };
      for (const developer of developers) {
        next[developer.user_id] ??= {
          daily_limit: String(developer.daily_limit),
        };
      }
      return next;
    });
  }, [developers]);

  useEffect(() => {
    if (!expandedUserId) {
      return;
    }
    if (!developers.some((developer) => developer.user_id === expandedUserId)) {
      setExpandedUserId(null);
    }
  }, [developers, expandedUserId]);

  function setKeyPanelState(userId: string, updater: (current: KeyPanelState) => KeyPanelState) {
    setKeyPanels((current) => {
      const existing = current[userId] ?? {
        loading: false,
        usage: null,
        keyName: DEFAULT_KEY_NAME,
        latestKey: null,
      };
      return {
        ...current,
        [userId]: updater(existing),
      };
    });
  }

  async function loadDeveloperKeys(user: AdminDeveloperRecord, force = false) {
    const panel = keyPanels[user.user_id];
    if (!force && panel?.usage && !panel.loading) {
      return;
    }

    setKeyPanelState(user.user_id, (current) => ({
      ...current,
      loading: true,
      usage: force ? current.usage : current.usage,
    }));

    try {
      const usage = await getAdminDeveloperKeys(token, user.user_id);
      setKeyPanelState(user.user_id, (current) => ({
        ...current,
        loading: false,
        usage,
      }));
    } catch (error) {
      setKeyPanelState(user.user_id, (current) => ({
        ...current,
        loading: false,
      }));
      setFlash(getErrorMessage(error, `Failed to load keys for ${user.username}`));
    }
  }

  async function handleToggleDeveloperEnabled(user: AdminDeveloperRecord) {
    setBusy(true);
    setFlash(null);
    try {
      await updateDeveloper(token, user.user_id, { enabled: !user.enabled });
      await refreshAll();
    } catch (error) {
      setFlash(getErrorMessage(error, "Developer update failed"));
    } finally {
      setBusy(false);
    }
  }

  async function handleSaveDeveloperQuota(userId: string) {
    const draft = developerDrafts[userId];
    if (!draft) {
      return;
    }

    setBusy(true);
    setFlash(null);
    try {
      await updateDeveloper(token, userId, {
        daily_limit: Math.max(1, Number(draft.daily_limit) || 1),
      });
      await refreshAll();
    } catch (error) {
      setFlash(getErrorMessage(error, "Quota update failed"));
    } finally {
      setBusy(false);
    }
  }

  async function handleToggleKeyPanel(user: AdminDeveloperRecord) {
    if (expandedUserId === user.user_id) {
      setExpandedUserId(null);
      return;
    }

    setExpandedUserId(user.user_id);
    setFlash(null);
    await loadDeveloperKeys(user);
  }

  async function handleCreateKey(event: FormEvent<HTMLFormElement>, user: AdminDeveloperRecord) {
    event.preventDefault();
    const panel = keyPanels[user.user_id];
    const name = (panel?.keyName ?? DEFAULT_KEY_NAME).trim() || DEFAULT_KEY_NAME;

    setBusy(true);
    setFlash(null);
    try {
      const created = await createAdminDeveloperKey(token, user.user_id, name);
      setKeyPanelState(user.user_id, (current) => ({
        ...current,
        latestKey: created,
        keyName: DEFAULT_KEY_NAME,
      }));
      await Promise.all([refreshAll(), loadDeveloperKeys(user, true)]);
    } catch (error) {
      setFlash(getErrorMessage(error, `Failed to create key for ${user.username}`));
    } finally {
      setBusy(false);
    }
  }

  async function handleRevokeKey(user: AdminDeveloperRecord, keyId: string) {
    setBusy(true);
    setFlash(null);
    try {
      await revokeAdminDeveloperKey(token, user.user_id, keyId);
      setKeyPanelState(user.user_id, (current) => ({
        ...current,
        latestKey: current.latestKey?.id === keyId ? null : current.latestKey,
      }));
      await Promise.all([refreshAll(), loadDeveloperKeys(user, true)]);
    } catch (error) {
      setFlash(getErrorMessage(error, `Failed to revoke key for ${user.username}`));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel panel-wide compact-panel">
      <div className="section-header">
        <h2>Developer users</h2>
        <span className="section-meta">{developers.length} accounts</span>
      </div>
      <div className="table-head developer-table">
        <span>User</span>
        <span>Status</span>
        <span>Daily</span>
        <span>Used</span>
        <span>Keys</span>
        <span>Created</span>
        <span>Actions</span>
      </div>
      <div className="dense-list">
        {developers.length ? (
          developers.map((developer) => {
            const draft = developerDrafts[developer.user_id] ?? {
              daily_limit: String(developer.daily_limit),
            };
            const isExpanded = expandedUserId === developer.user_id;
            const panel = keyPanels[developer.user_id] ?? {
              loading: false,
              usage: null,
              keyName: DEFAULT_KEY_NAME,
              latestKey: null,
            };
            const keyTotal = panel.usage?.keys.length ?? developer.key_count;
            return (
              <div key={developer.user_id} className="developer-card-stack">
                <div className="table-row developer-table">
                  <div className="cell cell-primary">
                    <strong>{developer.username}</strong>
                    <span>{developer.user_id}</span>
                  </div>
                  <div className="cell">
                    <span className={developer.enabled ? "status-pill" : "status-pill status-pill-muted"}>
                      {developer.enabled ? "Enabled" : "Disabled"}
                    </span>
                  </div>
                  <div className="cell">
                    <input
                      aria-label={`Daily quota for ${developer.username}`}
                      value={draft.daily_limit}
                      onChange={(event) =>
                        setDeveloperDrafts((current) => ({
                          ...current,
                          [developer.user_id]: {
                            ...draft,
                            daily_limit: event.target.value,
                          },
                        }))
                      }
                      placeholder="Daily quota"
                    />
                  </div>
                  <div className="cell">
                    <strong>{panel.usage?.used_today ?? developer.used_today}</strong>
                  </div>
                  <div className="cell">
                    <strong>{keyTotal}</strong>
                  </div>
                  <div className="cell">
                    <span>{developer.created_at}</span>
                  </div>
                  <div className="cell cell-actions">
                    <button
                      type="button"
                      disabled={busy}
                      onClick={() => void handleSaveDeveloperQuota(developer.user_id)}
                    >
                      Save
                    </button>
                    <button
                      type="button"
                      className="plain-link"
                      disabled={busy}
                      onClick={() => void handleToggleDeveloperEnabled(developer)}
                    >
                      {developer.enabled ? "Disable" : "Enable"}
                    </button>
                    <button
                      type="button"
                      className="plain-link"
                      disabled={busy || panel.loading}
                      onClick={() => void handleToggleKeyPanel(developer)}
                    >
                      {isExpanded ? "Hide keys" : "Manage keys"}
                    </button>
                  </div>
                </div>
                {isExpanded ? (
                  <div className="developer-key-panel">
                    <div className="section-header developer-key-panel-header">
                      <div>
                        <h3>{developer.username} API keys</h3>
                        <span className="section-meta">Create a key here once, then copy it before leaving this panel.</span>
                      </div>
                      <div className="row-actions">
                        <span className="section-meta">{keyTotal} total</span>
                        <button
                          type="button"
                          className="plain-link"
                          disabled={busy || panel.loading}
                          onClick={() => void loadDeveloperKeys(developer, true)}
                        >
                          Refresh keys
                        </button>
                      </div>
                    </div>

                    {panel.latestKey ? (
                      <div className="flash inline-flash">
                        New key for {developer.username}: <code>{panel.latestKey.token}</code>
                      </div>
                    ) : null}

                    <form className="inline-form developer-key-form" onSubmit={(event) => void handleCreateKey(event, developer)}>
                      <input
                        aria-label={`New key name for ${developer.username}`}
                        value={panel.keyName}
                        onChange={(event) =>
                          setKeyPanelState(developer.user_id, (current) => ({
                            ...current,
                            keyName: event.target.value,
                          }))
                        }
                        placeholder="Search key"
                      />
                      <button type="submit" disabled={busy || panel.loading}>
                        Create key
                      </button>
                    </form>

                    <div className="developer-key-summary">
                      <span>Daily limit: <strong>{panel.usage?.daily_limit ?? developer.daily_limit}</strong></span>
                      <span>Used today: <strong>{panel.usage?.used_today ?? developer.used_today}</strong></span>
                    </div>

                    {panel.loading && !panel.usage ? <div className="list-row">Loading keys…</div> : null}

                    {panel.usage?.keys.length ? (
                      <div className="dense-list compact-list">
                        {panel.usage.keys.map((key) => (
                          <div className="list-row developer-key-row" key={key.id}>
                            <div className="row-primary">
                              <strong>{key.name}</strong>
                              <span>{key.preview}</span>
                            </div>
                            <div className="row-meta">
                              <span>Created {key.created_at}</span>
                              <span className={key.revoked_at ? "status-pill status-pill-muted" : "status-pill"}>
                                {key.revoked_at ? `Revoked ${key.revoked_at}` : "Active"}
                              </span>
                            </div>
                            <div className="row-actions">
                              <button
                                type="button"
                                className="plain-link"
                                disabled={busy || panel.loading || Boolean(key.revoked_at)}
                                onClick={() => void handleRevokeKey(developer, key.id)}
                              >
                                Revoke
                              </button>
                            </div>
                          </div>
                        ))}
                      </div>
                    ) : panel.loading ? null : (
                      <div className="list-row">No API keys for this developer yet.</div>
                    )}
                  </div>
                ) : null}
              </div>
            );
          })
        ) : (
          <div className="list-row">No developer accounts yet.</div>
        )}
      </div>
    </section>
  );
}

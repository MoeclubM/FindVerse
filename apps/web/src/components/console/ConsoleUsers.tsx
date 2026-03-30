import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  AdminDeveloperRecord,
  DeveloperUsage,
  deleteDeveloper,
  getAdminDeveloperKeys,
  revokeAdminDeveloperKey,
  updateDeveloper,
} from "../../api";
import { FieldShell, PanelSection, StatStrip } from "../common/PanelPrimitives";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";
import { Card, CardContent } from "../ui/card";
import { Input } from "../ui/input";
import { useConsole } from "./ConsoleContext";

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

type KeyPanelState = {
  loading: boolean;
  usage: DeveloperUsage | null;
};

export function ConsoleUsers() {
  const { token, busy, setBusy, setFlash, refreshAll, developers } = useConsole();
  const { t } = useTranslation();

  const [developerDrafts, setDeveloperDrafts] = useState<Record<string, { daily_limit: string; password: string }>>({});
  const [expandedUserId, setExpandedUserId] = useState<string | null>(null);
  const [keyPanels, setKeyPanels] = useState<Record<string, KeyPanelState>>({});

  useEffect(() => {
    setDeveloperDrafts((current) => {
      const next = { ...current };
      for (const developer of developers) {
        next[developer.user_id] ??= {
          daily_limit: String(developer.daily_limit),
          password: "",
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
    }));

    try {
      const usage = await getAdminDeveloperKeys(token, user.user_id);
      setKeyPanelState(user.user_id, () => ({
        loading: false,
        usage,
      }));
    } catch (error) {
      setKeyPanelState(user.user_id, (current) => ({
        ...current,
        loading: false,
      }));
      setFlash(getErrorMessage(error, t("console.users.load_keys_failed", { username: user.username })));
    }
  }

  async function handleSaveDeveloperQuota(user: AdminDeveloperRecord) {
    const draft = developerDrafts[user.user_id];
    if (!draft) {
      return;
    }

    const nextLimit = Math.max(1, Number(draft.daily_limit) || 1);
    setBusy(true);
    setFlash(null);
    try {
      await updateDeveloper(token, user.user_id, { daily_limit: nextLimit });
      setDeveloperDrafts((current) => ({
        ...current,
        [user.user_id]: {
          ...current[user.user_id],
          daily_limit: String(nextLimit),
        },
      }));
      await refreshAll();
      setFlash(t("console.users.quota_update_success", { username: user.username }));
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.users.quota_update_failed")));
    } finally {
      setBusy(false);
    }
  }

  async function handleSaveDeveloperPassword(user: AdminDeveloperRecord) {
    const password = developerDrafts[user.user_id]?.password.trim() ?? "";
    if (!password) {
      setFlash(t("console.users.password_required"));
      return;
    }

    setBusy(true);
    setFlash(null);
    try {
      await updateDeveloper(token, user.user_id, { password });
      setDeveloperDrafts((current) => ({
        ...current,
        [user.user_id]: {
          ...current[user.user_id],
          password: "",
        },
      }));
      setFlash(t("console.users.password_update_success", { username: user.username }));
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.users.password_update_failed", { username: user.username })));
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

  async function handleDeleteDeveloper(user: AdminDeveloperRecord) {
    if (!window.confirm(t("console.users.delete_confirm", { username: user.username }))) {
      return;
    }

    setBusy(true);
    setFlash(null);
    try {
      await deleteDeveloper(token, user.user_id);
      setDeveloperDrafts((current) => {
        const next = { ...current };
        delete next[user.user_id];
        return next;
      });
      setKeyPanels((current) => {
        const next = { ...current };
        delete next[user.user_id];
        return next;
      });
      if (expandedUserId === user.user_id) {
        setExpandedUserId(null);
      }
      await refreshAll();
      setFlash(t("console.users.delete_success", { username: user.username }));
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.users.delete_failed", { username: user.username })));
    } finally {
      setBusy(false);
    }
  }

  async function handleRevokeKey(user: AdminDeveloperRecord, keyId: string) {
    setBusy(true);
    setFlash(null);
    try {
      await revokeAdminDeveloperKey(token, user.user_id, keyId);
      await Promise.all([refreshAll(), loadDeveloperKeys(user, true)]);
      setFlash(t("console.users.key_delete_success", { username: user.username }));
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.users.revoke_key_failed", { username: user.username })));
    } finally {
      setBusy(false);
    }
  }

  const totalIssuedKeys = developers.reduce((sum, developer) => sum + developer.key_count, 0);
  const totalUsageToday = developers.reduce((sum, developer) => sum + developer.used_today, 0);
  const totalDailyLimit = developers.reduce((sum, developer) => sum + developer.daily_limit, 0);

  return (
    <PanelSection title={t("console.users.title")} meta={t("console.users.accounts", { count: developers.length })} contentClassName="space-y-5">
      <StatStrip
        className="xl:grid-cols-4"
        items={[
          { label: t("console.users.accounts_total"), value: developers.length },
          { label: t("console.users.keys"), value: totalIssuedKeys },
          { label: t("console.users.usage_today"), value: totalUsageToday },
          { label: t("console.users.total_daily_limit"), value: totalDailyLimit },
        ]}
      />
      <div className="grid gap-3">
        {developers.length ? (
          developers.map((developer) => {
            const draft = developerDrafts[developer.user_id] ?? {
              daily_limit: String(developer.daily_limit),
              password: "",
            };
            const isExpanded = expandedUserId === developer.user_id;
            const panel = keyPanels[developer.user_id] ?? {
              loading: false,
              usage: null,
            };
            const keyTotal = panel.usage?.keys.length ?? developer.key_count;
            const usageToday = panel.usage?.used_today ?? developer.used_today;
            const dailyLimit = panel.usage?.daily_limit ?? developer.daily_limit;

            return (
              <article key={developer.user_id} className="grid gap-3">
                <Card className="rounded-2xl">
                  <CardContent className="grid gap-4 p-4">
                    <div className="flex flex-col gap-4 xl:flex-row xl:items-start xl:justify-between">
                      <div className="grid gap-2">
                        <div className="grid gap-1">
                          <strong>{developer.username}</strong>
                          <div className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
                            <code>{developer.user_id}</code>
                            <span>{t("console.users.created_at", { createdAt: developer.created_at })}</span>
                          </div>
                        </div>
                      </div>
                      <div className="grid gap-3 sm:grid-cols-3 xl:min-w-[360px]">
                        <div className="rounded-xl border border-border bg-muted/40 p-4">
                          <span>{t("console.users.daily_limit")}</span>
                          <strong className="mt-2 block text-base font-semibold text-foreground">{dailyLimit}</strong>
                        </div>
                        <div className="rounded-xl border border-border bg-muted/40 p-4">
                          <span>{t("console.users.usage_today")}</span>
                          <strong className="mt-2 block text-base font-semibold text-foreground">{usageToday}</strong>
                        </div>
                        <div className="rounded-xl border border-border bg-muted/40 p-4">
                          <span>{t("console.users.keys")}</span>
                          <strong className="mt-2 block text-base font-semibold text-foreground">{keyTotal}</strong>
                        </div>
                      </div>
                    </div>

                    <div className="grid gap-3">
                      <form
                        className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_auto]"
                        onSubmit={(event) => {
                          event.preventDefault();
                          void handleSaveDeveloperQuota(developer);
                        }}
                      >
                        <FieldShell label={t("console.users.daily_limit")} hint={t("console.users.quota_label", { username: developer.username })}>
                          <Input
                            aria-label={t("console.users.quota_label", { username: developer.username })}
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
                            placeholder={t("console.users.quota_placeholder")}
                          />
                        </FieldShell>
                        <Button className="lg:self-end" type="submit" disabled={busy}>
                          {t("console.users.save")}
                        </Button>
                      </form>

                      <form
                        className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_auto]"
                        onSubmit={(event) => {
                          event.preventDefault();
                          void handleSaveDeveloperPassword(developer);
                        }}
                      >
                        <FieldShell label={t("console.users.password_label")} hint={t("console.users.password_hint")}>
                          <Input
                            type="password"
                            aria-label={t("console.users.password_label")}
                            value={draft.password}
                            onChange={(event) =>
                              setDeveloperDrafts((current) => ({
                                ...current,
                                [developer.user_id]: {
                                  ...draft,
                                  password: event.target.value,
                                },
                              }))
                            }
                            placeholder={t("console.users.password_placeholder")}
                          />
                        </FieldShell>
                        <Button className="lg:self-end" type="submit" disabled={busy}>
                          {t("console.users.update_password")}
                        </Button>
                      </form>

                      <div className="flex flex-wrap items-center gap-2">
                        <Button
                          type="button"
                          variant="ghost"
                          disabled={busy || panel.loading}
                          onClick={() => void handleToggleKeyPanel(developer)}
                        >
                          {isExpanded ? t("console.users.hide_keys") : t("console.users.manage_keys")}
                        </Button>
                        <Button
                          type="button"
                          variant="destructive"
                          disabled={busy}
                          onClick={() => void handleDeleteDeveloper(developer)}
                        >
                          {t("console.users.delete_user")}
                        </Button>
                      </div>
                    </div>
                  </CardContent>
                </Card>
                {isExpanded ? (
                  <Card className="rounded-2xl border-dashed">
                    <CardContent className="grid gap-4 p-4">
                      <div className="flex flex-col gap-2 border-b border-border pb-3 sm:flex-row sm:items-end sm:justify-between">
                        <div className="space-y-1">
                          <h3 className="text-base font-semibold text-foreground">{t("console.users.key_panel_title", { username: developer.username })}</h3>
                          <p className="text-sm text-muted-foreground">{t("console.users.key_panel_hint")}</p>
                        </div>
                        <span className="text-sm text-muted-foreground">{t("console.users.key_total", { count: keyTotal })}</span>
                      </div>

                      <div className="flex flex-wrap gap-4 text-sm text-muted-foreground">
                        <span>{t("console.users.daily_limit_summary")} <strong className="text-foreground">{dailyLimit}</strong></span>
                        <span>{t("console.users.used_today_summary")} <strong className="text-foreground">{usageToday}</strong></span>
                      </div>

                      {panel.loading && !panel.usage ? (
                        <div className="rounded-2xl border border-dashed border-border bg-muted/40 px-4 py-8 text-center text-sm text-muted-foreground">
                          {t("console.users.loading_keys")}
                        </div>
                      ) : null}

                      {panel.usage?.keys.length ? (
                        <div className="grid gap-3">
                          {panel.usage.keys.map((key) => (
                            <div className="grid gap-3 rounded-2xl border border-border bg-muted/30 p-4" key={key.id}>
                              <div className="grid gap-1">
                                <strong>{key.name}</strong>
                                <span className="text-sm text-muted-foreground">{key.preview}</span>
                              </div>
                              <div className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
                                <span>{t("console.users.created_at", { createdAt: key.created_at })}</span>
                                <Badge variant={key.revoked_at ? "outline" : "success"}>
                                  {key.revoked_at ? t("console.users.revoked_at", { revokedAt: key.revoked_at }) : t("console.users.active")}
                                </Badge>
                              </div>
                              <div className="flex flex-wrap items-center gap-2">
                                <Button
                                  type="button"
                                  variant="ghost"
                                  size="sm"
                                  disabled={busy || panel.loading || Boolean(key.revoked_at)}
                                  onClick={() => void handleRevokeKey(developer, key.id)}
                                >
                                  {t("console.users.revoke")}
                                </Button>
                              </div>
                            </div>
                          ))}
                        </div>
                      ) : panel.loading ? null : (
                        <div className="rounded-2xl border border-dashed border-border bg-muted/40 px-4 py-8 text-center text-sm text-muted-foreground">
                          {t("console.users.no_keys")}
                        </div>
                      )}
                    </CardContent>
                  </Card>
                ) : null}
              </article>
            );
          })
        ) : (
          <div className="rounded-2xl border border-dashed border-border bg-muted/40 px-4 py-8 text-center text-sm text-muted-foreground">
            {t("console.users.no_users")}
          </div>
        )}
      </div>
    </PanelSection>
  );
}

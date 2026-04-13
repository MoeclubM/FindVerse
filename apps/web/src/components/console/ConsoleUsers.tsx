import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  AdminUserRecord,
  DeveloperUsage,
  UserRole,
  createUser,
  deleteUser,
  getAdminUserKeys,
  revokeAdminUserKey,
  updateUser,
} from "../../api";
import { FieldShell, PanelSection, StatStrip } from "../common/PanelPrimitives";
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
import { Input } from "../ui/input";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../ui/select";
import { Skeleton } from "../ui/skeleton";
import { useConsole } from "./ConsoleContext";

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

function getRoleLabel(role: UserRole, t: (key: string) => string) {
  return role === "admin"
    ? t("console.users.roles.admin")
    : t("console.users.roles.developer");
}

function getRoleBadgeVariant(role: UserRole) {
  return role === "admin" ? "warning" : "outline";
}

type UserDraft = {
  username: string;
  role: UserRole;
  daily_limit: string;
  password: string;
};

type KeyPanelState = {
  loading: boolean;
  usage: DeveloperUsage | null;
};

export function ConsoleUsers() {
  const { token, busy, setBusy, setFlash, refreshAll, users } = useConsole();
  const { t } = useTranslation();

  const [createDraft, setCreateDraft] = useState<{
    username: string;
    password: string;
    role: UserRole;
  }>({
    username: "",
    password: "",
    role: "developer",
  });
  const [userDrafts, setUserDrafts] = useState<Record<string, UserDraft>>({});
  const [expandedUserId, setExpandedUserId] = useState<string | null>(null);
  const [keyPanels, setKeyPanels] = useState<Record<string, KeyPanelState>>({});
  const [deleteUserId, setDeleteUserId] = useState<string | null>(null);

  useEffect(() => {
    setUserDrafts((current) => {
      const next = { ...current };
      for (const user of users) {
        next[user.user_id] ??= {
          username: user.username,
          role: user.role,
          daily_limit: String(user.daily_limit),
          password: "",
        };
      }
      return next;
    });
  }, [users]);

  useEffect(() => {
    if (!expandedUserId) {
      return;
    }
    if (!users.some((user) => user.user_id === expandedUserId)) {
      setExpandedUserId(null);
    }
  }, [users, expandedUserId]);

  function setKeyPanelState(
    userId: string,
    updater: (current: KeyPanelState) => KeyPanelState,
  ) {
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

  async function loadUserKeys(user: AdminUserRecord, force = false) {
    const panel = keyPanels[user.user_id];
    if (!force && panel?.usage && !panel.loading) {
      return;
    }

    setKeyPanelState(user.user_id, (current) => ({
      ...current,
      loading: true,
    }));

    try {
      const usage = await getAdminUserKeys(token, user.user_id);
      setKeyPanelState(user.user_id, () => ({
        loading: false,
        usage,
      }));
    } catch (error) {
      setKeyPanelState(user.user_id, (current) => ({
        ...current,
        loading: false,
      }));
      setFlash(
        getErrorMessage(
          error,
          t("console.users.load_keys_failed", { username: user.username }),
        ),
      );
    }
  }

  async function handleCreateUser() {
    const username = createDraft.username.trim();
    const password = createDraft.password.trim();
    if (!username) {
      setFlash(t("console.users.username_required"));
      return;
    }
    if (!password) {
      setFlash(t("console.users.password_required"));
      return;
    }

    setBusy(true);
    setFlash(null);
    try {
      await createUser(token, {
        username,
        password,
        role: createDraft.role,
      });
      setCreateDraft({
        username: "",
        password: "",
        role: "developer",
      });
      await refreshAll();
      setFlash(
        t("console.users.create_success", {
          username,
        }),
      );
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.users.create_failed")));
    } finally {
      setBusy(false);
    }
  }

  async function handleSaveUser(user: AdminUserRecord) {
    const draft = userDrafts[user.user_id];
    if (!draft) {
      return;
    }

    const username = draft.username.trim();
    if (!username) {
      setFlash(t("console.users.username_required"));
      return;
    }

    const nextLimit = Math.max(1, Number(draft.daily_limit) || 1);
    setBusy(true);
    setFlash(null);
    try {
      await updateUser(token, user.user_id, {
        username,
        role: draft.role,
        daily_limit: nextLimit,
      });
      setUserDrafts((current) => ({
        ...current,
        [user.user_id]: {
          ...current[user.user_id],
          username: username.toLowerCase(),
          daily_limit: String(nextLimit),
        },
      }));
      await refreshAll();
      setFlash(t("console.users.update_success", { username }));
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.users.update_failed")));
    } finally {
      setBusy(false);
    }
  }

  async function handleSaveUserPassword(user: AdminUserRecord) {
    const password = userDrafts[user.user_id]?.password.trim() ?? "";
    if (!password) {
      setFlash(t("console.users.password_required"));
      return;
    }

    setBusy(true);
    setFlash(null);
    try {
      await updateUser(token, user.user_id, { password });
      setUserDrafts((current) => ({
        ...current,
        [user.user_id]: {
          ...current[user.user_id],
          password: "",
        },
      }));
      setFlash(
        t("console.users.password_update_success", { username: user.username }),
      );
    } catch (error) {
      setFlash(
        getErrorMessage(
          error,
          t("console.users.password_update_failed", { username: user.username }),
        ),
      );
    } finally {
      setBusy(false);
    }
  }

  async function handleToggleEnabled(user: AdminUserRecord) {
    setBusy(true);
    setFlash(null);
    try {
      await updateUser(token, user.user_id, {
        enabled: !user.enabled,
      });
      await refreshAll();
      setFlash(
        t(
          user.enabled
            ? "console.users.disable_success"
            : "console.users.enable_success",
          { username: user.username },
        ),
      );
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.users.update_failed")));
    } finally {
      setBusy(false);
    }
  }

  async function handleToggleKeyPanel(user: AdminUserRecord) {
    if (expandedUserId === user.user_id) {
      setExpandedUserId(null);
      return;
    }

    setExpandedUserId(user.user_id);
    setFlash(null);
    await loadUserKeys(user);
  }

  async function handleDeleteUser(user: AdminUserRecord) {
    setDeleteUserId(null);
    setBusy(true);
    setFlash(null);
    try {
      await deleteUser(token, user.user_id);
      setUserDrafts((current) => {
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
      setFlash(
        getErrorMessage(
          error,
          t("console.users.delete_failed", { username: user.username }),
        ),
      );
    } finally {
      setBusy(false);
    }
  }

  async function handleRevokeKey(user: AdminUserRecord, keyId: string) {
    setBusy(true);
    setFlash(null);
    try {
      await revokeAdminUserKey(token, user.user_id, keyId);
      await Promise.all([refreshAll(), loadUserKeys(user, true)]);
      setFlash(t("console.users.key_delete_success", { username: user.username }));
    } catch (error) {
      setFlash(
        getErrorMessage(
          error,
          t("console.users.revoke_key_failed", { username: user.username }),
        ),
      );
    } finally {
      setBusy(false);
    }
  }

  const totalIssuedKeys = users.reduce((sum, user) => sum + user.key_count, 0);
  const totalUsageToday = users.reduce((sum, user) => sum + user.used_today, 0);
  const totalDailyLimit = users.reduce((sum, user) => sum + user.daily_limit, 0);
  const deleteUserRecord = users.find((user) => user.user_id === deleteUserId) ?? null;

  return (
    <PanelSection
      title={t("console.users.title")}
      meta={t("console.users.accounts", { count: users.length })}
      contentClassName="space-y-5"
    >
      <StatStrip
        className="xl:grid-cols-4"
        items={[
          { label: t("console.users.accounts_total"), value: users.length },
          { label: t("console.users.keys"), value: totalIssuedKeys },
          { label: t("console.users.usage_today"), value: totalUsageToday },
          { label: t("console.users.total_daily_limit"), value: totalDailyLimit },
        ]}
      />

      <Card className="rounded-2xl border-dashed">
        <CardContent className="grid gap-4 p-4">
          <div className="grid gap-1">
            <strong>{t("console.users.create_title")}</strong>
            <span className="text-sm text-muted-foreground">
              {t("console.users.create_hint")}
            </span>
          </div>
          <form
            className="grid gap-3 xl:grid-cols-[minmax(0,1fr)_minmax(180px,220px)_minmax(0,1fr)_auto]"
            onSubmit={(event) => {
              event.preventDefault();
              void handleCreateUser();
            }}
          >
            <FieldShell label={t("console.users.username_label")}>
              <Input
                value={createDraft.username}
                onChange={(event) =>
                  setCreateDraft((current) => ({
                    ...current,
                    username: event.target.value,
                  }))
                }
                placeholder={t("console.users.username_placeholder")}
              />
            </FieldShell>
            <FieldShell
              label={t("console.users.role_label")}
              hint={t("console.users.role_hint")}
            >
              <Select
                value={createDraft.role}
                onValueChange={(role) =>
                  setCreateDraft((current) => ({
                    ...current,
                    role: role as UserRole,
                  }))
                }
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    <SelectItem value="developer">
                      {t("console.users.roles.developer")}
                    </SelectItem>
                    <SelectItem value="admin">
                      {t("console.users.roles.admin")}
                    </SelectItem>
                  </SelectGroup>
                </SelectContent>
              </Select>
            </FieldShell>
            <FieldShell label={t("console.users.password_label")}>
              <Input
                type="password"
                value={createDraft.password}
                onChange={(event) =>
                  setCreateDraft((current) => ({
                    ...current,
                    password: event.target.value,
                  }))
                }
                placeholder={t("console.users.password_placeholder")}
              />
            </FieldShell>
            <Button className="xl:self-end" type="submit" disabled={busy}>
              {t("console.users.create_user")}
            </Button>
          </form>
        </CardContent>
      </Card>

      <div className="grid gap-3">
        {users.length ? (
          users.map((user) => {
            const draft = userDrafts[user.user_id] ?? {
              username: user.username,
              role: user.role,
              daily_limit: String(user.daily_limit),
              password: "",
            };
            const isExpanded = expandedUserId === user.user_id;
            const panel = keyPanels[user.user_id] ?? {
              loading: false,
              usage: null,
            };
            const keyTotal = panel.usage?.keys.length ?? user.key_count;
            const usageToday = panel.usage?.used_today ?? user.used_today;
            const dailyLimit = panel.usage?.daily_limit ?? user.daily_limit;

            return (
              <article key={user.user_id} className="grid gap-3">
                <Card className="rounded-2xl">
                  <CardContent className="grid gap-4 p-4">
                    <div className="flex flex-col gap-4 xl:flex-row xl:items-start xl:justify-between">
                      <div className="grid gap-2">
                        <div className="flex flex-wrap items-center gap-2">
                          <strong>{user.username}</strong>
                          <Badge variant={getRoleBadgeVariant(user.role)}>
                            {getRoleLabel(user.role, t)}
                          </Badge>
                          <Badge variant={user.enabled ? "success" : "outline"}>
                            {user.enabled
                              ? t("console.users.enabled")
                              : t("console.users.disabled")}
                          </Badge>
                        </div>
                        <div className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
                          <code>{user.user_id}</code>
                          <span>
                            {t("console.users.created_at", {
                              createdAt: user.created_at,
                            })}
                          </span>
                        </div>
                      </div>
                      <div className="grid gap-3 sm:grid-cols-3 xl:min-w-[360px]">
                        <div className="rounded-xl border border-border bg-muted/40 p-4">
                          <span>{t("console.users.daily_limit")}</span>
                          <strong className="mt-2 block text-base font-semibold text-foreground">
                            {dailyLimit}
                          </strong>
                        </div>
                        <div className="rounded-xl border border-border bg-muted/40 p-4">
                          <span>{t("console.users.usage_today")}</span>
                          <strong className="mt-2 block text-base font-semibold text-foreground">
                            {usageToday}
                          </strong>
                        </div>
                        <div className="rounded-xl border border-border bg-muted/40 p-4">
                          <span>{t("console.users.keys")}</span>
                          <strong className="mt-2 block text-base font-semibold text-foreground">
                            {keyTotal}
                          </strong>
                        </div>
                      </div>
                    </div>

                    <div className="grid gap-3">
                      <form
                        className="grid gap-3 xl:grid-cols-[minmax(0,1fr)_minmax(180px,220px)_minmax(140px,180px)_auto]"
                        onSubmit={(event) => {
                          event.preventDefault();
                          void handleSaveUser(user);
                        }}
                      >
                        <FieldShell label={t("console.users.username_label")}>
                          <Input
                            value={draft.username}
                            onChange={(event) =>
                              setUserDrafts((current) => ({
                                ...current,
                                [user.user_id]: {
                                  ...draft,
                                  username: event.target.value,
                                },
                              }))
                            }
                            placeholder={t("console.users.username_placeholder")}
                          />
                        </FieldShell>
                        <FieldShell label={t("console.users.role_label")}>
                          <Select
                            value={draft.role}
                            onValueChange={(role) =>
                              setUserDrafts((current) => ({
                                ...current,
                                [user.user_id]: {
                                  ...draft,
                                  role: role as UserRole,
                                },
                              }))
                            }
                          >
                            <SelectTrigger>
                              <SelectValue />
                            </SelectTrigger>
                            <SelectContent>
                              <SelectGroup>
                                <SelectItem value="developer">
                                  {t("console.users.roles.developer")}
                                </SelectItem>
                                <SelectItem value="admin">
                                  {t("console.users.roles.admin")}
                                </SelectItem>
                              </SelectGroup>
                            </SelectContent>
                          </Select>
                        </FieldShell>
                        <FieldShell
                          label={t("console.users.daily_limit")}
                          hint={t("console.users.quota_label", {
                            username: user.username,
                          })}
                        >
                          <Input
                            value={draft.daily_limit}
                            onChange={(event) =>
                              setUserDrafts((current) => ({
                                ...current,
                                [user.user_id]: {
                                  ...draft,
                                  daily_limit: event.target.value,
                                },
                              }))
                            }
                            placeholder={t("console.users.quota_placeholder")}
                          />
                        </FieldShell>
                        <Button className="xl:self-end" type="submit" disabled={busy}>
                          {t("console.users.save")}
                        </Button>
                      </form>

                      <form
                        className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_auto]"
                        onSubmit={(event) => {
                          event.preventDefault();
                          void handleSaveUserPassword(user);
                        }}
                      >
                        <FieldShell
                          label={t("console.users.password_label")}
                          hint={t("console.users.password_hint")}
                        >
                          <Input
                            type="password"
                            aria-label={t("console.users.password_label")}
                            value={draft.password}
                            onChange={(event) =>
                              setUserDrafts((current) => ({
                                ...current,
                                [user.user_id]: {
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
                          variant="outline"
                          disabled={busy}
                          onClick={() => void handleToggleEnabled(user)}
                        >
                          {user.enabled
                            ? t("console.users.disable")
                            : t("console.users.enable")}
                        </Button>
                        <Button
                          type="button"
                          variant="ghost"
                          disabled={busy || panel.loading}
                          onClick={() => void handleToggleKeyPanel(user)}
                        >
                          {isExpanded
                            ? t("console.users.hide_keys")
                            : t("console.users.manage_keys")}
                        </Button>
                        <Button
                          type="button"
                          variant="destructive"
                          disabled={busy}
                          onClick={() => setDeleteUserId(user.user_id)}
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
                          <h3 className="text-base font-semibold text-foreground">
                            {t("console.users.key_panel_title", {
                              username: user.username,
                            })}
                          </h3>
                          <p className="text-sm text-muted-foreground">
                            {t("console.users.key_panel_hint")}
                          </p>
                        </div>
                        <span className="text-sm text-muted-foreground">
                          {t("console.users.key_total", { count: keyTotal })}
                        </span>
                      </div>

                      <div className="flex flex-wrap gap-4 text-sm text-muted-foreground">
                        <span>
                          {t("console.users.daily_limit_summary")}{" "}
                          <strong className="text-foreground">{dailyLimit}</strong>
                        </span>
                        <span>
                          {t("console.users.used_today_summary")}{" "}
                          <strong className="text-foreground">{usageToday}</strong>
                        </span>
                      </div>

                      {panel.loading && !panel.usage ? (
                        <div className="grid gap-3">
                          {Array.from({ length: 2 }).map((_, index) => (
                            <div
                              className="grid gap-3 rounded-2xl border border-border bg-muted/30 p-4"
                              key={index}
                            >
                              <Skeleton className="h-5 w-32" />
                              <Skeleton className="h-4 w-full max-w-sm" />
                              <div className="flex flex-wrap items-center gap-2">
                                <Skeleton className="h-4 w-36" />
                                <Skeleton className="h-6 w-20 rounded-full" />
                              </div>
                              <Skeleton className="h-8 w-20 rounded-lg" />
                            </div>
                          ))}
                        </div>
                      ) : null}

                      {panel.usage?.keys.length ? (
                        <div className="grid gap-3">
                          {panel.usage.keys.map((key) => (
                            <div
                              className="grid gap-3 rounded-2xl border border-border bg-muted/30 p-4"
                              key={key.id}
                            >
                              <div className="grid gap-1">
                                <strong>{key.name}</strong>
                                <span className="text-sm text-muted-foreground">
                                  {key.preview}
                                </span>
                              </div>
                              <div className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
                                <span>
                                  {t("console.users.created_at", {
                                    createdAt: key.created_at,
                                  })}
                                </span>
                                <Badge
                                  variant={key.revoked_at ? "outline" : "success"}
                                >
                                  {key.revoked_at
                                    ? t("console.users.revoked_at", {
                                        revokedAt: key.revoked_at,
                                      })
                                    : t("console.users.active")}
                                </Badge>
                              </div>
                              <div className="flex flex-wrap items-center gap-2">
                                <Button
                                  type="button"
                                  variant="ghost"
                                  size="sm"
                                  disabled={
                                    busy || panel.loading || Boolean(key.revoked_at)
                                  }
                                  onClick={() => void handleRevokeKey(user, key.id)}
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

      <AlertDialog
        open={Boolean(deleteUserRecord)}
        onOpenChange={(open) => {
          if (!open) {
            setDeleteUserId(null);
          }
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("console.users.delete_user")}</AlertDialogTitle>
            <AlertDialogDescription>
              {deleteUserRecord
                ? t("console.users.delete_confirm", {
                    username: deleteUserRecord.username,
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
              disabled={busy || !deleteUserRecord}
              onClick={() => deleteUserRecord && void handleDeleteUser(deleteUserRecord)}
            >
              {t("console.users.delete_user")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </PanelSection>
  );
}

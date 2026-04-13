import { ExitIcon, MagnifyingGlassIcon } from "@radix-ui/react-icons";
import {
  Settings,
  Users,
  Bot,
  FileText,
  ListTodo,
  LayoutDashboard,
  Orbit,
  Globe2,
  type LucideIcon,
} from "lucide-react";
import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";

import {
  AdminUserRecord,
  CrawlOverview,
  getCrawlOverview,
  getUserSession,
  listAdminUsers,
  listDocuments,
  loginUser,
  logoutUser,
  UserSession,
} from "../api";
import { AppTopbar, TopbarActionButton } from "./common/AppTopbar";
import {
  ConsoleProvider,
  type ConsoleContextValue,
} from "./console/ConsoleContext";
import { ConsoleOverview } from "./console/ConsoleOverview";
import { ConsoleUsers } from "./console/ConsoleUsers";
import { ConsoleCrawlTasks } from "./console/ConsoleCrawlTasks";
import { ConsoleDomains } from "./console/ConsoleDomains";
import { ConsoleWorkers } from "./console/ConsoleWorkers";
import { ConsoleDocuments } from "./console/ConsoleDocuments";
import { ConsoleJobs } from "./console/ConsoleJobs";
import { ConsoleSettings } from "./console/ConsoleSettings";
import type { ThemeMode } from "./ThemeSwitcher";
import { Alert, AlertDescription } from "./ui/alert";
import { Button } from "./ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "./ui/card";
import { Input } from "./ui/input";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarSeparator,
  SidebarTrigger,
} from "./ui/sidebar";

const USER_SESSION_KEY = "findverse_user_session";
const SITE_NAME =
  (import.meta.env.VITE_FINDVERSE_SITE_NAME || "FindVerse").trim() ||
  "FindVerse";

type ConsoleTab =
  | "overview"
  | "users"
  | "tasks"
  | "domains"
  | "jobs"
  | "workers"
  | "documents"
  | "settings";

type ConsoleSidebarItem = {
  key: ConsoleTab;
  label: string;
  icon: LucideIcon;
  badge?: string;
};

type ConsoleSidebarGroup = {
  label: string;
  items: ConsoleSidebarItem[];
};

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

async function refreshConsoleData(
  token: string,
  actions: {
    setOverview: (value: CrawlOverview | null) => void;
    setUsers: (value: AdminUserRecord[]) => void;
    setFlash: (value: string | null) => void;
  },
  refreshFailedMessage: string,
  silent = false,
) {
  try {
    const [overview, users] = await Promise.all([
      getCrawlOverview(token),
      listAdminUsers(token),
    ]);
    actions.setOverview(overview);
    actions.setUsers(users);
  } catch (error) {
    if (!silent) {
      actions.setFlash(getErrorMessage(error, refreshFailedMessage));
    }
  }
}

async function refreshDocuments(
  token: string,
  actions: {
    setDocuments: (
      value: Awaited<ReturnType<typeof listDocuments>> | null,
    ) => void;
    setFlash: (value: string | null) => void;
  },
  refreshFailedMessage: string,
  silent = false,
) {
  try {
    const documents = await listDocuments(token);
    actions.setDocuments(documents);
  } catch (error) {
    if (!silent) {
      actions.setFlash(getErrorMessage(error, refreshFailedMessage));
    }
  }
}

export function ConsolePage(props: {
  theme: "light" | "dark";
  themeMode: ThemeMode;
  onThemeModeChange: (theme: ThemeMode) => void;
  onNavigateHome: () => void;
  onNavigateDevPortal: () => void;
}) {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<ConsoleTab>("overview");
  const [token, setToken] = useState<string | null>(() =>
    localStorage.getItem(USER_SESSION_KEY),
  );
  const [session, setSession] = useState<UserSession | null>(null);
  const [overview, setOverview] = useState<CrawlOverview | null>(null);
  const [users, setUsers] = useState<AdminUserRecord[]>([]);
  const [documents, setDocuments] = useState<Awaited<
    ReturnType<typeof listDocuments>
  > | null>(null);
  const [authLoading, setAuthLoading] = useState(Boolean(token));
  const [busy, setBusy] = useState(false);
  const [loginError, setLoginError] = useState<string | null>(null);
  const [loginUsername, setLoginUsername] = useState("");
  const [loginPassword, setLoginPassword] = useState("");

  const consoleLabel = t("console.title").startsWith(SITE_NAME)
    ? t("console.title").slice(SITE_NAME.length).trim()
    : t("console.title");
  const hasConsoleAccess = session?.role === "admin";

  const setFlash = useCallback((value: string | null) => {
    if (!value) {
      toast.dismiss();
      return;
    }
    toast(value);
  }, []);

  const refreshAll = useCallback(
    () =>
      token && hasConsoleAccess
        ? refreshConsoleData(
            token,
            {
              setOverview,
              setUsers,
              setFlash,
            },
            t("console.refresh_failed"),
          )
        : Promise.resolve(),
    [token, hasConsoleAccess, setFlash, t],
  );

  const refreshDocumentList = useCallback(
    () =>
      token && hasConsoleAccess
        ? refreshDocuments(
            token,
            {
              setDocuments,
              setFlash,
            },
            t("console.refresh_failed"),
          )
        : Promise.resolve(),
    [token, hasConsoleAccess, setFlash, t],
  );

  useEffect(() => {
    if (!token) {
      setAuthLoading(false);
      setSession(null);
      return;
    }

    let cancelled = false;
    setAuthLoading(true);
    getUserSession(token)
      .then((nextSession) => {
        if (!cancelled) {
          setSession(nextSession);
        }
      })
      .catch(() => {
        if (!cancelled) {
          localStorage.removeItem(USER_SESSION_KEY);
          setToken(null);
          setSession(null);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setAuthLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [token]);

  useEffect(() => {
    if (!token || !session || !hasConsoleAccess) {
      return;
    }

    let cancelled = false;
    let running = false;

    const run = async () => {
      if (cancelled || running) {
        return;
      }
      running = true;
      try {
        await refreshConsoleData(
          token,
          {
            setOverview,
            setUsers,
            setFlash,
          },
          t("console.refresh_failed"),
          true,
        );
      } finally {
        running = false;
      }
    };

    void run();
    const timer = window.setInterval(() => {
      void run();
    }, 1000);

    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [token, session, hasConsoleAccess, setFlash, t]);

  useEffect(() => {
    if (!token || !session || !hasConsoleAccess) {
      return;
    }
    const timer = window.setTimeout(() => {
      void refreshDocuments(
        token,
        {
          setDocuments,
          setFlash,
        },
        t("console.refresh_failed"),
        true,
      );
    }, 150);
    return () => window.clearTimeout(timer);
  }, [token, session, hasConsoleAccess, setFlash, t]);

  async function handleLogin(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setBusy(true);
    setLoginError(null);
    try {
      const nextSession = await loginUser(loginUsername, loginPassword);
      localStorage.setItem(USER_SESSION_KEY, nextSession.token);
      setToken(nextSession.token);
      setSession(nextSession);
    } catch (error) {
      setLoginError(getErrorMessage(error, t("console.login.error")));
    } finally {
      setBusy(false);
    }
  }

  async function handleLogout() {
    if (!token) {
      return;
    }
    setBusy(true);
    setFlash(null);
    try {
      await logoutUser(token);
    } catch {
      // Ignore logout failures and clear local state anyway.
    } finally {
      localStorage.removeItem(USER_SESSION_KEY);
      setToken(null);
      setSession(null);
      setOverview(null);
      setUsers([]);
      setDocuments(null);
      setLoginError(null);
      setBusy(false);
    }
  }

  const contextValue = useMemo<ConsoleContextValue>(
    () => ({
      token: token!,
      busy,
      setBusy,
      setFlash,
      refreshAll,
      refreshDocumentList,
      overview,
      users,
      documents,
    }),
    [
      token,
      busy,
      setFlash,
      refreshAll,
      refreshDocumentList,
      overview,
      users,
      documents,
    ],
  );

  const activeRules =
    overview?.rules.filter((rule) => rule.enabled).length ?? 0;
  const queuedJobs = overview?.frontier_depth ?? 0;
  const inFlightJobs = overview?.in_flight_jobs ?? 0;
  const indexedDocuments = overview?.indexed_documents ?? 0;
  const totalWorkers = overview?.crawlers.length ?? 0;
  const onlineWorkers =
    overview?.crawlers.filter((crawler) => crawler.online).length ?? 0;
  const terminalFailures = overview?.terminal_failures ?? 0;
  const tabGroups: ConsoleSidebarGroup[] = [
    {
      label: t("console.sidebar.groups.system"),
      items: [
        {
          key: "overview" as const,
          label: t("console.tabs.overview"),
          icon: LayoutDashboard,
          badge: String(terminalFailures),
        },
        {
          key: "users" as const,
          label: t("console.tabs.users"),
          icon: Users,
          badge: String(users.length),
        },
        {
          key: "settings" as const,
          label: t("console.tabs.settings"),
          icon: Settings,
          badge: undefined,
        },
      ],
    },
    {
      label: t("console.sidebar.groups.crawl"),
      items: [
        {
          key: "tasks" as const,
          label: t("console.tabs.tasks"),
          icon: Orbit,
          badge: String(activeRules),
        },
        {
          key: "domains" as const,
          label: t("console.tabs.domains"),
          icon: Globe2,
          badge: undefined,
        },
        {
          key: "jobs" as const,
          label: t("console.tabs.jobs"),
          icon: ListTodo,
          badge: String(inFlightJobs || queuedJobs),
        },
        {
          key: "workers" as const,
          label: t("console.tabs.workers"),
          icon: Bot,
          badge: `${onlineWorkers}/${totalWorkers}`,
        },
        {
          key: "documents" as const,
          label: t("console.tabs.documents"),
          icon: FileText,
          badge: String(indexedDocuments),
        },
      ],
    },
  ];
  const activeTabLabel =
    tabGroups
      .flatMap((group) => group.items)
      .find((item) => item.key === activeTab)?.label ?? t("console.title");

  const sidebar = (
    <>
      <SidebarHeader>
        <div className="rounded-[24px] bg-sidebar-primary px-4 py-5 text-sidebar-primary-foreground">
          <div className="text-lg font-semibold tracking-[-0.03em]">{SITE_NAME}</div>
        </div>
      </SidebarHeader>

      <SidebarContent>
        {tabGroups.map((group, index) => (
          <div key={group.label} className="flex flex-col gap-4">
            {index > 0 ? <SidebarSeparator /> : null}
            <SidebarGroup>
              <SidebarGroupLabel>{group.label}</SidebarGroupLabel>
              <SidebarMenu>
                {group.items.map((item) => {
                  const Icon = item.icon;
                  return (
                    <SidebarMenuItem key={item.key}>
                      <SidebarMenuButton
                        isActive={activeTab === item.key}
                        onClick={() => {
                          setActiveTab(item.key);
                        }}
                      >
                        <span className="flex min-w-0 items-center gap-3">
                          <Icon data-icon="inline-start" />
                          <span className="truncate font-medium">
                            {item.label}
                          </span>
                        </span>
                        {item.badge ? (
                          <SidebarMenuBadge>{item.badge}</SidebarMenuBadge>
                        ) : null}
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  );
                })}
              </SidebarMenu>
            </SidebarGroup>
          </div>
        ))}
      </SidebarContent>

      <SidebarFooter>
        <div className="rounded-2xl border border-sidebar-border bg-sidebar-accent/60 p-4">
          <div className="flex items-center justify-between gap-3">
            <div>
              <div className="text-[11px] font-medium uppercase tracking-[0.18em] text-sidebar-foreground/55">
                {t("console.sidebar.footer.label")}
              </div>
              <div className="mt-1 text-sm font-semibold text-sidebar-foreground">
                {session?.username}
              </div>
            </div>
            <SidebarMenuBadge>{users.length}</SidebarMenuBadge>
          </div>
          <div className="mt-4 grid grid-cols-2 gap-3">
            <div>
              <div className="text-[11px] uppercase tracking-[0.16em] text-sidebar-foreground/55">
                {t("console.sidebar.footer.rules")}
              </div>
              <div className="mt-1 text-sm font-semibold text-sidebar-foreground">
                {activeRules}
              </div>
            </div>
            <div>
              <div className="text-[11px] uppercase tracking-[0.16em] text-sidebar-foreground/55">
                {t("console.sidebar.footer.failures")}
              </div>
              <div className="mt-1 text-sm font-semibold text-sidebar-foreground">
                {terminalFailures}
              </div>
            </div>
          </div>
        </div>
      </SidebarFooter>
    </>
  );

  if (authLoading) {
    return (
      <div className="grid min-h-screen place-items-center bg-background text-foreground">
        {t("console.login.checking")}
      </div>
    );
  }

  if (!session || !token) {
    return (
      <div className="min-h-screen bg-background text-foreground">
        <AppTopbar
          theme={props.theme}
          themeMode={props.themeMode}
          onThemeModeChange={props.onThemeModeChange}
          title={`${SITE_NAME} · ${consoleLabel}`}
          onTitleClick={props.onNavigateHome}
          afterControls={
            <TopbarActionButton
              leading={<MagnifyingGlassIcon className="size-4" />}
              onClick={props.onNavigateHome}
              ariaLabel={t("console.login.search_link")}
              compactOnMobile
            >
              {t("console.login.search_link")}
            </TopbarActionButton>
          }
        />
        <main className="mx-auto flex min-h-[calc(100vh-73px)] w-full max-w-md items-center px-4 py-10">
          <Card className="w-full rounded-3xl">
            <CardHeader className="pb-4">
              <CardTitle>{t("console.login.title")}</CardTitle>
              <CardDescription>{SITE_NAME}</CardDescription>
            </CardHeader>
            <CardContent>
              <form className="grid gap-3" onSubmit={handleLogin}>
                <Input
                  value={loginUsername}
                  onChange={(event) => setLoginUsername(event.target.value)}
                  placeholder={t("console.login.username")}
                />
                <Input
                  type="password"
                  value={loginPassword}
                  onChange={(event) => setLoginPassword(event.target.value)}
                  placeholder={t("console.login.password")}
                />
                <Button type="submit" disabled={busy}>
                  {busy
                    ? t("console.login.submitting")
                    : t("console.login.submit")}
                </Button>
              </form>
              {loginError ? (
                <Alert variant="destructive" className="mt-4">
                  <AlertDescription>{loginError}</AlertDescription>
                </Alert>
              ) : null}
            </CardContent>
          </Card>
        </main>
      </div>
    );
  }

  if (!hasConsoleAccess) {
    const roleLabel =
      session.role === "admin"
        ? t("console.users.roles.admin")
        : t("console.users.roles.developer");

    return (
      <div className="min-h-screen bg-background text-foreground">
        <AppTopbar
          theme={props.theme}
          themeMode={props.themeMode}
          onThemeModeChange={props.onThemeModeChange}
          title={`${SITE_NAME} · ${consoleLabel}`}
          onTitleClick={props.onNavigateHome}
          afterControls={
            <>
              <TopbarActionButton
                leading={<MagnifyingGlassIcon className="size-4" />}
                onClick={props.onNavigateHome}
                ariaLabel={t("console.search")}
                compactOnMobile
              >
                {t("console.search")}
              </TopbarActionButton>
              <TopbarActionButton
                leading={<Users className="size-4" />}
                onClick={props.onNavigateDevPortal}
                ariaLabel={t("console.access_denied.open_portal")}
                compactOnMobile
              >
                {t("console.access_denied.open_portal")}
              </TopbarActionButton>
              <TopbarActionButton
                leading={<ExitIcon className="size-4" />}
                onClick={() => void handleLogout()}
                ariaLabel={t("console.logout")}
                compactOnMobile
              >
                {t("console.logout")}
              </TopbarActionButton>
            </>
          }
        />
        <main className="mx-auto flex min-h-[calc(100vh-73px)] w-full max-w-md items-center px-4 py-10">
          <Card className="w-full rounded-3xl">
            <CardHeader className="pb-4">
              <CardTitle>{t("console.access_denied.title")}</CardTitle>
              <CardDescription>
                {t("console.access_denied.description")}
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <Alert>
                <AlertDescription>
                  {t("console.access_denied.current_role", {
                    role: roleLabel,
                  })}
                </AlertDescription>
              </Alert>
              <div className="flex flex-wrap gap-3">
                <Button type="button" onClick={props.onNavigateDevPortal}>
                  {t("console.access_denied.open_portal")}
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  onClick={props.onNavigateHome}
                >
                  {t("console.search")}
                </Button>
              </div>
            </CardContent>
          </Card>
        </main>
      </div>
    );
  }

  return (
    <ConsoleProvider value={contextValue}>
      <SidebarProvider>
        <div className="min-h-screen min-w-0 flex-1 bg-background text-foreground">
          <AppTopbar
            theme={props.theme}
            themeMode={props.themeMode}
            onThemeModeChange={props.onThemeModeChange}
            containerClassName="flex w-full flex-col gap-3 px-4 py-3 sm:flex-row sm:items-center sm:justify-between sm:px-6 sm:py-4 lg:px-8 xl:px-10"
            title={`${SITE_NAME} · ${consoleLabel}`}
            onTitleClick={props.onNavigateHome}
            afterControls={
              <>
                <TopbarActionButton
                  leading={<MagnifyingGlassIcon className="size-4" />}
                  onClick={props.onNavigateHome}
                  ariaLabel={t("console.search")}
                  compactOnMobile
                >
                  {t("console.search")}
                </TopbarActionButton>
                <TopbarActionButton
                  leading={<ExitIcon className="size-4" />}
                  onClick={() => void handleLogout()}
                  ariaLabel={t("console.logout")}
                  compactOnMobile
                >
                  {t("console.logout")}
                </TopbarActionButton>
              </>
            }
          />
          <div className="bg-background">
            <div className="flex w-full gap-4 px-4 pb-8 pt-4 sm:px-6 lg:gap-6 lg:px-8 xl:px-10">
              <Sidebar className="md:sticky md:top-[73px] md:h-[calc(100svh-89px)] md:w-72 xl:w-[19rem]">
                {sidebar}
              </Sidebar>

              <SidebarInset className="flex flex-1 flex-col gap-4 pl-0">
                <div className="flex items-center justify-between md:hidden">
                  <SidebarTrigger>{activeTabLabel}</SidebarTrigger>
                </div>
                {activeTab === "overview" && <ConsoleOverview />}
                {activeTab === "users" && <ConsoleUsers />}
                {activeTab === "tasks" && <ConsoleCrawlTasks />}
                {activeTab === "domains" && <ConsoleDomains />}
                {activeTab === "jobs" && <ConsoleJobs />}
                {activeTab === "documents" && <ConsoleDocuments />}
                {activeTab === "workers" && <ConsoleWorkers />}
                {activeTab === "settings" && <ConsoleSettings />}
              </SidebarInset>
            </div>
          </div>
        </div>
      </SidebarProvider>
    </ConsoleProvider>
  );
}

import { ExitIcon, MagnifyingGlassIcon } from "@radix-ui/react-icons";
import { Settings, Users, Bot, FileText, ListTodo, LayoutDashboard, Orbit } from "lucide-react";
import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";

import {
  AdminDeveloperRecord,
  AdminSession,
  CrawlOverview,
  getCrawlOverview,
  getAdminSession,
  listAdminDevelopers,
  listDocuments,
  login,
  logout,
} from "../api";
import { AppTopbar, TopbarActionButton } from "./common/AppTopbar";
import { ConsoleProvider, type ConsoleContextValue } from "./console/ConsoleContext";
import { ConsoleOverview } from "./console/ConsoleOverview";
import { ConsoleUsers } from "./console/ConsoleUsers";
import { ConsoleCrawlTasks } from "./console/ConsoleCrawlTasks";
import { ConsoleWorkers } from "./console/ConsoleWorkers";
import { ConsoleDocuments } from "./console/ConsoleDocuments";
import { ConsoleJobs } from "./console/ConsoleJobs";
import { ConsoleSettings } from "./console/ConsoleSettings";
import type { ThemeMode } from "./ThemeSwitcher";
import { Alert, AlertDescription } from "./ui/alert";
import { Button } from "./ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "./ui/card";
import { Input } from "./ui/input";
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarTrigger,
} from "./ui/sidebar";

const CONSOLE_TOKEN_KEY = "findverse_console_token";
const SITE_NAME = (import.meta.env.VITE_FINDVERSE_SITE_NAME || "FindVerse").trim() || "FindVerse";

type ConsoleTab = "overview" | "users" | "tasks" | "jobs" | "workers" | "documents" | "settings";

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

async function refreshConsoleData(
  token: string,
  actions: {
    setOverview: (value: CrawlOverview | null) => void;
    setDevelopers: (value: AdminDeveloperRecord[]) => void;
    setFlash: (value: string | null) => void;
  },
  refreshFailedMessage: string,
  silent = false,
) {
  try {
    const [overview, developers] = await Promise.all([
      getCrawlOverview(token),
      listAdminDevelopers(token),
    ]);
    actions.setOverview(overview);
    actions.setDevelopers(developers);
  } catch (error) {
    if (!silent) {
      actions.setFlash(getErrorMessage(error, refreshFailedMessage));
    }
  }
}

async function refreshDocuments(
  token: string,
  actions: {
    setDocuments: (value: Awaited<ReturnType<typeof listDocuments>> | null) => void;
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
}) {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<ConsoleTab>("overview");
  const [token, setToken] = useState<string | null>(() => localStorage.getItem(CONSOLE_TOKEN_KEY));
  const [session, setSession] = useState<AdminSession | null>(null);
  const [overview, setOverview] = useState<CrawlOverview | null>(null);
  const [developers, setDevelopers] = useState<AdminDeveloperRecord[]>([]);
  const [documents, setDocuments] = useState<Awaited<ReturnType<typeof listDocuments>> | null>(null);
  const [authLoading, setAuthLoading] = useState(Boolean(token));
  const [busy, setBusy] = useState(false);
  const [loginError, setLoginError] = useState<string | null>(null);
  const [loginUsername, setLoginUsername] = useState("");
  const [loginPassword, setLoginPassword] = useState("");

  const consoleLabel = t("console.title").startsWith(SITE_NAME)
    ? t("console.title").slice(SITE_NAME.length).trim()
    : t("console.title");

  const setFlash = useCallback((value: string | null) => {
    if (!value) {
      toast.dismiss();
      return;
    }
    toast(value);
  }, []);

  const refreshAll = useCallback(
    () =>
      token
        ? refreshConsoleData(
            token,
            {
              setOverview,
              setDevelopers,
              setFlash,
            },
            t("console.refresh_failed"),
          )
        : Promise.resolve(),
    [token, setFlash, t],
  );

  const refreshDocumentList = useCallback(
    () =>
      token
        ? refreshDocuments(
            token,
            {
              setDocuments,
              setFlash,
            },
            t("console.refresh_failed"),
          )
        : Promise.resolve(),
    [token, setFlash, t],
  );

  useEffect(() => {
    if (!token) {
      setAuthLoading(false);
      setSession(null);
      return;
    }

    let cancelled = false;
    setAuthLoading(true);
    getAdminSession(token)
      .then((nextSession) => {
        if (!cancelled) {
          setSession(nextSession);
        }
      })
      .catch(() => {
        if (!cancelled) {
          localStorage.removeItem(CONSOLE_TOKEN_KEY);
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
    if (!token || !session) {
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
            setDevelopers,
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
  }, [token, session, setFlash, t]);

  useEffect(() => {
    if (!token || !session) {
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
  }, [token, session, setFlash, t]);

  async function handleLogin(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setBusy(true);
    setLoginError(null);
    try {
      const nextSession = await login(loginUsername, loginPassword);
      localStorage.setItem(CONSOLE_TOKEN_KEY, nextSession.token);
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
      await logout(token);
    } catch {
      // Ignore logout failures and clear local state anyway.
    } finally {
      localStorage.removeItem(CONSOLE_TOKEN_KEY);
      setToken(null);
      setSession(null);
      setOverview(null);
      setDevelopers([]);
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
      developers,
      documents,
    }),
    [token, busy, setFlash, refreshAll, refreshDocumentList, overview, developers, documents],
  );

  const tabItems = [
    { key: "overview" as const, label: t("console.tabs.overview"), icon: LayoutDashboard },
    { key: "users" as const, label: t("console.tabs.users"), icon: Users },
    { key: "tasks" as const, label: t("console.tabs.tasks"), icon: Orbit },
    { key: "jobs" as const, label: t("console.tabs.jobs"), icon: ListTodo },
    { key: "workers" as const, label: t("console.tabs.workers"), icon: Bot },
    { key: "documents" as const, label: t("console.tabs.documents"), icon: FileText },
    { key: "settings" as const, label: t("console.tabs.settings"), icon: Settings },
  ];

  const sidebar = (
    <SidebarContent>
      <SidebarGroup>
        <SidebarGroupLabel>{t("console.title")}</SidebarGroupLabel>
        <SidebarMenu>
          {tabItems.map((item) => {
            const Icon = item.icon;
            const active = activeTab === item.key;
            return (
              <SidebarMenuItem key={item.key}>
                <SidebarMenuButton
                  isActive={active}
                  onClick={() => {
                    setActiveTab(item.key);
                  }}
                >
                  <span className="flex items-center gap-3">
                    <Icon data-icon="inline-start" />
                    <span className="font-medium">{item.label}</span>
                  </span>
                </SidebarMenuButton>
              </SidebarMenuItem>
            );
          })}
        </SidebarMenu>
      </SidebarGroup>
    </SidebarContent>
  );

  if (authLoading) {
    return <div className="grid min-h-screen place-items-center bg-background text-foreground">{t("console.login.checking")}</div>;
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
                  {busy ? t("console.login.submitting") : t("console.login.submit")}
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

  return (
    <ConsoleProvider value={contextValue}>
      <SidebarProvider>
        <div className="min-h-screen min-w-0 flex-1 bg-background text-foreground">
          <AppTopbar
            theme={props.theme}
            themeMode={props.themeMode}
            onThemeModeChange={props.onThemeModeChange}
            containerClassName="flex w-full flex-col gap-4 px-4 py-4 sm:flex-row sm:items-center sm:justify-between sm:px-6 lg:px-8 xl:px-10"
            title={`${SITE_NAME} · ${consoleLabel}`}
            onTitleClick={props.onNavigateHome}
            afterControls={
              <>
                <TopbarActionButton
                  leading={<MagnifyingGlassIcon className="size-4" />}
                  onClick={props.onNavigateHome}
                >
                  {t("console.search")}
                </TopbarActionButton>
                <TopbarActionButton
                  leading={<ExitIcon className="size-4" />}
                  onClick={() => void handleLogout()}
                >
                  {t("console.logout")}
                </TopbarActionButton>
              </>
            }
          />
          <div className="bg-background">
            <div className="flex w-full px-4 pb-8 pt-4 sm:px-6 lg:px-8 xl:px-10">
              <Sidebar className="md:sticky md:top-[73px] md:h-[calc(100svh-73px)]">
                {sidebar}
              </Sidebar>

              <SidebarInset className="flex flex-1 flex-col gap-4 pl-0 md:pl-4">
                <div className="flex items-center justify-between md:hidden">
                  <SidebarTrigger>{t("console.title")}</SidebarTrigger>
                </div>
                {activeTab === "overview" && <ConsoleOverview />}
                {activeTab === "users" && <ConsoleUsers />}
                {activeTab === "tasks" && <ConsoleCrawlTasks />}
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

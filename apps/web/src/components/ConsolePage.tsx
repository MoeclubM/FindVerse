import { ExitIcon, MagnifyingGlassIcon } from "@radix-ui/react-icons";
import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

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
import { SectionHeader, StatStrip } from "./common/PanelPrimitives";
import { ConsoleProvider, type ConsoleContextValue } from "./console/ConsoleContext";
import { ConsoleOverview } from "./console/ConsoleOverview";
import { ConsoleUsers } from "./console/ConsoleUsers";
import { ConsoleCrawlTasks } from "./console/ConsoleCrawlTasks";
import { ConsoleWorkers } from "./console/ConsoleWorkers";
import { ConsoleDocuments } from "./console/ConsoleDocuments";
import { ConsoleJobs } from "./console/ConsoleJobs";
import { ConsoleSettings } from "./console/ConsoleSettings";
import type { ThemeMode } from "./ThemeSwitcher";

const CONSOLE_TOKEN_KEY = "findverse_console_token";
const SITE_NAME = (import.meta.env.VITE_FINDVERSE_SITE_NAME || "FindVerse").trim() || "FindVerse";
const ONLINE_THRESHOLD_MS = 5 * 60 * 1000;

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

function isCrawlerOnline(lastSeenAt: string | null) {
  if (!lastSeenAt) {
    return false;
  }
  return Date.now() - new Date(lastSeenAt).getTime() < ONLINE_THRESHOLD_MS;
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
  const [toasts, setToasts] = useState<Array<{ id: number; message: string }>>([]);
  const [loginUsername, setLoginUsername] = useState("");
  const [loginPassword, setLoginPassword] = useState("");
  const toastIdRef = useRef(0);
  const toastTimeoutsRef = useRef<number[]>([]);

  const consoleLabel = t("console.title").startsWith(SITE_NAME)
    ? t("console.title").slice(SITE_NAME.length).trim()
    : t("console.title");
  const activeCrawlerCount =
    overview?.crawlers.filter((crawler) => isCrawlerOnline(crawler.last_seen_at)).length ?? 0;
  const enabledRuleCount = overview?.rules.filter((rule) => rule.enabled).length ?? 0;

  useEffect(
    () => () => {
      for (const timeoutId of toastTimeoutsRef.current) {
        window.clearTimeout(timeoutId);
      }
      toastTimeoutsRef.current = [];
    },
    [],
  );

  const setFlash = useCallback((value: string | null) => {
    if (!value) {
      for (const timeoutId of toastTimeoutsRef.current) {
        window.clearTimeout(timeoutId);
      }
      toastTimeoutsRef.current = [];
      setToasts([]);
      return;
    }

    const id = ++toastIdRef.current;
    setToasts((current) => [...current, { id, message: value }]);

    const timeoutId = window.setTimeout(() => {
      setToasts((current) => current.filter((toast) => toast.id !== id));
      toastTimeoutsRef.current = toastTimeoutsRef.current.filter((currentId) => currentId !== timeoutId);
    }, 3600);
    toastTimeoutsRef.current.push(timeoutId);
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
    { key: "overview" as const, label: t("console.tabs.overview"), badge: overview?.recent_events.length ?? 0 },
    { key: "users" as const, label: t("console.tabs.users"), badge: developers.length },
    { key: "tasks" as const, label: t("console.tabs.tasks"), badge: enabledRuleCount },
    { key: "jobs" as const, label: t("console.tabs.jobs"), badge: overview?.in_flight_jobs ?? 0 },
    { key: "workers" as const, label: t("console.tabs.workers"), badge: activeCrawlerCount },
    { key: "documents" as const, label: t("console.tabs.documents"), badge: overview?.indexed_documents ?? 0 },
    { key: "settings" as const, label: t("console.tabs.settings"), badge: null },
  ];

  if (authLoading) {
    return <div className="console-loading">{t("console.login.checking")}</div>;
  }

  if (!session || !token) {
    return (
      <div className="console-page">
        <AppTopbar
          theme={props.theme}
          themeMode={props.themeMode}
          onThemeModeChange={props.onThemeModeChange}
          title={`${SITE_NAME} · ${consoleLabel}`}
          onTitleClick={props.onNavigateHome}
          afterControls={
            <TopbarActionButton
              theme={props.theme}
              leading={<MagnifyingGlassIcon className="size-4" />}
              onClick={props.onNavigateHome}
            >
              {t("console.login.search_link")}
            </TopbarActionButton>
          }
        />
        <main className="console-login">
          <h1>{t("console.login.title")}</h1>
          <form onSubmit={handleLogin}>
            <input
              value={loginUsername}
              onChange={(event) => setLoginUsername(event.target.value)}
              placeholder={t("console.login.username")}
            />
            <input
              type="password"
              value={loginPassword}
              onChange={(event) => setLoginPassword(event.target.value)}
              placeholder={t("console.login.password")}
            />
            <button type="submit" disabled={busy}>
              {busy ? t("console.login.submitting") : t("console.login.submit")}
            </button>
          </form>
          {loginError ? <p className="search-error">{loginError}</p> : null}
        </main>
      </div>
    );
  }

  return (
    <ConsoleProvider value={contextValue}>
      <div className="console-page">
        <AppTopbar
          theme={props.theme}
          themeMode={props.themeMode}
          onThemeModeChange={props.onThemeModeChange}
          title={`${SITE_NAME} · ${consoleLabel}`}
          onTitleClick={props.onNavigateHome}
          afterControls={
            <>
              <TopbarActionButton
                theme={props.theme}
                leading={<MagnifyingGlassIcon className="size-4" />}
                onClick={props.onNavigateHome}
              >
                {t("console.search")}
              </TopbarActionButton>
              <TopbarActionButton
                theme={props.theme}
                leading={<ExitIcon className="size-4" />}
                onClick={() => void handleLogout()}
              >
                {t("console.logout")}
              </TopbarActionButton>
            </>
          }
        />
        {toasts.length ? (
          <div className="console-toast-stack" aria-live="polite" aria-atomic="true">
            {toasts.map((toast) => (
              <div key={toast.id} className="console-toast">
                {toast.message}
              </div>
            ))}
          </div>
        ) : null}

        <div className="console-shell">
          <aside className="console-sidebar">
            <section className="panel compact-panel console-sidebar-panel">
              <SectionHeader
                title={t("console.title")}
                meta={t("console.live_refresh")}
                heading="h3"
              />
              <div className="console-sidebar-user">
                <span>{t("console.summary.console_user")}</span>
                <strong>{session.username}</strong>
              </div>
              <nav className="console-tabs">
                {tabItems.map((item) => (
                  <button
                    key={item.key}
                    className={activeTab === item.key ? "active" : ""}
                    onClick={() => setActiveTab(item.key)}
                  >
                    <span>{item.label}</span>
                    {item.badge != null ? <strong>{item.badge}</strong> : null}
                  </button>
                ))}
              </nav>
            </section>
          </aside>

          <main className="console-content">
            <section className="panel panel-wide compact-panel">
              <SectionHeader
                title={tabItems.find((item) => item.key === activeTab)?.label}
                meta={t("console.live_refresh")}
                heading="h3"
              />
              <StatStrip
                items={[
                  { label: t("console.summary.indexed_docs"), value: overview?.indexed_documents ?? 0 },
                  { label: t("console.summary.queued_jobs"), value: overview?.frontier_depth ?? 0 },
                  { label: t("console.overview.in_flight"), value: overview?.in_flight_jobs ?? 0 },
                  { label: t("console.summary.workers"), value: activeCrawlerCount },
                  { label: t("console.overview.active_rules"), value: enabledRuleCount },
                  { label: t("console.summary.failures"), value: overview?.terminal_failures ?? 0 },
                ]}
              />
            </section>

            {activeTab === "overview" && <ConsoleOverview />}
            {activeTab === "users" && <ConsoleUsers />}
            {activeTab === "tasks" && <ConsoleCrawlTasks />}
            {activeTab === "jobs" && <ConsoleJobs />}
            {activeTab === "documents" && <ConsoleDocuments />}
            {activeTab === "workers" && <ConsoleWorkers />}
            {activeTab === "settings" && <ConsoleSettings />}
          </main>
        </div>
      </div>
    </ConsoleProvider>
  );
}

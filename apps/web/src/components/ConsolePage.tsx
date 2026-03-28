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
import { StatStrip } from "./common/PanelPrimitives";
import { ConsoleProvider, ConsoleContextValue } from "./console/ConsoleContext";
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
) {
  try {
    const [overview, developers] = await Promise.all([
      getCrawlOverview(token),
      listAdminDevelopers(token),
    ]);
    actions.setOverview(overview);
    actions.setDevelopers(developers);
  } catch (error) {
    actions.setFlash(getErrorMessage(error, refreshFailedMessage));
  }
}

async function refreshDocuments(
  token: string,
  actions: {
    setDocuments: (value: Awaited<ReturnType<typeof listDocuments>> | null) => void;
    setFlash: (value: string | null) => void;
  },
  refreshFailedMessage: string,
) {
  try {
    const documents = await listDocuments(token);
    actions.setDocuments(documents);
  } catch (error) {
    actions.setFlash(getErrorMessage(error, refreshFailedMessage));
  }
}

export function ConsolePage(props: {
  theme: "light" | "dark";
  themeMode: ThemeMode;
  onThemeModeChange: (theme: ThemeMode) => void;
  onNavigateHome: () => void;
}) {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<"overview" | "users" | "tasks" | "jobs" | "workers" | "documents" | "settings">("overview");
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
    overview?.crawlers.filter((crawler) => {
      if (!crawler.last_seen_at) return false;
      return Date.now() - new Date(crawler.last_seen_at).getTime() < 5 * 60 * 1000;
    }).length ?? 0;
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
        ? refreshConsoleData(token, {
            setOverview,
            setDevelopers,
            setFlash,
          }, t("console.refresh_failed"))
        : Promise.resolve(),
    [token, setFlash, t],
  );

  const refreshDocumentList = useCallback(
    () =>
      token
        ? refreshDocuments(token, {
            setDocuments,
            setFlash,
          }, t("console.refresh_failed"))
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
    void refreshAll();
  }, [token, session, refreshAll]);

  useEffect(() => {
    if (!token || !session) {
      return;
    }
    const timer = window.setTimeout(() => {
      void refreshDocumentList();
    }, 250);
    return () => window.clearTimeout(timer);
  }, [token, session, refreshDocumentList]);

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

        <nav className="console-tabs">
          <button className={activeTab === "overview" ? "active" : ""} onClick={() => setActiveTab("overview")}>{t("console.tabs.overview")}</button>
          <button className={activeTab === "users" ? "active" : ""} onClick={() => setActiveTab("users")}>{t("console.tabs.users")}</button>
          <button className={activeTab === "tasks" ? "active" : ""} onClick={() => setActiveTab("tasks")}>{t("console.tabs.tasks")}</button>
          <button className={activeTab === "jobs" ? "active" : ""} onClick={() => setActiveTab("jobs")}>{t("console.tabs.jobs")}</button>
          <button className={activeTab === "workers" ? "active" : ""} onClick={() => setActiveTab("workers")}>{t("console.tabs.workers")}</button>
          <button className={activeTab === "documents" ? "active" : ""} onClick={() => setActiveTab("documents")}>{t("console.tabs.documents")}</button>
          <button className={activeTab === "settings" ? "active" : ""} onClick={() => setActiveTab("settings")}>{t("console.tabs.settings")}</button>
        </nav>

        <main className="console-grid">
          <section className="panel panel-wide compact-panel">
            <StatStrip
              items={[
                { label: t("console.summary.console_user"), value: session.username },
                { label: t("console.summary.indexed_docs"), value: overview?.indexed_documents ?? 0 },
                { label: t("console.overview.known_urls"), value: overview?.known_urls ?? 0 },
                { label: t("console.summary.queued_jobs"), value: overview?.frontier_depth ?? 0 },
                { label: t("console.overview.in_flight"), value: overview?.in_flight_jobs ?? 0 },
                { label: t("console.summary.workers"), value: activeCrawlerCount },
                { label: t("console.overview.active_rules"), value: enabledRuleCount },
                { label: t("console.summary.duplicates"), value: overview?.duplicate_documents ?? 0 },
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
    </ConsoleProvider>
  );
}

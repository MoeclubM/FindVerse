import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";

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
import { ConsoleProvider, ConsoleContextValue } from "./console/ConsoleContext";
import { ConsoleOverview } from "./console/ConsoleOverview";
import { ConsoleUsers } from "./console/ConsoleUsers";
import { ConsoleCrawlTasks } from "./console/ConsoleCrawlTasks";
import { ConsoleWorkers } from "./console/ConsoleWorkers";
import { ConsoleDocuments } from "./console/ConsoleDocuments";
import { ConsoleJobs } from "./console/ConsoleJobs";
import { ConsoleSettings } from "./console/ConsoleSettings";

const CONSOLE_TOKEN_KEY = "findverse_console_token";

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
) {
  try {
    const [overview, developers] = await Promise.all([
      getCrawlOverview(token),
      listAdminDevelopers(token),
    ]);
    actions.setOverview(overview);
    actions.setDevelopers(developers);
  } catch (error) {
    actions.setFlash(getErrorMessage(error, "Refresh failed"));
  }
}

async function refreshDocuments(
  token: string,
  actions: {
    setDocuments: (value: Awaited<ReturnType<typeof listDocuments>> | null) => void;
    setFlash: (value: string | null) => void;
  },
) {
  try {
    const documents = await listDocuments(token);
    actions.setDocuments(documents);
  } catch (error) {
    actions.setFlash(getErrorMessage(error, "Refresh failed"));
  }
}

export function ConsolePage(props: { onNavigateHome: () => void }) {
  const [activeTab, setActiveTab] = useState<"overview" | "users" | "tasks" | "jobs" | "workers" | "documents" | "settings">("overview");
  const [token, setToken] = useState<string | null>(() => localStorage.getItem(CONSOLE_TOKEN_KEY));
  const [session, setSession] = useState<AdminSession | null>(null);
  const [overview, setOverview] = useState<CrawlOverview | null>(null);
  const [developers, setDevelopers] = useState<AdminDeveloperRecord[]>([]);
  const [documents, setDocuments] = useState<Awaited<ReturnType<typeof listDocuments>> | null>(null);
  const [authLoading, setAuthLoading] = useState(Boolean(token));
  const [busy, setBusy] = useState(false);
  const [flash, setFlash] = useState<string | null>(null);
  const [loginUsername, setLoginUsername] = useState("");
  const [loginPassword, setLoginPassword] = useState("");

  const refreshAll = useCallback(
    () =>
      token
        ? refreshConsoleData(token, {
            setOverview,
            setDevelopers,
            setFlash,
          })
        : Promise.resolve(),
    [token],
  );

  const refreshDocumentList = useCallback(
    () =>
      token
        ? refreshDocuments(token, {
            setDocuments,
            setFlash,
          })
        : Promise.resolve(),
    [token],
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
    setFlash(null);
    try {
      const nextSession = await login(loginUsername, loginPassword);
      localStorage.setItem(CONSOLE_TOKEN_KEY, nextSession.token);
      setToken(nextSession.token);
      setSession(nextSession);
    } catch (error) {
      setFlash(getErrorMessage(error, "Login failed"));
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
    [token, busy, refreshAll, refreshDocumentList, overview, developers, documents],
  );

  if (authLoading) {
    return <div className="console-loading">Checking session…</div>;
  }

  if (!session || !token) {
    return (
      <div className="console-page">
        <header className="console-topbar">
          <button type="button" className="plain-link" onClick={props.onNavigateHome}>
            Search
          </button>
        </header>
        <main className="console-login">
          <h1>Sign in</h1>
          <form onSubmit={handleLogin}>
            <input
              value={loginUsername}
              onChange={(event) => setLoginUsername(event.target.value)}
              placeholder="Username"
            />
            <input
              type="password"
              value={loginPassword}
              onChange={(event) => setLoginPassword(event.target.value)}
              placeholder="Password"
            />
            <button type="submit" disabled={busy}>
              {busy ? "Signing in…" : "Sign in"}
            </button>
          </form>
          {flash ? <p className="search-error">{flash}</p> : null}
        </main>
      </div>
    );
  }

  return (
    <ConsoleProvider value={contextValue}>
      <div className="console-page">
        <header className="console-topbar">
          <div>
            <strong>FindVerse Console</strong>
            <span>{session.username} · {session.user_id}</span>
          </div>
          <div className="topbar-actions">
            <button
              type="button"
              className="plain-link"
              onClick={() =>
                refreshAll().then(refreshDocumentList)
              }
            >
              Refresh
            </button>
            <button type="button" className="plain-link" onClick={props.onNavigateHome}>
              Search
            </button>
            <button type="button" className="plain-link" onClick={() => void handleLogout()}>
              Sign out
            </button>
          </div>
        </header>

        {flash ? <div className="flash">{flash}</div> : null}

        <nav className="console-tabs">
          <button className={activeTab === "overview" ? "active" : ""} onClick={() => setActiveTab("overview")}>Overview</button>
          <button className={activeTab === "users" ? "active" : ""} onClick={() => setActiveTab("users")}>Users</button>
          <button className={activeTab === "tasks" ? "active" : ""} onClick={() => setActiveTab("tasks")}>Crawl Tasks</button>
          <button className={activeTab === "jobs" ? "active" : ""} onClick={() => setActiveTab("jobs")}>Jobs</button>
          <button className={activeTab === "workers" ? "active" : ""} onClick={() => setActiveTab("workers")}>Workers</button>
          <button className={activeTab === "documents" ? "active" : ""} onClick={() => setActiveTab("documents")}>Documents</button>
          <button className={activeTab === "settings" ? "active" : ""} onClick={() => setActiveTab("settings")}>Settings</button>
        </nav>

        <main className="console-grid">
          <section className="panel panel-wide compact-panel">
            <div className="summary-strip">
              <div>
                <span>Console user</span>
                <strong>{session.username}</strong>
              </div>
              <div>
                <span>Indexed docs</span>
                <strong>{overview?.indexed_documents ?? 0}</strong>
              </div>
              <div>
                <span>Queued jobs</span>
                <strong>{overview?.frontier_depth ?? 0}</strong>
              </div>
              <div>
                <span>Workers</span>
                <strong>{overview?.crawlers.length ?? 0}</strong>
              </div>
              <div>
                <span>Rules</span>
                <strong>{overview?.rules.length ?? 0}</strong>
              </div>
              <div>
                <span>Duplicates</span>
                <strong>{overview?.duplicate_documents ?? 0}</strong>
              </div>
              <div>
                <span>Failures</span>
                <strong>{overview?.terminal_failures ?? 0}</strong>
              </div>
            </div>
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

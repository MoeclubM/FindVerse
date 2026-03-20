import { FormEvent, useEffect, useState } from "react";

import {
  CreatedApiKey,
  DevSession,
  DeveloperUsage,
  createDeveloperKey,
  getDeveloperKeys,
  getDeveloperSession,
  loginDeveloper,
  logoutDeveloper,
  registerDeveloper,
  revokeDeveloperKey,
} from "../api";

const DEV_SESSION_KEY = "findverse_dev_session";

function persistDevSession(token: string | null, setToken: (value: string | null) => void) {
  if (token) {
    localStorage.setItem(DEV_SESSION_KEY, token);
  } else {
    localStorage.removeItem(DEV_SESSION_KEY);
  }
  setToken(token);
}

function tokenPreview(token: string) {
  return `${token.slice(0, 8)}...${token.slice(-4)}`;
}

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

export function DevPortalPage(props: {
  devToken: string | null;
  onTokenChange: (token: string | null) => void;
  onNavigateSearch: () => void;
}) {
  const [sessionToken, setSessionToken] = useState<string | null>(() => localStorage.getItem(DEV_SESSION_KEY));
  const [session, setSession] = useState<DevSession | null>(null);
  const [usage, setUsage] = useState<DeveloperUsage | null>(null);
  const [mode, setMode] = useState<"login" | "register">("login");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [keyName, setKeyName] = useState("Search key");
  const [latestKey, setLatestKey] = useState<CreatedApiKey | null>(null);
  const [busy, setBusy] = useState(false);
  const [loadingSession, setLoadingSession] = useState(Boolean(sessionToken));
  const [flash, setFlash] = useState<string | null>(null);

  useEffect(() => {
    if (!sessionToken) {
      setLoadingSession(false);
      setSession(null);
      setUsage(null);
      return;
    }

    let cancelled = false;
    setLoadingSession(true);
    Promise.all([getDeveloperSession(sessionToken), getDeveloperKeys(sessionToken)])
      .then(([nextSession, nextUsage]) => {
        if (!cancelled) {
          setSession(nextSession);
          setUsage(nextUsage);
        }
      })
      .catch(() => {
        if (!cancelled) {
          persistDevSession(null, setSessionToken);
          setSession(null);
          setUsage(null);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoadingSession(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [sessionToken]);

  const activePreview = props.devToken ? tokenPreview(props.devToken) : null;

  async function refreshUsage(token: string) {
    const nextUsage = await getDeveloperKeys(token);
    setUsage(nextUsage);
  }

  async function handleAuthSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setBusy(true);
    setFlash(null);
    try {
      const nextSession =
        mode === "register"
          ? await registerDeveloper(username, password)
          : await loginDeveloper(username, password);
      persistDevSession(nextSession.token, setSessionToken);
      setSession(nextSession);
      setUsername("");
      setPassword("");
      setLatestKey(null);
      await refreshUsage(nextSession.token);
    } catch (error) {
      setFlash(getErrorMessage(error, mode === "register" ? "Register failed" : "Login failed"));
    } finally {
      setBusy(false);
    }
  }

  async function handleSignOut() {
    setBusy(true);
    setFlash(null);
    try {
      if (sessionToken) {
        await logoutDeveloper(sessionToken);
      }
    } catch {
      // Ignore logout failures and clear local state anyway.
    } finally {
      persistDevSession(null, setSessionToken);
      setSession(null);
      setUsage(null);
      setLatestKey(null);
      props.onTokenChange(null);
      setBusy(false);
    }
  }

  async function handleCreateKey(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!sessionToken) {
      return;
    }

    setBusy(true);
    setFlash(null);
    try {
      const created = await createDeveloperKey(sessionToken, keyName);
      setLatestKey(created);
      setKeyName("Search key");
      await refreshUsage(sessionToken);
    } catch (error) {
      setFlash(getErrorMessage(error, "API key creation failed"));
    } finally {
      setBusy(false);
    }
  }

  async function handleRevokeKey(id: string, preview: string) {
    if (!sessionToken) {
      return;
    }

    setBusy(true);
    setFlash(null);
    try {
      await revokeDeveloperKey(sessionToken, id);
      if (activePreview === preview) {
        props.onTokenChange(null);
      }
      if (latestKey?.id === id) {
        setLatestKey(null);
      }
      await refreshUsage(sessionToken);
    } catch (error) {
      setFlash(getErrorMessage(error, "API key revoke failed"));
    } finally {
      setBusy(false);
    }
  }

  function handleUseSearchKey(token: string) {
    props.onTokenChange(token);
    setFlash("Active search key updated");
  }

  if (loadingSession) {
    return <div className="console-loading">Checking developer session...</div>;
  }

  if (!session || !sessionToken) {
    return (
      <div className="console-page">
        <header className="console-topbar">
          <strong>FindVerse Developer Portal</strong>
          {props.devToken ? (
            <div className="topbar-actions">
              <span className="status-pill">Search key active</span>
              <button type="button" className="plain-link" onClick={props.onNavigateSearch}>
                Search
              </button>
            </div>
          ) : null}
        </header>
        <main className="console-login">
          <div className="auth-mode-switch">
            <button
              type="button"
              className={mode === "login" ? "active" : ""}
              onClick={() => setMode("login")}
            >
              Sign in
            </button>
            <button
              type="button"
              className={mode === "register" ? "active" : ""}
              onClick={() => setMode("register")}
            >
              Register
            </button>
          </div>
          <h1>{mode === "register" ? "Create developer account" : "Developer sign in"}</h1>
          <form onSubmit={handleAuthSubmit}>
            <input
              value={username}
              onChange={(event) => setUsername(event.target.value)}
              placeholder="Username"
              autoComplete="username"
            />
            <input
              type="password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              placeholder="Password"
              autoComplete={mode === "register" ? "new-password" : "current-password"}
            />
            <button type="submit" disabled={busy}>
              {busy ? "Submitting..." : mode === "register" ? "Create account" : "Sign in"}
            </button>
          </form>
          {flash ? <p className="search-error">{flash}</p> : null}
          <p className="dev-hint">
            Create an account, generate an <code>fvk_</code> key, then use that key for search API access.
          </p>
        </main>
      </div>
    );
  }

  return (
    <div className="console-page">
      <header className="console-topbar">
        <div>
          <strong>FindVerse Developer Portal</strong>
          <span>{session.username}</span>
        </div>
        <div className="topbar-actions">
          {props.devToken ? <span className="status-pill">Search key active</span> : null}
          <button type="button" className="plain-link" onClick={props.onNavigateSearch}>
            Search
          </button>
          <button type="button" className="plain-link" disabled={busy} onClick={() => void handleSignOut()}>
            Sign out
          </button>
        </div>
      </header>

      {flash ? <div className="flash">{flash}</div> : null}

      <main className="console-grid">
        <section className="panel">
          <h2>Account</h2>
          <div className="stats-grid single-column-stats">
            <div>
              <span>User</span>
              <strong>{session.username}</strong>
            </div>
            <div>
              <span>QPS</span>
              <strong>{usage?.qps_limit ?? 0}</strong>
            </div>
            <div>
              <span>Daily quota</span>
              <strong>{usage?.daily_limit ?? 0}</strong>
            </div>
            <div>
              <span>Used today</span>
              <strong>{usage?.used_today ?? 0}</strong>
            </div>
          </div>
        </section>

        <section className="panel">
          <h2>Create API key</h2>
          <form onSubmit={handleCreateKey}>
            <input
              value={keyName}
              onChange={(event) => setKeyName(event.target.value)}
              placeholder="Key name"
            />
            <button type="submit" disabled={busy}>
              {busy ? "Creating..." : "Create key"}
            </button>
          </form>
          <p className="dev-hint">Raw keys are only shown once. Save them before leaving this page.</p>
          {latestKey ? (
            <div className="key-reveal">
              <pre>{latestKey.token}</pre>
              <div className="topbar-actions">
                <button type="button" onClick={() => handleUseSearchKey(latestKey.token)}>
                  Use for search
                </button>
              </div>
            </div>
          ) : null}
        </section>

        <section className="panel panel-wide">
          <h2>API keys</h2>
          <div className="list">
            {usage?.keys.length ? (
              usage.keys.map((key) => (
                <div className="list-row stacked" key={key.id}>
                  <div className="user-row-header">
                    <strong>{key.name}</strong>
                    <div className="topbar-actions">
                      {activePreview === key.preview ? <span className="status-pill">Active</span> : null}
                      {latestKey?.id === key.id ? (
                        <button type="button" onClick={() => handleUseSearchKey(latestKey.token)}>
                          Use for search
                        </button>
                      ) : null}
                      <button
                        type="button"
                        className="plain-link"
                        disabled={busy || Boolean(key.revoked_at)}
                        onClick={() => void handleRevokeKey(key.id, key.preview)}
                      >
                        {key.revoked_at ? "Revoked" : "Revoke"}
                      </button>
                    </div>
                  </div>
                  <div>{key.preview}</div>
                  <div>{key.revoked_at ? `revoked ${key.revoked_at}` : `created ${key.created_at}`}</div>
                </div>
              ))
            ) : (
              <div className="list-row">No API keys yet.</div>
            )}
          </div>
        </section>
      </main>
    </div>
  );
}

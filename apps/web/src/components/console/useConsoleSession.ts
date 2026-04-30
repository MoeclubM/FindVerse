import { useEffect, useState } from "react";

import { getUserSession, type UserSession } from "../../api";

const USER_SESSION_KEY = "findverse_user_session";

export function useConsoleSession() {
  const [token, setToken] = useState<string | null>(() =>
    localStorage.getItem(USER_SESSION_KEY),
  );
  const [session, setSession] = useState<UserSession | null>(null);
  const [authLoading, setAuthLoading] = useState(Boolean(token));

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

  function storeSession(nextSession: UserSession) {
    localStorage.setItem(USER_SESSION_KEY, nextSession.token);
    setToken(nextSession.token);
    setSession(nextSession);
  }

  function clearSession() {
    localStorage.removeItem(USER_SESSION_KEY);
    setToken(null);
    setSession(null);
  }

  return {
    token,
    session,
    authLoading,
    setSession,
    setToken,
    storeSession,
    clearSession,
  };
}

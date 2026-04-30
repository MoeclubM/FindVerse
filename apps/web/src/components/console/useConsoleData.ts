import { useCallback, useEffect, useState } from "react";

import {
  getCrawlOverview,
  listAdminUsers,
  listDocuments,
  type AdminUserRecord,
  type CrawlOverview,
} from "../../api";

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

export function useConsoleData(options: {
  token: string | null;
  enabled: boolean;
  setFlash: (value: string | null) => void;
  refreshFailedMessage: string;
}) {
  const { token, enabled, setFlash, refreshFailedMessage } = options;
  const [overview, setOverview] = useState<CrawlOverview | null>(null);
  const [users, setUsers] = useState<AdminUserRecord[]>([]);
  const [documents, setDocuments] = useState<Awaited<
    ReturnType<typeof listDocuments>
  > | null>(null);

  const refreshAll = useCallback(
    () =>
      token && enabled
        ? refreshConsoleData(
            token,
            {
              setOverview,
              setUsers,
              setFlash,
            },
            refreshFailedMessage,
          )
        : Promise.resolve(),
    [token, enabled, setFlash, refreshFailedMessage],
  );

  const refreshDocumentList = useCallback(
    () =>
      token && enabled
        ? refreshDocuments(
            token,
            {
              setDocuments,
              setFlash,
            },
            refreshFailedMessage,
          )
        : Promise.resolve(),
    [token, enabled, setFlash, refreshFailedMessage],
  );

  const clearData = useCallback(() => {
    setOverview(null);
    setUsers([]);
    setDocuments(null);
  }, []);

  useEffect(() => {
    if (!token || !enabled) {
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
          refreshFailedMessage,
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
  }, [token, enabled, setFlash, refreshFailedMessage]);

  useEffect(() => {
    if (!token || !enabled) {
      return;
    }
    const timer = window.setTimeout(() => {
      void refreshDocuments(
        token,
        {
          setDocuments,
          setFlash,
        },
        refreshFailedMessage,
        true,
      );
    }, 150);
    return () => window.clearTimeout(timer);
  }, [token, enabled, setFlash, refreshFailedMessage]);

  return {
    overview,
    users,
    documents,
    refreshAll,
    refreshDocumentList,
    clearData,
  };
}

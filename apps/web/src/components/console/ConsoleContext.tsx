import { createContext, useContext } from "react";

import type {
  AdminUserRecord,
  CrawlOverview,
  DocumentList,
} from "../../api";

export type ConsoleContextValue = {
  token: string;
  busy: boolean;
  setBusy: (value: boolean) => void;
  setFlash: (value: string | null) => void;
  refreshAll: () => Promise<void>;
  refreshDocumentList: () => Promise<void>;

  overview: CrawlOverview | null;
  users: AdminUserRecord[];
  documents: DocumentList | null;
};

const ConsoleContext = createContext<ConsoleContextValue | null>(null);

export const ConsoleProvider = ConsoleContext.Provider;

export function useConsole(): ConsoleContextValue {
  const value = useContext(ConsoleContext);
  if (!value) {
    throw new Error("useConsole must be used within a ConsoleProvider");
  }
  return value;
}

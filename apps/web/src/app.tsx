import { useEffect, useState } from "react";

import { ConsolePage } from "./components/ConsolePage";
import { DevPortalPage } from "./components/DevPortal";
import { SearchPage } from "./components/SearchPage";

const DEV_TOKEN_KEY = "findverse_dev_token";

function navigate(path: string, setPath: (path: string) => void) {
  window.history.pushState({}, "", path);
  setPath(path);
}

export function App() {
  const [path, setPath] = useState(() => window.location.pathname);
  const [devToken, setDevToken] = useState<string | null>(() => localStorage.getItem(DEV_TOKEN_KEY));

  useEffect(() => {
    const onPopState = () => setPath(window.location.pathname);
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, []);

  function handleDevToken(token: string | null) {
    if (token) {
      localStorage.setItem(DEV_TOKEN_KEY, token);
    } else {
      localStorage.removeItem(DEV_TOKEN_KEY);
    }
    setDevToken(token);
  }

  if (path.startsWith("/console")) {
    return <ConsolePage onNavigateHome={() => navigate("/", setPath)} />;
  }

  if (path.startsWith("/dev")) {
    return (
      <DevPortalPage
        devToken={devToken}
        onTokenChange={handleDevToken}
        onNavigateSearch={() => navigate("/", setPath)}
      />
    );
  }

  return (
    <SearchPage
      devToken={devToken}
      onNavigateDev={() => navigate("/dev", setPath)}
      onTokenExpired={() => {
        handleDevToken(null);
      }}
    />
  );
}

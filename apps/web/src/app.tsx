import { useEffect, useState } from "react";
import { Toaster } from "sonner";

import { ConsolePage } from "./components/ConsolePage";
import { DevPortalPage } from "./components/DevPortal";
import { SearchPage } from "./components/SearchPage";
import type { ThemeMode } from "./components/ThemeSwitcher";

const DEV_TOKEN_KEY = "findverse_dev_token";
const THEME_KEY = "findverse_theme";

function navigate(path: string, setPath: (path: string) => void) {
  window.history.pushState({}, "", path);
  setPath(path);
}

export function App() {
  const [path, setPath] = useState(() => window.location.pathname);
  const [devToken, setDevToken] = useState<string | null>(() => localStorage.getItem(DEV_TOKEN_KEY));
  const [themeMode, setThemeMode] = useState<ThemeMode>(() => {
    const savedTheme = localStorage.getItem(THEME_KEY);
    if (savedTheme === "dark" || savedTheme === "light" || savedTheme === "system") {
      return savedTheme;
    }
    return "system";
  });
  const [systemTheme, setSystemTheme] = useState<"light" | "dark">(() =>
    window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light",
  );
  const theme = themeMode === "system" ? systemTheme : themeMode;

  useEffect(() => {
    const onPopState = () => setPath(window.location.pathname);
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, []);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (event: MediaQueryListEvent) => {
      setSystemTheme(event.matches ? "dark" : "light");
    };
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, []);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    localStorage.setItem(THEME_KEY, themeMode);
  }, [theme, themeMode]);

  function handleDevToken(token: string | null) {
    if (token) {
      localStorage.setItem(DEV_TOKEN_KEY, token);
    } else {
      localStorage.removeItem(DEV_TOKEN_KEY);
    }
    setDevToken(token);
  }

  let page = (
    <SearchPage
      devToken={devToken}
      theme={theme}
      themeMode={themeMode}
      onThemeModeChange={setThemeMode}
      onNavigateDev={() => navigate("/dev", setPath)}
      onTokenExpired={() => {
        handleDevToken(null);
      }}
    />
  );

  if (path.startsWith("/console")) {
    page = (
      <ConsolePage
        theme={theme}
        themeMode={themeMode}
        onThemeModeChange={setThemeMode}
        onNavigateHome={() => navigate("/", setPath)}
      />
    );
  }

  if (path.startsWith("/dev")) {
    page = (
      <DevPortalPage
        devToken={devToken}
        theme={theme}
        themeMode={themeMode}
        onThemeModeChange={setThemeMode}
        onTokenChange={handleDevToken}
        onNavigateSearch={() => navigate("/", setPath)}
      />
    );
  }

  return (
    <>
      {page}
      <Toaster closeButton richColors theme={theme} />
    </>
  );
}

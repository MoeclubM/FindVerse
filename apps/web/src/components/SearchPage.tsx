import * as Collapsible from "@radix-ui/react-collapsible";
import {
  CodeIcon,
  ExternalLinkIcon,
  MagnifyingGlassIcon,
  MixerHorizontalIcon,
} from "@radix-ui/react-icons";
import { FormEvent, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { searchWithParams, suggestSearch, type SearchResponse } from "../api";
import { AppTopbar, TopbarActionButton, TopbarBadge } from "./common/AppTopbar";
import { FieldShell } from "./common/PanelPrimitives";
import type { ThemeMode } from "./ThemeSwitcher";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "./ui/select";

const SITE_NAME = (import.meta.env.VITE_FINDVERSE_SITE_NAME || "FindVerse").trim() || "FindVerse";

type SearchFreshness = "all" | "24h" | "7d" | "30d";
type SearchNetwork = "clearnet" | "tor" | null;

type SearchState = {
  query: string;
  site: string;
  lang: string;
  freshness: SearchFreshness;
  network: SearchNetwork;
  offset: number;
};

function currentSearchState(): SearchState {
  const search = new URLSearchParams(window.location.search);
  const freshness = search.get("freshness");
  const network = search.get("network");
  const offset = Number(search.get("offset") ?? "0");

  return {
    query: search.get("q") ?? "",
    site: search.get("site") ?? "",
    lang: search.get("lang") ?? "",
    freshness:
      freshness === "24h" || freshness === "7d" || freshness === "30d" ? freshness : "all",
    network: network === "clearnet" || network === "tor" ? network : null,
    offset: Number.isFinite(offset) && offset > 0 ? offset : 0,
  };
}

function buildSearchUrl(state: SearchState) {
  const search = new URLSearchParams();
  if (state.query) {
    search.set("q", state.query);
  }
  if (state.site) {
    search.set("site", state.site);
  }
  if (state.lang) {
    search.set("lang", state.lang);
  }
  if (state.freshness !== "all") {
    search.set("freshness", state.freshness);
  }
  if (state.network) {
    search.set("network", state.network);
  }
  if (state.offset > 0) {
    search.set("offset", String(state.offset));
  }

  const query = search.toString();
  return query ? `/?${query}` : "/";
}

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

export function SearchPage(props: {
  devToken: string | null;
  theme: "light" | "dark";
  themeMode: ThemeMode;
  onThemeModeChange: (theme: ThemeMode) => void;
  onTokenExpired: () => void;
  onNavigateDev: () => void;
}) {
  const { t } = useTranslation();
  const [submittedSearch, setSubmittedSearch] = useState(currentSearchState);
  const [query, setQuery] = useState(() => submittedSearch.query);
  const [siteFilter, setSiteFilter] = useState(() => submittedSearch.site);
  const [langFilter, setLangFilter] = useState(() => submittedSearch.lang);
  const [freshnessFilter, setFreshnessFilter] = useState<SearchFreshness>(
    () => submittedSearch.freshness,
  );
  const [networkFilter, setNetworkFilter] = useState<SearchNetwork>(() => submittedSearch.network);
  const [filtersOpen, setFiltersOpen] = useState(
    () =>
      Boolean(
        submittedSearch.site ||
          submittedSearch.lang ||
          submittedSearch.freshness !== "all" ||
          submittedSearch.network,
      ),
  );
  const [results, setResults] = useState<SearchResponse | null>(null);
  const [loading, setLoading] = useState(() => Boolean(submittedSearch.query.trim()));
  const [error, setError] = useState<string | null>(null);
  const [usingProtectedSearch, setUsingProtectedSearch] = useState(false);
  const [suggestions, setSuggestions] = useState<string[]>([]);

  useEffect(() => {
    document.title = SITE_NAME;
  }, []);

  useEffect(() => {
    const onPopState = () => {
      const next = currentSearchState();
      setSubmittedSearch(next);
      setQuery(next.query);
      setSiteFilter(next.site);
      setLangFilter(next.lang);
      setFreshnessFilter(next.freshness);
      setNetworkFilter(next.network);
      setFiltersOpen(
        Boolean(next.site || next.lang || next.freshness !== "all" || next.network),
      );
    };

    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, []);

  useEffect(() => {
    const nextQuery = query.trim();
    if (nextQuery.length < 2) {
      setSuggestions([]);
      return;
    }

    let cancelled = false;
    const timer = window.setTimeout(async () => {
      try {
        const response = await suggestSearch(nextQuery);
        if (!cancelled) {
          setSuggestions(
            response.suggestions.filter(
              (suggestion) => suggestion.trim() !== "" && suggestion !== nextQuery,
            ),
          );
        }
      } catch {
        if (!cancelled) {
          setSuggestions([]);
        }
      }
    }, 150);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [query]);

  useEffect(() => {
    if (!submittedSearch.query.trim()) {
      setResults(null);
      setLoading(false);
      setError(null);
      setUsingProtectedSearch(false);
      return;
    }

    let cancelled = false;
    setLoading(true);
    setError(null);
    const runSearch = async () => {
      try {
        const response = await searchWithParams(
          submittedSearch.query,
          {
            offset: submittedSearch.offset,
            site: submittedSearch.site || undefined,
            lang: submittedSearch.lang || undefined,
            freshness:
              submittedSearch.freshness === "all" ? undefined : submittedSearch.freshness,
            network: submittedSearch.network ?? undefined,
          },
          props.devToken ?? undefined,
        );
        if (!cancelled) {
          setResults(response);
          setUsingProtectedSearch(Boolean(props.devToken));
        }
      } catch (nextError) {
        const errorWithStatus = nextError as Error & { status?: number };
        if (!cancelled && errorWithStatus.status === 401 && props.devToken) {
          props.onTokenExpired();
          try {
            const fallbackResponse = await searchWithParams(submittedSearch.query, {
              offset: submittedSearch.offset,
              site: submittedSearch.site || undefined,
              lang: submittedSearch.lang || undefined,
              freshness:
                submittedSearch.freshness === "all" ? undefined : submittedSearch.freshness,
              network: submittedSearch.network ?? undefined,
            });
            if (!cancelled) {
              setResults(fallbackResponse);
              setUsingProtectedSearch(false);
              setError(t("search.stored_key_expired"));
            }
          } catch (fallbackError) {
            if (!cancelled) {
              setResults(null);
              setUsingProtectedSearch(false);
              setError(getErrorMessage(fallbackError, t("search.failed")));
            }
          }
          return;
        }

        if (!cancelled) {
          setResults(null);
          setUsingProtectedSearch(Boolean(props.devToken));
          setError(getErrorMessage(nextError, t("search.failed")));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    };

    void runSearch();

    return () => {
      cancelled = true;
    };
  }, [submittedSearch, props.devToken, props.onTokenExpired, t]);

  function commitSearch(nextSearch: SearchState) {
    window.history.pushState({}, "", buildSearchUrl(nextSearch));
    setSubmittedSearch(nextSearch);
  }

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    commitSearch({
      query: query.trim(),
      site: siteFilter.trim(),
      lang: langFilter.trim().toLowerCase(),
      freshness: freshnessFilter,
      network: networkFilter,
      offset: 0,
    });
  }

  function handleClearFilters() {
    setSiteFilter("");
    setLangFilter("");
    setFreshnessFilter("all");
    setNetworkFilter(null);
    setFiltersOpen(false);

    commitSearch({
      query: query.trim(),
      site: "",
      lang: "",
      freshness: "all",
      network: null,
      offset: 0,
    });
  }

  function handleNextPage() {
    if (results?.next_offset != null) {
      commitSearch({
        ...submittedSearch,
        offset: results.next_offset,
      });
    }
  }

  function handlePrevPage() {
    commitSearch({
      ...submittedSearch,
      offset: Math.max(0, submittedSearch.offset - 10),
    });
  }

  function handleGoHome() {
    setQuery("");
    setSuggestions([]);
    setSiteFilter("");
    setLangFilter("");
    setFreshnessFilter("all");
    setNetworkFilter(null);
    setFiltersOpen(false);
    commitSearch({
      query: "",
      site: "",
      lang: "",
      freshness: "all",
      network: null,
      offset: 0,
    });
  }

  const hasResults = Boolean(results);
  const activeFilterCount =
    Number(Boolean(siteFilter.trim())) +
    Number(Boolean(langFilter.trim())) +
    Number(freshnessFilter !== "all") +
    Number(Boolean(networkFilter));
  const resultsMode = hasResults || loading || error;
  const shellTone = "bg-[var(--fv-bg)] text-[var(--fv-text)]";
  const panelTone = "border-[var(--fv-border)] bg-[var(--fv-panel)]";
  const elevatedPanelTone = "border-[var(--fv-border)] bg-[var(--fv-panel)]";
  const inputTone = "text-[var(--fv-text)] placeholder:text-[var(--fv-text-soft)]";
  const mutedTone = "text-[var(--fv-text-muted)]";
  const secondaryTextTone = "text-[var(--fv-text-soft)]";
  const badgeTone =
    "border-[var(--fv-border)] bg-[var(--fv-accent-soft)] text-[var(--fv-text-muted)]";
  const labelTone =
    "text-[11px] font-medium uppercase tracking-[0.12em] text-[var(--fv-text-muted)]";
  const homeBrandTone = "text-[var(--fv-text)]";
  const freshnessOptions = [
    { value: "all", label: t("search.freshness_all") },
    { value: "24h", label: t("search.freshness_24h") },
    { value: "7d", label: t("search.freshness_7d") },
    { value: "30d", label: t("search.freshness_30d") },
  ];
  const networkOptions = [
    { value: "all", label: t("search.network_all") },
    { value: "clearnet", label: t("search.network_clearnet") },
    { value: "tor", label: t("search.network_tor") },
  ];

  return (
    <div className={`min-h-screen ${shellTone}`}>
      <div className="mx-auto flex min-h-screen w-full max-w-7xl flex-col px-4 py-4 sm:px-6 lg:px-8">
        <AppTopbar
          theme={props.theme}
          themeMode={props.themeMode}
          onThemeModeChange={props.onThemeModeChange}
          containerClassName={
            resultsMode
              ? "flex min-h-10 w-full flex-col gap-4 pb-4 sm:min-h-14 sm:flex-row sm:items-center sm:justify-between"
              : "flex min-h-10 w-full items-center justify-end pb-4 sm:min-h-14"
          }
          title={
            resultsMode ? (
              <span className={`fv-brand-mark fv-brand-mark-compact ${homeBrandTone}`}>
                {SITE_NAME}
              </span>
            ) : undefined
          }
          onTitleClick={resultsMode ? handleGoHome : undefined}
          beforeControls={props.devToken ? <TopbarBadge>Dev</TopbarBadge> : null}
          afterControls={
            <TopbarActionButton
              leading={<CodeIcon className="size-4" />}
              onClick={props.onNavigateDev}
            >
              {t("search.developer_portal")}
            </TopbarActionButton>
          }
        />

        <main
          className={`flex w-full flex-1 ${
            resultsMode ? "items-start justify-center py-6" : "items-start justify-center pt-[11vh] pb-14"
          }`}
        >
          <section
            className={`app-stagger w-full ${resultsMode ? "max-w-[820px]" : "max-w-[720px]"} space-y-5`}
          >
            {!resultsMode ? (
              <div className="flex flex-col items-center gap-3 pb-3 text-center">
                <h1
                  className={`fv-brand-mark text-[clamp(3.8rem,11vw,6.8rem)] ${homeBrandTone}`}
                >
                  {SITE_NAME}
                </h1>
              </div>
            ) : null}
            <form
              onSubmit={handleSubmit}
              className="app-rise-in space-y-3"
            >
              <div
                className={`flex flex-col gap-3 rounded-[28px] border px-4 py-3 sm:flex-row sm:items-center ${elevatedPanelTone}`}
              >
                <div className="flex min-h-12 flex-1 items-center gap-3">
                  <MagnifyingGlassIcon className={`size-4 shrink-0 ${mutedTone}`} />
                  <Input
                    aria-label={t("search.button")}
                    className={`h-full min-h-12 flex-1 border-0 bg-transparent px-0 py-0 text-base shadow-none focus-visible:ring-0 ${inputTone}`}
                    value={query}
                    onChange={(event) => setQuery(event.target.value)}
                    placeholder={t("search.placeholder")}
                  />
                </div>
                <Button
                  type="submit"
                  aria-label={t("search.button")}
                  className="size-11 shrink-0 rounded-full border-[var(--fv-accent)] bg-[var(--fv-accent)] text-white hover:border-[var(--fv-accent-hover)] hover:bg-[var(--fv-accent-hover)]"
                >
                  <MagnifyingGlassIcon className="size-4" />
                </Button>
              </div>

              {suggestions.length > 0 ? (
                <div className="flex flex-wrap gap-2 pt-1">
                  {suggestions.slice(0, 6).map((suggestion) => (
                    <Button
                      key={suggestion}
                      type="button"
                      variant="outline"
                      size="sm"
                      className="rounded-full border-[var(--fv-border)] bg-[var(--fv-panel)] text-[var(--fv-text)] hover:bg-[var(--fv-panel-soft)]"
                      onClick={() => {
                        setQuery(suggestion);
                        commitSearch({
                          query: suggestion,
                          site: siteFilter.trim(),
                          lang: langFilter.trim().toLowerCase(),
                          freshness: freshnessFilter,
                          network: networkFilter,
                          offset: 0,
                        });
                      }}
                    >
                      {suggestion}
                    </Button>
                  ))}
                </div>
              ) : null}

              <Collapsible.Root open={filtersOpen} onOpenChange={setFiltersOpen}>
                <div className="flex flex-wrap items-center gap-2">
                  <Collapsible.Trigger asChild>
                    <Button
                      type="button"
                      variant="outline"
                      className="h-10 rounded-full border-[var(--fv-border)] bg-[var(--fv-panel)] px-4 text-[var(--fv-text)] hover:bg-[var(--fv-panel-soft)]"
                    >
                      <MixerHorizontalIcon className="size-4" />
                      <span>
                        {t(filtersOpen ? "search.filters_hide" : "search.filters_show")}
                      </span>
                      <Badge variant="outline" className={badgeTone}>
                        {activeFilterCount > 0
                          ? t("search.filters_active", { count: activeFilterCount })
                          : t("search.filters_none")}
                      </Badge>
                    </Button>
                  </Collapsible.Trigger>
                  {activeFilterCount > 0 ? (
                    <Button
                      type="button"
                      variant="outline"
                      className="h-10 rounded-full border-[var(--fv-border)] bg-[var(--fv-panel)] px-4 text-[var(--fv-text)] hover:bg-[var(--fv-panel-soft)]"
                      onClick={handleClearFilters}
                    >
                      {t("search.clear_filters")}
                    </Button>
                  ) : null}
                </div>

                <Collapsible.Content className="pt-2">
                  <div className={`grid gap-3 rounded-[24px] border p-4 md:grid-cols-2 xl:grid-cols-4 ${panelTone}`}>
                    <FieldShell className="gap-1.5" label={<span className={labelTone}>{t("search.site_label")}</span>}>
                      <Input
                        aria-label={t("search.site_label")}
                        className={`h-10 rounded-2xl border-[var(--fv-border)] bg-[var(--fv-panel-soft)] text-sm ${inputTone}`}
                        value={siteFilter}
                        onChange={(event) => setSiteFilter(event.target.value)}
                        placeholder={t("search.site_placeholder")}
                      />
                    </FieldShell>
                    <FieldShell className="gap-1.5" label={<span className={labelTone}>{t("search.lang_label")}</span>}>
                      <Input
                        aria-label={t("search.lang_label")}
                        className={`h-10 rounded-2xl border-[var(--fv-border)] bg-[var(--fv-panel-soft)] text-sm ${inputTone}`}
                        value={langFilter}
                        onChange={(event) => setLangFilter(event.target.value)}
                        placeholder={t("search.lang_placeholder")}
                      />
                    </FieldShell>
                    <FieldShell className="gap-1.5" label={<span className={labelTone}>{t("search.freshness_label")}</span>}>
                      <Select
                        value={freshnessFilter}
                        onValueChange={(value) =>
                          setFreshnessFilter(
                            value === "24h" || value === "7d" || value === "30d" ? value : "all",
                          )
                        }
                      >
                        <SelectTrigger aria-label={t("search.freshness_label")} className="w-full rounded-2xl">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectGroup>
                            {freshnessOptions.map((option) => (
                              <SelectItem key={option.value} value={option.value}>
                                {option.label}
                              </SelectItem>
                            ))}
                          </SelectGroup>
                        </SelectContent>
                      </Select>
                    </FieldShell>
                    <FieldShell className="gap-1.5" label={<span className={labelTone}>{t("search.network_label")}</span>}>
                      <Select
                        value={networkFilter ?? "all"}
                        onValueChange={(value) =>
                          setNetworkFilter(value === "clearnet" || value === "tor" ? value : null)
                        }
                      >
                        <SelectTrigger aria-label={t("search.network_label")} className="w-full rounded-2xl">
                          <SelectValue />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectGroup>
                            {networkOptions.map((option) => (
                              <SelectItem key={option.value} value={option.value}>
                                {option.label}
                              </SelectItem>
                            ))}
                          </SelectGroup>
                        </SelectContent>
                      </Select>
                    </FieldShell>
                  </div>
                </Collapsible.Content>
              </Collapsible.Root>
            </form>

            {loading ? (
              <div className={`pt-1 text-sm ${mutedTone}`}>
                {t("search.loading")}
              </div>
            ) : null}

            {error ? (
              <div
                className={`rounded-[20px] border px-4 py-3 text-sm ${
                  "border-[rgba(182,92,61,0.22)] bg-[rgba(182,92,61,0.08)] text-[var(--fv-danger)]"
                }`}
              >
                {error}
              </div>
            ) : null}

            {results ? (
              <section className="space-y-6 pt-2">
                <div className={`flex flex-wrap items-center gap-x-4 gap-y-2 text-sm ${mutedTone}`}>
                  <span>
                    {t("search.results_meta", {
                      mode: usingProtectedSearch
                        ? t("search.dev_search")
                        : t("search.browser_search"),
                      count: results.total_estimate,
                      took: results.took_ms,
                    })}
                  </span>
                  <span>{t("search.page", { page: Math.floor(submittedSearch.offset / 10) + 1 })}</span>
                  {results.did_you_mean ? (
                    <span className={secondaryTextTone}>
                      {t("search.did_you_mean")}{" "}
                      <button
                        type="button"
                        className="font-medium underline underline-offset-4"
                        onClick={() => {
                          setQuery(results.did_you_mean ?? "");
                          commitSearch({
                            query: results.did_you_mean ?? "",
                            site: siteFilter.trim(),
                            lang: langFilter.trim().toLowerCase(),
                            freshness: freshnessFilter,
                            network: networkFilter,
                            offset: 0,
                          });
                        }}
                      >
                        {results.did_you_mean}
                      </button>
                      ?
                    </span>
                  ) : null}
                </div>

                <div className="space-y-0">
                  {results.results.map((result, index) => (
                    <article
                      key={result.id}
                      className="app-rise-in group space-y-1.5 border-b border-[var(--fv-border-soft)] py-5 first:pt-0 last:border-b-0 last:pb-0"
                      style={{ animationDelay: `${Math.min(index, 5) * 45}ms` }}
                    >
                      <div className="text-sm text-[var(--fv-text-soft)]">
                        {result.display_url}
                      </div>
                      <a
                        href={result.url}
                        target="_blank"
                        rel="noreferrer"
                        className="inline-flex items-center gap-2 text-[1.35rem] font-medium leading-tight tracking-[-0.03em] text-[var(--fv-text)] transition-colors hover:text-[var(--fv-accent)]"
                      >
                        <span>{result.title}</span>
                        <ExternalLinkIcon className="size-4 shrink-0 opacity-60" />
                      </a>
                      <p className={`text-[15px] leading-7 ${secondaryTextTone}`}>{result.snippet}</p>
                    </article>
                  ))}
                </div>

                {(submittedSearch.offset > 0 || results.next_offset != null) && (
                  <div className="flex items-center justify-between gap-3 pt-2">
                    <Button
                      type="button"
                      variant="outline"
                      className="h-10 rounded-full border-[var(--fv-border)] bg-[var(--fv-panel)] px-4 text-[var(--fv-text)] hover:bg-[var(--fv-panel-soft)] disabled:opacity-40"
                      disabled={submittedSearch.offset === 0}
                      onClick={handlePrevPage}
                    >
                      {t("search.previous")}
                    </Button>
                    <span className={`text-sm ${mutedTone}`}>
                      {t("search.page", { page: Math.floor(submittedSearch.offset / 10) + 1 })}
                    </span>
                    <Button
                      type="button"
                      variant="outline"
                      className="h-10 rounded-full border-[var(--fv-border)] bg-[var(--fv-panel)] px-4 text-[var(--fv-text)] hover:bg-[var(--fv-panel-soft)] disabled:opacity-40"
                      disabled={results.next_offset == null}
                      onClick={handleNextPage}
                    >
                      {t("search.next")}
                    </Button>
                  </div>
                )}
              </section>
            ) : null}
          </section>
        </main>
      </div>
    </div>
  );
}

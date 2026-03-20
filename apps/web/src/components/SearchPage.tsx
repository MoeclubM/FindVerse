import { FormEvent, useEffect, useState } from "react";

import { developerSearch, search } from "../api";

function currentSearchQuery() {
  return new URLSearchParams(window.location.search).get("q") ?? "";
}

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

export function SearchPage(props: {
  devToken: string | null;
  onTokenExpired: () => void;
  onNavigateDev: () => void;
}) {
  const [query, setQuery] = useState(currentSearchQuery);
  const [submittedQuery, setSubmittedQuery] = useState(currentSearchQuery);
  const [results, setResults] = useState<Awaited<ReturnType<typeof search>> | null>(null);
  const [loading, setLoading] = useState(() => Boolean(currentSearchQuery()));
  const [error, setError] = useState<string | null>(null);
  const [usingProtectedSearch, setUsingProtectedSearch] = useState(false);

  useEffect(() => {
    if (!submittedQuery.trim()) {
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
        const response = props.devToken
          ? await developerSearch(submittedQuery, props.devToken)
          : await search(submittedQuery);
        if (!cancelled) {
          setResults(response);
          setUsingProtectedSearch(Boolean(props.devToken));
        }
      } catch (nextError) {
        const errorWithStatus = nextError as Error & { status?: number };
        if (!cancelled && errorWithStatus.status === 401 && props.devToken) {
          props.onTokenExpired();
          try {
            const fallbackResponse = await search(submittedQuery);
            if (!cancelled) {
              setResults(fallbackResponse);
              setUsingProtectedSearch(false);
              setError("Stored developer key expired. Switched back to browser search.");
            }
          } catch (fallbackError) {
            if (!cancelled) {
              setResults(null);
              setUsingProtectedSearch(false);
              setError(getErrorMessage(fallbackError, "Search failed"));
            }
          }
          return;
        }

        if (!cancelled) {
          setResults(null);
          setUsingProtectedSearch(Boolean(props.devToken));
          setError(getErrorMessage(nextError, "Search failed"));
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
  }, [submittedQuery, props.devToken, props.onTokenExpired]);

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const nextQuery = query.trim();
    const nextUrl = nextQuery ? `/?q=${encodeURIComponent(nextQuery)}` : "/";
    window.history.pushState({}, "", nextUrl);
    setSubmittedQuery(nextQuery);
  }

  const hasResults = Boolean(results);

  return (
    <div className="search-shell">
      <main className={hasResults ? "search-page search-page-top" : "search-page"}>
        {!hasResults && <h1 className="search-brand">FindVerse</h1>}
        <div className="search-toolbar">
          <div className="search-access-strip">
            <span className={usingProtectedSearch ? "status-pill" : "status-pill status-pill-muted"}>
              {usingProtectedSearch ? "Developer key active" : "Browser search"}
            </span>
            <button type="button" className="plain-link" onClick={props.onNavigateDev}>
              Developer portal
            </button>
          </div>
        </div>
        <form className="search-form" onSubmit={handleSubmit}>
          <input
            aria-label="Search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search"
          />
          <button type="submit">Search</button>
        </form>

        {loading ? <p className="search-meta">Searching...</p> : null}
        {error ? <p className="search-error">{error}</p> : null}
        {results ? (
          <section className="results-list">
            <p className="search-meta">
              {results.total_estimate} results in {results.took_ms}ms
            </p>
            {results.results.map((result) => (
              <article key={result.id} className="result-item">
                <a href={result.url} target="_blank" rel="noreferrer">
                  {result.title}
                </a>
                <div className="result-url">{result.display_url}</div>
                <p>{result.snippet}</p>
              </article>
            ))}
          </section>
        ) : null}
      </main>
    </div>
  );
}

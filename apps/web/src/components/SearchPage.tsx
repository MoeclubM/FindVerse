import { FormEvent, useEffect, useState } from "react";

import { searchWithParams, type SearchResponse } from "../api";

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
  const [results, setResults] = useState<SearchResponse | null>(null);
  const [loading, setLoading] = useState(() => Boolean(currentSearchQuery()));
  const [error, setError] = useState<string | null>(null);
  const [usingProtectedSearch, setUsingProtectedSearch] = useState(false);
  const [offset, setOffset] = useState(0);

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
        const response = await searchWithParams(
          submittedQuery,
          { offset },
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
            const fallbackResponse = await searchWithParams(submittedQuery, { offset });
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
  }, [submittedQuery, offset, props.devToken, props.onTokenExpired]);

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const nextQuery = query.trim();
    const nextUrl = nextQuery ? `/?q=${encodeURIComponent(nextQuery)}` : "/";
    window.history.pushState({}, "", nextUrl);
    setOffset(0);
    setSubmittedQuery(nextQuery);
  }

  function handleNextPage() {
    if (results?.next_offset != null) {
      setOffset(results.next_offset);
    }
  }

  function handlePrevPage() {
    setOffset((prev) => Math.max(0, prev - 10));
  }

  const hasResults = Boolean(results);

  return (
    <div className="search-shell">
      <div className="search-corner">
        {props.devToken ? (
          <span className="search-corner-status">Developer key active</span>
        ) : null}
        <button type="button" className="search-corner-link" onClick={props.onNavigateDev}>
          Developer portal
        </button>
      </div>
      <main className={hasResults ? "search-page search-page-top" : "search-page"}>
        {!hasResults && <h1 className="search-brand">FindVerse</h1>}
        <form className="search-form" onSubmit={handleSubmit}>
          <input
            aria-label="Search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search the web..."
          />
          <button type="submit">Search</button>
        </form>

        {loading ? <p className="search-meta">Searching...</p> : null}
        {error ? <p className="search-error">{error}</p> : null}
        {results ? (
          <section className="results-list">
            <p className="search-meta">
              {usingProtectedSearch ? "Developer search" : "Browser search"} · {results.total_estimate} results in {results.took_ms}ms
            </p>
            {results.did_you_mean ? (
              <p className="search-meta">
                Did you mean <strong>{results.did_you_mean}</strong>?
              </p>
            ) : null}
            {results.results.map((result) => (
              <article key={result.id} className="result-item">
                <a href={result.url} target="_blank" rel="noreferrer">
                  {result.title}
                </a>
                <div className="result-url">{result.display_url}</div>
                <p>{result.snippet}</p>
              </article>
            ))}
            {(offset > 0 || results.next_offset != null) && (
              <div className="search-pagination">
                <button
                  type="button"
                  disabled={offset === 0}
                  onClick={handlePrevPage}
                >
                  Previous
                </button>
                <span className="search-pagination-info">
                  Page {Math.floor(offset / 10) + 1}
                </span>
                <button
                  type="button"
                  disabled={results.next_offset == null}
                  onClick={handleNextPage}
                >
                  Next
                </button>
              </div>
            )}
          </section>
        ) : null}
      </main>
    </div>
  );
}

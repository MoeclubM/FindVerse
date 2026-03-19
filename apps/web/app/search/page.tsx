import { SearchForm } from "@/components/search-form";
import { ResultCard } from "@/components/result-card";
import { searchIndex } from "@/lib/api";

type SearchPageProps = {
  searchParams: Promise<Record<string, string | string[] | undefined>>;
};

function pickValue(value: string | string[] | undefined) {
  return Array.isArray(value) ? value[0] : value;
}

export default async function SearchPage({ searchParams }: SearchPageProps) {
  const params = await searchParams;
  const query = pickValue(params.q) ?? "search ranking";
  const lang = pickValue(params.lang);
  const site = pickValue(params.site);
  const freshness = pickValue(params.freshness) as
    | "24h"
    | "7d"
    | "30d"
    | "all"
    | undefined;
  const offset = Number(pickValue(params.offset) ?? "0");
  const results = await searchIndex({
    q: query,
    lang,
    site,
    freshness,
    offset: Number.isFinite(offset) ? offset : 0,
  });

  const freshnessLinks = [
    ["all", "All"],
    ["24h", "Last 24h"],
    ["7d", "Last 7d"],
    ["30d", "Last 30d"],
  ] as const;

  return (
    <main className="page-shell">
      <header className="site-header">
        <div className="brand-lockup">
          <div className="brand-mark">FV</div>
          <div className="brand-copy">
            <strong>FindVerse</strong>
            <span>Query the bootstrap index</span>
          </div>
        </div>
        <nav className="top-nav">
          <a href="/">Home</a>
          <a href="/developers">Developers</a>
          <a href="/docs">Docs</a>
        </nav>
      </header>

      <div className="results-layout">
        <aside>
          <SearchForm defaultQuery={query} compact />
          <div className="filter-block">
            <h2>Freshness</h2>
            <div className="filter-list">
              {freshnessLinks.map(([value, label]) => {
                const href = new URLSearchParams({
                  q: query,
                  freshness: value,
                });
                if (lang) href.set("lang", lang);
                if (site) href.set("site", site);
                return (
                  <a
                    key={value}
                    className={(freshness ?? "all") === value ? "active" : ""}
                    href={`/search?${href.toString()}`}
                  >
                    {label}
                  </a>
                );
              })}
            </div>
          </div>
          <div className="filter-block">
            <h2>Active filters</h2>
            <p>{lang ? `language: ${lang}` : "language: any"}</p>
            <p>{site ? `site: ${site}` : "site: any"}</p>
          </div>
        </aside>

        <section className="results-column">
          <p>
            {results.total_estimate} results in {results.took_ms}ms
          </p>
          <h1>{results.query}</h1>
          <div className="result-stack">
            {results.results.map((result) => (
              <ResultCard key={result.id} result={result} />
            ))}
          </div>
          {results.next_offset !== null ? (
            <p>
              <a
                className="pagination-link"
                href={`/search?${new URLSearchParams({
                  q: query,
                  freshness: freshness ?? "all",
                  offset: String(results.next_offset),
                  ...(lang ? { lang } : {}),
                  ...(site ? { site } : {}),
                }).toString()}`}
              >
                Next page
              </a>
            </p>
          ) : null}
        </section>
      </div>
    </main>
  );
}

import { SearchForm } from "@/components/search-form";

const starterQueries = [
  "crawl frontier",
  "developer portal",
  "search ranking",
  "opensearch",
  "robots txt",
];

export default function HomePage() {
  return (
    <main className="page-shell">
      <header className="site-header">
        <div className="brand-lockup">
          <div className="brand-mark">FV</div>
          <div className="brand-copy">
            <strong>FindVerse</strong>
            <span>Search infrastructure for humans and agents</span>
          </div>
        </div>
        <nav className="top-nav">
          <a href="/search?q=search+ranking">Search</a>
          <a href="/developers">Developers</a>
          <a href="/docs">Docs</a>
        </nav>
      </header>

      <section className="hero">
        <div className="hero-copy">
          <p>Anonymous search for people. Stable REST responses for tools.</p>
          <h1>Build your own search surface, not just another wrapper.</h1>
          <p>
            FindVerse ships a public search UI, a Rust query API, developer key
            management, and a dedicated crawler service you can evolve into a larger
            indexing system.
          </p>
          <SearchForm suggestions={starterQueries} />
        </div>

        <aside className="hero-panel">
          <div>
            <p>Current slice</p>
            <h2>Beta architecture on one screen</h2>
          </div>
          <div className="stat-grid">
            <div className="stat-card">
              <strong>12</strong>
              <span>bootstrap documents</span>
            </div>
            <div className="stat-card">
              <strong>4</strong>
              <span>crawler commands</span>
            </div>
            <div className="stat-card">
              <strong>5</strong>
              <span>public endpoints</span>
            </div>
            <div className="stat-card">
              <strong>6</strong>
              <span>infra services defined</span>
            </div>
          </div>
          <pre>{`GET /v1/search?q=search+ranking
Authorization: Bearer fvk_xxx

{
  "query": "search ranking",
  "results": [...]
}`}</pre>
        </aside>
      </section>

      <section className="feature-strip">
        <article>
          <h2>Public search</h2>
          <p>
            Search results use deterministic lexical ranking with site authority and
            freshness boosts, making relevance easy to inspect and tune.
          </p>
        </article>
        <article>
          <h2>Developer access</h2>
          <p>
            The same REST API powers the web app and external callers. Key lifecycle
            and usage views live in the developer portal.
          </p>
        </article>
        <article>
          <h2>Crawler service</h2>
          <p>
            Seed discovery, HTML fetch, and local index generation are already wired
            as Rust CLI commands so you can expand the corpus without rewriting the
            whole stack.
          </p>
        </article>
      </section>
    </main>
  );
}

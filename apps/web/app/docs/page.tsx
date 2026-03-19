export default function DocsPage() {
  return (
    <main className="page-shell">
      <header className="site-header">
        <div className="brand-lockup">
          <div className="brand-mark">FV</div>
          <div className="brand-copy">
            <strong>FindVerse</strong>
            <span>API and system docs</span>
          </div>
        </div>
        <nav className="top-nav">
          <a href="/">Home</a>
          <a href="/search?q=search+api">Search</a>
          <a href="/developers">Developers</a>
        </nav>
      </header>

      <section className="docs-shell">
        <p>Web, API, crawler, and docs stay split on purpose.</p>
        <h1>FindVerse interfaces</h1>
        <p>
          The public web UI talks to the same search API as external clients. The
          crawler talks to the server through separate internal endpoints using
          crawler <code>id + key</code>. The developer portal itself uses local
          username and password auth.
        </p>

        <div className="docs-grid">
          <article>
            <h2>Search</h2>
            <p>
              <code>GET /v1/search?q=&limit=&offset=&lang=&site=&freshness=</code>
            </p>
            <pre>{`curl "http://localhost:8080/v1/search?q=search+ranking"`}</pre>
          </article>
          <article>
            <h2>Suggest</h2>
            <p>
              <code>GET /v1/suggest?q=search</code>
            </p>
            <pre>{`curl "http://localhost:8080/v1/suggest?q=craw"`}</pre>
          </article>
          <article>
            <h2>Search API key</h2>
            <p>
              <code>POST /v1/developer/keys</code> with internal developer identity
              forwarding from the web portal.
            </p>
            <pre>{`{
  "name": "CLI key"
}`}</pre>
          </article>
          <article>
            <h2>Crawler workflow</h2>
            <p>
              <code>POST /internal/crawlers/claim</code> and{" "}
              <code>POST /internal/crawlers/report</code> form the worker loop.
            </p>
            <pre>{`findverse-crawler worker \\
  --server http://api:8080 \\
  --crawler-id YOUR_ID \\
  --crawler-key YOUR_KEY`}</pre>
          </article>
          <article>
            <h2>Structure</h2>
            <p>
              Runtime services live in <code>apps/web</code>, <code>services/api</code>,
              and <code>services/crawler</code>. Human-readable docs stay in{" "}
              <code>docs/</code>.
            </p>
          </article>
          <article>
            <h2>Source of truth</h2>
            <p>
              The OpenAPI document checked into the repository lives at{" "}
              <code>openapi/search-api.yaml</code>.
            </p>
          </article>
        </div>
      </section>
    </main>
  );
}

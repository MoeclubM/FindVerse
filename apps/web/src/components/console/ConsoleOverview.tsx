import { useConsole } from "./ConsoleContext";

export function ConsoleOverview() {
  const { overview, developers } = useConsole();

  return (
    <>
      <section className="panel panel-wide compact-panel">
        <div className="section-header">
          <h2>System overview</h2>
          <span className="section-meta">{overview?.recent_events.length ?? 0} recent events</span>
        </div>
        <div className="dense-grid">
          <div className="metric-card">
            <span>Indexed docs</span>
            <strong>{overview?.indexed_documents ?? 0}</strong>
          </div>
          <div className="metric-card">
            <span>Duplicates</span>
            <strong>{overview?.duplicate_documents ?? 0}</strong>
          </div>
          <div className="metric-card">
            <span>Queued jobs</span>
            <strong>{overview?.frontier_depth ?? 0}</strong>
          </div>
          <div className="metric-card">
            <span>Known URLs</span>
            <strong>{overview?.known_urls ?? 0}</strong>
          </div>
          <div className="metric-card">
            <span>In flight</span>
            <strong>{overview?.in_flight_jobs ?? 0}</strong>
          </div>
          <div className="metric-card">
            <span>Active rules</span>
            <strong>{overview?.rules.filter((r) => r.enabled).length ?? 0}</strong>
          </div>
          <div className="metric-card">
            <span>Workers</span>
            <strong>{overview?.crawlers.length ?? 0}</strong>
          </div>
          <div className="metric-card">
            <span>Terminal failures</span>
            <strong>{overview?.terminal_failures ?? 0}</strong>
          </div>
          <div className="metric-card">
            <span>Developer accounts</span>
            <strong>{developers.length}</strong>
          </div>
        </div>
      </section>

      <section className="panel panel-wide compact-panel">
        <div className="section-header">
          <h2>Recent crawl events</h2>
          <span className="section-meta">Automation health and worker activity</span>
        </div>
        <div className="dense-list">
          {overview?.recent_events.length ? (
            overview.recent_events.map((event) => (
              <div className="compact-row event-row" key={event.id}>
                <div className="row-primary">
                  <strong>{event.kind}</strong>
                  <span>{event.message}</span>
                </div>
                <div className="row-meta">
                  <span className={event.status === "ok" ? "status-pill" : "status-pill status-pill-muted"}>
                    {event.status}
                  </span>
                  <span>{event.created_at}</span>
                  {event.url ? <span>{event.url}</span> : null}
                  {event.crawler_id ? <span>{event.crawler_id}</span> : null}
                </div>
              </div>
            ))
          ) : (
            <div className="list-row">No crawl events yet.</div>
          )}
        </div>
      </section>
    </>
  );
}

import { useConsole } from "./ConsoleContext";
import { useTranslation } from "react-i18next";
import { SectionHeader, StatStrip } from "../common/PanelPrimitives";

export function ConsoleOverview() {
  const { overview, developers } = useConsole();
  const { t } = useTranslation();
  const recentEvents = overview?.recent_events ?? [];
  const activeCrawlerCount =
    overview?.crawlers.filter((crawler) => {
      if (!crawler.last_seen_at) return false;
      return Date.now() - new Date(crawler.last_seen_at).getTime() < 5 * 60 * 1000;
    }).length ?? 0;

  return (
    <>
      <section className="panel panel-wide compact-panel">
        <SectionHeader
          title={t("console.overview.title")}
          meta={t("console.overview.recent_events_count", { count: recentEvents.length })}
        />
        <StatStrip
          items={[
            { label: t("console.overview.indexed_docs"), value: overview?.indexed_documents ?? 0 },
            { label: t("console.overview.duplicates"), value: overview?.duplicate_documents ?? 0 },
            { label: t("console.overview.queued_jobs"), value: overview?.frontier_depth ?? 0 },
            { label: t("console.overview.known_urls"), value: overview?.known_urls ?? 0 },
            { label: t("console.overview.in_flight"), value: overview?.in_flight_jobs ?? 0 },
            { label: t("console.overview.active_rules"), value: overview?.rules.filter((r) => r.enabled).length ?? 0 },
            { label: t("console.overview.workers"), value: activeCrawlerCount },
            { label: t("console.overview.terminal_failures"), value: overview?.terminal_failures ?? 0 },
            { label: t("console.overview.developer_accounts"), value: developers.length },
          ]}
        />
      </section>

      <section className="panel panel-wide compact-panel">
        <SectionHeader title={t("console.overview.recent_events")} meta={t("console.overview.recent_events_meta")} />
        <div className="dense-list">
          {recentEvents.length ? (
            recentEvents.map((event) => (
              <div className="compact-row event-row" key={event.id}>
                <div className="row-meta row-meta-tight">
                  <strong>{event.kind}</strong>
                  <span className={event.status === "ok" ? "status-pill" : "status-pill status-pill-muted"}>
                    {event.status}
                  </span>
                  <span>{event.created_at}</span>
                  {event.crawler_id ? <code>{event.crawler_id}</code> : null}
                </div>
                <div className="row-primary">
                  <span>{event.message}</span>
                  {event.url ? <code>{event.url}</code> : null}
                </div>
              </div>
            ))
          ) : (
            <div className="list-row">{t("console.overview.no_events")}</div>
          )}
        </div>
      </section>
    </>
  );
}

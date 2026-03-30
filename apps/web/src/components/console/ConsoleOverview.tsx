import { useConsole } from "./ConsoleContext";
import { useTranslation } from "react-i18next";
import { PanelSection, StatStrip } from "../common/PanelPrimitives";
import { Badge } from "../ui/badge";
import { Card, CardContent } from "../ui/card";

const ONLINE_THRESHOLD_MS = 90 * 1000;

export function ConsoleOverview() {
  const { overview, developers } = useConsole();
  const { t } = useTranslation();
  const recentEvents = overview?.recent_events ?? [];
  const activeCrawlerCount =
    overview?.crawlers.filter((crawler) => {
      if (!crawler.last_seen_at) return false;
      return Date.now() - new Date(crawler.last_seen_at).getTime() < ONLINE_THRESHOLD_MS;
    }).length ?? 0;

  return (
    <>
      <PanelSection
          title={t("console.overview.title")}
          meta={t("console.overview.recent_events_count", { count: recentEvents.length })}
          contentClassName="space-y-5"
      >
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
          className="xl:grid-cols-3"
        />
      </PanelSection>

      <PanelSection title={t("console.overview.recent_events")} meta={t("console.overview.recent_events_meta")}>
        <div className="grid gap-3">
          {recentEvents.length ? (
            recentEvents.map((event) => (
              <Card key={event.id} className="rounded-2xl">
                <CardContent className="grid gap-3 p-4">
                <div className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
                  <strong className="text-foreground">{event.kind}</strong>
                  <Badge variant={event.status === "ok" ? "success" : "outline"}>{event.status}</Badge>
                  <span>{event.created_at}</span>
                  {event.crawler_id ? <code>{event.crawler_id}</code> : null}
                </div>
                <div className="mt-3 grid gap-2">
                  <span className="text-sm text-foreground">{event.message}</span>
                  {event.url ? <code>{event.url}</code> : null}
                </div>
                </CardContent>
              </Card>
            ))
          ) : (
            <div className="rounded-2xl border border-dashed border-border bg-muted/40 px-4 py-8 text-center text-sm text-muted-foreground">{t("console.overview.no_events")}</div>
          )}
        </div>
      </PanelSection>
    </>
  );
}

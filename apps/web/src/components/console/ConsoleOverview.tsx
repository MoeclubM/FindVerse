import { useConsole } from "./ConsoleContext";
import {
  getConsoleEventKindLabel,
  getConsoleEventStatusLabel,
} from "./consoleLabels";
import { useTranslation } from "react-i18next";
import { PanelSection, StatStrip } from "../common/PanelPrimitives";
import { Badge } from "../ui/badge";

function formatTimestamp(value: string) {
  return value.replace("T", " ").replace("Z", "").slice(0, 16);
}

export function ConsoleOverview() {
  const { overview, users } = useConsole();
  const { t } = useTranslation();
  const recentEvents = overview?.recent_events ?? [];
  const totalCrawlerCount = overview?.crawlers.length ?? 0;
  const activeCrawlerCount =
    overview?.crawlers.filter((crawler) => crawler.online).length ?? 0;

  return (
    <>
      <PanelSection
        title={t("console.overview.title")}
        meta={t("console.overview.recent_events_count", {
          count: recentEvents.length,
        })}
        contentClassName="space-y-3"
      >
        <StatStrip
          compact
          items={[
            {
              label: t("console.overview.indexed_docs"),
              value: overview?.indexed_documents ?? 0,
            },
            {
              label: t("console.overview.duplicates"),
              value: overview?.duplicate_documents ?? 0,
            },
            {
              label: t("console.overview.queued_jobs"),
              value: overview?.frontier_depth ?? 0,
            },
            {
              label: t("console.overview.known_urls"),
              value: overview?.known_urls ?? 0,
            },
            {
              label: t("console.overview.in_flight"),
              value: overview?.in_flight_jobs ?? 0,
            },
            {
              label: t("console.overview.active_rules"),
              value: overview?.rules.filter((r) => r.enabled).length ?? 0,
            },
            { label: t("console.overview.workers"), value: totalCrawlerCount },
            {
              label: t("console.workers.online_count"),
              value: activeCrawlerCount,
            },
            {
              label: t("console.overview.terminal_failures"),
              value: overview?.terminal_failures ?? 0,
            },
            {
              label: t("console.overview.user_accounts"),
              value: users.length,
            },
          ]}
          className="xl:grid-cols-5 2xl:grid-cols-10"
        />
      </PanelSection>

      <PanelSection
        title={t("console.overview.recent_events")}
        meta={t("console.overview.recent_events_meta")}
        contentClassName="space-y-3"
      >
        {recentEvents.length ? (
          <div className="overflow-hidden rounded-lg border border-border bg-card">
            {recentEvents.map((event, index) => (
              <div
                key={event.id}
                className={index === 0 ? "px-3 py-2" : "border-t border-border px-3 py-2"}
              >
                <div className="grid gap-2 md:grid-cols-[minmax(0,210px)_110px_130px_minmax(0,1fr)] md:items-start">
                  <div className="min-w-0 flex flex-wrap items-center gap-1.5">
                    <strong className="text-xs font-semibold text-foreground">
                      {getConsoleEventKindLabel(t, event.kind)}
                    </strong>
                    <Badge variant={event.status === "ok" ? "success" : "outline"}>
                      {getConsoleEventStatusLabel(t, event.status)}
                    </Badge>
                  </div>
                  <span className="text-[11px] text-muted-foreground">
                    {formatTimestamp(event.created_at)}
                  </span>
                  <code className="max-w-full truncate text-[11px] text-muted-foreground">
                    {event.crawler_id ?? "-"}
                  </code>
                  <div className="min-w-0">
                    <p className="wrap-break-word text-xs leading-5 text-foreground">{event.message}</p>
                    {event.url ? (
                      <code className="mt-0.5 block max-w-full break-all text-[11px] text-muted-foreground">
                        {event.url}
                      </code>
                    ) : null}
                  </div>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="rounded-lg border border-dashed border-border bg-muted/40 px-4 py-6 text-center text-sm text-muted-foreground">
            {t("console.overview.no_events")}
          </div>
        )}
      </PanelSection>
    </>
  );
}

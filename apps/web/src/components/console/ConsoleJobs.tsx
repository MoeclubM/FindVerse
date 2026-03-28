import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  cleanupCompletedJobs,
  getCrawlJobStats,
  listCrawlJobs,
  retryFailedJobs,
  stopAllCrawlJobs,
  type CrawlJobList,
  type CrawlJobStats,
} from "../../api";
import { SectionHeader, StatStrip } from "../common/PanelPrimitives";
import { useConsole } from "./ConsoleContext";

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

const PAGE_SIZE = 20;

export function ConsoleJobs() {
  const { token, busy, setBusy, setFlash, refreshAll } = useConsole();
  const { t } = useTranslation();

  const [statusFilter, setStatusFilter] = useState("");
  const [offset, setOffset] = useState(0);
  const [jobs, setJobs] = useState<CrawlJobList | null>(null);
  const [stats, setStats] = useState<CrawlJobStats | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);

    Promise.all([
      getCrawlJobStats(token),
      listCrawlJobs(token, {
        status: statusFilter || undefined,
        offset,
        limit: PAGE_SIZE,
      }),
    ])
      .then(([nextStats, nextJobs]) => {
        if (!cancelled) {
          setStats(nextStats);
          setJobs(nextJobs);
        }
      })
      .catch((error) => {
        if (!cancelled) {
          setFlash(getErrorMessage(error, t("console.jobs.load_failed")));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [token, statusFilter, offset, setFlash, t]);

  async function handleRetryFailed() {
    setBusy(true);
    setFlash(null);
    try {
      const response = await retryFailedJobs(token);
      setFlash(t("console.jobs.retry_success", { count: response.retried }));
      await refreshAll();
      setOffset(0);
      const [nextStats, nextJobs] = await Promise.all([
        getCrawlJobStats(token),
        listCrawlJobs(token, {
          status: statusFilter || undefined,
          offset: 0,
          limit: PAGE_SIZE,
        }),
      ]);
      setStats(nextStats);
      setJobs(nextJobs);
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.jobs.retry_failed_error")));
    } finally {
      setBusy(false);
    }
  }

  async function handleCleanupSucceeded() {
    setBusy(true);
    setFlash(null);
    try {
      const response = await cleanupCompletedJobs(token);
      setFlash(t("console.jobs.cleanup_success", { count: response.cleaned }));
      await refreshAll();
      setOffset(0);
      const [nextStats, nextJobs] = await Promise.all([
        getCrawlJobStats(token),
        listCrawlJobs(token, {
          status: statusFilter || undefined,
          offset: 0,
          limit: PAGE_SIZE,
        }),
      ]);
      setStats(nextStats);
      setJobs(nextJobs);
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.jobs.cleanup_failed_error")));
    } finally {
      setBusy(false);
    }
  }

  async function handleStopAll() {
    if (!window.confirm(t("console.jobs.stop_all_confirm"))) {
      return;
    }

    setBusy(true);
    setFlash(null);
    try {
      const response = await stopAllCrawlJobs(token);
      setFlash(
        t("console.jobs.stop_all_success", {
          rules: response.disabled_rules,
          jobs: response.removed_jobs,
        }),
      );
      await refreshAll();
      setOffset(0);
      const [nextStats, nextJobs] = await Promise.all([
        getCrawlJobStats(token),
        listCrawlJobs(token, {
          status: statusFilter || undefined,
          offset: 0,
          limit: PAGE_SIZE,
        }),
      ]);
      setStats(nextStats);
      setJobs(nextJobs);
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.jobs.stop_all_failed")));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <section className="panel panel-wide compact-panel">
        <SectionHeader
          title={t("console.jobs.status_title")}
          meta={t("console.jobs.visible_jobs", { count: jobs?.total ?? 0 })}
        />
        <StatStrip
          items={[
            { label: t("console.jobs.stats.queued"), value: stats?.queued ?? 0 },
            { label: t("console.jobs.stats.claimed"), value: stats?.claimed ?? 0 },
            { label: t("console.jobs.stats.succeeded"), value: stats?.succeeded ?? 0 },
            { label: t("console.jobs.stats.failed"), value: stats?.failed ?? 0 },
            { label: t("console.jobs.stats.blocked"), value: stats?.blocked ?? 0 },
            { label: t("console.jobs.stats.dead_letter"), value: stats?.dead_letter ?? 0 },
          ]}
        />
      </section>

      <section className="panel panel-wide compact-panel">
        <SectionHeader title={t("console.jobs.queue_title")} meta={t("console.jobs.queue_meta")} />

        <div className="inline-form">
          <select
            value={statusFilter}
            onChange={(event) => {
              setStatusFilter(event.target.value);
              setOffset(0);
            }}
          >
            <option value="">{t("console.jobs.all_statuses")}</option>
            <option value="queued">{t("console.jobs.stats.queued")}</option>
            <option value="claimed">{t("console.jobs.stats.claimed")}</option>
            <option value="succeeded">{t("console.jobs.stats.succeeded")}</option>
            <option value="failed">{t("console.jobs.stats.failed")}</option>
            <option value="blocked">{t("console.jobs.stats.blocked")}</option>
            <option value="dead_letter">{t("console.jobs.stats.dead_letter")}</option>
          </select>
          <button type="button" disabled={busy} onClick={() => void handleRetryFailed()}>
            {t("console.jobs.retry_failed")}
          </button>
          <button type="button" disabled={busy} onClick={() => void handleCleanupSucceeded()}>
            {t("console.jobs.cleanup_succeeded")}
          </button>
          <button type="button" disabled={busy} className="danger-button" onClick={() => void handleStopAll()}>
            {t("console.jobs.stop_all")}
          </button>
        </div>

        <div className="dense-list">
          {loading ? (
            <div className="list-row">{t("console.jobs.loading")}</div>
          ) : jobs?.jobs.length ? (
            jobs.jobs.map((job) => (
              <div className="compact-row worker-row" key={job.id}>
                <div className="row-primary">
                  <strong>{job.url}</strong>
                  <span>{job.source}</span>
                </div>
                <div className="row-meta">
                  <span className={job.status === "succeeded" ? "status-pill" : "status-pill status-pill-muted"}>
                    {job.status}
                  </span>
                  {job.http_status != null ? <span>HTTP {job.http_status}</span> : null}
                  <span>{t("console.jobs.attempt_progress", { current: job.attempt_count, max: job.max_attempts })}</span>
                  <span>{t("console.jobs.depth_progress", { current: job.depth, max: job.max_depth })}</span>
                  <span>{t("console.jobs.discovered_count", { count: job.discovered_urls_count })}</span>
                  {job.claimed_by ? <span>{t("console.jobs.worker_id", { id: job.claimed_by })}</span> : null}
                  {job.next_retry_at ? <span>{t("console.jobs.retry_at", { time: job.next_retry_at })}</span> : null}
                  {job.finished_at ? <span>{t("console.jobs.finished_at", { time: job.finished_at })}</span> : null}
                </div>
                <div className="row-meta">
                  {job.final_url ? <span>{t("console.jobs.final", { url: job.final_url })}</span> : null}
                  {job.content_type ? <span>{job.content_type}</span> : null}
                  {job.accepted_document_id ? <span>{t("console.jobs.doc", { id: job.accepted_document_id })}</span> : null}
                  {job.llm_decision ? <span>{t("console.jobs.llm", { decision: job.llm_decision })}</span> : null}
                  {job.llm_relevance_score != null ? (
                    <span>{t("console.jobs.score", { score: job.llm_relevance_score.toFixed(2) })}</span>
                  ) : null}
                </div>
                {job.llm_reason ? (
                  <div className="row-meta">
                    <span>{job.llm_reason}</span>
                  </div>
                ) : null}
                {job.failure_kind || job.failure_message ? (
                  <div className="row-meta">
                    {job.failure_kind ? <span>{job.failure_kind}</span> : null}
                    {job.failure_message ? <span>{job.failure_message}</span> : null}
                  </div>
                ) : null}
              </div>
            ))
          ) : (
            <div className="list-row">{t("console.jobs.no_jobs_match")}</div>
          )}
        </div>

        <div className="inline-form" style={{ marginTop: 12, marginBottom: 0 }}>
          <button
            type="button"
            disabled={offset === 0}
            onClick={() => setOffset((current) => Math.max(0, current - PAGE_SIZE))}
          >
            {t("search.previous")}
          </button>
          <span className="section-meta">{t("console.jobs.offset", { offset })}</span>
          <button
            type="button"
            disabled={jobs?.next_offset == null}
            onClick={() => setOffset(jobs?.next_offset ?? offset)}
          >
            {t("search.next")}
          </button>
        </div>
      </section>
    </>
  );
}

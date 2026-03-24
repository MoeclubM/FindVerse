import { useEffect, useState } from "react";

import {
  cleanupCompletedJobs,
  getCrawlJobStats,
  listCrawlJobs,
  retryFailedJobs,
  type CrawlJobList,
  type CrawlJobStats,
} from "../../api";
import { useConsole } from "./ConsoleContext";

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

const PAGE_SIZE = 20;

export function ConsoleJobs() {
  const { token, busy, setBusy, setFlash, refreshAll } = useConsole();

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
          setFlash(getErrorMessage(error, "Failed to load crawl jobs"));
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
  }, [token, statusFilter, offset, setFlash]);

  async function handleRetryFailed() {
    setBusy(true);
    setFlash(null);
    try {
      const response = await retryFailedJobs(token);
      setFlash(`Re-queued ${response.retried} failed or dead-letter jobs`);
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
      setFlash(getErrorMessage(error, "Retry failed"));
    } finally {
      setBusy(false);
    }
  }

  async function handleCleanupSucceeded() {
    setBusy(true);
    setFlash(null);
    try {
      const response = await cleanupCompletedJobs(token);
      setFlash(`Removed ${response.cleaned} succeeded jobs`);
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
      setFlash(getErrorMessage(error, "Cleanup failed"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <section className="panel panel-wide compact-panel">
        <div className="section-header">
          <h2>Job status</h2>
          <span className="section-meta">{jobs?.total ?? 0} visible jobs</span>
        </div>
        <div className="dense-grid">
          <div className="metric-card">
            <span>Queued</span>
            <strong>{stats?.queued ?? 0}</strong>
          </div>
          <div className="metric-card">
            <span>Claimed</span>
            <strong>{stats?.claimed ?? 0}</strong>
          </div>
          <div className="metric-card">
            <span>Succeeded</span>
            <strong>{stats?.succeeded ?? 0}</strong>
          </div>
          <div className="metric-card">
            <span>Failed</span>
            <strong>{stats?.failed ?? 0}</strong>
          </div>
          <div className="metric-card">
            <span>Blocked</span>
            <strong>{stats?.blocked ?? 0}</strong>
          </div>
          <div className="metric-card">
            <span>Dead letter</span>
            <strong>{stats?.dead_letter ?? 0}</strong>
          </div>
        </div>
      </section>

      <section className="panel panel-wide compact-panel">
        <div className="section-header">
          <h2>Job queue</h2>
          <span className="section-meta">Failures, retries, and terminal states</span>
        </div>

        <div className="inline-form">
          <select
            value={statusFilter}
            onChange={(event) => {
              setStatusFilter(event.target.value);
              setOffset(0);
            }}
          >
            <option value="">All statuses</option>
            <option value="queued">Queued</option>
            <option value="claimed">Claimed</option>
            <option value="succeeded">Succeeded</option>
            <option value="failed">Failed</option>
            <option value="blocked">Blocked</option>
            <option value="dead_letter">Dead letter</option>
          </select>
          <button type="button" disabled={busy} onClick={() => void handleRetryFailed()}>
            Retry failed
          </button>
          <button type="button" disabled={busy} onClick={() => void handleCleanupSucceeded()}>
            Cleanup succeeded
          </button>
        </div>

        <div className="dense-list">
          {loading ? (
            <div className="list-row">Loading jobs…</div>
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
                  <span>Attempt {job.attempt_count} / {job.max_attempts}</span>
                  <span>Depth {job.depth} / {job.max_depth}</span>
                  <span>{job.discovered_urls_count} discovered</span>
                  {job.claimed_by ? <span>Worker {job.claimed_by}</span> : null}
                  {job.next_retry_at ? <span>Retry at {job.next_retry_at}</span> : null}
                  {job.finished_at ? <span>Finished {job.finished_at}</span> : null}
                </div>
                <div className="row-meta">
                  {job.final_url ? <span>Final {job.final_url}</span> : null}
                  {job.content_type ? <span>{job.content_type}</span> : null}
                  {job.accepted_document_id ? <span>Doc {job.accepted_document_id}</span> : null}
                </div>
                {job.failure_kind || job.failure_message ? (
                  <div className="row-meta">
                    {job.failure_kind ? <span>{job.failure_kind}</span> : null}
                    {job.failure_message ? <span>{job.failure_message}</span> : null}
                  </div>
                ) : null}
              </div>
            ))
          ) : (
            <div className="list-row">No crawl jobs match the current filter.</div>
          )}
        </div>

        <div className="inline-form" style={{ marginTop: 12, marginBottom: 0 }}>
          <button
            type="button"
            disabled={offset === 0}
            onClick={() => setOffset((current) => Math.max(0, current - PAGE_SIZE))}
          >
            Previous
          </button>
          <span className="section-meta">Offset: {offset}</span>
          <button
            type="button"
            disabled={jobs?.next_offset == null}
            onClick={() => setOffset(jobs?.next_offset ?? offset)}
          >
            Next
          </button>
        </div>
      </section>
    </>
  );
}

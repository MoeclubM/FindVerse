import { FormEvent, useState } from "react";

import { createRule, deleteRule, seedFrontier, updateRule } from "../../api";
import { useConsole } from "./ConsoleContext";

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

export function ConsoleCrawlTasks() {
  const { token, busy, setBusy, setFlash, refreshAll, refreshDocumentList, overview } = useConsole();

  const [seedUrls, setSeedUrls] = useState("");
  const [seedDepth, setSeedDepth] = useState("2");
  const [seedAllowRevisit, setSeedAllowRevisit] = useState(false);
  const [ruleName, setRuleName] = useState("");
  const [ruleUrl, setRuleUrl] = useState("");
  const [ruleInterval, setRuleInterval] = useState("60");
  const [ruleDepth, setRuleDepth] = useState("2");

  async function handleSeedFrontier(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const urls = seedUrls
      .split(/\r?\n/)
      .map((value) => value.trim())
      .filter(Boolean);

    setBusy(true);
    setFlash(null);
    try {
      const response = await seedFrontier(
        token,
        urls,
        Number(seedDepth) || 2,
        seedAllowRevisit,
      );
      setFlash(`Queued ${response.accepted_urls} URLs`);
      await refreshAll();
      await refreshDocumentList();
    } catch (error) {
      setFlash(getErrorMessage(error, "Frontier seed failed"));
    } finally {
      setBusy(false);
    }
  }

  async function handleCreateRule(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setBusy(true);
    setFlash(null);
    try {
      await createRule(token, {
        name: ruleName,
        seed_url: ruleUrl,
        interval_minutes: Number(ruleInterval) || 60,
        max_depth: Number(ruleDepth) || 2,
        enabled: true,
      });
      setRuleName("");
      setRuleUrl("");
      await refreshAll();
      await refreshDocumentList();
    } catch (error) {
      setFlash(getErrorMessage(error, "Rule creation failed"));
    } finally {
      setBusy(false);
    }
  }

  async function handleToggleRule(ruleId: string, enabled: boolean) {
    setBusy(true);
    setFlash(null);
    try {
      await updateRule(token, ruleId, { enabled: !enabled });
      await refreshAll();
      await refreshDocumentList();
    } catch (error) {
      setFlash(getErrorMessage(error, "Rule update failed"));
    } finally {
      setBusy(false);
    }
  }

  async function handleDeleteRule(ruleId: string) {
    setBusy(true);
    setFlash(null);
    try {
      await deleteRule(token, ruleId);
      await refreshAll();
      await refreshDocumentList();
    } catch (error) {
      setFlash(getErrorMessage(error, "Rule delete failed"));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <section className="panel compact-panel">
        <h2>Manual crawl</h2>
        <form onSubmit={handleSeedFrontier}>
          <textarea
            value={seedUrls}
            onChange={(event) => setSeedUrls(event.target.value)}
            placeholder="One URL per line"
          />
          <div className="inline-form">
            <input
              value={seedDepth}
              onChange={(event) => setSeedDepth(event.target.value)}
              placeholder="Max depth"
            />
            <label className="checkbox">
              <input
                type="checkbox"
                checked={seedAllowRevisit}
                onChange={(event) => setSeedAllowRevisit(event.target.checked)}
              />
              Allow revisit
            </label>
            <button type="submit" disabled={busy}>
              Queue
            </button>
          </div>
        </form>
      </section>

      <section className="panel compact-panel">
        <h2>New auto rule</h2>
        <form onSubmit={handleCreateRule}>
          <input
            value={ruleName}
            onChange={(event) => setRuleName(event.target.value)}
            placeholder="Rule name"
          />
          <input
            value={ruleUrl}
            onChange={(event) => setRuleUrl(event.target.value)}
            placeholder="Seed URL"
          />
          <div className="inline-form">
            <input
              value={ruleInterval}
              onChange={(event) => setRuleInterval(event.target.value)}
              placeholder="Interval minutes"
            />
            <input
              value={ruleDepth}
              onChange={(event) => setRuleDepth(event.target.value)}
              placeholder="Max depth"
            />
            <button type="submit" disabled={busy}>
              Save
            </button>
          </div>
        </form>
      </section>

      <section className="panel panel-wide compact-panel">
        <div className="section-header">
          <h2>Auto crawl rules</h2>
          <span className="section-meta">{overview?.rules.length ?? 0} configured</span>
        </div>
        <div className="dense-list">
          {overview?.rules.length ? (
            overview.rules.map((rule) => (
              <div className="compact-row rule-row" key={rule.id}>
                <div className="row-primary">
                  <strong>{rule.name}</strong>
                  <span>{rule.seed_url}</span>
                </div>
                <div className="metadata-grid compact-metadata">
                  <div>
                    <span>Interval</span>
                    <strong>{rule.interval_minutes} min</strong>
                  </div>
                  <div>
                    <span>Depth</span>
                    <strong>{rule.max_depth}</strong>
                  </div>
                  <div>
                    <span>Created</span>
                    <strong>{rule.created_at}</strong>
                  </div>
                  <div>
                    <span>Updated</span>
                    <strong>{rule.updated_at}</strong>
                  </div>
                  <div>
                    <span>Last enqueue</span>
                    <strong>{rule.last_enqueued_at ?? "never"}</strong>
                  </div>
                  <div>
                    <span>Status</span>
                    <strong>{rule.enabled ? "enabled" : "disabled"}</strong>
                  </div>
                </div>
                <div className="row-actions topbar-actions">
                  <span className={rule.enabled ? "status-pill" : "status-pill status-pill-muted"}>
                    {rule.enabled ? "Enabled" : "Disabled"}
                  </span>
                  <button
                    type="button"
                    className="plain-link"
                    disabled={busy}
                    onClick={() => void handleToggleRule(rule.id, rule.enabled)}
                  >
                    {rule.enabled ? "Disable" : "Enable"}
                  </button>
                  <button
                    type="button"
                    className="plain-link"
                    disabled={busy}
                    onClick={() => void handleDeleteRule(rule.id)}
                  >
                    Delete
                  </button>
                </div>
              </div>
            ))
          ) : (
            <div className="list-row">No crawl rules yet.</div>
          )}
        </div>
      </section>
    </>
  );
}

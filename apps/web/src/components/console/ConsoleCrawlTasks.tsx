import { FormEvent, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  createRule,
  deleteRule,
  seedFrontier,
  updateRule,
  type CrawlRule,
  type DiscoveryScope,
} from "../../api";
import { PanelSection } from "../common/PanelPrimitives";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { Textarea } from "../ui/textarea";
import { useConsole } from "./ConsoleContext";

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

export function ConsoleCrawlTasks() {
  const { token, busy, setBusy, setFlash, refreshAll, refreshDocumentList, overview } = useConsole();
  const { t } = useTranslation();

  const [seedUrls, setSeedUrls] = useState("");
  const [seedDepth, setSeedDepth] = useState("2");
  const [seedMaxPages, setSeedMaxPages] = useState("50");
  const [seedSameOriginConcurrency, setSeedSameOriginConcurrency] = useState("1");
  const [seedScope, setSeedScope] = useState<DiscoveryScope>("same_domain");
  const [seedMaxDiscovered, setSeedMaxDiscovered] = useState("50");
  const [seedAllowRevisit, setSeedAllowRevisit] = useState(false);
  const [ruleName, setRuleName] = useState("");
  const [ruleUrl, setRuleUrl] = useState("");
  const [ruleInterval, setRuleInterval] = useState("60");
  const [ruleDepth, setRuleDepth] = useState("2");
  const [ruleMaxPages, setRuleMaxPages] = useState("50");
  const [ruleSameOriginConcurrency, setRuleSameOriginConcurrency] = useState("1");
  const [ruleScope, setRuleScope] = useState<DiscoveryScope>("same_domain");
  const [ruleMaxDiscovered, setRuleMaxDiscovered] = useState("50");
  const [editingRuleId, setEditingRuleId] = useState<string | null>(null);
  const [editRuleName, setEditRuleName] = useState("");
  const [editRuleUrl, setEditRuleUrl] = useState("");
  const [editRuleInterval, setEditRuleInterval] = useState("60");
  const [editRuleDepth, setEditRuleDepth] = useState("2");
  const [editRuleMaxPages, setEditRuleMaxPages] = useState("50");
  const [editRuleSameOriginConcurrency, setEditRuleSameOriginConcurrency] = useState("1");
  const [editRuleScope, setEditRuleScope] = useState<DiscoveryScope>("same_domain");
  const [editRuleMaxDiscovered, setEditRuleMaxDiscovered] = useState("50");
  const [editRuleEnabled, setEditRuleEnabled] = useState(true);

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
        Number(seedMaxPages) || 50,
        Number(seedSameOriginConcurrency) || 1,
        seedScope,
        Number(seedMaxDiscovered) || 50,
        seedAllowRevisit,
      );
      setFlash(t("console.tasks.queued_urls", { count: response.accepted_urls }));
      await refreshAll();
      await refreshDocumentList();
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.tasks.frontier_seed_failed")));
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
        max_pages: Number(ruleMaxPages) || 50,
        same_origin_concurrency: Number(ruleSameOriginConcurrency) || 1,
        discovery_scope: ruleScope,
        max_discovered_urls_per_page: Number(ruleMaxDiscovered) || 50,
        enabled: true,
      });
      setRuleName("");
      setRuleUrl("");
      setRuleMaxPages("50");
      setRuleSameOriginConcurrency("1");
      setRuleScope("same_domain");
      setRuleMaxDiscovered("50");
      await refreshAll();
      await refreshDocumentList();
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.tasks.rule_creation_failed")));
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
      setFlash(getErrorMessage(error, t("console.tasks.rule_update_failed")));
    } finally {
      setBusy(false);
    }
  }

  function handleStartEdit(rule: CrawlRule) {
    setEditingRuleId(rule.id);
    setEditRuleName(rule.name);
    setEditRuleUrl(rule.seed_url);
    setEditRuleInterval(String(rule.interval_minutes));
    setEditRuleDepth(String(rule.max_depth));
    setEditRuleMaxPages(String(rule.max_pages));
    setEditRuleSameOriginConcurrency(String(rule.same_origin_concurrency));
    setEditRuleScope(rule.discovery_scope);
    setEditRuleMaxDiscovered(String(rule.max_discovered_urls_per_page));
    setEditRuleEnabled(rule.enabled);
  }

  function handleCancelEdit() {
    setEditingRuleId(null);
  }

  async function handleSaveRule(ruleId: string) {
    setBusy(true);
    setFlash(null);
    try {
      await updateRule(token, ruleId, {
        name: editRuleName,
        seed_url: editRuleUrl,
        interval_minutes: Number(editRuleInterval) || 60,
        max_depth: Number(editRuleDepth) || 2,
        max_pages: Number(editRuleMaxPages) || 50,
        same_origin_concurrency: Number(editRuleSameOriginConcurrency) || 1,
        discovery_scope: editRuleScope,
        max_discovered_urls_per_page: Number(editRuleMaxDiscovered) || 50,
        enabled: editRuleEnabled,
      });
      setEditingRuleId(null);
      await refreshAll();
      await refreshDocumentList();
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.tasks.rule_update_failed")));
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
      setFlash(getErrorMessage(error, t("console.tasks.rule_delete_failed")));
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <PanelSection title={t("console.tasks.manual_seed_title")} contentClassName="space-y-5">
        <form onSubmit={handleSeedFrontier}>
          <label className="field-group">
            <span className="field-label">{t("console.tasks.urls_label")}</span>
            <Textarea
              value={seedUrls}
              onChange={(event) => setSeedUrls(event.target.value)}
              placeholder={t("console.tasks.urls_placeholder")}
            />
          </label>
          <div className="inline-form form-fields">
            <label className="field-group compact-field">
              <span className="field-label">{t("console.tasks.max_depth_label")}</span>
              <Input value={seedDepth} onChange={(event) => setSeedDepth(event.target.value)} />
              <span className="field-hint">{t("console.tasks.max_depth_hint")}</span>
            </label>
            <label className="field-group compact-field">
              <span className="field-label">{t("console.tasks.max_pages_label")}</span>
              <Input value={seedMaxPages} onChange={(event) => setSeedMaxPages(event.target.value)} />
              <span className="field-hint">{t("console.tasks.max_pages_hint")}</span>
            </label>
            <label className="field-group compact-field">
              <span className="field-label">{t("console.tasks.same_origin_concurrency_label")}</span>
              <Input
                value={seedSameOriginConcurrency}
                onChange={(event) => setSeedSameOriginConcurrency(event.target.value)}
              />
              <span className="field-hint">{t("console.tasks.same_origin_concurrency_hint")}</span>
            </label>
            <label className="field-group compact-field">
              <span className="field-label">{t("console.tasks.scope_label")}</span>
              <select
                value={seedScope}
                onChange={(event) => setSeedScope(event.target.value as DiscoveryScope)}
              >
                <option value="same_host">{t("console.tasks.scope_same_host")}</option>
                <option value="same_domain">{t("console.tasks.scope_same_domain")}</option>
                <option value="any">{t("console.tasks.scope_any")}</option>
              </select>
              <span className="field-hint">{t("console.tasks.scope_hint")}</span>
            </label>
            <label className="field-group compact-field">
              <span className="field-label">{t("console.tasks.max_links_label")}</span>
              <Input
                value={seedMaxDiscovered}
                onChange={(event) => setSeedMaxDiscovered(event.target.value)}
              />
              <span className="field-hint">{t("console.tasks.max_links_hint")}</span>
            </label>
            <label className="checkbox field-checkbox">
              <input
                type="checkbox"
                checked={seedAllowRevisit}
                onChange={(event) => setSeedAllowRevisit(event.target.checked)}
              />
              <span>{t("console.tasks.allow_revisit_label")}</span>
            </label>
            <Button type="submit" disabled={busy}>
              {t("console.tasks.submit_seed")}
            </Button>
          </div>
        </form>
      </PanelSection>

      <PanelSection title={t("console.tasks.create_rule_title")} contentClassName="space-y-5">
        <form onSubmit={handleCreateRule}>
          <div className="inline-form form-fields">
            <label className="field-group compact-field field-group-wide">
              <span className="field-label">{t("console.tasks.rule_name_label")}</span>
              <Input value={ruleName} onChange={(event) => setRuleName(event.target.value)} />
            </label>
            <label className="field-group compact-field field-group-wide">
              <span className="field-label">{t("console.tasks.seed_url_label")}</span>
              <Input value={ruleUrl} onChange={(event) => setRuleUrl(event.target.value)} />
            </label>
            <label className="field-group compact-field">
              <span className="field-label">{t("console.tasks.interval_label")}</span>
              <Input value={ruleInterval} onChange={(event) => setRuleInterval(event.target.value)} />
              <span className="field-hint">{t("console.tasks.interval_hint")}</span>
            </label>
            <label className="field-group compact-field">
              <span className="field-label">{t("console.tasks.max_depth_label")}</span>
              <Input value={ruleDepth} onChange={(event) => setRuleDepth(event.target.value)} />
              <span className="field-hint">{t("console.tasks.max_depth_hint")}</span>
            </label>
            <label className="field-group compact-field">
              <span className="field-label">{t("console.tasks.max_pages_label")}</span>
              <Input value={ruleMaxPages} onChange={(event) => setRuleMaxPages(event.target.value)} />
              <span className="field-hint">{t("console.tasks.max_pages_hint")}</span>
            </label>
            <label className="field-group compact-field">
              <span className="field-label">{t("console.tasks.same_origin_concurrency_label")}</span>
              <Input
                value={ruleSameOriginConcurrency}
                onChange={(event) => setRuleSameOriginConcurrency(event.target.value)}
              />
              <span className="field-hint">{t("console.tasks.same_origin_concurrency_hint")}</span>
            </label>
            <label className="field-group compact-field">
              <span className="field-label">{t("console.tasks.scope_label")}</span>
              <select
                value={ruleScope}
                onChange={(event) => setRuleScope(event.target.value as DiscoveryScope)}
              >
                <option value="same_host">{t("console.tasks.scope_same_host")}</option>
                <option value="same_domain">{t("console.tasks.scope_same_domain")}</option>
                <option value="any">{t("console.tasks.scope_any")}</option>
              </select>
              <span className="field-hint">{t("console.tasks.scope_hint")}</span>
            </label>
            <label className="field-group compact-field">
              <span className="field-label">{t("console.tasks.max_links_label")}</span>
              <Input
                value={ruleMaxDiscovered}
                onChange={(event) => setRuleMaxDiscovered(event.target.value)}
              />
              <span className="field-hint">{t("console.tasks.max_links_hint")}</span>
            </label>
            <Button type="submit" disabled={busy}>
              {t("console.users.save")}
            </Button>
          </div>
        </form>
      </PanelSection>

      <PanelSection
          title={t("console.tasks.rules_title")}
          meta={t("console.tasks.rules_configured", { count: overview?.rules.length ?? 0 })}
      >
        <div className="dense-list">
          {overview?.rules.length ? (
            overview.rules.map((rule) => (
              <div className="compact-row rule-row" key={rule.id}>
                {editingRuleId === rule.id ? (
                  <>
                    <div className="inline-form form-fields">
                      <label className="field-group compact-field field-group-wide">
                        <span className="field-label">{t("console.tasks.rule_name_label")}</span>
                        <Input
                          value={editRuleName}
                          onChange={(event) => setEditRuleName(event.target.value)}
                        />
                      </label>
                      <label className="field-group compact-field field-group-wide">
                        <span className="field-label">{t("console.tasks.seed_url_label")}</span>
                        <Input
                          value={editRuleUrl}
                          onChange={(event) => setEditRuleUrl(event.target.value)}
                        />
                      </label>
                      <label className="field-group compact-field">
                        <span className="field-label">{t("console.tasks.interval_label")}</span>
                        <Input
                          value={editRuleInterval}
                          onChange={(event) => setEditRuleInterval(event.target.value)}
                        />
                      </label>
                      <label className="field-group compact-field">
                        <span className="field-label">{t("console.tasks.max_depth_label")}</span>
                        <Input
                          value={editRuleDepth}
                          onChange={(event) => setEditRuleDepth(event.target.value)}
                        />
                      </label>
                      <label className="field-group compact-field">
                        <span className="field-label">{t("console.tasks.max_pages_label")}</span>
                        <Input
                          value={editRuleMaxPages}
                          onChange={(event) => setEditRuleMaxPages(event.target.value)}
                        />
                      </label>
                      <label className="field-group compact-field">
                        <span className="field-label">
                          {t("console.tasks.same_origin_concurrency_label")}
                        </span>
                        <Input
                          value={editRuleSameOriginConcurrency}
                          onChange={(event) =>
                            setEditRuleSameOriginConcurrency(event.target.value)
                          }
                        />
                      </label>
                      <label className="field-group compact-field">
                        <span className="field-label">{t("console.tasks.scope_label")}</span>
                        <select
                          value={editRuleScope}
                          onChange={(event) =>
                            setEditRuleScope(event.target.value as DiscoveryScope)
                          }
                        >
                          <option value="same_host">{t("console.tasks.scope_same_host")}</option>
                          <option value="same_domain">
                            {t("console.tasks.scope_same_domain")}
                          </option>
                          <option value="any">{t("console.tasks.scope_any")}</option>
                        </select>
                      </label>
                      <label className="field-group compact-field">
                        <span className="field-label">{t("console.tasks.max_links_label")}</span>
                        <Input
                          value={editRuleMaxDiscovered}
                          onChange={(event) => setEditRuleMaxDiscovered(event.target.value)}
                        />
                      </label>
                      <label className="checkbox field-checkbox">
                        <input
                          type="checkbox"
                          checked={editRuleEnabled}
                          onChange={(event) => setEditRuleEnabled(event.target.checked)}
                        />
                        <span>{t("console.tasks.enabled")}</span>
                      </label>
                    </div>
                  </>
                ) : (
                  <>
                    <div className="row-primary">
                      <strong>{rule.name}</strong>
                      <span>{rule.seed_url}</span>
                    </div>
                    <div className="metadata-grid compact-metadata">
                      <div>
                        <span>{t("console.tasks.interval")}</span>
                        <strong>{rule.interval_minutes} min</strong>
                      </div>
                      <div>
                        <span>{t("console.tasks.depth")}</span>
                        <strong>{rule.max_depth}</strong>
                      </div>
                      <div>
                        <span>{t("console.tasks.pages_limit")}</span>
                        <strong>{rule.max_pages}</strong>
                      </div>
                      <div>
                        <span>{t("console.tasks.same_origin_concurrency_label")}</span>
                        <strong>{rule.same_origin_concurrency}</strong>
                      </div>
                      <div>
                        <span>{t("console.tasks.created")}</span>
                        <strong>{rule.created_at}</strong>
                      </div>
                      <div>
                        <span>{t("console.tasks.updated")}</span>
                        <strong>{rule.updated_at}</strong>
                      </div>
                      <div>
                        <span>{t("console.tasks.last_enqueue")}</span>
                        <strong>{rule.last_enqueued_at ?? t("console.tasks.never")}</strong>
                      </div>
                      <div>
                        <span>{t("console.tasks.status")}</span>
                        <strong>{rule.enabled ? t("console.tasks.enabled") : t("console.tasks.disabled")}</strong>
                      </div>
                      <div>
                        <span>{t("console.tasks.scope_label")}</span>
                        <strong>{rule.discovery_scope}</strong>
                      </div>
                      <div>
                        <span>{t("console.tasks.links_per_page")}</span>
                        <strong>{rule.max_discovered_urls_per_page}</strong>
                      </div>
                    </div>
                  </>
                )}
                <div className="row-actions topbar-actions">
                  {editingRuleId === rule.id ? (
                    <>
                      <Button type="button" variant="outline" disabled={busy} onClick={() => void handleSaveRule(rule.id)}>
                        {t("console.users.save")}
                      </Button>
                      <Button type="button" variant="ghost" disabled={busy} onClick={handleCancelEdit}>
                        {t("console.workers.cancel")}
                      </Button>
                    </>
                  ) : (
                    <>
                      <span className={rule.enabled ? "status-pill" : "status-pill status-pill-muted"}>
                        {rule.enabled ? t("console.tasks.enabled") : t("console.tasks.disabled")}
                      </span>
                      <Button type="button" variant="ghost" size="sm" disabled={busy} onClick={() => handleStartEdit(rule)}>
                        {t("console.tasks.edit_rule")}
                      </Button>
                      <Button type="button" variant="ghost" size="sm" disabled={busy} onClick={() => void handleToggleRule(rule.id, rule.enabled)}>
                        {rule.enabled ? t("console.tasks.disable_rule") : t("console.tasks.enable_rule")}
                      </Button>
                      <Button type="button" variant="destructive" size="sm" disabled={busy} onClick={() => void handleDeleteRule(rule.id)}>
                        {t("console.tasks.delete_rule")}
                      </Button>
                    </>
                  )}
                </div>
              </div>
            ))
          ) : (
            <div className="list-row">{t("console.tasks.no_rules")}</div>
          )}
        </div>
      </PanelSection>
    </>
  );
}

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
import { FieldShell, PanelSection } from "../common/PanelPrimitives";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";
import { Card, CardContent } from "../ui/card";
import { Checkbox } from "../ui/checkbox";
import { Input } from "../ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../ui/select";
import { Textarea } from "../ui/textarea";
import { useConsole } from "./ConsoleContext";

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

function getScopeLabel(t: (key: string) => string, scope: DiscoveryScope) {
  switch (scope) {
    case "same_host":
      return t("console.tasks.scope_same_host");
    case "same_domain":
      return t("console.tasks.scope_same_domain");
    default:
      return t("console.tasks.scope_any");
  }
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
          <FieldShell label={t("console.tasks.urls_label")} className="mb-4">
            <Textarea
              value={seedUrls}
              onChange={(event) => setSeedUrls(event.target.value)}
              placeholder={t("console.tasks.urls_placeholder")}
            />
          </FieldShell>
          <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
            <FieldShell label={t("console.tasks.max_depth_label")} hint={t("console.tasks.max_depth_hint")}>
              <Input value={seedDepth} onChange={(event) => setSeedDepth(event.target.value)} />
            </FieldShell>
            <FieldShell label={t("console.tasks.max_pages_label")} hint={t("console.tasks.max_pages_hint")}>
              <Input value={seedMaxPages} onChange={(event) => setSeedMaxPages(event.target.value)} />
            </FieldShell>
            <FieldShell
              label={t("console.tasks.same_origin_concurrency_label")}
              hint={t("console.tasks.same_origin_concurrency_hint")}
            >
              <Input
                value={seedSameOriginConcurrency}
                onChange={(event) => setSeedSameOriginConcurrency(event.target.value)}
              />
            </FieldShell>
            <FieldShell label={t("console.tasks.scope_label")} hint={t("console.tasks.scope_hint")}>
              <Select value={seedScope} onValueChange={(value) => setSeedScope(value as DiscoveryScope)}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="same_host">{t("console.tasks.scope_same_host")}</SelectItem>
                  <SelectItem value="same_domain">{t("console.tasks.scope_same_domain")}</SelectItem>
                  <SelectItem value="any">{t("console.tasks.scope_any")}</SelectItem>
                </SelectContent>
              </Select>
            </FieldShell>
            <FieldShell label={t("console.tasks.max_links_label")} hint={t("console.tasks.max_links_hint")}>
              <Input
                value={seedMaxDiscovered}
                onChange={(event) => setSeedMaxDiscovered(event.target.value)}
              />
            </FieldShell>
            <label className="flex items-center gap-3 rounded-xl border border-border bg-muted/40 px-4 py-3 text-sm text-foreground">
              <Checkbox checked={seedAllowRevisit} onCheckedChange={(checked) => setSeedAllowRevisit(checked === true)} />
              <span>{t("console.tasks.allow_revisit_label")}</span>
            </label>
          </div>
          <div className="mt-4 flex justify-end">
            <Button type="submit" disabled={busy}>
              {t("console.tasks.submit_seed")}
            </Button>
          </div>
        </form>
      </PanelSection>

      <PanelSection title={t("console.tasks.create_rule_title")} contentClassName="space-y-5">
        <form onSubmit={handleCreateRule}>
          <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
            <FieldShell label={t("console.tasks.rule_name_label")}>
              <Input value={ruleName} onChange={(event) => setRuleName(event.target.value)} />
            </FieldShell>
            <FieldShell label={t("console.tasks.seed_url_label")}>
              <Input value={ruleUrl} onChange={(event) => setRuleUrl(event.target.value)} />
            </FieldShell>
            <FieldShell label={t("console.tasks.interval_label")} hint={t("console.tasks.interval_hint")}>
              <Input value={ruleInterval} onChange={(event) => setRuleInterval(event.target.value)} />
            </FieldShell>
            <FieldShell label={t("console.tasks.max_depth_label")} hint={t("console.tasks.max_depth_hint")}>
              <Input value={ruleDepth} onChange={(event) => setRuleDepth(event.target.value)} />
            </FieldShell>
            <FieldShell label={t("console.tasks.max_pages_label")} hint={t("console.tasks.max_pages_hint")}>
              <Input value={ruleMaxPages} onChange={(event) => setRuleMaxPages(event.target.value)} />
            </FieldShell>
            <FieldShell
              label={t("console.tasks.same_origin_concurrency_label")}
              hint={t("console.tasks.same_origin_concurrency_hint")}
            >
              <Input
                value={ruleSameOriginConcurrency}
                onChange={(event) => setRuleSameOriginConcurrency(event.target.value)}
              />
            </FieldShell>
            <FieldShell label={t("console.tasks.scope_label")} hint={t("console.tasks.scope_hint")}>
              <Select value={ruleScope} onValueChange={(value) => setRuleScope(value as DiscoveryScope)}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="same_host">{t("console.tasks.scope_same_host")}</SelectItem>
                  <SelectItem value="same_domain">{t("console.tasks.scope_same_domain")}</SelectItem>
                  <SelectItem value="any">{t("console.tasks.scope_any")}</SelectItem>
                </SelectContent>
              </Select>
            </FieldShell>
            <FieldShell label={t("console.tasks.max_links_label")} hint={t("console.tasks.max_links_hint")}>
              <Input
                value={ruleMaxDiscovered}
                onChange={(event) => setRuleMaxDiscovered(event.target.value)}
              />
            </FieldShell>
          </div>
          <div className="mt-4 flex justify-end">
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
        <div className="grid gap-3">
          {overview?.rules.length ? (
            overview.rules.map((rule) => (
              <Card key={rule.id} className="rounded-2xl">
                <CardContent className="grid gap-4 p-4">
                {editingRuleId === rule.id ? (
                  <>
                    <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
                      <FieldShell label={t("console.tasks.rule_name_label")}>
                        <Input
                          value={editRuleName}
                          onChange={(event) => setEditRuleName(event.target.value)}
                        />
                      </FieldShell>
                      <FieldShell label={t("console.tasks.seed_url_label")}>
                        <Input
                          value={editRuleUrl}
                          onChange={(event) => setEditRuleUrl(event.target.value)}
                        />
                      </FieldShell>
                      <FieldShell label={t("console.tasks.interval_label")}>
                        <Input
                          value={editRuleInterval}
                          onChange={(event) => setEditRuleInterval(event.target.value)}
                        />
                      </FieldShell>
                      <FieldShell label={t("console.tasks.max_depth_label")}>
                        <Input
                          value={editRuleDepth}
                          onChange={(event) => setEditRuleDepth(event.target.value)}
                        />
                      </FieldShell>
                      <FieldShell label={t("console.tasks.max_pages_label")}>
                        <Input
                          value={editRuleMaxPages}
                          onChange={(event) => setEditRuleMaxPages(event.target.value)}
                        />
                      </FieldShell>
                      <FieldShell label={t("console.tasks.same_origin_concurrency_label")}>
                        <Input
                          value={editRuleSameOriginConcurrency}
                          onChange={(event) =>
                            setEditRuleSameOriginConcurrency(event.target.value)
                          }
                        />
                      </FieldShell>
                      <FieldShell label={t("console.tasks.scope_label")}>
                        <Select value={editRuleScope} onValueChange={(value) => setEditRuleScope(value as DiscoveryScope)}>
                          <SelectTrigger>
                            <SelectValue />
                          </SelectTrigger>
                          <SelectContent>
                            <SelectItem value="same_host">{t("console.tasks.scope_same_host")}</SelectItem>
                            <SelectItem value="same_domain">{t("console.tasks.scope_same_domain")}</SelectItem>
                            <SelectItem value="any">{t("console.tasks.scope_any")}</SelectItem>
                          </SelectContent>
                        </Select>
                      </FieldShell>
                      <FieldShell label={t("console.tasks.max_links_label")}>
                        <Input
                          value={editRuleMaxDiscovered}
                          onChange={(event) => setEditRuleMaxDiscovered(event.target.value)}
                        />
                      </FieldShell>
                      <label className="flex items-center gap-3 rounded-xl border border-border bg-muted/40 px-4 py-3 text-sm text-foreground">
                        <Checkbox checked={editRuleEnabled} onCheckedChange={(checked) => setEditRuleEnabled(checked === true)} />
                        <span>{t("console.tasks.enabled")}</span>
                      </label>
                    </div>
                  </>
                ) : (
                  <>
                    <div className="grid gap-1">
                      <strong className="text-sm font-semibold text-foreground">{rule.name}</strong>
                      <span className="break-all text-sm text-muted-foreground">{rule.seed_url}</span>
                    </div>
                    <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-5">
                      <div className="rounded-xl border border-border bg-muted/40 p-4">
                        <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.tasks.interval")}</span>
                        <strong className="mt-2 block text-sm font-semibold text-foreground">{rule.interval_minutes} min</strong>
                      </div>
                      <div className="rounded-xl border border-border bg-muted/40 p-4">
                        <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.tasks.depth")}</span>
                        <strong className="mt-2 block text-sm font-semibold text-foreground">{rule.max_depth}</strong>
                      </div>
                      <div className="rounded-xl border border-border bg-muted/40 p-4">
                        <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.tasks.pages_limit")}</span>
                        <strong className="mt-2 block text-sm font-semibold text-foreground">{rule.max_pages}</strong>
                      </div>
                      <div className="rounded-xl border border-border bg-muted/40 p-4">
                        <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.tasks.same_origin_concurrency_label")}</span>
                        <strong className="mt-2 block text-sm font-semibold text-foreground">{rule.same_origin_concurrency}</strong>
                      </div>
                      <div className="rounded-xl border border-border bg-muted/40 p-4">
                        <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.tasks.created")}</span>
                        <strong className="mt-2 block text-sm font-semibold text-foreground">{rule.created_at}</strong>
                      </div>
                      <div className="rounded-xl border border-border bg-muted/40 p-4">
                        <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.tasks.updated")}</span>
                        <strong className="mt-2 block text-sm font-semibold text-foreground">{rule.updated_at}</strong>
                      </div>
                      <div className="rounded-xl border border-border bg-muted/40 p-4">
                        <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.tasks.last_enqueue")}</span>
                        <strong className="mt-2 block text-sm font-semibold text-foreground">{rule.last_enqueued_at ?? t("console.tasks.never")}</strong>
                      </div>
                      <div className="rounded-xl border border-border bg-muted/40 p-4">
                        <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.tasks.status")}</span>
                        <strong className="mt-2 block text-sm font-semibold text-foreground">{rule.enabled ? t("console.tasks.enabled") : t("console.tasks.disabled")}</strong>
                      </div>
                      <div className="rounded-xl border border-border bg-muted/40 p-4">
                        <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.tasks.scope_label")}</span>
                        <strong className="mt-2 block text-sm font-semibold text-foreground">{getScopeLabel(t, rule.discovery_scope)}</strong>
                      </div>
                      <div className="rounded-xl border border-border bg-muted/40 p-4">
                        <span className="text-xs uppercase tracking-[0.14em] text-muted-foreground">{t("console.tasks.links_per_page")}</span>
                        <strong className="mt-2 block text-sm font-semibold text-foreground">{rule.max_discovered_urls_per_page}</strong>
                      </div>
                    </div>
                  </>
                )}
                <div className="flex flex-wrap items-center gap-2">
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
                      <Badge variant={rule.enabled ? "success" : "outline"}>{rule.enabled ? t("console.tasks.enabled") : t("console.tasks.disabled")}</Badge>
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
                </CardContent>
              </Card>
            ))
          ) : (
            <div className="rounded-2xl border border-dashed border-border bg-muted/40 px-4 py-8 text-center text-sm text-muted-foreground">{t("console.tasks.no_rules")}</div>
          )}
        </div>
      </PanelSection>
    </>
  );
}

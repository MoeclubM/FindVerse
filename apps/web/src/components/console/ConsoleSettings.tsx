import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Copy, KeyRound, Shield, Waypoints } from "lucide-react";

import { getSystemConfig, setSystemConfig } from "../../api";
import { FieldShell, PanelSection } from "../common/PanelPrimitives";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import { Skeleton } from "../ui/skeleton";
import { Switch } from "../ui/switch";
import { useConsole } from "./ConsoleContext";

type ConfigMap = Record<string, string>;

function getErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

function toConfigMap(entries: Array<{ key: string; value: string }>): ConfigMap {
  return Object.fromEntries(entries.map((entry) => [entry.key, entry.value]));
}

export function ConsoleSettings() {
  const { token, setFlash } = useConsole();
  const { t } = useTranslation();
  const installServer =
    typeof window === "undefined" ? "<API_URL>" : `${window.location.origin}/api`;
  const [config, setConfig] = useState<ConfigMap>({});
  const [loading, setLoading] = useState(true);
  const [savingCrawler, setSavingCrawler] = useState(false);
  const [savingTor, setSavingTor] = useState(false);
  const [crawlerAuthKey, setCrawlerAuthKey] = useState("");
  const [workerConcurrency, setWorkerConcurrency] = useState("16");
  const [jsRenderConcurrency, setJsRenderConcurrency] = useState("1");
  const [maxJobs, setMaxJobs] = useState("16");
  const [claimTimeout, setClaimTimeout] = useState("");
  const [maxAttempts, setMaxAttempts] = useState("");
  const [torEnabled, setTorEnabled] = useState(false);
  const [torProxyUrl, setTorProxyUrl] = useState("");

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    getSystemConfig(token)
      .then((response) => {
        if (cancelled) return;
        const nextConfig = toConfigMap(response.entries);
        setConfig(nextConfig);
        setCrawlerAuthKey(nextConfig["crawler.auth_key"] ?? "");
        setWorkerConcurrency(nextConfig["crawler.total_concurrency"] ?? "16");
        setJsRenderConcurrency(nextConfig["crawler.js_render_concurrency"] ?? "1");
        setMaxJobs(nextConfig["crawler.max_jobs"] ?? "16");
        setClaimTimeout(nextConfig["crawler.claim_timeout_secs"] ?? "");
        setMaxAttempts(nextConfig["crawler.max_attempts"] ?? "");
        setTorEnabled(nextConfig["crawler.tor_enabled"] === "true");
        setTorProxyUrl(nextConfig["crawler.tor_proxy_url"] ?? "");
      })
      .catch((error) => {
        if (!cancelled) setFlash(getErrorMessage(error, t("console.settings.save_error")));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [token, setFlash, t]);

  const nextWorkerConcurrency = String(Math.max(1, Number(workerConcurrency) || 16));
  const nextJsRenderConcurrency = String(Math.max(1, Number(jsRenderConcurrency) || 1));
  const nextMaxJobs = String(Math.max(1, Number(maxJobs) || 16));

  const crawlerDirty = useMemo(
    () =>
      crawlerAuthKey !== (config["crawler.auth_key"] ?? "") ||
      workerConcurrency !== (config["crawler.total_concurrency"] ?? "16") ||
      jsRenderConcurrency !== (config["crawler.js_render_concurrency"] ?? "1") ||
      maxJobs !== (config["crawler.max_jobs"] ?? "16") ||
      claimTimeout !== (config["crawler.claim_timeout_secs"] ?? "") ||
      maxAttempts !== (config["crawler.max_attempts"] ?? ""),
    [
      crawlerAuthKey,
      workerConcurrency,
      jsRenderConcurrency,
      maxJobs,
      claimTimeout,
      maxAttempts,
      config,
    ],
  );

  const torDirty = useMemo(
    () =>
      torEnabled !== (config["crawler.tor_enabled"] === "true") ||
      torProxyUrl !== (config["crawler.tor_proxy_url"] ?? ""),
    [torEnabled, torProxyUrl, config],
  );

  const installCommand = crawlerAuthKey.trim()
    ? `curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo bash -s -- --server ${installServer} --crawler-key ${crawlerAuthKey.trim()}`
    : "";

  async function handleSaveCrawlerConfig() {
    setSavingCrawler(true);
    setFlash(null);
    try {
      await Promise.all([
        setSystemConfig(token, "crawler.auth_key", crawlerAuthKey.trim() || null),
        setSystemConfig(token, "crawler.total_concurrency", nextWorkerConcurrency),
        setSystemConfig(token, "crawler.js_render_concurrency", nextJsRenderConcurrency),
        setSystemConfig(token, "crawler.max_jobs", nextMaxJobs),
        setSystemConfig(token, "crawler.claim_timeout_secs", claimTimeout.trim() || null),
        setSystemConfig(token, "crawler.max_attempts", maxAttempts.trim() || null),
      ]);
      setConfig((current) => ({
        ...current,
        "crawler.auth_key": crawlerAuthKey.trim(),
        "crawler.total_concurrency": nextWorkerConcurrency,
        "crawler.js_render_concurrency": nextJsRenderConcurrency,
        "crawler.max_jobs": nextMaxJobs,
        "crawler.claim_timeout_secs": claimTimeout.trim(),
        "crawler.max_attempts": maxAttempts.trim(),
      }));
      setWorkerConcurrency(nextWorkerConcurrency);
      setJsRenderConcurrency(nextJsRenderConcurrency);
      setMaxJobs(nextMaxJobs);
      setFlash(t("console.settings.save_success"));
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.settings.save_error")));
    } finally {
      setSavingCrawler(false);
    }
  }

  async function handleSaveTorConfig() {
    setSavingTor(true);
    setFlash(null);
    try {
      await Promise.all([
        setSystemConfig(token, "crawler.tor_enabled", torEnabled ? "true" : "false"),
        setSystemConfig(token, "crawler.tor_proxy_url", torProxyUrl.trim() || null),
      ]);
      setConfig((current) => ({
        ...current,
        "crawler.tor_enabled": torEnabled ? "true" : "false",
        "crawler.tor_proxy_url": torProxyUrl.trim(),
      }));
      setFlash(t("console.settings.save_success"));
    } catch (error) {
      setFlash(getErrorMessage(error, t("console.settings.save_error")));
    } finally {
      setSavingTor(false);
    }
  }

  async function handleCopyInstallCommand() {
    if (!installCommand) return;
    try {
      await navigator.clipboard.writeText(installCommand);
      setFlash(t("console.settings.save_success"));
    } catch {
      setFlash(installCommand);
    }
  }

  if (loading) {
    return (
      <div className="grid gap-4">
        <div className="rounded-2xl border border-border p-6">
          <div className="grid gap-5">
            <Skeleton className="h-5 w-56" />
            <div className="grid gap-4 xl:grid-cols-[minmax(0,1.6fr)_minmax(0,0.8fr)_minmax(0,0.8fr)_minmax(0,0.8fr)_minmax(0,0.8fr)_minmax(0,0.8fr)_auto] xl:items-end">
              <Skeleton className="h-10 rounded-lg" />
              <Skeleton className="h-10 rounded-lg" />
              <Skeleton className="h-10 rounded-lg" />
              <Skeleton className="h-10 rounded-lg" />
              <Skeleton className="h-10 rounded-lg" />
              <Skeleton className="h-10 rounded-lg" />
              <Skeleton className="h-8 w-24 rounded-lg" />
            </div>
            <div className="grid gap-3 md:grid-cols-6">
              <Skeleton className="h-24 rounded-xl" />
              <Skeleton className="h-24 rounded-xl" />
              <Skeleton className="h-24 rounded-xl" />
              <Skeleton className="h-24 rounded-xl" />
              <Skeleton className="h-24 rounded-xl" />
              <Skeleton className="h-24 rounded-xl" />
            </div>
          </div>
        </div>
        <div className="rounded-2xl border border-border p-6">
          <div className="grid gap-5">
            <Skeleton className="h-5 w-40" />
            <Skeleton className="h-12 rounded-xl" />
            <div className="grid gap-4 lg:grid-cols-[1fr_auto] lg:items-end">
              <Skeleton className="h-10 rounded-lg" />
              <Skeleton className="h-8 w-24 rounded-lg" />
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <PanelSection
        title={t("console.settings.crawler_config_section")}
        contentClassName="space-y-5"
      >
        <div className="grid gap-4 xl:grid-cols-[minmax(0,1.6fr)_minmax(0,0.8fr)_minmax(0,0.8fr)_minmax(0,0.8fr)_minmax(0,0.8fr)_minmax(0,0.8fr)_auto] xl:items-end">
          <FieldShell className="lg:col-span-1" label={t("console.settings.auth_key_label")}>
            <Input
              value={crawlerAuthKey}
              onChange={(event) => setCrawlerAuthKey(event.target.value)}
              placeholder={t("console.settings.auth_key_placeholder")}
            />
          </FieldShell>
          <FieldShell label={t("console.settings.total_concurrency_label")}>
            <Input
              type="number"
              min={1}
              value={workerConcurrency}
              onChange={(event) => setWorkerConcurrency(event.target.value)}
            />
          </FieldShell>
          <FieldShell label={t("console.settings.js_render_concurrency_label")}>
            <Input
              type="number"
              min={1}
              value={jsRenderConcurrency}
              onChange={(event) => setJsRenderConcurrency(event.target.value)}
            />
          </FieldShell>
          <FieldShell label={t("console.settings.max_jobs_label")}>
            <Input
              type="number"
              min={1}
              value={maxJobs}
              onChange={(event) => setMaxJobs(event.target.value)}
            />
          </FieldShell>
          <FieldShell label={t("console.settings.claim_timeout_label")}>
            <Input value={claimTimeout} onChange={(event) => setClaimTimeout(event.target.value)} />
          </FieldShell>
          <FieldShell label={t("console.settings.max_attempts_label")}>
            <Input value={maxAttempts} onChange={(event) => setMaxAttempts(event.target.value)} />
          </FieldShell>
          <Button disabled={savingCrawler || !crawlerDirty} onClick={() => void handleSaveCrawlerConfig()}>
            <KeyRound data-icon="inline-start" />
            {t("console.settings.save")}
          </Button>
        </div>

        <div className="grid gap-3 md:grid-cols-6">
          <div className="rounded-xl border border-border bg-muted/40 p-4">
            <div className="flex items-center gap-2 text-sm font-medium text-foreground"><Shield className="size-4" />{t("console.settings.summary.auth")}</div>
            <p className="mt-2 text-sm text-muted-foreground">{t("console.settings.auth_key_label")}</p>
          </div>
          <div className="rounded-xl border border-border bg-muted/40 p-4">
            <div className="flex items-center gap-2 text-sm font-medium text-foreground"><Waypoints className="size-4" />{t("console.settings.summary.total")}</div>
            <p className="mt-2 text-sm text-muted-foreground">{nextWorkerConcurrency}</p>
          </div>
          <div className="rounded-xl border border-border bg-muted/40 p-4">
            <div className="flex items-center gap-2 text-sm font-medium text-foreground"><Waypoints className="size-4" />{t("console.settings.summary.render")}</div>
            <p className="mt-2 text-sm text-muted-foreground">{nextJsRenderConcurrency}</p>
          </div>
          <div className="rounded-xl border border-border bg-muted/40 p-4">
            <div className="flex items-center gap-2 text-sm font-medium text-foreground"><Waypoints className="size-4" />{t("console.settings.summary.max_jobs")}</div>
            <p className="mt-2 text-sm text-muted-foreground">{nextMaxJobs}</p>
          </div>
          <div className="rounded-xl border border-border bg-muted/40 p-4">
            <div className="flex items-center gap-2 text-sm font-medium text-foreground"><Waypoints className="size-4" />{t("console.settings.summary.claim")}</div>
            <p className="mt-2 text-sm text-muted-foreground">{t("console.settings.claim_timeout_label")}</p>
          </div>
          <div className="rounded-xl border border-border bg-muted/40 p-4">
            <div className="flex items-center gap-2 text-sm font-medium text-foreground"><Waypoints className="size-4" />{t("console.settings.summary.retry")}</div>
            <p className="mt-2 text-sm text-muted-foreground">{t("console.settings.max_attempts_label")}</p>
          </div>
        </div>

        {installCommand ? (
          <div className="rounded-2xl border border-border bg-foreground p-4 text-background shadow-sm">
            <div className="mb-3 flex items-center justify-between gap-2">
              <div>
                <p className="text-sm font-medium">{t("console.workers.setup_hint")}</p>
                <p className="text-xs text-background/70">{installServer}</p>
              </div>
              <Button variant="secondary" size="sm" onClick={() => void handleCopyInstallCommand()}>
                <Copy data-icon="inline-start" />
                {t("console.actions.copy")}
              </Button>
            </div>
            <pre className="overflow-x-auto whitespace-pre-wrap break-all text-xs leading-6 text-background/90">{installCommand}</pre>
          </div>
        ) : null}
      </PanelSection>

      <PanelSection title={t("console.settings.tor_section")} contentClassName="space-y-5">
        <label className="flex items-center justify-between gap-3 rounded-xl border border-border bg-muted/40 px-4 py-3 text-sm text-foreground">
          <span>{t("console.settings.tor_enabled_label")}</span>
          <Switch checked={torEnabled} onCheckedChange={setTorEnabled} />
        </label>
        <div className="grid gap-4 lg:grid-cols-[1fr_auto] lg:items-end">
          <FieldShell className="lg:col-span-1" label={t("console.settings.tor_proxy_url_label")}>
            <Input
              value={torProxyUrl}
              onChange={(event) => setTorProxyUrl(event.target.value)}
              placeholder={t("console.settings.tor_proxy_placeholder")}
            />
          </FieldShell>
          <Button disabled={savingTor || !torDirty} onClick={() => void handleSaveTorConfig()}>
            {t("console.settings.save")}
          </Button>
        </div>
      </PanelSection>
    </div>
  );
}

import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Copy, KeyRound, Shield, Waypoints } from "lucide-react";

import { getSystemConfig, setSystemConfig } from "../../api";
import { FieldShell, PanelSection } from "../common/PanelPrimitives";
import { Button } from "../ui/button";
import { Checkbox } from "../ui/checkbox";
import { Input } from "../ui/input";
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

  const crawlerDirty = useMemo(
    () =>
      crawlerAuthKey !== (config["crawler.auth_key"] ?? "") ||
      claimTimeout !== (config["crawler.claim_timeout_secs"] ?? "") ||
      maxAttempts !== (config["crawler.max_attempts"] ?? ""),
    [crawlerAuthKey, claimTimeout, maxAttempts, config],
  );

  const torDirty = useMemo(
    () =>
      torEnabled !== (config["crawler.tor_enabled"] === "true") ||
      torProxyUrl !== (config["crawler.tor_proxy_url"] ?? ""),
    [torEnabled, torProxyUrl, config],
  );

  const installCommand = crawlerAuthKey.trim()
    ? `curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo bash -s -- --server ${installServer} --crawler-key ${crawlerAuthKey.trim()} --channel release --concurrency 16`
    : "";

  async function handleSaveCrawlerConfig() {
    setSavingCrawler(true);
    setFlash(null);
    try {
      await Promise.all([
        setSystemConfig(token, "crawler.auth_key", crawlerAuthKey.trim() || null),
        setSystemConfig(token, "crawler.claim_timeout_secs", claimTimeout.trim() || null),
        setSystemConfig(token, "crawler.max_attempts", maxAttempts.trim() || null),
      ]);
      setConfig((current) => ({
        ...current,
        "crawler.auth_key": crawlerAuthKey.trim(),
        "crawler.claim_timeout_secs": claimTimeout.trim(),
        "crawler.max_attempts": maxAttempts.trim(),
      }));
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
    return <div className="rounded-2xl border border-dashed border-border bg-muted/40 px-4 py-8 text-center text-sm text-muted-foreground">{t("console.settings.loading")}</div>;
  }

  return (
    <div className="space-y-4">
      <PanelSection
        title={t("console.settings.crawler_config_section")}
        meta={t("console.live_refresh")}
        contentClassName="space-y-5"
      >
        <div className="grid gap-4 lg:grid-cols-[1.6fr_0.7fr_0.7fr_auto] lg:items-end">
          <FieldShell className="lg:col-span-1" label={t("console.settings.auth_key_label")}>
            <Input
              value={crawlerAuthKey}
              onChange={(event) => setCrawlerAuthKey(event.target.value)}
              placeholder={t("console.settings.auth_key_placeholder")}
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

        <div className="grid gap-3 md:grid-cols-3">
          <div className="rounded-xl border border-border bg-muted/40 p-4">
            <div className="flex items-center gap-2 text-sm font-medium text-foreground"><Shield className="size-4" />{t("console.settings.summary.auth")}</div>
            <p className="mt-2 text-sm text-muted-foreground">{t("console.settings.auth_key_label")}</p>
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
        <label className="flex items-center gap-3 rounded-xl border border-border bg-muted/40 px-4 py-3 text-sm text-foreground">
          <Checkbox checked={torEnabled} onCheckedChange={(checked) => setTorEnabled(checked === true)} />
          <span>{t("console.settings.tor_enabled_label")}</span>
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

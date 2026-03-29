import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";

import { getSystemConfig, setSystemConfig } from "../../api";
import { FieldShell, SectionHeader } from "../common/PanelPrimitives";
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
  const [config, setConfig] = useState<ConfigMap>({});
  const [loading, setLoading] = useState(true);
  const [savingCrawler, setSavingCrawler] = useState(false);
  const [savingTor, setSavingTor] = useState(false);
  const [claimTimeout, setClaimTimeout] = useState("");
  const [maxAttempts, setMaxAttempts] = useState("");
  const [torEnabled, setTorEnabled] = useState(false);
  const [torProxyUrl, setTorProxyUrl] = useState("");

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    getSystemConfig(token)
      .then((response) => {
        if (cancelled) {
          return;
        }
        const nextConfig = toConfigMap(response.entries);
        setConfig(nextConfig);
        setClaimTimeout(nextConfig["crawler.claim_timeout_secs"] ?? "");
        setMaxAttempts(nextConfig["crawler.max_attempts"] ?? "");
        setTorEnabled(nextConfig["crawler.tor_enabled"] === "true");
        setTorProxyUrl(nextConfig["crawler.tor_proxy_url"] ?? "");
      })
      .catch((error) => {
        if (!cancelled) {
          setFlash(getErrorMessage(error, t("console.settings.save_error")));
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
  }, [token, setFlash, t]);

  const crawlerDirty = useMemo(
    () =>
      claimTimeout !== (config["crawler.claim_timeout_secs"] ?? "") ||
      maxAttempts !== (config["crawler.max_attempts"] ?? ""),
    [claimTimeout, maxAttempts, config],
  );

  const torDirty = useMemo(
    () =>
      torEnabled !== (config["crawler.tor_enabled"] === "true") ||
      torProxyUrl !== (config["crawler.tor_proxy_url"] ?? ""),
    [torEnabled, torProxyUrl, config],
  );

  async function handleSaveCrawlerConfig() {
    setSavingCrawler(true);
    setFlash(null);
    try {
      await Promise.all([
        setSystemConfig(token, "crawler.claim_timeout_secs", claimTimeout.trim() || null),
        setSystemConfig(token, "crawler.max_attempts", maxAttempts.trim() || null),
      ]);
      setConfig((current) => ({
        ...current,
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

  if (loading) {
    return <div className="list-row">{t("console.settings.loading")}</div>;
  }

  return (
    <>
      <section className="panel panel-wide compact-panel">
        <SectionHeader title={t("console.settings.crawler_config_section")} />
        <div className="inline-form form-fields">
          <FieldShell className="compact-field" label={t("console.settings.claim_timeout_label")}>
            <input value={claimTimeout} onChange={(event) => setClaimTimeout(event.target.value)} />
          </FieldShell>
          <FieldShell className="compact-field" label={t("console.settings.max_attempts_label")}>
            <input value={maxAttempts} onChange={(event) => setMaxAttempts(event.target.value)} />
          </FieldShell>
          <button type="button" disabled={savingCrawler || !crawlerDirty} onClick={() => void handleSaveCrawlerConfig()}>
            {t("console.settings.save")}
          </button>
        </div>
      </section>

      <section className="panel panel-wide compact-panel">
        <SectionHeader title={t("console.settings.tor_section")} />
        <div className="inline-form">
          <label className="checkbox">
            <input type="checkbox" checked={torEnabled} onChange={(event) => setTorEnabled(event.target.checked)} />
            {t("console.settings.tor_enabled_label")}
          </label>
        </div>
        <div className="inline-form form-fields">
          <FieldShell className="compact-field field-group-wide" label={t("console.settings.tor_proxy_url_label")}>
            <input
              value={torProxyUrl}
              onChange={(event) => setTorProxyUrl(event.target.value)}
              placeholder={t("console.settings.tor_proxy_placeholder")}
            />
          </FieldShell>
          <button type="button" disabled={savingTor || !torDirty} onClick={() => void handleSaveTorConfig()}>
            {t("console.settings.save")}
          </button>
        </div>
      </section>
    </>
  );
}

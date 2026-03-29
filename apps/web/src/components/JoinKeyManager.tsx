import { FormEvent, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { getCrawlerJoinKey, setCrawlerJoinKey } from "../api";
import { FieldShell } from "./common/PanelPrimitives";

export function JoinKeyManager(props: { token: string; setFlash: (value: string | null) => void }) {
  const { token, setFlash } = props;
  const { t } = useTranslation();
  const [joinKey, setJoinKey] = useState<string | null>(null);
  const [newKey, setNewKey] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    getCrawlerJoinKey(token)
      .then((res) => {
        setJoinKey(res.join_key);
        setNewKey(res.join_key ?? "");
      })
      .catch(() => setFlash(t("console.settings.join_key_load_error")))
      .finally(() => setLoading(false));
  }, [token, setFlash, t]);

  async function handleSave(e: FormEvent) {
    e.preventDefault();
    setSaving(true);
    setFlash(null);
    try {
      await setCrawlerJoinKey(token, newKey.trim() || null);
      setJoinKey(newKey.trim() || null);
      setFlash(t("console.settings.join_key_update_success"));
    } catch {
      setFlash(t("console.settings.join_key_update_error"));
    } finally {
      setSaving(false);
    }
  }

  if (loading) return <p>{t("console.settings.loading")}</p>;

  return (
    <div>
      <form className="inline-form form-fields" onSubmit={handleSave}>
        <FieldShell className="compact-field field-group-wide" label={t("console.settings.join_key_section")}>
          <input
            value={newKey}
            onChange={(e) => setNewKey(e.target.value)}
            placeholder={t("console.settings.join_key_placeholder")}
          />
        </FieldShell>
        <button type="submit" disabled={saving}>
          {t("console.settings.save_key")}
        </button>
      </form>
      {joinKey ? (
        <p className="section-meta" style={{ marginTop: 8 }}>
          {t("console.settings.current_join_key")} <code>{joinKey}</code>
        </p>
      ) : (
        <p className="section-meta" style={{ marginTop: 8 }}>
          {t("console.settings.no_join_key")}
        </p>
      )}
      <details style={{ marginTop: 8 }}>
        <summary className="section-meta">{t("console.settings.worker_setup")}</summary>
        <pre style={{ fontSize: "0.85em", marginTop: 4 }}>
{`Install or update a worker with the same command:

curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo bash -s -- --server <API_URL> --join-key <KEY> --channel release --concurrency 16 --skip-browser-install

# only use GITHUB_TOKEN when following the latest CI build with --channel dev
# curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo env GITHUB_TOKEN=<TOKEN> bash -s -- --server <API_URL> --join-key <KEY> --channel dev --concurrency 16 --skip-browser-install`}
        </pre>
      </details>
    </div>
  );
}

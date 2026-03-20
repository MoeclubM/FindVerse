import { FormEvent, useEffect, useState } from "react";

import { getCrawlerJoinKey, setCrawlerJoinKey } from "../api";

export function JoinKeyManager({ token }: { token: string }) {
  const [joinKey, setJoinKey] = useState<string | null>(null);
  const [newKey, setNewKey] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [flash, setFlash] = useState<string | null>(null);

  useEffect(() => {
    getCrawlerJoinKey(token)
      .then((res) => {
        setJoinKey(res.join_key);
        setNewKey(res.join_key ?? "");
      })
      .catch(() => setFlash("Failed to load join key"))
      .finally(() => setLoading(false));
  }, [token]);

  async function handleSave(e: FormEvent) {
    e.preventDefault();
    setSaving(true);
    setFlash(null);
    try {
      await setCrawlerJoinKey(token, newKey.trim() || null);
      setJoinKey(newKey.trim() || null);
      setFlash("Join key updated");
    } catch {
      setFlash("Failed to update join key");
    } finally {
      setSaving(false);
    }
  }

  function handleGenerate() {
    const chars = "abcdefghijklmnopqrstuvwxyz0123456789";
    let key = "fvjk_";
    for (let i = 0; i < 32; i++) key += chars[Math.floor(Math.random() * chars.length)];
    setNewKey(key);
  }

  if (loading) return <p>Loading...</p>;

  return (
    <div>
      {flash && <p className="flash">{flash}</p>}
      <form className="inline-form" onSubmit={handleSave}>
        <input
          value={newKey}
          onChange={(e) => setNewKey(e.target.value)}
          placeholder="Join key (leave empty to disable)"
          style={{ minWidth: 300 }}
        />
        <button type="button" onClick={handleGenerate}>Generate</button>
        <button type="submit" disabled={saving}>Save</button>
      </form>
      {joinKey ? (
        <p className="section-meta" style={{ marginTop: 8 }}>
          Current key: <code>{joinKey}</code>
        </p>
      ) : (
        <p className="section-meta" style={{ marginTop: 8 }}>No join key configured. External crawlers cannot self-register.</p>
      )}
      <details style={{ marginTop: 8 }}>
        <summary className="section-meta">Setup instructions</summary>
        <pre style={{ fontSize: "0.85em", marginTop: 4 }}>
{`# Unix
./scripts/crawler-setup.sh --server <API_URL> --join-key <KEY> --start

# Windows
.\\scripts\\crawler-setup.ps1 -Server <API_URL> -JoinKey <KEY> -Start

# Or directly with cargo:
cargo run -p findverse-crawler -- worker --server <API_URL> --join-key <KEY>`}
        </pre>
      </details>
    </div>
  );
}

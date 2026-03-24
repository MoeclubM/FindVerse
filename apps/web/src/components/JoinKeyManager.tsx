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

  if (loading) return <p>Loading...</p>;

  return (
    <div>
      {flash && <p className="flash">{flash}</p>}
      <form className="inline-form" onSubmit={handleSave}>
        <input
          value={newKey}
          onChange={(e) => setNewKey(e.target.value)}
          placeholder="Join key for enrolling new workers"
          style={{ minWidth: 300 }}
        />
        <button type="submit" disabled={saving}>Save key</button>
      </form>
      {joinKey ? (
        <p className="section-meta" style={{ marginTop: 8 }}>
          Current join key: <code>{joinKey}</code>
        </p>
      ) : (
        <p className="section-meta" style={{ marginTop: 8 }}>
          No join key configured. New workers cannot enroll until you set one.
        </p>
      )}
      <details style={{ marginTop: 8 }}>
        <summary className="section-meta">Worker setup</summary>
        <pre style={{ fontSize: "0.85em", marginTop: 4 }}>
{`Share this key with crawler operators so they can register a worker. The join key is only used during enrollment; after joining, each worker continues with its own crawler credentials.

Use any supported worker startup flow that passes the join key, for example:

# Unix setup script
./scripts/crawler-setup.sh --server <API_URL> --join-key <KEY> --start

# Windows setup script
.\\scripts\\crawler-setup.ps1 -Server <API_URL> -JoinKey <KEY> -Start

# Direct worker launch
cargo run -p findverse-crawler -- worker --server <API_URL> --join-key <KEY>`}
        </pre>
      </details>
    </div>
  );
}

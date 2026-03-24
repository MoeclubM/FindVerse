import { JoinKeyManager } from "../JoinKeyManager";
import { useConsole } from "./ConsoleContext";

export function ConsoleSettings() {
  const { token } = useConsole();

  return (
    <section className="panel panel-wide compact-panel">
      <div className="section-header">
        <h2>Crawler join key</h2>
        <span className="section-meta">Workers join with this key, then receive their own crawler credentials</span>
      </div>
      <JoinKeyManager token={token} />
    </section>
  );
}

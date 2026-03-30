function humanizeConsoleToken(value: string) {
  return value
    .split(/[-_]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

export function getConsoleEventKindLabel(t: (key: string) => string, kind: string) {
  const key = `console.events.kinds.${kind.replace(/-/g, "_")}`;
  const label = t(key);
  return label === key ? humanizeConsoleToken(kind) : label;
}

export function getConsoleEventStatusLabel(t: (key: string) => string, status: string) {
  const key = `console.events.statuses.${status}`;
  const label = t(key);
  return label === key ? humanizeConsoleToken(status) : label;
}

export function getConsoleJobStatusLabel(t: (key: string) => string, status: string) {
  const jobStatusKeys: Record<string, string> = {
    queued: "console.jobs.stats.queued",
    claimed: "console.jobs.stats.claimed",
    succeeded: "console.jobs.stats.succeeded",
    failed: "console.jobs.stats.failed",
    blocked: "console.jobs.stats.blocked",
    dead_letter: "console.jobs.stats.dead_letter",
  };
  const key = jobStatusKeys[status];
  return key ? t(key) : humanizeConsoleToken(status);
}

export function getConsoleValueLabel(value: string | null) {
  return value ? humanizeConsoleToken(value) : "-";
}

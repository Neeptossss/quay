export function age(updatedAt: string, now: number = Date.now()): string {
  const moment = Date.parse(updatedAt);
  if (Number.isNaN(moment)) return "?";
  const seconds = Math.max(0, Math.floor((now - moment) / 1000));
  if (seconds < 90) return `${seconds}s`;
  if (seconds < 5400) return `${Math.floor(seconds / 60)}m`;
  if (seconds < 172800) return `${Math.floor(seconds / 3600)}h`;
  return `${Math.floor(seconds / 86400)}j`;
}

export function checksTone(state: string | null): string {
  if (state === "success") return "good";
  if (state === "failure" || state === "error") return "bad";
  if (state === "pending" || state === "expected") return "waiting";
  return "absent";
}

export function checksIcon(state: string | null): string {
  if (state === "success") return "circle-check";
  if (state === "failure" || state === "error") return "circle-alert";
  if (state === "pending" || state === "expected") return "loader";
  return "dot";
}

export function reviewTone(state: string | null): string {
  if (state === "approved") return "good";
  if (state === "changes_requested") return "bad";
  if (state === "review_required") return "waiting";
  return "absent";
}

export function reviewIcon(state: string | null): string {
  if (state === "approved") return "check";
  if (state === "changes_requested") return "pencil-line";
  if (state === "review_required") return "circle-dot";
  return "dot";
}

export function syncIcon(phase: string): string {
  if (phase === "offline") return "circle-alert";
  if (phase === "idle") return "circle-check";
  return "loader";
}

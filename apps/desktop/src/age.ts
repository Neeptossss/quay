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

export function checksLabel(state: string | null): string {
  if (state === "success") return "ok";
  if (state === "failure" || state === "error") return "ko";
  if (state === "pending" || state === "expected") return "…";
  return "—";
}

export function reviewTone(state: string | null): string {
  if (state === "approved") return "good";
  if (state === "changes_requested") return "bad";
  if (state === "review_required") return "waiting";
  return "absent";
}

export function reviewLabel(state: string | null): string {
  if (state === "approved") return "appr";
  if (state === "changes_requested") return "chng";
  if (state === "review_required") return "attn";
  return "—";
}

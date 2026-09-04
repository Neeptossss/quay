import type { EventEntry, FeedEntry, ThreadEntry } from "./ipc";

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

export interface ScopeSplit<T> {
  shown: T[];
  hidden: T[];
}

export function activeFirst<T extends { openPullRequests: number }>(
  scopes: T[],
  expanded: boolean,
  keep = 6,
): ScopeSplit<T> {
  if (expanded) return { shown: scopes, hidden: [] };
  const active = scopes.filter((scope) => scope.openPullRequests > 0);
  const shown = active.length > 0 ? active.slice(0, keep) : scopes.slice(0, keep);
  const hidden = scopes.filter((scope) => !shown.includes(scope));
  return { shown, hidden };
}

export function unresolvedThreads(feed: FeedEntry[]): ThreadEntry[] {
  return feed.filter(
    (item): item is ThreadEntry => item.item === "thread" && !item.isResolved,
  );
}

export function stepThrough(threads: ThreadEntry[], focused: string | null, delta: number): string | null {
  if (threads.length === 0) return null;
  const at = threads.findIndex((thread) => thread.nodeId === focused);
  if (at === -1) return (delta > 0 ? threads[0] : threads[threads.length - 1]).nodeId;
  const next = (at + delta + threads.length) % threads.length;
  return threads[next].nodeId;
}

export function reviewVerdict(event: EventEntry): string {
  const state = event.reference ?? "commented";
  return ["approved", "changes_requested", "dismissed"].includes(state) ? state : "commented";
}

export function feedIcon(item: FeedEntry): string {
  if (item.item === "thread") return item.isResolved ? "circle-check" : "message-circle";
  switch (item.kind) {
    case "commit":
      return "git-commit-horizontal";
    case "comment":
      return "message-square";
    case "review": {
      const verdict = reviewVerdict(item);
      if (verdict === "approved") return "check";
      if (verdict === "changes_requested") return "pencil-line";
      return "message-square";
    }
    case "review_requested":
      return "user-plus";
    case "ready_for_review":
      return "send";
    case "force_push":
      return "rotate-ccw";
    case "merged":
      return "git-merge";
    case "closed":
      return "circle-x";
    case "reopened":
      return "git-pull-request";
    default:
      return "dot";
  }
}

export function feedTone(item: FeedEntry): string {
  if (item.item === "thread") return item.isResolved ? "absent" : "waiting";
  if (item.kind === "merged") return "good";
  if (item.kind === "closed") return "bad";
  if (item.kind !== "review") return "absent";
  const verdict = reviewVerdict(item);
  if (verdict === "approved") return "good";
  if (verdict === "changes_requested") return "bad";
  return "absent";
}

export function feedTitleKey(item: FeedEntry): string {
  if (item.item === "thread") return "feed.thread";
  if (item.kind === "review") return `feed.review.${reviewVerdict(item)}`;
  if (item.kind === "review_requested" && item.reference === null) {
    return "feed.review_requested.anyone";
  }
  return `feed.${item.kind}`;
}

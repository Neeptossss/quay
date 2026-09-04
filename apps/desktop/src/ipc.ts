import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Scope = "global" | "list" | "pull_request" | "review_panel" | "diff";

export interface KeyBinding {
  chord: string;
  command: string;
  titleKey: string;
  icon: string;
  enabled: boolean;
}

export interface CommandEntry {
  id: string;
  titleKey: string;
  icon: string;
  scope: string;
  bindings: string[];
  enabled: boolean;
}

export interface InboxEntry {
  key: string;
  owner: string;
  name: string;
  number: number;
  title: string;
  author: string;
  isDraft: boolean;
  reviewState: string | null;
  checksState: string | null;
  unresolvedThreads: number;
  updatedAt: string;
}

export interface ViewEntry {
  name: string;
  query: string;
  shortcut: string | null;
  position: number;
}

export interface ThreadEntry {
  nodeId: string;
  path: string;
  line: number | null;
  isResolved: boolean;
  isOutdated: boolean;
  comments: { author: string; body: string; createdAt: string }[];
}

export interface PullRequestEntry extends InboxEntry {
  state: string;
  threads: ThreadEntry[];
}

export const keyMap = (scope: Scope) => invoke<KeyBinding[]>("key_map", { scope });
export const palette = (needle: string, scope: Scope) =>
  invoke<CommandEntry[]>("palette", { needle, scope });
export const savedViews = () => invoke<ViewEntry[]>("saved_views");
export const runView = (name: string) => invoke<InboxEntry[]>("run_view", { name });
export const runQuery = (dsl: string) => invoke<InboxEntry[]>("run_query", { dsl });
export const pullRequest = (key: string) => invoke<PullRequestEntry | null>("pull_request", { key });
export const completions = (partial: string) => invoke<string[]>("completions", { partial });

export interface SyncState {
  phase: string;
  detail: string;
  healthy: boolean;
}

export const syncNow = () => invoke<void>("sync_now");
export const syncState = () => invoke<SyncState>("sync_state");
export const firstPaint = () => invoke<number>("first_paint");
export const mark = (phase: string) => invoke<void>("mark", { phase }).catch(() => {});

export function onInboxChanged(handler: () => void) {
  return listen("inbox_changed", handler);
}

export function onSyncState(handler: (state: SyncState) => void) {
  return listen<SyncState>("sync_state", (event) => handler(event.payload));
}

export function onCapabilitiesChanged(handler: () => void) {
  return listen("capabilities_changed", handler);
}

export interface QueuedMutation {
  id: number;
  kind: string;
  target: string;
  state: string;
  attempts: number;
  lastError: string | null;
}

export const queued = () => invoke<QueuedMutation[]>("queued");
export const approve = (key: string) => invoke<number>("approve", { key });
export const resolveThread = (nodeId: string) => invoke<number>("resolve_thread", { nodeId });
export const requestChanges = (key: string) => invoke<number>("request_changes", { key });
export const merge = (key: string) => invoke<number>("merge", { key });
export const cancel = (id: number) => invoke<void>("cancel", { id });

export function onMutationFailed(handler: (failures: string[]) => void) {
  return listen<string[]>("mutation_failed", (event) => handler(event.payload));
}

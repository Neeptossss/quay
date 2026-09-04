import { invoke } from "@tauri-apps/api/core";

export type Scope = "global" | "list" | "pull_request" | "review_panel" | "diff";

export interface KeyBinding {
  chord: string;
  command: string;
  title: string;
  enabled: boolean;
}

export interface CommandEntry {
  id: string;
  title: string;
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

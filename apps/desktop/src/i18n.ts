import { invoke } from "@tauri-apps/api/core";

let catalogue: Record<string, string> = {};
let active = "fr";

export async function load(): Promise<void> {
  const [entries, locale] = await Promise.all([
    invoke<Record<string, string>>("catalogue"),
    invoke<string>("locale"),
  ]);
  catalogue = entries;
  active = locale;
}

export function locale(): string {
  return active;
}

export function adopt(entries: Record<string, string>, chosen = "fr"): void {
  catalogue = entries;
  active = chosen;
}

export function t(key: string, values: Record<string, string | number> = {}): string {
  const template = catalogue[key] ?? key;
  return template.replace(/\{(\w+)\}/g, (whole, name: string) =>
    name in values ? String(values[name]) : whole,
  );
}

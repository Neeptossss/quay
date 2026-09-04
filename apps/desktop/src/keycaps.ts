const SYMBOLS: Record<string, string> = {
  Enter: "↵",
  Tab: "⇥",
  Backspace: "⌫",
  Space: "␣",
  Esc: "esc",
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
};

export function capsOf(chord: string): string[] {
  return chord.split(/\s+/).filter((token) => token.length > 0);
}

export function displayKey(token: string): string {
  const modified = token.startsWith("⌘");
  const bare = modified ? token.slice(1) : token;
  const shown = SYMBOLS[bare] ?? bare;
  return modified ? `⌘${shown}` : shown;
}

export interface Segment {
  kind: "text" | "key";
  value: string;
}

export function segments(template: string, keys: Record<string, string>): Segment[] {
  const parts: Segment[] = [];
  const pattern = /\{(\w+)\}/g;
  let cursor = 0;
  let found: RegExpExecArray | null;
  while ((found = pattern.exec(template)) !== null) {
    if (found.index > cursor) {
      parts.push({ kind: "text", value: template.slice(cursor, found.index) });
    }
    const name = found[1] as string;
    if (name in keys) {
      parts.push({ kind: "key", value: keys[name] as string });
    } else {
      parts.push({ kind: "text", value: found[0] });
    }
    cursor = found.index + found[0].length;
  }
  if (cursor < template.length) {
    parts.push({ kind: "text", value: template.slice(cursor) });
  }
  return parts;
}

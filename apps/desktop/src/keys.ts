import type { KeyBinding } from "./ipc";

const NAMED: Record<string, string> = {
  Enter: "Enter",
  " ": "Space",
  Escape: "Esc",
  Tab: "Tab",
  Backspace: "Backspace",
  ArrowUp: "ArrowUp",
  ArrowDown: "ArrowDown",
  ArrowLeft: "ArrowLeft",
  ArrowRight: "ArrowRight",
};

export function tokenOf(event: KeyboardEvent): string | null {
  const named = NAMED[event.key];
  const base = named ?? (event.key.length === 1 ? event.key : null);
  if (base === null) return null;
  return event.metaKey ? `⌘${base}` : base;
}

export class ChordReader {
  private pending: string[] = [];

  constructor(private bindings: KeyBinding[]) {}

  replace(bindings: KeyBinding[]) {
    this.bindings = bindings;
    this.pending = [];
  }

  get buffer(): string {
    return this.pending.join(" ");
  }

  reset() {
    this.pending = [];
  }

  read(token: string): KeyBinding | "pending" | null {
    const candidate = [...this.pending, token];
    const chord = candidate.join(" ");
    const exact = this.bindings.find((binding) => binding.chord === chord);
    if (exact) {
      this.pending = [];
      return exact;
    }
    const prefixed = this.bindings.some((binding) => binding.chord.startsWith(`${chord} `));
    if (prefixed) {
      this.pending = candidate;
      return "pending";
    }
    this.pending = [];
    return null;
  }
}

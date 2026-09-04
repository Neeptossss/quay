import { describe, expect, it } from "vitest";
import { ChordReader, tokenOf } from "../src/keys";
import type { KeyBinding } from "../src/ipc";

const bindings: KeyBinding[] = [
  { chord: "j", command: "list.next", title: "Ligne suivante", enabled: true },
  { chord: "J", command: "list.next_and_open", title: "Suivante et ouvrir", enabled: true },
  { chord: "g i", command: "goto.inbox", title: "Inbox", enabled: true },
  { chord: "g g", command: "list.first", title: "Début", enabled: true },
  { chord: "⌘k", command: "palette.open", title: "Palette", enabled: true },
  { chord: "m", command: "pr.merge", title: "Merger", enabled: false },
];

const press = (key: string, meta = false) =>
  tokenOf({ key, metaKey: meta } as KeyboardEvent);

describe("tokenOf", () => {
  it("rend une lettre telle quelle", () => {
    expect(press("j")).toBe("j");
  });

  it("distingue la casse parce que Maj fait partie de la touche", () => {
    expect(press("J")).toBe("J");
    expect(press("j")).not.toBe(press("J"));
  });

  it("nomme les touches qui n'ont pas de caractère", () => {
    expect(press("Enter")).toBe("Enter");
    expect(press(" ")).toBe("Space");
    expect(press("Escape")).toBe("Esc");
  });

  it("porte le modificateur commande sur la touche", () => {
    expect(press("k", true)).toBe("⌘k");
  });

  it("ignore une touche sans caractère ni nom connu", () => {
    expect(press("Shift")).toBeNull();
    expect(press("F5")).toBeNull();
  });
});

describe("ChordReader", () => {
  it("résout un accord d'une seule touche", () => {
    const reader = new ChordReader(bindings);
    expect(reader.read("j")).toMatchObject({ command: "list.next" });
  });

  it("attend la suite d'une séquence sans rien déclencher", () => {
    const reader = new ChordReader(bindings);
    expect(reader.read("g")).toBe("pending");
    expect(reader.buffer).toBe("g");
    expect(reader.read("i")).toMatchObject({ command: "goto.inbox" });
    expect(reader.buffer).toBe("");
  });

  it("abandonne une séquence que rien ne complète", () => {
    const reader = new ChordReader(bindings);
    reader.read("g");
    expect(reader.read("z")).toBeNull();
    expect(reader.buffer).toBe("");
  });

  it("rend la liaison désactivée plutôt que de la cacher", () => {
    const reader = new ChordReader(bindings);
    expect(reader.read("m")).toMatchObject({ command: "pr.merge", enabled: false });
  });

  it("oublie la séquence en cours quand la portée change", () => {
    const reader = new ChordReader(bindings);
    reader.read("g");
    reader.replace(bindings);
    expect(reader.buffer).toBe("");
  });

  it("ne déclenche rien sur une touche inconnue", () => {
    const reader = new ChordReader(bindings);
    expect(reader.read("q")).toBeNull();
  });
});

import { describe, expect, it } from "vitest";
import { capsOf, displayKey, segments } from "../src/keycaps";

describe("capsOf", () => {
  it("rend une touche seule comme une seule pastille", () => {
    expect(capsOf("j")).toEqual(["j"]);
  });

  it("découpe une séquence à la vim en autant de pastilles", () => {
    expect(capsOf("g r")).toEqual(["g", "r"]);
  });

  it("ne rend aucune pastille pour un accord vide", () => {
    expect(capsOf("   ")).toEqual([]);
  });
});

describe("displayKey", () => {
  it("garde la casse, parce que deux casses sont deux commandes", () => {
    expect(displayKey("j")).toBe("j");
    expect(displayKey("J")).toBe("J");
    expect(displayKey("j")).not.toBe(displayKey("J"));
  });

  it("garde le modificateur collé à sa touche sans la dénaturer", () => {
    expect(displayKey("⌘k")).toBe("⌘k");
  });

  it("rend les touches nommées par leur symbole", () => {
    expect(displayKey("Enter")).toBe("↵");
    expect(displayKey("Tab")).toBe("⇥");
    expect(displayKey("Backspace")).toBe("⌫");
    expect(displayKey("Space")).toBe("␣");
  });

  it("laisse échap en toutes lettres, comme le clavier", () => {
    expect(displayKey("Esc")).toBe("esc");
  });

  it("ne dénature pas une touche qu'il ne connaît pas", () => {
    expect(displayKey("F13")).toBe("F13");
  });
});

describe("segments", () => {
  it("rend une phrase sans marqueur en un seul morceau de texte", () => {
    expect(segments("Rien à relire", {})).toEqual([{ kind: "text", value: "Rien à relire" }]);
  });

  it("remplace un marqueur connu par une touche", () => {
    const parts = segments("{palette} ouvre la palette.", { palette: "⌘k" });
    expect(parts[0]).toEqual({ kind: "key", value: "⌘k" });
    expect(parts[1]).toEqual({ kind: "text", value: " ouvre la palette." });
  });

  it("laisse visible un marqueur qu'on ne lui donne pas, plutôt que de le vider", () => {
    expect(segments("{absent} ici", {})).toEqual([
      { kind: "text", value: "{absent}" },
      { kind: "text", value: " ici" },
    ]);
  });

  it("gère plusieurs marqueurs dans l'ordre", () => {
    const parts = segments("{a} puis {b}", { a: "g i", b: "g p" });
    expect(parts.map((part) => part.kind)).toEqual(["key", "text", "key"]);
  });
});

import { describe, expect, it } from "vitest";
import { age, checksIcon, checksTone, reviewIcon, reviewTone, syncIcon } from "../src/state";

const now = Date.parse("2026-09-04T10:00:00Z");

describe("age", () => {
  it("rend l'unité la plus lisible", () => {
    expect(age("2026-09-04T09:59:30Z", now)).toBe("30s");
    expect(age("2026-09-04T09:30:00Z", now)).toBe("30m");
    expect(age("2026-09-04T04:00:00Z", now)).toBe("6h");
    expect(age("2026-09-01T10:00:00Z", now)).toBe("3j");
  });

  it("ne rend jamais un âge négatif", () => {
    expect(age("2026-09-05T10:00:00Z", now)).toBe("0s");
  });

  it("rend une date illisible comme inconnue plutôt que comme maintenant", () => {
    expect(age("pas une date", now)).toBe("?");
  });
});

describe("encodage d'état", () => {
  it("réserve la couleur à l'état de CI et de revue", () => {
    expect(checksTone("success")).toBe("good");
    expect(checksTone("failure")).toBe("bad");
    expect(checksTone("pending")).toBe("waiting");
    expect(checksTone(null)).toBe("absent");
    expect(reviewTone("approved")).toBe("good");
    expect(reviewTone("changes_requested")).toBe("bad");
    expect(reviewTone(null)).toBe("absent");
  });

  it("donne une icône distincte à chaque état connu", () => {
    const icons = [checksIcon("success"), checksIcon("failure"), checksIcon("pending"), checksIcon(null)];
    expect(new Set(icons).size).toBe(icons.length);
  });

  it("ne confond pas un état absent avec un état connu", () => {
    expect(reviewIcon(null)).not.toBe(reviewIcon("approved"));
    expect(syncIcon("offline")).not.toBe(syncIcon("idle"));
  });
});

import { activeFirst } from "../src/state";

const scope = (login: string, openPullRequests: number) => ({ login, openPullRequests });

describe("activeFirst", () => {
  it("ne montre que les organisations qui ont des pull requests ouvertes", () => {
    const split = activeFirst([scope("a", 3), scope("b", 0), scope("c", 1)], false);
    expect(split.shown.map((entry) => entry.login)).toEqual(["a", "c"]);
    expect(split.hidden.map((entry) => entry.login)).toEqual(["b"]);
  });

  it("montre tout quand on déplie", () => {
    const split = activeFirst([scope("a", 3), scope("b", 0)], true);
    expect(split.shown).toHaveLength(2);
    expect(split.hidden).toHaveLength(0);
  });

  it("ne cache rien quand tout est actif", () => {
    const split = activeFirst([scope("a", 1), scope("b", 2)], false);
    expect(split.hidden).toHaveLength(0);
  });

  it("plafonne la liste repliée pour garder la barre latérale dense", () => {
    const many = Array.from({ length: 20 }, (_, index) => scope(`o${index}`, index + 1));
    const split = activeFirst(many, false);
    expect(split.shown).toHaveLength(6);
    expect(split.hidden).toHaveLength(14);
  });

  it("montre les premières organisations quand aucune n'a de pull request", () => {
    const split = activeFirst([scope("a", 0), scope("b", 0)], false);
    expect(split.shown).toHaveLength(2);
    expect(split.hidden).toHaveLength(0);
  });

  it("ne cache ni ne montre rien sur une liste vide", () => {
    const split = activeFirst([], false);
    expect(split.shown).toHaveLength(0);
    expect(split.hidden).toHaveLength(0);
  });
});

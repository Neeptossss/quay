import { describe, expect, it } from "vitest";
import {
  activeFirst,
  age,
  checksIcon,
  checksTone,
  feedIcon,
  feedTitleKey,
  feedTone,
  reviewIcon,
  reviewTone,
  stepThrough,
  syncIcon,
  unresolvedThreads,
} from "../src/state";

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

describe("fil de revue", () => {
  const thread = (nodeId: string, resolved: boolean) => ({
    item: "thread" as const,
    nodeId,
    path: "src/lib.rs",
    line: 12,
    isResolved: resolved,
    isOutdated: false,
    at: "2026-09-04T09:00:00Z",
    comments: [],
  });

  const event = (kind: string, reference: string | null = null) => ({
    item: "event" as const,
    nodeId: `${kind}-1`,
    kind,
    actor: "avery",
    body: null,
    reference,
    at: "2026-09-04T08:00:00Z",
  });

  it("ne retient du flux que les fils non résolus", () => {
    const feed = [event("commit"), thread("A", false), thread("B", true), thread("C", false)];
    expect(unresolvedThreads(feed).map((item) => item.nodeId)).toEqual(["A", "C"]);
  });

  it("part du premier fil quand rien n'est encore visé", () => {
    const threads = [thread("A", false), thread("B", false)];
    expect(stepThrough(threads, null, 1)).toBe("A");
  });

  it("part du dernier fil quand on remonte sans rien viser", () => {
    const threads = [thread("A", false), thread("B", false)];
    expect(stepThrough(threads, null, -1)).toBe("B");
  });

  it("boucle en fin de liste plutôt que de rester bloqué", () => {
    const threads = [thread("A", false), thread("B", false)];
    expect(stepThrough(threads, "B", 1)).toBe("A");
    expect(stepThrough(threads, "A", -1)).toBe("B");
  });

  it("ne vise rien quand il n'y a aucun fil non résolu", () => {
    expect(stepThrough([], null, 1)).toBeNull();
  });

  it("revient au début quand le fil visé a disparu du flux", () => {
    const threads = [thread("A", false)];
    expect(stepThrough(threads, "parti", 1)).toBe("A");
  });

  it("donne à chaque nature d'événement son icône", () => {
    expect(feedIcon(event("commit"))).toBe("git-commit-horizontal");
    expect(feedIcon(event("merged"))).toBe("git-merge");
    expect(feedIcon(event("review", "approved"))).toBe("check");
    expect(feedIcon(event("review", "changes_requested"))).toBe("pencil-line");
    expect(feedIcon(event("review", "commented"))).toBe("message-square");
    expect(feedIcon(thread("A", false))).toBe("message-circle");
    expect(feedIcon(thread("A", true))).toBe("circle-check");
  });

  it("ne colore que ce qui encode un état", () => {
    expect(feedTone(event("merged"))).toBe("good");
    expect(feedTone(event("closed"))).toBe("bad");
    expect(feedTone(event("review", "approved"))).toBe("good");
    expect(feedTone(event("review", "changes_requested"))).toBe("bad");
    expect(feedTone(event("commit"))).toBe("absent");
    expect(feedTone(event("comment"))).toBe("absent");
  });

  it("range une revue d'un état inattendu avec les simples commentaires", () => {
    expect(feedTitleKey(event("review", "SOMETHING_NEW"))).toBe("feed.review.commented");
    expect(feedTitleKey(event("review", null))).toBe("feed.review.commented");
  });

  it("nomme une revue demandée à personne d'identifiable sans laisser de trou", () => {
    expect(feedTitleKey(event("review_requested", null))).toBe("feed.review_requested.anyone");
    expect(feedTitleKey(event("review_requested", "avery"))).toBe("feed.review_requested");
  });

  it("nomme chaque nature d'événement par une clef de langue", () => {
    expect(feedTitleKey(event("commit"))).toBe("feed.commit");
    expect(feedTitleKey(event("review_requested", "avery"))).toBe("feed.review_requested");
    expect(feedTitleKey(event("review", "approved"))).toBe("feed.review.approved");
    expect(feedTitleKey(thread("A", false))).toBe("feed.thread");
  });
});

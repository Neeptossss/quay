import { describe, expect, it } from "vitest";
import { ROW_HEIGHT, scrollToKeep, windowOf } from "../src/virtual";

describe("windowOf", () => {
  it("ne rend rien sur une liste vide", () => {
    expect(windowOf(0, 0, 800)).toEqual({ first: 0, count: 0, above: 0, below: 0 });
  });

  it("ne rend qu'une fraction d'une longue liste", () => {
    const window = windowOf(5000, 0, 800);
    expect(window.count).toBeLessThan(60);
    expect(window.first).toBe(0);
  });

  it("réserve la hauteur des lignes qu'elle ne rend pas", () => {
    const window = windowOf(5000, 0, 800);
    expect(window.above + window.count * ROW_HEIGHT + window.below).toBe(5000 * ROW_HEIGHT);
  });

  it("suit le défilement", () => {
    const window = windowOf(5000, 28 * 100, 800);
    expect(window.first).toBeLessThanOrEqual(100);
    expect(window.first).toBeGreaterThan(80);
    expect(window.above).toBe(window.first * ROW_HEIGHT);
  });

  it("ne dépasse jamais la fin de la liste", () => {
    const window = windowOf(30, 28 * 25, 800);
    expect(window.first + window.count).toBeLessThanOrEqual(30);
    expect(window.below).toBe(0);
  });

  it("rend toute une liste plus courte que la fenêtre", () => {
    const window = windowOf(10, 0, 800);
    expect(window.count).toBe(10);
    expect(window.below).toBe(0);
  });
});

describe("scrollToKeep", () => {
  it("ne bouge pas quand la ligne est déjà visible", () => {
    expect(scrollToKeep(5, 0, 800)).toBe(0);
  });

  it("remonte juste ce qu'il faut quand la ligne est au-dessus", () => {
    expect(scrollToKeep(2, 500, 800)).toBe(2 * ROW_HEIGHT);
  });

  it("descend juste ce qu'il faut quand la ligne est en dessous", () => {
    expect(scrollToKeep(100, 0, 800)).toBe(101 * ROW_HEIGHT - 800);
  });
});

import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { adopt, t } from "../src/i18n";
import french from "../../../crates/quay-app/i18n/fr.json";
import english from "../../../crates/quay-app/i18n/en.json";

describe("catalogue", () => {
  beforeEach(() => {
    adopt(french as Record<string, string>, "fr");
  });

  it("porte exactement les mêmes clés dans chaque langue", () => {
    expect(Object.keys(french).sort()).toEqual(Object.keys(english).sort());
  });

  it("ne laisse aucune traduction vide", () => {
    for (const [locale, catalogue] of [["fr", french], ["en", english]] as const) {
      for (const [key, value] of Object.entries(catalogue)) {
        expect(value.trim(), `${locale} laisse ${key} vide`).not.toBe("");
      }
    }
  });

  it("résout une clé connue", () => {
    expect(t("app.name")).toBe("Quay");
  });

  it("rend une clé inconnue telle quelle, pour qu'une vue nommée par l'utilisateur survive", () => {
    expect(t("Mon truc à moi")).toBe("Mon truc à moi");
  });

  it("interpole les valeurs nommées", () => {
    expect(t("status.entries", { count: 12 })).toContain("12");
  });

  it("laisse en place un paramètre qu'on ne lui donne pas", () => {
    expect(t("status.entries")).toContain("{count}");
  });

  it("change de langue sans perdre la résolution", () => {
    adopt(english as Record<string, string>, "en");
    expect(t("view.to_review")).toBe("To review");
  });
});

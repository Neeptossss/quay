import { describe, expect, it } from "vitest";
import { Confirmation, needsConfirmation } from "../src/confirm";

describe("confirmation en ligne", () => {
  it("laisse passer une commande sans conséquence publique", () => {
    const confirmation = new Confirmation();
    expect(confirmation.accept("list.next")).toBe(true);
    expect(confirmation.pendingCommand).toBeNull();
  });

  it("retient une action publique jusqu'à ce que la touche soit retapée", () => {
    const confirmation = new Confirmation();
    expect(confirmation.accept("pr.merge")).toBe(false);
    expect(confirmation.pendingCommand).toBe("pr.merge");
    expect(confirmation.accept("pr.merge")).toBe(true);
    expect(confirmation.pendingCommand).toBeNull();
  });

  it("abandonne l'attente quand une autre commande arrive", () => {
    const confirmation = new Confirmation();
    confirmation.accept("pr.merge");
    expect(confirmation.accept("list.next")).toBe(true);
    expect(confirmation.pendingCommand).toBeNull();
  });

  it("remplace une attente par une autre plutôt que de les cumuler", () => {
    const confirmation = new Confirmation();
    confirmation.accept("pr.merge");
    expect(confirmation.accept("review.approve")).toBe(false);
    expect(confirmation.pendingCommand).toBe("review.approve");
  });

  it("nomme les actions qui engagent l'utilisateur en public", () => {
    expect(needsConfirmation("pr.merge")).toBe(true);
    expect(needsConfirmation("review.approve")).toBe(true);
    expect(needsConfirmation("list.archive")).toBe(true);
    expect(needsConfirmation("pr.resolve")).toBe(false);
    expect(needsConfirmation("list.next")).toBe(false);
  });

  it("oublie l'attente quand on la vide explicitement", () => {
    const confirmation = new Confirmation();
    confirmation.accept("pr.merge");
    confirmation.clear();
    expect(confirmation.pendingCommand).toBeNull();
  });
});

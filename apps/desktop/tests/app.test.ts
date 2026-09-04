import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

const view = { name: "À relire", query: "is:pr is:open", shortcut: "g r", position: 0 };
const entry = {
  key: "acme/api#1",
  owner: "acme",
  name: "api",
  number: 1,
  title: "Réécrire le transport",
  author: "avery",
  isDraft: false,
  reviewState: "review_required",
  checksState: "success",
  unresolvedThreads: 2,
  updatedAt: new Date().toISOString(),
};

function answer(overrides: Record<string, unknown> = {}) {
  invoke.mockImplementation((command: string) => {
    const table: Record<string, unknown> = {
      saved_views: [view],
      run_view: [entry],
      key_map: [{ chord: "j", command: "list.next", title: "Suivante", enabled: true }],
      palette: [],
      pull_request: null,
      sync_state: { phase: "idle", detail: "au repos", healthy: true },
      ...overrides,
    };
    if (!(command in table)) return Promise.reject(new Error(`commande inconnue ${command}`));
    const value = table[command];
    return value instanceof Error ? Promise.reject(value) : Promise.resolve(value);
  });
}

async function render() {
  const { mount, flushSync } = await import("svelte");
  const App = (await import("../src/App.svelte")).default;
  document.body.innerHTML = '<div id="app"></div>';
  const target = document.getElementById("app");
  if (target === null) throw new Error("point de montage absent");
  mount(App, { target });
  flushSync();
  await new Promise((resolve) => setTimeout(resolve, 0));
  flushSync();
  return document.body;
}

describe("App", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("peint la coque dès le montage", async () => {
    answer();
    const body = await render();
    expect(body.querySelector(".shell")).not.toBeNull();
    expect(body.textContent).toContain("Quay");
  });

  it("affiche les entrées rendues par le cœur", async () => {
    answer();
    const body = await render();
    expect(body.textContent).toContain("Réécrire le transport");
    expect(body.querySelectorAll(".row").length).toBe(1);
  });

  it("peint quand même la coque si le cœur refuse la vue", async () => {
    answer({ run_view: new Error("aucune vue") });
    const body = await render();
    expect(body.querySelector(".shell")).not.toBeNull();
    expect(body.textContent).toContain("aucune vue");
  });

  it("garde le premier échec plutôt que celui d'une lecture accessoire", async () => {
    answer({ run_view: new Error("aucune vue"), sync_state: new Error("état illisible") });
    const body = await render();
    expect(body.textContent).toContain("aucune vue");
    expect(body.textContent).not.toContain("état illisible");
  });

  it("affiche l'état de synchronisation rendu par le cœur", async () => {
    answer();
    const body = await render();
    expect(body.textContent).toContain("au repos");
  });

  it("peint quand même la coque si le cœur ne répond à rien", async () => {
    invoke.mockRejectedValue(new Error("pont IPC absent"));
    const body = await render();
    expect(body.querySelector(".shell")).not.toBeNull();
    expect(body.textContent).toContain("pont IPC absent");
  });

  it("ne rend qu'une fenêtre des lignes quand la liste est longue", async () => {
    const many = Array.from({ length: 4000 }, (_, index) => ({
      ...entry,
      key: `acme/api#${index}`,
      number: index,
    }));
    answer({ run_view: many });
    const body = await render();
    const painted = body.querySelectorAll(".row").length;
    expect(painted).toBeGreaterThan(0);
    expect(painted).toBeLessThan(200);
    expect(body.textContent).toContain("4000 entrée(s)");
  });
});

import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

const view = { name: "view.to_review", query: "is:pr is:open", shortcut: "g r", position: 0 };
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
      key_map: [
        {
          chord: "j",
          command: "list.next",
          titleKey: "command.list.next",
          icon: "arrow-down",
          enabled: true,
        },
        {
          chord: "m",
          command: "pr.merge",
          titleKey: "command.pr.merge",
          icon: "git-merge",
          enabled: true,
        },
      ],
      merge: 1,
      catalogue: { "app.name": "Quay", "view.to_review": "À relire" },
      locale: "fr",
      palette: [],
      pull_request: null,
      sync_state: { phase: "idle", detail: "au repos", healthy: true },
      queued: [],
      view_query: "is:pr is:open",
      organizations: [],
      selected_organization: null,
      ...overrides,
    };
    if (!(command in table)) return Promise.reject(new Error(`commande inconnue ${command}`));
    const value = table[command];
    return value instanceof Error ? Promise.reject(value) : Promise.resolve(value);
  });
}

let live: Record<string, unknown> | null = null;

async function render() {
  const { adopt } = await import("../src/i18n");
  adopt(
    {
      "app.name": "Quay",
      "view.to_review": "À relire",
      "status.entries": "{count} entrée(s)",
      "confirm.retype": "Retaper {key} pour confirmer : {action}",
      "feed.commit": "a poussé",
      "feed.merged": "a fusionné",
      "feed.review.approved": "a approuvé",
      "action.needs_thread": "Aucun fil non résolu ici",
    },
    "fr",
  );
  const { mount, unmount, flushSync } = await import("svelte");
  const App = (await import("../src/App.svelte")).default;
  if (live !== null) {
    unmount(live);
    live = null;
  }
  document.body.innerHTML = '<div id="app"></div>';
  const target = document.getElementById("app");
  if (target === null) throw new Error("point de montage absent");
  live = mount(App, { target }) as Record<string, unknown>;
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

  it("aucune lecture accessoire du démarrage n'écrase l'échec de la vue", async () => {
    for (const accessory of ["sync_state", "queued", "key_map", "organizations"]) {
      answer({ run_view: new Error("aucune vue"), [accessory]: new Error(`bruit ${accessory}`) });
      const body = await render();
      expect(body.textContent, accessory).toContain("aucune vue");
      expect(body.textContent, accessory).not.toContain("bruit");
    }
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

describe("action publique", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  async function press(key: string) {
    const { flushSync } = await import("svelte");
    window.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
    await new Promise((resolve) => setTimeout(resolve, 0));
    flushSync();
  }

  it("ne part pas à la première frappe et demande de retaper", async () => {
    answer();
    const body = await render();
    await press("m");
    expect(invoke).not.toHaveBeenCalledWith("merge", expect.anything());
    expect(body.querySelector(".notice.confirm")).not.toBeNull();
  });

  it("part quand la touche est retapée", async () => {
    answer();
    await render();
    await press("m");
    await press("m");
    expect(invoke).toHaveBeenCalledWith("merge", { key: entry.key });
  });

  it("abandonne la confirmation quand une autre touche arrive", async () => {
    answer();
    const body = await render();
    await press("m");
    await press("j");
    expect(invoke).not.toHaveBeenCalledWith("merge", expect.anything());
    expect(body.querySelector(".notice.confirm")).toBeNull();
  });
});

describe("écran d'une pull request", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  const commit = {
    item: "event",
    nodeId: "PRC_1",
    kind: "commit",
    actor: "avery",
    body: "Déplacer le budget de retry",
    reference: "aaaaaaa",
    at: "2026-09-04T08:00:00Z",
  };

  const openThread = (nodeId: string, at: string) => ({
    item: "thread",
    nodeId,
    path: "src/transport.rs",
    line: 42,
    isResolved: false,
    isOutdated: false,
    at,
    comments: [{ author: "avery", body: "il manque un test", createdAt: at }],
  });

  const merged = {
    item: "event",
    nodeId: "ME_1",
    kind: "merged",
    actor: "avery",
    body: null,
    reference: "cafebab",
    at: "2026-09-04T11:00:00Z",
  };

  function opened(feed: unknown[]) {
    return {
      ...entry,
      state: "open",
      baseRef: "main",
      headSha: "deadbeefcafe",
      feed,
    };
  }

  const navigation = [
    { chord: "o", command: "list.open", titleKey: "command.list.open", icon: "eye", enabled: true },
    {
      chord: "n",
      command: "pr.next_unresolved",
      titleKey: "command.pr.next_unresolved",
      icon: "chevron-down",
      enabled: true,
    },
    {
      chord: "p",
      command: "pr.previous_unresolved",
      titleKey: "command.pr.previous_unresolved",
      icon: "chevron-up",
      enabled: true,
    },
    {
      chord: "R",
      command: "pr.resolve",
      titleKey: "command.pr.resolve",
      icon: "circle-check",
      enabled: true,
    },
  ];

  async function press(key: string) {
    const { flushSync } = await import("svelte");
    window.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
    await new Promise((resolve) => setTimeout(resolve, 0));
    flushSync();
  }

  it("rend le flux dans l'ordre où les choses se sont produites", async () => {
    answer({
      key_map: navigation,
      pull_request: opened([commit, openThread("RT_1", "2026-09-04T09:00:00Z"), merged]),
    });
    const body = await render();
    await press("o");
    const painted = [...body.querySelectorAll(".feed .entry")].map((node) =>
      (node.textContent ?? "").trim(),
    );
    expect(painted.length).toBe(3);
    expect(painted[0]).toContain("a poussé");
    expect(painted[1]).toContain("src/transport.rs:42");
    expect(painted[2]).toContain("a fusionné");
  });

  it("montre la branche de base et le sha court dans l'en-tête", async () => {
    answer({ key_map: navigation, pull_request: opened([commit]) });
    const body = await render();
    await press("o");
    expect(body.textContent).toContain("main ← deadbee");
  });

  it("vise le premier fil non résolu puis boucle", async () => {
    answer({
      key_map: navigation,
      pull_request: opened([
        openThread("RT_1", "2026-09-04T09:00:00Z"),
        openThread("RT_2", "2026-09-04T10:00:00Z"),
      ]),
    });
    const body = await render();
    await press("o");
    await press("n");
    expect(body.querySelector(".entry.thread.focused")?.id).toBe("thread-RT_1");
    await press("n");
    expect(body.querySelector(".entry.thread.focused")?.id).toBe("thread-RT_2");
    await press("n");
    expect(body.querySelector(".entry.thread.focused")?.id).toBe("thread-RT_1");
  });

  it("remonte au dernier fil quand on recule depuis le premier", async () => {
    answer({
      key_map: navigation,
      pull_request: opened([
        openThread("RT_1", "2026-09-04T09:00:00Z"),
        openThread("RT_2", "2026-09-04T10:00:00Z"),
      ]),
    });
    const body = await render();
    await press("o");
    await press("p");
    expect(body.querySelector(".entry.thread.focused")?.id).toBe("thread-RT_2");
  });

  it("résout le fil visé et non le premier venu", async () => {
    answer({
      key_map: navigation,
      pull_request: opened([
        openThread("RT_1", "2026-09-04T09:00:00Z"),
        openThread("RT_2", "2026-09-04T10:00:00Z"),
      ]),
      resolve_thread: 1,
    });
    await render();
    await press("o");
    await press("n");
    await press("n");
    await press("R");
    expect(invoke).toHaveBeenCalledWith("resolve_thread", { nodeId: "RT_2" });
  });

  it("dit qu'il n'y a rien à viser plutôt que de rester muet", async () => {
    answer({ key_map: navigation, pull_request: opened([commit]) });
    const body = await render();
    await press("o");
    await press("n");
    expect(body.textContent).toContain("Aucun fil non résolu ici");
  });
});

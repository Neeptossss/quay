<script lang="ts">
  import Detail from "./Detail.svelte";
  import Palette from "./Palette.svelte";
  import Row from "./Row.svelte";
  import { ChordReader, tokenOf } from "./keys";
  import { measurePaint, observed, percentile } from "./latency";
  import { ROW_HEIGHT, scrollToKeep, windowOf } from "./virtual";
  import { observePaint } from "./paint";
  import * as ipc from "./ipc";
  import type {
    CommandEntry,
    InboxEntry,
    PullRequestEntry,
    Scope,
    SyncState,
    ViewEntry,
  } from "./ipc";

  let views: ViewEntry[] = $state([]);
  let entries: InboxEntry[] = $state([]);
  let opened: PullRequestEntry | null = $state(null);
  let currentView = $state("");
  let cursor = $state(0);
  let failure: string | null = $state(null);

  let paletteOpen = $state(false);
  let paletteEntries: CommandEntry[] = $state([]);
  let paletteNeedle = $state("");
  let paletteCursor = $state(0);

  let pending = $state("");
  let sync: SyncState | null = $state(null);
  let coldStart: number | null = $state(null);
  let list: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewport = $state(800);
  let rendered = $derived(windowOf(entries.length, scrollTop, viewport));
  let scope: Scope = $derived(paletteOpen ? "global" : opened ? "pull_request" : "list");
  let reader = new ChordReader([]);
  let bindings: ipc.KeyBinding[] = $state([]);

  async function loadKeyMap(current: Scope) {
    try {
      const map = await ipc.keyMap(current);
      bindings = map;
      reader.replace(map);
      pending = "";
    } catch (error) {
      failure = String(error);
    }
  }

  $effect(() => {
    void loadKeyMap(scope);
  });

  $effect(() => {
    const attaching = [
      ipc.onInboxChanged(() => {
        if (currentView) void openView(currentView, false);
      }),
      ipc.onSyncState((state) => {
        sync = state;
      }),
      ipc.onCapabilitiesChanged(() => {
        void loadKeyMap(scope);
      }),
    ];
    for (const attachment of attaching) {
      attachment.catch((error) => {
        failure = `la fenêtre ne peut pas écouter le cœur : ${error}`;
      });
    }
    return () => {
      for (const attachment of attaching) {
        attachment.then((stop) => stop()).catch(() => {});
      }
    };
  });

  async function boot() {
    void ipc.mark("boot started");
    try {
      views = await ipc.savedViews();
      void ipc.mark("views loaded");
      const first = views[0];
      if (first) await openView(first.name);
      void ipc.mark("view loaded");
    } catch (error) {
      failure = String(error);
    }
    try {
      sync = await ipc.syncState();
    } catch (error) {
      failure = failure ?? String(error);
    }
    void ipc.mark("boot finished");
    observePaint((name, startTime) => {
      void ipc.mark(`${name} at ${startTime.toFixed(0)} ms into the page`);
      if (name !== "first-contentful-paint") return;
      ipc
        .firstPaint()
        .then((milliseconds) => {
          coldStart = milliseconds;
          return ipc.syncNow();
        })
        .catch(() => {});
    });
  }

  async function openView(name: string, reset = true) {
    try {
      const loaded = await ipc.runView(name);
      entries = loaded;
      currentView = name;
      if (reset) {
        cursor = 0;
        opened = null;
        scrollTop = 0;
        if (list) list.scrollTop = 0;
      } else {
        cursor = Math.min(cursor, Math.max(0, loaded.length - 1));
      }
      failure = null;
    } catch (error) {
      failure = String(error);
    }
  }

  async function openCursor() {
    const entry = entries[cursor];
    if (!entry) return;
    try {
      opened = await ipc.pullRequest(entry.key);
      failure = null;
    } catch (error) {
      failure = String(error);
    }
  }

  async function refreshPalette() {
    paletteEntries = await ipc.palette(paletteNeedle, opened ? "pull_request" : "list");
    paletteCursor = 0;
  }

  function move(delta: number) {
    if (entries.length === 0) return;
    cursor = Math.min(entries.length - 1, Math.max(0, cursor + delta));
    keepCursorVisible();
  }

  function keepCursorVisible() {
    if (!list) return;
    const wanted = scrollToKeep(cursor, list.scrollTop, list.clientHeight);
    if (wanted !== list.scrollTop) list.scrollTop = wanted;
  }

  async function dispatch(command: string) {
    switch (command) {
      case "palette.open":
        paletteOpen = true;
        paletteNeedle = "";
        await refreshPalette();
        break;
      case "list.next":
        move(1);
        break;
      case "list.previous":
        move(-1);
        break;
      case "list.next_and_open":
        move(1);
        await openCursor();
        break;
      case "list.previous_and_open":
        move(-1);
        await openCursor();
        break;
      case "list.first":
        cursor = 0;
        keepCursorVisible();
        break;
      case "list.last":
        cursor = Math.max(0, entries.length - 1);
        keepCursorVisible();
        break;
      case "list.open":
        await openCursor();
        break;
      case "list.clear":
        opened = null;
        break;
      case "goto.inbox":
        if (views[0]) await openView(views[0].name);
        break;
      case "goto.my_pull_requests":
        if (views[1]) await openView(views[1].name);
        break;
      default:
        break;
    }
  }

  async function onKey(event: KeyboardEvent) {
    const started = performance.now();
    const token = tokenOf(event);
    if (token === null) return;

    if (paletteOpen) {
      if (token === "Esc") {
        event.preventDefault();
        paletteOpen = false;
        measurePaint("palette.close", started);
        return;
      }
      if (token === "Enter") {
        event.preventDefault();
        const chosen = paletteEntries[paletteCursor];
        paletteOpen = false;
        if (chosen && chosen.enabled) await dispatch(chosen.id);
        measurePaint("palette.run", started);
        return;
      }
      if (token === "⌘k") {
        event.preventDefault();
        paletteOpen = false;
        measurePaint("palette.close", started);
        return;
      }
      return;
    }

    const outcome = reader.read(token);
    if (outcome === "pending") {
      event.preventDefault();
      pending = reader.buffer;
      measurePaint("chord.pending", started);
      return;
    }
    pending = "";
    if (outcome === null) return;
    event.preventDefault();
    if (!outcome.enabled) {
      failure = `${outcome.title} — indisponible avec ce jeton.`;
      return;
    }
    await dispatch(outcome.command);
    measurePaint(outcome.command, started);
  }

  $effect(() => {
    if (paletteOpen) void paletteNeedle, refreshPalette();
  });

  boot();
</script>

<svelte:window on:keydown={onKey} />

<div class="shell">
  <div class="bar">
    <strong>Quay</strong>
    <div class="views">
      {#each views as view (view.name)}
        <button aria-current={view.name === currentView} onclick={() => openView(view.name)}>
          {view.name}
          {#if view.shortcut}<kbd>{view.shortcut}</kbd>{/if}
        </button>
      {/each}
    </div>
  </div>

  {#if failure}
    <div class="error">{failure}</div>
  {/if}

  {#if opened}
    <Detail entry={opened} />
  {:else if entries.length === 0}
    <div class="empty">
      Rien à relire ici. <kbd>⌘K</kbd> ouvre la palette, <kbd>g i</kbd> revient à l'inbox.
    </div>
  {:else}
    <div
      class="list"
      role="listbox"
      tabindex="-1"
      bind:this={list}
      bind:clientHeight={viewport}
      onscroll={(event) => {
        scrollTop = (event.currentTarget as HTMLDivElement).scrollTop;
      }}
    >
      <div class="spacer" style="height: {rendered.above}px"></div>
      {#each entries.slice(rendered.first, rendered.first + rendered.count) as entry, offset (entry.key)}
        <Row {entry} selected={rendered.first + offset === cursor} />
      {/each}
      <div class="spacer" style="height: {rendered.below}px"></div>
    </div>
  {/if}

  <div class="status">
    <span>{entries.length} entrée(s)</span>
    <span>{currentView}</span>
    {#if pending}<span class="pending">{pending}…</span>{/if}
    <span>
      frappe→pixel p99
      {#if observed() > 0}{percentile(0.99)?.toFixed(1)} ms sur {observed()}{:else}non mesuré{/if}
    </span>
    <span>{bindings.length} raccourci(s) ici</span>
    {#if sync}
      <span class={sync.healthy ? "" : "pending"}>{sync.phase} · {sync.detail}</span>
    {/if}
    <span>{ROW_HEIGHT * entries.length}px virtualisés</span>
    <span>
      démarrage {#if coldStart === null}non mesuré{:else}{coldStart.toFixed(0)} ms{/if}
    </span>
  </div>
</div>

{#if paletteOpen}
  <Palette
    entries={paletteEntries}
    needle={paletteNeedle}
    selected={paletteCursor}
    onNeedle={(value) => {
      paletteNeedle = value;
    }}
  />
{/if}

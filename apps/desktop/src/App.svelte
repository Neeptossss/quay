<script lang="ts">
  import Detail from "./Detail.svelte";
  import Palette from "./Palette.svelte";
  import Row from "./Row.svelte";
  import { ChordReader, tokenOf } from "./keys";
  import { measurePaint, observed, percentile } from "./latency";
  import * as ipc from "./ipc";
  import type { CommandEntry, InboxEntry, PullRequestEntry, Scope, ViewEntry } from "./ipc";

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
  let scope: Scope = $derived(paletteOpen ? "global" : opened ? "pull_request" : "list");
  let reader = new ChordReader([]);
  let bindings: ipc.KeyBinding[] = $state([]);

  $effect(() => {
    ipc.keyMap(scope).then((map) => {
      bindings = map;
      reader.replace(map);
      pending = "";
    });
  });

  async function boot() {
    try {
      views = await ipc.savedViews();
      const first = views[0];
      if (first) await openView(first.name);
    } catch (error) {
      failure = String(error);
    }
  }

  async function openView(name: string) {
    try {
      entries = await ipc.runView(name);
      currentView = name;
      cursor = 0;
      opened = null;
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
        break;
      case "list.last":
        cursor = Math.max(0, entries.length - 1);
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
    <div class="list" role="listbox" tabindex="-1">
      {#each entries as entry, index (entry.key)}
        <Row {entry} selected={index === cursor} />
      {/each}
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

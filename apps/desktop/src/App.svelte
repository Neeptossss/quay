<script lang="ts">
  import Detail from "./Detail.svelte";
  import Icon from "./Icon.svelte";
  import Kbd from "./Kbd.svelte";
  import Palette from "./Palette.svelte";
  import Row from "./Row.svelte";
  import Sidebar from "./Sidebar.svelte";
  import TopBar from "./TopBar.svelte";
  import { ChordReader, tokenOf } from "./keys";
  import { measurePaint, observed, percentile } from "./latency";
  import { observePaint } from "./paint";
  import { scrollToKeep, windowOf } from "./virtual";
  import { stepThrough, unresolvedThreads } from "./state";
  import { t } from "./i18n";
  import { segments } from "./keycaps";
  import { Confirmation } from "./confirm";
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
  let focusedThread: string | null = $state(null);
  let currentView = $state("");
  let cursor = $state(0);
  let failure: string | null = $state(null);

  let paletteOpen = $state(false);
  let paletteEntries: CommandEntry[] = $state([]);
  let paletteNeedle = $state("");
  let paletteCursor = $state(0);

  let pending = $state("");
  let awaiting: string | null = $state(null);
  let notice: string | null = $state(null);
  let queue: ipc.QueuedMutation[] = $state([]);
  let organizations: ipc.ScopeEntry[] = $state([]);
  let organization: string | null = $state(null);
  const confirmation = new Confirmation();
  let sync: SyncState | null = $state(null);
  let coldStart: number | null = $state(null);
  let keystroke: number | null = $state(null);
  let list: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let viewport = $state(800);

  let scope: Scope = $derived(paletteOpen ? "global" : opened ? "pull_request" : "list");
  let rendered = $derived(windowOf(entries.length, scrollTop, viewport));
  let heading = $derived(t(currentView));
  let currentQuery = $state("");
  let reader = new ChordReader([]);
  let bindings: ipc.KeyBinding[] = $state([]);
  let awaitingChord = $derived(
    bindings.find((binding) => binding.command === awaiting)?.chord ?? "",
  );

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
        void refreshOrganizations();
      }),
      ipc.onSyncState((state) => {
        sync = state;
      }),
      ipc.onCapabilitiesChanged(() => {
        void loadKeyMap(scope);
      }),
      ipc.onMutationFailed((failures) => {
        failure = failures[0] ?? null;
        void refreshQueue();
      }),
    ];
    for (const attachment of attaching) {
      attachment.catch((error) => {
        failure = t("error.listen", { reason: String(error) });
      });
    }
    return () => {
      for (const attachment of attaching) {
        attachment.then((stop) => stop()).catch(() => {});
      }
    };
  });

  async function boot() {
    try {
      views = await ipc.savedViews();
      const first = views[0];
      if (first) await openView(first.name);
    } catch (error) {
      failure = String(error);
    }
    await refreshQueue();
    await refreshOrganizations();
    try {
      sync = await ipc.syncState();
    } catch (error) {
      failure ??= String(error);
    }
    observePaint((name) => {
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
      const [loaded, query] = await Promise.all([ipc.runView(name), ipc.viewQuery(name)]);
      currentQuery = query;
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
      focusedThread = null;
      failure = null;
    } catch (error) {
      failure = String(error);
    }
  }

  async function refreshOrganizations() {
    try {
      const [listed, chosen] = await Promise.all([
        ipc.organizations(),
        ipc.selectedOrganization(),
      ]);
      organizations = listed;
      organization = chosen;
    } catch (error) {
      failure ??= String(error);
    }
  }

  async function chooseOrganization(login: string | null) {
    try {
      await ipc.selectOrganization(login);
      organization = login;
      if (currentView) await openView(currentView);
      await refreshOrganizations();
    } catch (error) {
      failure = String(error);
    }
  }

  async function refreshQueue() {
    try {
      queue = await ipc.queued();
    } catch (error) {
      failure ??= String(error);
    }
  }

  async function act(command: string, run: () => Promise<number>) {
    try {
      await run();
      notice = t("action.queued", { action: t(`command.${command}`) });
      failure = null;
      if (currentView) await openView(currentView, false);
      if (opened) opened = await ipc.pullRequest(opened.key);
      await refreshQueue();
    } catch (error) {
      notice = null;
      failure = t("error.mutation", { reason: String(error) });
    }
  }

  function openedKey(): string | null {
    return opened?.key ?? entries[cursor]?.key ?? null;
  }

  async function refreshPalette() {
    paletteEntries = await ipc.palette(paletteNeedle, opened ? "pull_request" : "list");
    paletteCursor = 0;
  }

  function keepCursorVisible() {
    if (!list) return;
    const wanted = scrollToKeep(cursor, list.scrollTop, list.clientHeight);
    if (wanted !== list.scrollTop) list.scrollTop = wanted;
  }

  function focusedUnresolved() {
    const threads = unresolvedThreads(opened?.feed ?? []);
    return threads.find((thread) => thread.nodeId === focusedThread) ?? threads[0] ?? null;
  }

  function moveThread(delta: number) {
    const threads = unresolvedThreads(opened?.feed ?? []);
    if (threads.length === 0) {
      failure = t("action.needs_thread");
      return;
    }
    focusedThread = stepThrough(threads, focusedThread, delta);
    const element = focusedThread === null ? null : document.getElementById(`thread-${focusedThread}`);
    if (element && typeof element.scrollIntoView === "function") {
      element.scrollIntoView({ block: "center" });
    }
  }

  function move(delta: number) {
    if (entries.length === 0) return;
    cursor = Math.min(entries.length - 1, Math.max(0, cursor + delta));
    keepCursorVisible();
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
        focusedThread = null;
        break;
      case "pr.next_unresolved":
        moveThread(1);
        break;
      case "pr.previous_unresolved":
        moveThread(-1);
        break;
      case "goto.inbox":
        if (views[0]) await openView(views[0].name);
        break;
      case "goto.my_pull_requests":
        if (views[1]) await openView(views[1].name);
        break;
      case "review.approve": {
        const key = openedKey();
        if (key === null) {
          failure = t("action.needs_open");
          break;
        }
        await act(command, () => ipc.approve(key));
        break;
      }
      case "review.request_changes": {
        const key = openedKey();
        if (key === null) {
          failure = t("action.needs_open");
          break;
        }
        await act(command, () => ipc.requestChanges(key));
        break;
      }
      case "pr.merge": {
        const key = openedKey();
        if (key === null) {
          failure = t("action.needs_open");
          break;
        }
        await act(command, () => ipc.merge(key));
        break;
      }
      case "pr.resolve": {
        const thread = focusedUnresolved();
        if (!thread) {
          failure = t("action.needs_thread");
          break;
        }
        await act(command, () => ipc.resolveThread(thread.nodeId));
        break;
      }
      default:
        break;
    }
  }

  async function onKey(event: KeyboardEvent) {
    const started = performance.now();
    const token = tokenOf(event);
    if (token === null) return;

    if (paletteOpen) {
      if (token === "Esc" || token === "⌘k") {
        event.preventDefault();
        paletteOpen = false;
      } else if (token === "Enter") {
        event.preventDefault();
        const chosen = paletteEntries[paletteCursor];
        paletteOpen = false;
        if (chosen && chosen.enabled) await dispatch(chosen.id);
      } else if (token === "ArrowDown" || token === "ArrowUp") {
        event.preventDefault();
        const step = token === "ArrowDown" ? 1 : -1;
        const total = paletteEntries.length;
        if (total > 0) {
          paletteCursor = Math.min(total - 1, Math.max(0, paletteCursor + step));
        }
      } else {
        return;
      }
      measurePaint("palette", started);
      keystroke = percentile(0.99);
      return;
    }

    const outcome = reader.read(token);
    if (outcome === "pending") {
      event.preventDefault();
      pending = reader.buffer;
      measurePaint("chord.pending", started);
      keystroke = percentile(0.99);
      return;
    }
    pending = "";
    if (outcome === null) {
      confirmation.clear();
      awaiting = null;
      return;
    }
    event.preventDefault();
    if (!outcome.enabled) {
      failure = t(outcome.titleKey);
      return;
    }
    if (!confirmation.accept(outcome.command)) {
      awaiting = confirmation.pendingCommand;
      measurePaint("confirm", started);
      keystroke = percentile(0.99);
      return;
    }
    awaiting = null;
    await dispatch(outcome.command);
    measurePaint(outcome.command, started);
    if (observed() > 0) keystroke = percentile(0.99);
  }

  $effect(() => {
    if (paletteOpen) void paletteNeedle, refreshPalette();
  });

  boot();
</script>

<svelte:window on:keydown={onKey} />

<div class="shell">
  <Sidebar
    {views}
    current={currentView}
    {sync}
    shortcuts={bindings.length}
    {queue}
    {coldStart}
    {keystroke}
    {organizations}
    {organization}
    onOpen={(name) => openView(name)}
    onOrganization={(login) => chooseOrganization(login)}
  />

  <section class="view">
    <TopBar {heading} query={currentQuery} count={entries.length} {pending} />

    {#if awaiting}
      <div class="notice confirm">
        <Icon name="circle-alert" size={13} />
        {#each segments(t("confirm.retype"), { key: awaitingChord }) as part, index (index)}
          {#if part.kind === "key"}<Kbd chord={part.value} tone="strong" />{:else}{part.value.replace(
              "{action}",
              t(`command.${awaiting}`),
            )}{/if}
        {/each}
      </div>
    {:else if notice}
      <div class="notice"><Icon name="circle-check" size={13} />{notice}</div>
    {/if}

    {#if failure}
      <div class="banner"><Icon name="circle-alert" size={13} />{failure}</div>
    {/if}

    {#if opened}
      <Detail entry={opened} focused={focusedThread} />
    {:else if entries.length === 0}
      <div class="empty">
        <h2>{t("empty.title")}</h2>
        <p>
          {#each segments(t("empty.hint"), { palette: "⌘k", inbox: "g i" }) as part, index (index)}
            {#if part.kind === "key"}<Kbd chord={part.value} tone="strong" />{:else}{part.value}{/if}
          {/each}
        </p>
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
  </section>
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

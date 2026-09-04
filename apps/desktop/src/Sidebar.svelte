<script lang="ts">
  import Icon from "./Icon.svelte";
  import Kbd from "./Kbd.svelte";
  import type { QueuedMutation, ScopeEntry, SyncState, ViewEntry } from "./ipc";
  import { t } from "./i18n";
  import { syncIcon } from "./state";

  let {
    views,
    current,
    sync,
    shortcuts,
    coldStart,
    keystroke,
    queue,
    organizations,
    organization,
    onOpen,
    onOrganization,
  }: {
    views: ViewEntry[];
    current: string;
    sync: SyncState | null;
    shortcuts: number;
    coldStart: number | null;
    keystroke: number | null;
    queue: QueuedMutation[];
    organizations: ScopeEntry[];
    organization: string | null;
    onOpen: (name: string) => void;
    onOrganization: (login: string | null) => void;
  } = $props();
</script>

<aside class="sidebar">
  <header>
    <Icon name="inbox" size={15} />
    {t("app.name")}
  </header>

  <nav>
    {#if organizations.length > 0}
      <div class="group">{t("nav.organizations")}</div>
      <button class="item" aria-current={organization === null} onclick={() => onOrganization(null)}>
        <Icon name="layout-list" size={13} />
        <span class="truncate">{t("org.all")}</span>
        <span class="tally">{organizations.reduce((total, entry) => total + entry.openPullRequests, 0)}</span>
      </button>
      {#each organizations as entry (entry.login)}
        <button class="item" aria-current={organization === entry.login} onclick={() => onOrganization(entry.login)}>
          <Icon name={entry.isMember ? "user" : "git-branch"} size={13} />
          <span class="truncate">{entry.displayName ?? entry.login}</span>
          <span class="tally">{entry.openPullRequests || ""}</span>
        </button>
      {/each}
    {/if}

    <div class="group">{t("nav.views")}</div>
    {#each views as view (view.name)}
      <button
        class="item"
        aria-current={view.name === current}
        onclick={() => onOpen(view.name)}
      >
        <Icon name="layout-list" size={13} />
        <span class="truncate">{t(view.name)}</span>
        {#if view.shortcut}<Kbd chord={view.shortcut} />{:else}<span></span>{/if}
      </button>
    {/each}
  </nav>

  <footer>
    {#if sync}
      <div class="line">
        <span class={sync.healthy ? "" : "tone-bad"}>
          <Icon name={syncIcon(sync.phase)} size={11} />
          {t(`sync.${sync.phase}`)}
        </span>
        <span class="truncate">{sync.detail}</span>
      </div>
    {/if}
    <div class="line">
      <span><Icon name="clock" size={11} />{t("status.startup")}</span>
      <span>{coldStart === null ? t("status.unmeasured") : `${coldStart.toFixed(0)} ms`}</span>
    </div>
    <div class="line">
      <span><Icon name="keyboard" size={11} />{t("status.keystroke")}</span>
      <span>{keystroke === null ? t("status.unmeasured") : `${keystroke.toFixed(1)} ms`}</span>
    </div>
    <div class="line">
      <span><Icon name="keyboard" size={11} />{t("status.shortcuts", { count: shortcuts })}</span>
      <span></span>
    </div>
    {#if queue.length > 0}
      <div class="line">
        <span class={queue.some((entry) => entry.state === "failed") ? "tone-bad" : "tone-waiting"}>
          <Icon name="git-merge" size={11} />
          {t("queue.pending", { count: queue.filter((entry) => entry.state !== "failed").length })}
        </span>
        <span>{t("queue.failed", { count: queue.filter((entry) => entry.state === "failed").length })}</span>
      </div>
    {/if}
  </footer>
</aside>

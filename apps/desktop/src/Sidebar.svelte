<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { SyncState, ViewEntry } from "./ipc";
  import { t } from "./i18n";
  import { syncIcon } from "./state";

  let {
    views,
    current,
    sync,
    shortcuts,
    coldStart,
    keystroke,
    onOpen,
  }: {
    views: ViewEntry[];
    current: string;
    sync: SyncState | null;
    shortcuts: number;
    coldStart: number | null;
    keystroke: number | null;
    onOpen: (name: string) => void;
  } = $props();
</script>

<aside class="sidebar">
  <header>
    <Icon name="inbox" size={15} />
    {t("app.name")}
  </header>

  <nav>
    <div class="group">{t("nav.views")}</div>
    {#each views as view (view.name)}
      <button
        class="item"
        aria-current={view.name === current}
        onclick={() => onOpen(view.name)}
      >
        <Icon name="layout-list" size={13} />
        <span class="truncate">{t(view.name)}</span>
        {#if view.shortcut}<kbd>{view.shortcut}</kbd>{/if}
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
  </footer>
</aside>

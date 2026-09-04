<script lang="ts">
  import Icon from "./Icon.svelte";
  import { t } from "./i18n";

  let {
    heading,
    query,
    count,
    pending,
  }: { heading: string; query: string; count: number; pending: string } = $props();
</script>

<div class="topbar">
  <span class="heading">
    <Icon name="layout-list" size={14} />
    <span class="truncate">{heading}</span>
  </span>
  <span class="count">{t("status.entries", { count })}</span>
  <span class="spacer"></span>
  {#if pending}
    <span class="pill"><Icon name="keyboard" size={12} />{pending}…</span>
  {/if}
  <span class="query truncate">{query}</span>
</div>

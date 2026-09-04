<script lang="ts">
  import Icon from "./Icon.svelte";
  import Kbd from "./Kbd.svelte";
  import type { CommandEntry } from "./ipc";
  import { t } from "./i18n";

  let {
    entries,
    needle,
    selected,
    onNeedle,
  }: {
    entries: CommandEntry[];
    needle: string;
    selected: number;
    onNeedle: (value: string) => void;
  } = $props();

  let field: HTMLInputElement | undefined = $state();
  $effect(() => {
    field?.focus();
  });
</script>

<div class="overlay">
  <div class="palette">
    <div class="field">
      <Icon name="command" size={14} />
      <input
        bind:this={field}
        value={needle}
        placeholder={t("palette.placeholder")}
        oninput={(event) => onNeedle((event.currentTarget as HTMLInputElement).value)}
      />
    </div>
    <ul role="listbox">
      {#each entries as entry, index (entry.id)}
        <li role="option" aria-selected={index === selected} class={entry.enabled ? "" : "disabled"}>
          <Icon name={entry.icon} size={13} />
          <span class="truncate">{t(entry.titleKey)}</span>
          {#if entry.bindings[0]}<Kbd chord={entry.bindings[0]} />{:else}<span></span>{/if}
        </li>
      {/each}
      {#if entries.length === 0}
        <li><Icon name="search" size={13} /><span>{t("palette.empty")}</span><span></span></li>
      {/if}
    </ul>
  </div>
</div>

<script lang="ts">
  import type { CommandEntry } from "./ipc";

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
  <div class="panel">
    <input
      bind:this={field}
      value={needle}
      placeholder="Commande…"
      oninput={(event) => onNeedle((event.currentTarget as HTMLInputElement).value)}
    />
    <ul role="listbox">
      {#each entries as entry, index (entry.id)}
        <li
          role="option"
          aria-selected={index === selected}
          class={entry.enabled ? "" : "disabled"}
        >
          <span>{entry.title}</span>
          <kbd>{entry.bindings.join(" / ")}</kbd>
        </li>
      {/each}
      {#if entries.length === 0}
        <li><span>Aucune commande ici.</span></li>
      {/if}
    </ul>
  </div>
</div>

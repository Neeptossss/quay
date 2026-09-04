<script lang="ts">
  import { capsOf, displayKey } from "./keycaps";

  let {
    chord,
    tone = "quiet",
  }: { chord: string; tone?: "quiet" | "strong" } = $props();

  const caps = $derived(capsOf(chord));
</script>

<span class="kbd tone-{tone}" aria-label={chord}>
  {#each caps as cap, index (cap + index)}
    <kbd>{displayKey(cap)}</kbd>
  {/each}
</span>

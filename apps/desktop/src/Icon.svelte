<script lang="ts">
  import { icons } from "./icons";
  import type { Component } from "svelte";

  let {
    name,
    size = 14,
    label,
  }: { name: string; size?: number; label?: string } = $props();

  const Glyph = $derived((icons[name] ?? icons.circle) as Component<Record<string, unknown>>);
</script>

<span
  class="icon"
  style="--icon-size: {size}px"
  aria-hidden={label === undefined}
  aria-label={label}
>
  <Glyph size={size} strokeWidth={1.75} />
</span>

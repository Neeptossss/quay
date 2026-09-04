<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { InboxEntry } from "./ipc";
  import { age, checksIcon, checksTone, reviewIcon, reviewTone } from "./state";
  import { t } from "./i18n";

  let { entry, selected }: { entry: InboxEntry; selected: boolean } = $props();
</script>

<div class="row" role="option" aria-selected={selected}>
  <span class="repository mono muted">
    <Icon name="git-branch" size={12} />
    <span class="truncate">{entry.owner}/{entry.name}</span>
  </span>
  <span class="number mono faint">#{entry.number}{entry.isDraft ? "~" : ""}</span>
  <span class="tone-{checksTone(entry.checksState)}" title={t(`checks.${entry.checksState ?? "none"}`)}>
    <Icon name={checksIcon(entry.checksState)} size={13} />
  </span>
  <span class="tone-{reviewTone(entry.reviewState)}" title={t(`review.${entry.reviewState ?? "none"}`)}>
    <Icon name={reviewIcon(entry.reviewState)} size={13} />
  </span>
  <span class="threads mono {entry.unresolvedThreads > 0 ? 'tone-waiting' : 'faint'}">
    {#if entry.unresolvedThreads > 0}
      <Icon name="message-circle" size={11} />{entry.unresolvedThreads}
    {/if}
  </span>
  <span class="title truncate">{entry.title}</span>
  <span class="truncate muted">{entry.author}</span>
  <span class="updated mono faint">{age(entry.updatedAt)}</span>
</div>

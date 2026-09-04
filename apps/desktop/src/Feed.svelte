<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { FeedEntry } from "./ipc";
  import { age, feedIcon, feedTitleKey, feedTone } from "./state";
  import { t } from "./i18n";

  let { item, focused }: { item: FeedEntry; focused: boolean } = $props();
</script>

{#if item.item === "thread"}
  <div
    class="entry thread {item.isResolved ? 'settled' : 'open'}"
    class:focused
    id="thread-{item.nodeId}"
  >
    <div class="line">
      <span class="gutter tone-{feedTone(item)}"><Icon name={feedIcon(item)} size={13} /></span>
      <span class="mono path">{item.path}{item.line === null ? "" : `:${item.line}`}</span>
      {#if item.isOutdated}<span class="tag">{t("feed.thread.outdated")}</span>{/if}
      {#if item.isResolved}<span class="tag">{t("feed.thread.resolved")}</span>{/if}
      <span class="when">{age(item.at)}</span>
    </div>
    {#each item.comments as comment (comment.createdAt + comment.author)}
      <div class="said">
        <span class="who">{comment.author}</span>
        <span class="body">{comment.body}</span>
      </div>
    {/each}
  </div>
{:else}
  <div class="entry event">
    <div class="line">
      <span class="gutter tone-{feedTone(item)}"><Icon name={feedIcon(item)} size={13} /></span>
      <span class="who">{item.actor}</span>
      <span class="what">{t(feedTitleKey(item), { target: item.reference ?? "" })}</span>
      {#if item.kind === "commit"}
        <span class="body">{item.body ?? ""}</span>
      {/if}
      {#if item.reference && item.kind !== "review_requested"}
        <span class="mono ref">{item.reference}</span>
      {/if}
      <span class="when">{age(item.at)}</span>
    </div>
    {#if item.body && item.kind !== "commit"}
      <div class="said"><span class="body">{item.body}</span></div>
    {/if}
  </div>
{/if}

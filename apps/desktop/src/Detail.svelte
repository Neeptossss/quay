<script lang="ts">
  import Feed from "./Feed.svelte";
  import Icon from "./Icon.svelte";
  import type { PullRequestEntry } from "./ipc";
  import { checksIcon, checksTone, reviewIcon, reviewTone } from "./state";
  import { t } from "./i18n";

  let {
    entry,
    focused = null,
  }: { entry: PullRequestEntry; focused?: string | null } = $props();

  const shortSha = $derived(entry.headSha.slice(0, 7));
</script>

<div class="detail">
  <header>
    <div class="title">
      <span class="tone-{entry.state === 'open' ? 'waiting' : 'absent'}">
        <Icon name={entry.state === "merged" ? "git-merge" : "git-pull-request"} size={14} />
      </span>
      <h1>{entry.title}</h1>
    </div>
    <div class="meta">
      <span class="mono">{entry.owner}/{entry.name}#{entry.number}</span>
      <span><Icon name="user" size={12} /> {entry.author}</span>
      <span class="mono"><Icon name="git-branch" size={12} /> {entry.baseRef} ← {shortSha}</span>
      <span class="tone-{checksTone(entry.checksState)}">
        <Icon name={checksIcon(entry.checksState)} size={12} />
        {t(`checks.${entry.checksState ?? "none"}`)}
      </span>
      <span class="tone-{reviewTone(entry.reviewState)}">
        <Icon name={reviewIcon(entry.reviewState)} size={12} />
        {t(`review.${entry.reviewState ?? "none"}`)}
      </span>
      <span class="muted">{t("detail.unresolved", { count: entry.unresolvedThreads })}</span>
    </div>
  </header>

  {#if entry.feed.length === 0}
    <p class="muted empty-feed">{t("feed.empty")}</p>
  {/if}

  <div class="feed">
    {#each entry.feed as item (item.nodeId)}
      <Feed {item} focused={item.item === "thread" && item.nodeId === focused} />
    {/each}
  </div>
</div>

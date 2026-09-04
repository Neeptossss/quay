<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { PullRequestEntry } from "./ipc";
  import { checksIcon, checksTone, reviewIcon, reviewTone } from "./state";
  import { t } from "./i18n";

  let { entry }: { entry: PullRequestEntry } = $props();
</script>

<div class="detail">
  <h1>{entry.title}</h1>
  <div class="meta">
    <span class="mono"><Icon name="git-branch" size={12} /> {entry.owner}/{entry.name}#{entry.number}</span>
    <span><Icon name="user" size={12} /> {entry.author}</span>
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

  {#if entry.threads.length === 0}
    <p class="muted">{t("detail.no_threads")}</p>
  {/if}

  {#each entry.threads as thread (thread.path + (thread.line ?? 0) + thread.comments.length)}
    <div class="thread {thread.isResolved ? '' : 'unresolved'}">
      <div class="where">
        <Icon name={thread.isResolved ? "circle-check" : "message-circle"} size={12} />
        {thread.path}{thread.line === null ? "" : `:${thread.line}`}
        {#if thread.isOutdated}<span class="faint">· {t("detail.thread.outdated")}</span>{/if}
        {#if thread.isResolved}<span class="faint">· {t("detail.thread.resolved")}</span>{/if}
      </div>
      {#each thread.comments as comment (comment.createdAt + comment.author)}
        <div class="comment"><span class="muted">{comment.author}</span> {comment.body}</div>
      {/each}
    </div>
  {/each}
</div>

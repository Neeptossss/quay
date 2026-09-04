<script lang="ts">
  import type { PullRequestEntry } from "./ipc";
  import { checksLabel, checksTone, reviewLabel, reviewTone } from "./age";

  let { entry }: { entry: PullRequestEntry } = $props();
</script>

<div class="detail">
  <h1>{entry.title}</h1>
  <div class="meta">
    <span class="repo">{entry.owner}/{entry.name}#{entry.number}</span>
    · {entry.state} · {entry.author}
    · <span class="state {checksTone(entry.checksState)}">{checksLabel(entry.checksState)}</span>
    · <span class="state {reviewTone(entry.reviewState)}">{reviewLabel(entry.reviewState)}</span>
    · {entry.unresolvedThreads} thread(s) non résolu(s)
  </div>

  {#if entry.threads.length === 0}
    <div class="empty">Aucun thread de review sur cette pull request.</div>
  {/if}

  {#each entry.threads as thread (thread.path + (thread.line ?? 0) + thread.comments.length)}
    <div class="thread {thread.isResolved ? '' : 'unresolved'}">
      <div class="where">
        {thread.path}{thread.line === null ? "" : `:${thread.line}`}
        {thread.isOutdated ? " · obsolète" : ""}
        {thread.isResolved ? " · résolu" : ""}
      </div>
      {#each thread.comments as comment (comment.createdAt + comment.author)}
        <div class="comment"><span class="who">{comment.author}</span> — {comment.body}</div>
      {/each}
    </div>
  {/each}
</div>

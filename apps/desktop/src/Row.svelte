<script lang="ts">
  import type { InboxEntry } from "./ipc";
  import { age, checksLabel, checksTone, reviewLabel, reviewTone } from "./age";

  let { entry, selected }: { entry: InboxEntry; selected: boolean } = $props();
</script>

<div class="row" role="option" aria-selected={selected}>
  <span class="repo">{entry.owner}/{entry.name}</span>
  <span class="number">#{entry.number}{entry.isDraft ? "~" : ""}</span>
  <span class="state {checksTone(entry.checksState)}">{checksLabel(entry.checksState)}</span>
  <span class="state {reviewTone(entry.reviewState)}">{reviewLabel(entry.reviewState)}</span>
  <span class="threads">{entry.unresolvedThreads || ""}</span>
  <span class="title">{entry.title}</span>
  <span class="author">{entry.author}</span>
  <span class="age">{age(entry.updatedAt)}</span>
</div>

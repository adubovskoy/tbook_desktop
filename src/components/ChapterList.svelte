<script lang="ts">
  import type { ChapterRef } from "../lib/types";

  let {
    chapters,
    currentIndex,
    onSelect,
    onDismiss,
  }: {
    chapters: ChapterRef[];
    currentIndex: number;
    onSelect: (index: number) => void;
    onDismiss: () => void;
  } = $props();
</script>

<div
  class="sheet-backdrop"
  role="button"
  tabindex="-1"
  onclick={(e) => e.target === e.currentTarget && onDismiss()}
  onkeydown={(e) => e.key === "Escape" && onDismiss()}
>
  <div class="sheet">
    <div class="sheet-handle"></div>
    <div class="sheet-title">Chapters</div>
    <div class="chapter-list">
      {#each chapters as ch, i (ch.id)}
        <button class="chapter-row" class:current={i === currentIndex} onclick={() => onSelect(i)}>
          {ch.title || `Chapter ${i + 1}`}
        </button>
      {/each}
    </div>
  </div>
</div>

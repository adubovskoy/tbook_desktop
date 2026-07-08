<script lang="ts">
  import { untrack } from "svelte";
  import type { SearchMatch } from "../lib/search";
  import type { ChapterRef } from "../lib/types";

  let {
    query,
    results,
    running,
    chapters,
    onSearch,
    onSelect,
    onClose,
  }: {
    query: string;
    results: SearchMatch[];
    running: boolean;
    chapters: ChapterRef[];
    onSearch: (q: string) => void;
    onSelect: (match: SearchMatch) => void;
    onClose: () => void;
  } = $props();

  let value = $state(untrack(() => query)); // seed once from the prop
  let timer: ReturnType<typeof setTimeout> | undefined;

  function onInput() {
    clearTimeout(timer);
    timer = setTimeout(() => onSearch(value), 250); // 250ms debounce (matches Android)
  }
</script>

<div class="search-panel">
  <div class="search-bar">
    <input
      class="search-input"
      placeholder="Search book…"
      bind:value
      oninput={onInput}
      onkeydown={(e) => {
        if (e.key === "Enter") {
          clearTimeout(timer);
          onSearch(value);
        } else if (e.key === "Escape") {
          onClose();
        }
      }}
    />
    <button class="icon-btn" title="Close search" onclick={onClose}>✕</button>
  </div>

  <div class="search-status">
    {#if running}
      Searching…
    {:else if value.trim().length > 0}
      {results.length} match{results.length === 1 ? "" : "es"}
    {/if}
  </div>

  <div class="search-results">
    {#each results as m, i (i)}
      <button class="search-result" onclick={() => onSelect(m)}>
        <span class="search-chapter">{chapters[m.chapterIndex]?.title ?? `Chapter ${m.chapterIndex + 1}`}</span>
        <span class="search-snippet">{m.snippet}</span>
      </button>
    {/each}
  </div>
</div>

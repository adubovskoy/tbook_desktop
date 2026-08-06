<script lang="ts">
  import { buildParagraphRender, paragraphHTML } from "../lib/render";
  import type { Note, Sentence } from "../lib/types";

  let {
    note,
    gloss = null,
    onWordTap,
    onDismiss,
  }: {
    note: Note;
    /**
     * Gloss language interleaved into the note body too, so a note read in
     * bilingual mode looks like the page it came from.
     */
    gloss?: string | null;
    /** Tapping a word inside the note body opens the translation sheet. */
    onWordTap: (sentence: Sentence, wordIndex: number) => void;
    onDismiss: () => void;
  } = $props();

  // Note bodies are ordinary tappable sentences, rendered like body prose.
  const paragraphs = $derived(
    note.paragraphs.map((p) => paragraphHTML(buildParagraphRender(p, "body", null, gloss))),
  );

  function onBodyClick(e: MouseEvent) {
    const wordEl = (e.target as HTMLElement).closest<HTMLElement>("[data-w]");
    if (!wordEl) return;
    // A gloss chunk indexes an align chunk, not a word: the interleaved
    // translation of a note is there to read, not to tap.
    if (wordEl.dataset.g) return;
    const paraEl = wordEl.closest<HTMLElement>("[data-np]");
    if (!paraEl) return;
    const sentence = note.paragraphs[+paraEl.dataset.np!]?.[+wordEl.dataset.s!];
    if (sentence) onWordTap(sentence, +wordEl.dataset.w!);
  }
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
    <div class="sheet-note-label">
      {note.kind === "citation" ? "Citation" : "Note"}
      {note.label}
    </div>
    <div class="sheet-note-body" onclick={onBodyClick} role="presentation">
      {#each paragraphs as html, i (i)}
        <p class="note-para" data-np={i}>{@html html}</p>
      {/each}
    </div>
  </div>
</div>

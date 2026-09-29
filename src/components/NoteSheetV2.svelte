<script lang="ts">
  import { paragraphHTML } from "../lib/render";
  import { buildBlockRender, type PreparedBlock } from "../lib/renderV2";
  import type { TranslationV2 } from "../lib/types";

  // The version-2 footnote sheet (v2 spec §4.12, §6.12): the note body is
  // skeleton-shaped paragraphs, rendered and tapped like the page's own.
  let {
    label,
    kind,
    blocks,
    translations,
    interleave = false,
    onWordTap,
    onDismiss,
  }: {
    label: string;
    kind?: string;
    blocks: PreparedBlock[];
    /** The active language's translations of note paragraph `p`, or null. */
    translations: (p: number) => (TranslationV2 | null)[] | null;
    /** Bilingual mode: the note reads like the page it came from. */
    interleave?: boolean;
    /** A word of note paragraph `p`, sentence `k`, word `i` was tapped. */
    onWordTap: (p: number, k: number, i: number) => void;
    onDismiss: () => void;
  } = $props();

  const paragraphs = $derived(
    blocks.map((b, p) => paragraphHTML(buildBlockRender(b, translations(p), interleave))),
  );

  function onBodyClick(e: MouseEvent) {
    const wordEl = (e.target as HTMLElement).closest<HTMLElement>("[data-w]");
    // An interleaved gloss chunk is there to read, not to tap.
    if (!wordEl || wordEl.dataset.g) return;
    const paraEl = wordEl.closest<HTMLElement>("[data-np]");
    if (!paraEl) return;
    onWordTap(+paraEl.dataset.np!, +wordEl.dataset.s!, +wordEl.dataset.w!);
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
      {kind === "citation" ? "Citation" : "Note"}
      {label}
    </div>
    <div class="sheet-note-body" onclick={onBodyClick} role="presentation">
      {#each paragraphs as html, i (i)}
        <p class="note-para" data-np={i}>{@html html}</p>
      {/each}
    </div>
  </div>
</div>

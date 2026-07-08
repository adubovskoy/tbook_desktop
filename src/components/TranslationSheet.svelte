<script lang="ts">
  import { ipaFor } from "../lib/api";
  import { highlightedHTML, translationHighlightRanges } from "../lib/align";
  import { langName } from "../lib/lang";
  import type { Sentence } from "../lib/types";

  let {
    sentence,
    wordIndex,
    glossLang,
    availableLangs,
    sourceLang,
    onGlossLangChange,
    onDismiss,
  }: {
    sentence: Sentence;
    wordIndex: number;
    glossLang: string;
    availableLangs: string[];
    sourceLang: string;
    onGlossLangChange: (lang: string) => void;
    onDismiss: () => void;
  } = $props();

  const word = $derived.by(() => {
    const w = sentence.words?.[wordIndex];
    if (!w || w.length < 2) return "";
    const n = sentence.src.length;
    const a = Math.max(0, Math.min(n, w[0]));
    const b = Math.max(a, Math.min(n, w[1]));
    return sentence.src.slice(a, b);
  });

  const tr = $derived(glossLang ? sentence.tr?.[glossLang] : undefined);
  const translationHTML = $derived(
    tr && tr.text.trim().length > 0
      ? highlightedHTML(tr.text, translationHighlightRanges(tr, wordIndex))
      : null,
  );

  // English IPA lookup (async, via the Rust dictionary).
  let ipa = $state<string | null>(null);
  $effect(() => {
    const w = word;
    ipa = null;
    if (w.trim().length > 0 && sourceLang.toLowerCase().startsWith("en")) {
      ipaFor(w).then((v) => {
        if (w === word) ipa = v;
      });
    }
  });
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
    <div class="sheet-word">{word}</div>
    {#if ipa}
      <div class="sheet-ipa">/{ipa}/</div>
    {/if}

    {#if availableLangs.length > 1}
      <div class="chips">
        {#each availableLangs as code (code)}
          <button
            class="chip"
            class:selected={code === glossLang}
            onclick={() => onGlossLangChange(code)}
          >
            {langName(code)}
          </button>
        {/each}
      </div>
    {/if}

    {#if translationHTML !== null}
      <div class="sheet-translation">{@html translationHTML}</div>
    {:else}
      <div class="sheet-empty">No {langName(glossLang)} translation available.</div>
    {/if}
  </div>
</div>

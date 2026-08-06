<script lang="ts">
  import { ipaFor } from "../lib/api";
  import { highlightedHTML, translationHighlightRanges } from "../lib/align";
  import { langName } from "../lib/lang";
  import { app } from "../lib/app.svelte";
  import { baseLang } from "../lib/dict";
  import * as tts from "../lib/tts";
  import type { Sentence } from "../lib/types";
  import DictionaryArticle from "./DictionaryArticle.svelte";

  let {
    sentence,
    wordIndex,
    glossLang,
    availableLangs,
    sourceLang,
    glossOnPage = false,
    onGlossLangChange,
    onOpenDictionaries,
    onDismiss,
  }: {
    sentence: Sentence;
    wordIndex: number;
    glossLang: string;
    availableLangs: string[];
    sourceLang: string;
    /**
     * Bilingual mode: the sentence translation is already interleaved into the
     * page, so the sheet opens on the dictionary article and stays there — with
     * nothing to toggle back to, neither the book icon nor the back arrow is
     * offered, and the language picker would only pin a language the page isn't
     * showing.
     */
    glossOnPage?: boolean;
    onGlossLangChange: (lang: string) => void;
    onOpenDictionaries: () => void;
    onDismiss: () => void;
  } = $props();

  // Sentence translation vs. dictionary article, toggled in place by the book
  // icon / back arrow. Reset whenever the selection changes, so clicking a new
  // word always reopens on the translation — or, in bilingual mode, on the
  // article. `$effect.pre` so the first paint is already the right side.
  let showDictionary = $state(false);
  $effect.pre(() => {
    void sentence;
    void wordIndex;
    showDictionary = glossOnPage;
  });

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
    const accent = app.settings.accent;
    ipa = null;
    if (w.trim().length > 0 && sourceLang.toLowerCase().startsWith("en")) {
      ipaFor(w, accent).then((v) => {
        if (w === word && accent === app.settings.accent) ipa = v;
      });
    }
  });

  // Read-aloud is offered only when something on this machine can actually
  // pronounce the source language; otherwise the button would be dead weight.
  let canSpeak = $state(false);
  $effect(() => {
    const lang = sourceLang;
    const accent = app.settings.accent;
    canSpeak = false;
    tts.canSpeak(lang, accent).then((ok) => {
      if (lang === sourceLang && accent === app.settings.accent) canSpeak = ok;
    });
  });

  const speakWord = () => void tts.speak(word, sourceLang, app.settings.accent);
  const speakSentence = () => void tts.speak(sentence.src, sourceLang, app.settings.accent);

  // A click reads the word, a hold or right-click the whole sentence — the
  // desktop reading of Android's tap / long-press on the same button.
  let holdTimer: ReturnType<typeof setTimeout> | undefined;
  let held = false;
  function onSpeakerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    held = false;
    clearTimeout(holdTimer);
    holdTimer = setTimeout(() => {
      held = true;
      speakSentence();
    }, 500);
  }
  function onSpeakerUp() {
    clearTimeout(holdTimer);
  }
  function onSpeakerClick() {
    // The hold already spoke the sentence; the release must not cut it off.
    if (held) {
      held = false;
      return;
    }
    speakWord();
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
    <div class="sheet-head">
      {#if showDictionary && !glossOnPage}
        <button
          class="icon-btn"
          title="Back to translation"
          aria-label="Back to translation"
          onclick={() => (showDictionary = false)}>←</button
        >
      {/if}
      <div class="sheet-word">{word}</div>
      {#if canSpeak}
        <button
          class="icon-btn speak"
          title="Read the word aloud (hold or right-click for the whole sentence)"
          aria-label="Read aloud"
          onpointerdown={onSpeakerDown}
          onpointerup={onSpeakerUp}
          onpointerleave={onSpeakerUp}
          onclick={onSpeakerClick}
          oncontextmenu={(e) => {
            e.preventDefault();
            speakSentence();
          }}>🔊</button
        >
      {/if}
      <!-- Dictionary entry point — always offered for a real language pair;
           without an installed dictionary it opens the download hint. -->
      {#if !showDictionary && word.trim().length > 0 && baseLang(glossLang) !== baseLang(sourceLang)}
        <button
          class="icon-btn"
          title="Dictionary"
          aria-label="Dictionary"
          onclick={() => (showDictionary = true)}>📖</button
        >
      {/if}
    </div>
    {#if ipa && !showDictionary}
      <div class="sheet-ipa">/{ipa}/</div>
    {/if}

    {#if showDictionary}
      <DictionaryArticle {word} {sourceLang} {glossLang} {onOpenDictionaries} />
    {:else if !glossOnPage && availableLangs.length > 1}
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

    {#if !showDictionary && !glossOnPage}
      {#if translationHTML !== null}
        <div class="sheet-translation">{@html translationHTML}</div>
      {:else}
        <div class="sheet-empty">No {langName(glossLang)} translation available.</div>
      {/if}
    {/if}
  </div>
</div>

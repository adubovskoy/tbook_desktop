<script lang="ts">
  // The dictionary side of the translation popup: article(s) for the clicked word,
  // or a download hint when no dictionary covers the current language pair.
  // Port of `ui/reader/DictionaryArticle.kt`.
  import { ipaFor } from "../lib/api";
  import { app } from "../lib/app.svelte";
  import { dictInstalledFor, dictLookup, type DictEntry } from "../lib/dict";
  import { langName } from "../lib/lang";

  let {
    word,
    sourceLang,
    glossLang,
    onOpenDictionaries,
  }: {
    word: string;
    sourceLang: string;
    glossLang: string;
    /** Opens Settings ▸ Dictionaries, for the "no dictionary" hint. */
    onOpenDictionaries: () => void;
  } = $props();

  /** One homonym as rendered: the entry plus the transcription to show. */
  interface Article {
    entry: DictEntry;
    ipa: string | null;
  }

  type Lookup =
    | { state: "loading" }
    | { state: "notInstalled" }
    | { state: "noMatch" }
    | { state: "found"; articles: Article[]; attribution: string };

  let lookup = $state<Lookup>({ state: "loading" });

  /**
   * Transcription for a headword, in bracket-wrapped display form.
   *
   * The bundled pronunciation dictionary wins: it is the same source the word
   * popup shows, in one consistent notation, so switching to the article doesn't
   * silently change the transcription of the same word (/ˈtitʃ/ → /ˈtiːt͡ʃ/). The
   * .tdict's own IPA is the fallback — it covers ~60% of entries, in whichever
   * accent Wiktionary listed first, and is all we have for non-English sources.
   *
   * Looked up by lemma, not by the clicked word: the transcription sits next to
   * the headword, so it must describe *it* (clicking "taught" opens articles for
   * teach and taut, each with its own).
   */
  async function headwordIpa(entry: DictEntry): Promise<string | null> {
    if (sourceLang.toLowerCase().startsWith("en")) {
      const bundled = await ipaFor(entry.lemma, app.settings.accent);
      if (bundled) return `/${bundled}/`;
    }
    const own = entry.ipa?.trim();
    if (!own) return null;
    // dictc keeps the transcription's own /…/ or […] brackets.
    return own.startsWith("/") || own.startsWith("[") ? own : `/${own}/`;
  }

  $effect(() => {
    const w = word;
    const source = sourceLang;
    const target = glossLang;
    lookup = { state: "loading" };
    void (async () => {
      const entries = await dictLookup(source, target, w).catch(() => null);
      // A newer click already replaced this lookup.
      if (w !== word || source !== sourceLang || target !== glossLang) return;
      if (entries === null) {
        lookup = { state: "notInstalled" };
        return;
      }
      if (entries.length === 0) {
        lookup = { state: "noMatch" };
        return;
      }
      const articles = await Promise.all(
        entries.map(async (entry) => ({ entry, ipa: await headwordIpa(entry) })),
      );
      const installed = await dictInstalledFor(source, target).catch(() => null);
      const attribution = [installed?.attribution, installed?.license]
        .filter((s) => s && s.trim().length > 0)
        .join(" · ");
      if (w === word && source === sourceLang && target === glossLang) {
        lookup = { state: "found", articles, attribution };
      }
    })();
  });

  const grammar = (entry: DictEntry) =>
    [entry.pos, entry.gender].filter((s) => s && s.trim().length > 0).join(" · ");
</script>

{#if lookup.state === "loading"}
  <!-- Lookups take milliseconds; drawing nothing beats flashing a spinner. -->
{:else if lookup.state === "notInstalled"}
  <div class="sheet-empty">
    No {langName(sourceLang)} → {langName(glossLang)} dictionary installed.
    <div><button class="link" onclick={onOpenDictionaries}>Download dictionary</button></div>
  </div>
{:else if lookup.state === "noMatch"}
  <div class="sheet-empty">No dictionary entry for “{word}”.</div>
{:else}
  <div class="article">
    {#each lookup.articles as article, i (i)}
      {#if i > 0}
        <hr class="article-sep" />
      {/if}
      <div class="article-head">
        <span class="article-lemma">{article.entry.lemma}</span>
        {#if grammar(article.entry)}
          <span class="article-grammar">{grammar(article.entry)}</span>
        {/if}
      </div>
      {#if article.ipa}
        <div class="article-ipa">{article.ipa}</div>
      {/if}
      <ol class="article-senses">
        {#each article.entry.senses as sense, si (si)}
          <li>
            <!-- Translations are the star: bold, first. The source-language
                 definition follows as a quieter second line. In X→en articles the
                 gloss itself is the translation, so it takes the lead. -->
            <span class="sense-lead">
              {sense.translations.length > 0 ? sense.translations.join(", ") : sense.gloss}
            </span>
            {#if sense.translations.length > 0 && sense.gloss.trim().length > 0}
              <div class="sense-gloss">{sense.gloss}</div>
            {/if}
          </li>
        {/each}
      </ol>
    {/each}
    {#if lookup.attribution}
      <div class="article-attribution">{lookup.attribution}</div>
    {/if}
  </div>
{/if}

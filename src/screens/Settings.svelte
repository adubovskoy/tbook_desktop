<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { app } from "../lib/app.svelte";
  import { langName } from "../lib/lang";
  import {
    FONT_MAX,
    FONT_MIN,
    PAD_MAX,
    PAD_MIN,
    PAD_STEP,
    POPUP_FONT_MAX,
    POPUP_FONT_MIN,
    POPUP_FONT_STEP,
    type Accent,
    type ReaderFont,
    type ReadMode,
    type TapMode,
    type TextAlignMode,
    type ThemeMode,
  } from "../lib/settings";

  const s = $derived(app.settings);

  const FONTS: { value: ReaderFont; label: string }[] = [
    { value: "serif", label: "Serif" },
    { value: "sans", label: "Sans" },
    { value: "mono", label: "Mono" },
  ];
  const THEMES: { value: ThemeMode; label: string }[] = [
    { value: "system", label: "System" },
    { value: "light", label: "Light" },
    { value: "dark", label: "Dark" },
  ];
  const MODES: { value: ReadMode; label: string }[] = [
    { value: "scroll", label: "Scroll" },
    { value: "paged", label: "Paged" },
  ];
  const ACCENTS: { value: Accent; label: string }[] = [
    { value: "american", label: "American" },
    { value: "british", label: "British" },
  ];
  const ALIGNS: { value: TextAlignMode; label: string }[] = [
    { value: "left", label: "Left" },
    { value: "justify", label: "Justified" },
  ];
  const TAPS: { value: TapMode; label: string }[] = [
    { value: "translate", label: "Translate" },
    { value: "pageTurn", label: "Turn page" },
  ];

  type PopupKey = "popupWordSizeSp" | "popupIpaSizeSp" | "popupSentenceSizeSp";
  const POPUP_SIZES: { key: PopupKey; label: string }[] = [
    { key: "popupWordSizeSp", label: "Word" },
    { key: "popupIpaSizeSp", label: "Pronunciation" },
    { key: "popupSentenceSizeSp", label: "Sentence" },
  ];

  /**
   * Gloss languages to offer: those the library's books carry, plus whatever is
   * currently picked — so a choice stays visible (and clearable) after its books
   * are deleted.
   */
  const glossLangs = $derived([
    ...new Set([
      ...(s.glossLang ? [s.glossLang] : []),
      ...app.books.flatMap((b) => b.targetLangs),
    ]),
  ]);

  type PadKey = "padLeft" | "padRight" | "padTop" | "padBottom";
  const PADS: { key: PadKey; label: string }[] = [
    { key: "padLeft", label: "Left" },
    { key: "padRight", label: "Right" },
    { key: "padTop", label: "Top" },
    { key: "padBottom", label: "Bottom" },
  ];
</script>

<div class="screen">
  <header class="appbar">
    <button class="icon-btn" title="Back" onclick={() => app.goLibrary()}>←</button>
    <h1 class="appbar-title">Settings</h1>
    <span></span>
  </header>

  <div class="settings">
    <div class="setting-row">
      <span class="setting-label">Show covers</span>
      <input
        type="checkbox"
        checked={s.showCover}
        onchange={(e) => app.update({ showCover: e.currentTarget.checked })}
      />
    </div>

    <div class="setting-row">
      <span class="setting-label">Font</span>
      <div class="segmented">
        {#each FONTS as f (f.value)}
          <button class:active={s.font === f.value} onclick={() => app.update({ font: f.value })}>
            {f.label}
          </button>
        {/each}
      </div>
    </div>

    <div class="setting-row">
      <span class="setting-label">Font size</span>
      <div class="stepper">
        <button
          disabled={s.fontSizeSp <= FONT_MIN}
          onclick={() => app.update({ fontSizeSp: s.fontSizeSp - 1 })}>−</button
        >
        <span class="stepper-value">{s.fontSizeSp}</span>
        <button
          disabled={s.fontSizeSp >= FONT_MAX}
          onclick={() => app.update({ fontSizeSp: s.fontSizeSp + 1 })}>+</button
        >
      </div>
    </div>

    <div class="setting-section">Translation popup</div>
    {#each POPUP_SIZES as p (p.key)}
      <div class="setting-row">
        <span class="setting-label">{p.label} size</span>
        <div class="stepper">
          <button
            disabled={s[p.key] <= POPUP_FONT_MIN}
            onclick={() => app.update({ [p.key]: s[p.key] - POPUP_FONT_STEP })}>−</button
          >
          <span class="stepper-value">{s[p.key]}</span>
          <button
            disabled={s[p.key] >= POPUP_FONT_MAX}
            onclick={() => app.update({ [p.key]: s[p.key] + POPUP_FONT_STEP })}>+</button
          >
        </div>
      </div>
    {/each}

    <div class="setting-row">
      <span class="setting-label">Theme</span>
      <div class="segmented">
        {#each THEMES as t (t.value)}
          <button class:active={s.theme === t.value} onclick={() => app.update({ theme: t.value })}>
            {t.label}
          </button>
        {/each}
      </div>
    </div>

    <!-- Which English is read aloud for a book tagged plain "en". -->
    <div class="setting-row">
      <span class="setting-label">Pronunciation</span>
      <div class="segmented">
        {#each ACCENTS as a (a.value)}
          <button
            class:active={s.accent === a.value}
            onclick={() => app.update({ accent: a.value })}
          >
            {a.label}
          </button>
        {/each}
      </div>
    </div>

    <div class="setting-row">
      <span class="setting-label">Reading mode</span>
      <div class="segmented">
        {#each MODES as m (m.value)}
          <button
            class:active={s.readMode === m.value}
            onclick={() => app.update({ readMode: m.value })}
          >
            {m.label}
          </button>
        {/each}
      </div>
    </div>

    <div class="setting-row">
      <span class="setting-label">Text alignment</span>
      <div class="segmented">
        {#each ALIGNS as a (a.value)}
          <button
            class:active={s.textAlign === a.value}
            onclick={() => app.update({ textAlign: a.value })}
          >
            {a.label}
          </button>
        {/each}
      </div>
    </div>

    <!-- What a click on the page does. Words translate either way; this trades
         the click between translating and turning the page. -->
    <div class="setting-row">
      <span class="setting-label">
        Click action
        <span class="setting-hint">
          {s.tapMode === "translate"
            ? "Click a word to translate it; turn pages with the arrows or ← → keys."
            : "Click the left half of the page to go back and the right half forward; the top strip opens the menu."}
        </span>
      </span>
      <div class="segmented">
        {#each TAPS as t (t.value)}
          <button class:active={s.tapMode === t.value} onclick={() => app.update({ tapMode: t.value })}>
            {t.label}
          </button>
        {/each}
      </div>
    </div>

    <!-- Default translation language. Options come from the books in the library. -->
    <div class="setting-section">Translation language</div>
    <div class="setting-row">
      <span class="setting-label">Default</span>
      <div class="chips">
        <button class="chip" class:selected={s.glossLang === null} onclick={() => app.setGloss(null)}>
          Auto
        </button>
        {#each glossLangs as code (code)}
          <button
            class="chip"
            class:selected={s.glossLang === code}
            onclick={() => app.setGloss(code)}
          >
            {langName(code)}
          </button>
        {/each}
      </div>
    </div>
    <div class="setting-row">
      <span class="setting-label">
        Hide language picker in popup
        <span class="setting-hint">
          In multilingual books the popup won't show the language buttons; the
          language chosen above is used.
        </span>
      </span>
      <input
        type="checkbox"
        checked={s.hideLangPicker}
        onchange={(e) => app.update({ hideLangPicker: e.currentTarget.checked })}
      />
    </div>

    <div class="setting-section">Margins</div>
    {#each PADS as p (p.key)}
      <div class="setting-row">
        <span class="setting-label">{p.label}</span>
        <div class="stepper">
          <button
            disabled={s[p.key] <= PAD_MIN}
            onclick={() => app.update({ [p.key]: s[p.key] - PAD_STEP })}>−</button
          >
          <span class="stepper-value">{s[p.key]}</span>
          <button
            disabled={s[p.key] >= PAD_MAX}
            onclick={() => app.update({ [p.key]: s[p.key] + PAD_STEP })}>+</button
          >
        </div>
      </div>
    {/each}

    <!-- Offline dictionaries for the word popup (own screen: the list, download
         progress, updates and deletion live there). -->
    <div class="setting-section">Dictionaries</div>
    <div class="setting-row">
      <span class="setting-label">
        <button class="link" onclick={() => app.goDictionaries()}>Offline dictionaries</button>
        <span class="setting-hint">Download dictionaries to look up words while reading.</span>
      </span>
    </div>

    <div class="setting-section">About</div>
    <div class="setting-row">
      <span class="setting-label">
        <button class="link" onclick={() => app.goHelp()}>How to use</button>
        <span class="setting-hint">Clicking, read-aloud, bilingual mode, dictionaries.</span>
      </span>
    </div>
    <div class="about">
      <p><strong>TReader</strong> — language-learning ebook reader.</p>
      <p>
        Pronunciation data from
        <button class="link" onclick={() => openUrl("https://github.com/open-dict-data/ipa-dict")}>
          open-dict-data/ipa-dict
        </button>
        (MIT) and Wiktionary via
        <button class="link" onclick={() => openUrl("https://kaikki.org/")}>kaikki.org</button>
        (CC BY-SA 4.0). Offline dictionaries: Wiktionary via kaikki.org (CC BY-SA 4.0).
        Phonetic transcriptions are set in a subset of Charis (SIL Global, OFL 1.1).
      </p>
      <p>
        Turn your own EPUB/FB2 into a .tbook with the
        <button class="link" onclick={() => openUrl("https://convert.tbook.dev/")}>
          web converter
        </button>.
      </p>
    </div>
  </div>
</div>

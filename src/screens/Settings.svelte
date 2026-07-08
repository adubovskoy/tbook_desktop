<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { app } from "../lib/app.svelte";
  import {
    FONT_MAX,
    FONT_MIN,
    PAD_MAX,
    PAD_MIN,
    PAD_STEP,
    type ReaderFont,
    type ReadMode,
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

    <div class="setting-section">About</div>
    <div class="about">
      <p><strong>TReader</strong> — language-learning ebook reader.</p>
      <p>
        Pronunciation data from
        <button class="link" onclick={() => openUrl("https://github.com/open-dict-data/ipa-dict")}>
          open-dict-data/ipa-dict
        </button> (MIT).
      </p>
    </div>
  </div>
</div>

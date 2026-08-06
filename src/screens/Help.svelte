<script lang="ts">
  // "How to use" — the desktop reading of Android's `ui/help/HelpScreen.kt`,
  // with the copy adapted from `res/values/strings.xml` (clicks and a file
  // dialog instead of taps and share intents). English only for now: the phone's
  // help page is localized into nine languages, which needs an i18n layer the
  // desktop app doesn't have yet.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { app } from "../lib/app.svelte";

  interface Section {
    title: string;
    paragraphs: string[];
  }

  // Links are written markdown-style and turned into real links below.
  const SECTIONS: Section[] = [
    {
      title: "Adding books",
      paragraphs: [
        "Click Import… in the Library to pick a .tbook file.",
        "You can also drag a .tbook file onto the Library window and it will be imported.",
      ],
    },
    {
      title: "Translations",
      paragraphs: [
        "Click any word while reading to see its translation, with the matching words highlighted.",
        "If a book has several translation languages, the buttons at the top of the popup switch between them. The first one is your default language — set it in Settings.",
        "The speaker icon reads aloud: a click pronounces the word, a hold (or right-click) the whole sentence.",
        "The 📖 icon opens the offline dictionary article for the word. Dictionaries are downloaded once in Settings ▸ Offline dictionaries and then work without a network.",
        "The / button in the reader's top bar makes the book bilingual: every sentence is followed by its translation, in a paler shade. Clicking a word then highlights its match on the other side instead of opening the translation.",
      ],
    },
    {
      title: "Tip: keep only your own language",
      paragraphs: [
        "A book often carries translations into several languages, and the popup offers all of them. Open Settings, pick your translation language, and turn on “Hide language picker in popup” — the popup will then show your language only, with nothing else in the way.",
      ],
    },
    {
      title: "Reading",
      paragraphs: [
        "Click the page background to show or hide the reader menu: search, table of contents, day/night theme and settings.",
        "← → (or PageUp/PageDown and Space) turn the page in paged mode and scroll a screenful in scroll mode. In Settings you can also make a click on the left or right half of the page turn it.",
        "After a search, n and p step through the matches, and the counter in the top bar shows where you are.",
      ],
    },
    {
      title: "Settings",
      paragraphs: [
        "Choose font, text size, popup text sizes, theme, margins, text alignment, scroll or paged reading, the pronunciation accent, and the default translation language.",
      ],
    },
    {
      title: "Books and the .tbook format",
      paragraphs: [
        "TReader reads the open .tbook format, which packs the original text together with its translations — that is why everything works offline. The format is described at [tbook.dev](https://tbook.dev).",
        "To read your own books, convert an EPUB or FB2 file at [convert.tbook.dev](https://convert.tbook.dev/), then import the .tbook it gives you.",
      ],
    },
  ];

  /** Split `[label](url)` links out of a paragraph, keeping the surrounding text. */
  function parts(text: string): Array<{ text: string; href?: string }> {
    const out: Array<{ text: string; href?: string }> = [];
    const pattern = /\[([^\]]+)\]\((https?:\/\/[^)\s]+)\)/g;
    let last = 0;
    for (const match of text.matchAll(pattern)) {
      if (match.index > last) out.push({ text: text.slice(last, match.index) });
      out.push({ text: match[1], href: match[2] });
      last = match.index + match[0].length;
    }
    if (last < text.length) out.push({ text: text.slice(last) });
    return out;
  }
</script>

<div class="screen">
  <header class="appbar">
    <button class="icon-btn" title="Back" onclick={() => app.goSettings()}>←</button>
    <h1 class="appbar-title">How to use</h1>
    <span></span>
  </header>

  <div class="settings help">
    {#each SECTIONS as section (section.title)}
      <h2 class="help-title">{section.title}</h2>
      {#each section.paragraphs as paragraph (paragraph)}
        <p class="help-para">
          {#each parts(paragraph) as part, i (i)}
            {#if part.href}
              <button class="link" onclick={() => openUrl(part.href!)}>{part.text}</button>
            {:else}{part.text}{/if}
          {/each}
        </p>
      {/each}
    {/each}
  </div>
</div>

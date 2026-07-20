<script lang="ts">
  import { onMount, tick } from "svelte";
  import { app } from "../lib/app.svelte";
  import * as api from "../lib/api";
  import { buildParagraphRender, type ParagraphRender } from "../lib/render";
  import { searchBook, searchParagraphs, type SearchMatch } from "../lib/search";
  import { bookProgress } from "../lib/progress";
  import { FONT_FAMILY } from "../lib/settings";
  import {
    roleAt,
    type Figure,
    type Manifest,
    type Note,
    type Sentence,
    type Table,
  } from "../lib/types";
  import Paragraph from "../components/Paragraph.svelte";
  import TranslationSheet from "../components/TranslationSheet.svelte";
  import NoteSheet from "../components/NoteSheet.svelte";
  import ChapterList from "../components/ChapterList.svelte";
  import SearchPanel from "../components/SearchPanel.svelte";

  const bookId = app.bookId!;

  let manifest = $state<Manifest | null>(null);
  let chapterSizes = $state<number[]>([]);
  let chapterIndex = $state(0);
  let raw = $state<Sentence[][]>([]);
  let renders = $state<ParagraphRender[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  let selection = $state<{
    paragraphIndex: number;
    sentenceIndex: number;
    wordIndex: number;
    /** Set when the word lives in a table cell rather than the paragraph. */
    row?: number;
    col?: number;
  } | null>(null);
  let chromeVisible = $state(false);
  let chapterListOpen = $state(false);

  // Figures, tables, footnotes
  let notesData = $state<Record<string, Note> | null>(null);
  let figuresByPara = $state<Record<number, Figure>>({});
  let tablesByPara = $state<Record<number, Table>>({});
  let imageUrls = $state<Record<string, string>>({});
  let activeNote = $state<Note | null>(null);
  let noteWordSel = $state<{ sentence: Sentence; wordIndex: number } | null>(null);

  // Search
  let searchActive = $state(false);
  let searchQuery = $state("");
  let searchResults = $state<SearchMatch[]>([]);
  let searchRunning = $state(false);
  let bookTextsCache: string[][] | null = null;

  // Layout
  let viewportEl = $state<HTMLDivElement | null>(null);
  let contentEl = $state<HTMLDivElement | null>(null);
  let pageIndex = $state(0);
  let pages = $state(1);
  let progressFraction = $state(0);
  let restoreParagraph = -1; // pending anchor to restore after a render

  const isPaged = $derived(app.settings.readMode === "paged");
  const availableLangs = $derived(manifest?.targetLangs ?? []);
  const glossLang = $derived.by(() => {
    const langs = availableLangs;
    const saved = app.settings.glossLang;
    if (saved && langs.includes(saved)) return saved;
    return langs[0] ?? "";
  });
  const chapters = $derived(manifest?.chapters ?? []);

  const fontVars = $derived(
    `--reader-font:${FONT_FAMILY[app.settings.font]};` +
      `--reader-size:${app.settings.fontSizeSp}px;` +
      `--pad-left:${app.settings.padLeft}px;--pad-right:${app.settings.padRight}px;` +
      `--pad-top:${app.settings.padTop}px;--pad-bottom:${app.settings.padBottom}px;`,
  );

  onMount(() => {
    void loadBook();
    window.addEventListener("keydown", onKeydown);
    // Expose the live position so a flush on app close saves the exact spot,
    // bypassing the debounce. Returns null while loading to avoid clobbering
    // the saved position with a mid-transition value.
    const provide = () =>
      loading
        ? null
        : { bookId, chapterIndex, paragraphIndex: currentTopParagraph(), fraction: progressFraction };
    app.registerPositionProvider(provide);
    return () => {
      window.removeEventListener("keydown", onKeydown);
      app.clearPositionProvider(provide);
    };
  });

  async function loadBook() {
    try {
      const ob = await api.openBook(bookId);
      manifest = ob.manifest;
      chapterSizes = ob.chapterSizes;
      // Footnote bodies load once per book; markers only render for known ids.
      notesData = await api.bookNotes(bookId).catch(() => null);
      const pos = await app.getPosition(bookId);
      const start = Math.max(0, Math.min(pos.chapterIndex, ob.manifest.chapters.length - 1));
      await loadChapter(start, pos.paragraphIndex);
    } catch (e) {
      error = String(e);
      loading = false;
    }
  }

  async function loadChapter(index: number, anchorParagraph = 0) {
    if (!manifest || index < 0 || index >= manifest.chapters.length) return;
    loading = true;
    selection = null;
    activeNote = null;
    noteWordSel = null;
    const ref = manifest.chapters[index];
    try {
      const chapter = await api.readChapter(bookId, ref.file);
      raw = chapter.paragraphs;
      const noteIds = notesData ? new Set(Object.keys(notesData)) : null;
      renders = chapter.paragraphs.map((p, i) =>
        buildParagraphRender(p, roleAt(chapter, i), noteIds),
      );

      // Index figures/tables by their anchor paragraph (skip out-of-range).
      const figs: Record<number, Figure> = {};
      for (const f of chapter.figures ?? []) {
        if (Number.isInteger(f.para) && f.para >= 0 && f.para < chapter.paragraphs.length) {
          figs[f.para] = f;
        }
      }
      figuresByPara = figs;
      const tbls: Record<number, Table> = {};
      for (const t of chapter.tables ?? []) {
        if (
          Number.isInteger(t.para) &&
          t.para >= 0 &&
          t.para < chapter.paragraphs.length &&
          (t.rows?.length ?? 0) > 0
        ) {
          tbls[t.para] = t;
        }
      }
      tablesByPara = tbls;
      for (const f of Object.values(figs)) void ensureImage(f.image);

      chapterIndex = index;
      loading = false;
      restoreParagraph = anchorParagraph;
      await tick();
      applyLayout();
      restoreToParagraph(anchorParagraph);
      savePosition();
    } catch (e) {
      error = String(e);
      loading = false;
    }
  }

  function nextChapter() {
    if (manifest && chapterIndex + 1 < manifest.chapters.length) loadChapter(chapterIndex + 1);
  }
  function prevChapter() {
    if (chapterIndex > 0) loadChapter(chapterIndex - 1);
  }

  /** Fetch a figure image as a data: URL once; cached for the whole book. */
  async function ensureImage(entry: string) {
    if (!entry || imageUrls[entry]) return;
    try {
      imageUrls[entry] = await api.bookImage(bookId, entry);
    } catch {
      // Missing/corrupt image entry: the figure renders caption-only.
    }
  }

  /** Images arrive async and change flow height, so re-layout at the anchor. */
  function onImageLoad() {
    if (loading || !contentEl) return;
    const anchor = currentTopParagraph();
    applyLayout();
    restoreToParagraph(anchor);
  }

  // --- Layout (paged columns vs scroll) ---

  function pageStride(): number {
    if (!contentEl) return 1;
    const cw = contentEl.clientWidth;
    const gap = app.settings.padLeft + app.settings.padRight;
    return cw + gap;
  }

  function applyLayout() {
    if (!contentEl) return;
    if (isPaged) {
      const cw = contentEl.clientWidth;
      const gap = app.settings.padLeft + app.settings.padRight;
      contentEl.style.columnWidth = `${cw}px`;
      contentEl.style.columnGap = `${gap}px`;
      const stride = cw + gap;
      const sw = contentEl.scrollWidth;
      pages = Math.max(1, Math.round((sw + gap) / stride));
      pageIndex = Math.min(pageIndex, pages - 1);
      applyTranslate();
    } else {
      contentEl.style.columnWidth = "";
      contentEl.style.columnGap = "";
      contentEl.style.transform = "";
    }
    updateProgress();
  }

  function applyTranslate() {
    if (contentEl && isPaged) {
      contentEl.style.transform = `translateX(${-pageIndex * pageStride()}px)`;
    }
  }

  function pageOfParagraph(pi: number): number {
    const el = contentEl?.querySelector<HTMLElement>(`[data-p="${pi}"]`);
    if (!el || !contentEl) return 0;
    const offset = el.getBoundingClientRect().left - contentEl.getBoundingClientRect().left;
    return Math.max(0, Math.min(pages - 1, Math.round(offset / pageStride())));
  }

  function restoreToParagraph(pi: number) {
    if (pi <= 0) {
      if (isPaged) {
        pageIndex = 0;
        applyTranslate();
      } else if (viewportEl) {
        viewportEl.scrollTop = 0;
      }
      updateProgress();
      return;
    }
    if (isPaged) {
      pageIndex = pageOfParagraph(pi);
      applyTranslate();
    } else {
      const el = contentEl?.querySelector<HTMLElement>(`[data-p="${pi}"]`);
      el?.scrollIntoView({ block: "start" });
    }
    updateProgress();
  }

  /** First paragraph currently at the top of the viewport (the saved anchor). */
  function currentTopParagraph(): number {
    if (!contentEl) return 0;
    const els = contentEl.querySelectorAll<HTMLElement>("[data-p]");
    if (isPaged) {
      for (const el of els) {
        if (pageOfParagraph(+el.dataset.p!) >= pageIndex) return +el.dataset.p!;
      }
    } else if (viewportEl) {
      const top = viewportEl.scrollTop;
      for (const el of els) {
        if (el.offsetTop + el.offsetHeight > top + 1) return +el.dataset.p!;
      }
    }
    return 0;
  }

  function updateProgress() {
    let frac = 0;
    if (isPaged) {
      frac = pages > 1 ? pageIndex / (pages - 1) : 0;
    } else if (viewportEl) {
      const max = viewportEl.scrollHeight - viewportEl.clientHeight;
      frac = max > 0 ? viewportEl.scrollTop / max : 0;
    }
    progressFraction = bookProgress(chapterSizes, chapterIndex, frac);
  }

  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  function savePosition() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      void app.savePosition(bookId, {
        chapterIndex,
        paragraphIndex: currentTopParagraph(),
        fraction: progressFraction,
      });
    }, 400);
  }

  function onScroll() {
    if (isPaged) return;
    updateProgress();
    savePosition();
  }

  function goToPage(p: number) {
    pageIndex = Math.max(0, Math.min(pages - 1, p));
    applyTranslate();
    updateProgress();
    savePosition();
  }

  function nextPage() {
    if (isPaged) {
      if (pageIndex + 1 < pages) goToPage(pageIndex + 1);
      else nextChapter();
    }
  }
  function prevPage() {
    if (isPaged) {
      if (pageIndex > 0) goToPage(pageIndex - 1);
      else if (chapterIndex > 0) loadChapter(chapterIndex - 1, Number.MAX_SAFE_INTEGER);
    }
  }

  // Re-layout when the reading-format settings change, preserving the anchor.
  $effect(() => {
    // track dependencies:
    void app.settings.readMode;
    void app.settings.fontSizeSp;
    void app.settings.font;
    void app.settings.padLeft;
    void app.settings.padRight;
    void app.settings.padTop;
    void app.settings.padBottom;
    void renders;
    if (loading || !contentEl) return;
    const anchor = currentTopParagraph();
    tick().then(() => {
      applyLayout();
      restoreToParagraph(anchor);
    });
  });

  // --- Interaction ---

  function onContentClick(e: MouseEvent) {
    const target = e.target as HTMLElement;
    const noteEl = target.closest<HTMLElement>("[data-note]");
    if (noteEl) {
      const note = notesData?.[noteEl.dataset.note!];
      if (note) activeNote = note;
      return;
    }
    const wordEl = target.closest<HTMLElement>("[data-w]");
    if (wordEl) {
      const paraEl = wordEl.closest<HTMLElement>("[data-p]");
      if (!paraEl) return;
      const cellEl = wordEl.closest<HTMLElement>("[data-r]");
      selection = {
        paragraphIndex: +paraEl.dataset.p!,
        sentenceIndex: +wordEl.dataset.s!,
        wordIndex: +wordEl.dataset.w!,
        row: cellEl ? +cellEl.dataset.r! : undefined,
        col: cellEl ? +cellEl.dataset.c! : undefined,
      };
      return;
    }
    chromeVisible = !chromeVisible;
  }

  const selectedSentence = $derived.by(() => {
    if (!selection) return null;
    if (selection.row !== undefined && selection.col !== undefined) {
      const table = tablesByPara[selection.paragraphIndex];
      return (
        table?.rows[selection.row]?.[selection.col]?.sentences?.[selection.sentenceIndex] ?? null
      );
    }
    return raw[selection.paragraphIndex]?.[selection.sentenceIndex] ?? null;
  });

  function highlightRangesFor(pi: number): Array<[number, number]> {
    if (searchQuery.trim().length === 0) return [];
    const text = renders[pi]?.text;
    if (!text) return [];
    return searchParagraphs(searchQuery, [text], chapterIndex).map((m) => [m.start, m.end]);
  }

  async function runSearch(q: string) {
    searchQuery = q;
    if (q.trim().length === 0) {
      searchResults = [];
      return;
    }
    searchRunning = true;
    try {
      if (!bookTextsCache) bookTextsCache = await api.bookTexts(bookId);
      searchResults = searchBook(q, bookTextsCache);
    } finally {
      searchRunning = false;
    }
  }

  function gotoMatch(m: SearchMatch) {
    searchActive = false;
    chromeVisible = false;
    if (m.chapterIndex === chapterIndex) {
      restoreToParagraph(m.paragraphIndex);
      savePosition();
    } else {
      loadChapter(m.chapterIndex, m.paragraphIndex);
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (noteWordSel) noteWordSel = null;
      else if (selection) selection = null;
      else if (activeNote) activeNote = null;
      else if (searchActive) searchActive = false;
      else if (chapterListOpen) chapterListOpen = false;
      else if (chromeVisible) chromeVisible = false;
      else app.goLibrary();
      return;
    }
    if (selection || noteWordSel || activeNote || searchActive) return;
    if (isPaged && (e.key === "ArrowRight" || e.key === "PageDown")) {
      e.preventDefault();
      nextPage();
    } else if (isPaged && (e.key === "ArrowLeft" || e.key === "PageUp")) {
      e.preventDefault();
      prevPage();
    } else if (e.key === " " && isPaged) {
      e.preventDefault();
      nextPage();
    }
  }
</script>

<div class="reader" class:paged={isPaged} style={fontVars}>
  {#if error}
    <div class="banner error">{error}</div>
  {/if}

  <div
    class="reader-viewport"
    bind:this={viewportEl}
    onscroll={onScroll}
    onclick={onContentClick}
    role="presentation"
  >
    <div class="reader-content" bind:this={contentEl}>
      {#each renders as r, i (i)}
        <Paragraph
          render={r}
          index={i}
          highlightRanges={highlightRangesFor(i)}
          figure={figuresByPara[i]}
          imageUrl={figuresByPara[i] ? (imageUrls[figuresByPara[i].image] ?? null) : null}
          table={tablesByPara[i]}
          {onImageLoad}
        />
      {/each}
    </div>
  </div>

  {#if loading}
    <div class="reader-loading">Loading…</div>
  {/if}

  {#if isPaged && chromeVisible}
    <button class="page-nav left" onclick={prevPage} aria-label="Previous page">‹</button>
    <button class="page-nav right" onclick={nextPage} aria-label="Next page">›</button>
  {/if}

  <!-- Chrome: top + bottom bars -->
  {#if chromeVisible}
    <header class="reader-top">
      <button class="icon-btn" title="Back to library" onclick={() => app.goLibrary()}>←</button>
      <div class="reader-titles">
        <div class="reader-book">{manifest?.title ?? ""}</div>
        <div class="reader-chapter">{chapters[chapterIndex]?.title ?? ""}</div>
      </div>
      <button class="icon-btn" title="Search" onclick={() => (searchActive = true)}>🔍</button>
      <button class="icon-btn" title="Chapters" onclick={() => (chapterListOpen = true)}>☰</button>
    </header>

    <footer class="reader-bottom">
      <button class="icon-btn" disabled={chapterIndex <= 0} onclick={prevChapter}>‹ Prev</button>
      <div class="reader-progress">
        <div class="progress-track"><div class="progress-fill" style="width:{progressFraction * 100}%"></div></div>
        <span class="progress-pct">{Math.round(progressFraction * 100)}%</span>
      </div>
      <button
        class="icon-btn"
        disabled={!manifest || chapterIndex >= manifest.chapters.length - 1}
        onclick={nextChapter}>Next ›</button
      >
    </footer>
  {/if}

  {#if searchActive}
    <SearchPanel
      query={searchQuery}
      results={searchResults}
      running={searchRunning}
      {chapters}
      onSearch={runSearch}
      onSelect={gotoMatch}
      onClose={() => (searchActive = false)}
    />
  {/if}

  {#if chapterListOpen}
    <ChapterList
      {chapters}
      currentIndex={chapterIndex}
      onSelect={(i) => {
        chapterListOpen = false;
        chromeVisible = false;
        loadChapter(i);
      }}
      onDismiss={() => (chapterListOpen = false)}
    />
  {/if}

  {#if selection && selectedSentence}
    <TranslationSheet
      sentence={selectedSentence}
      wordIndex={selection.wordIndex}
      {glossLang}
      {availableLangs}
      sourceLang={manifest?.sourceLang ?? "en"}
      onGlossLangChange={(lang) => app.setGloss(lang)}
      onDismiss={() => (selection = null)}
    />
  {/if}

  {#if activeNote}
    <NoteSheet
      note={activeNote}
      onWordTap={(sentence, wordIndex) => (noteWordSel = { sentence, wordIndex })}
      onDismiss={() => {
        activeNote = null;
        noteWordSel = null;
      }}
    />
  {/if}

  <!-- Word tapped inside the note body: translation sheet stacks above the note. -->
  {#if noteWordSel}
    <TranslationSheet
      sentence={noteWordSel.sentence}
      wordIndex={noteWordSel.wordIndex}
      {glossLang}
      {availableLangs}
      sourceLang={manifest?.sourceLang ?? "en"}
      onGlossLangChange={(lang) => app.setGloss(lang)}
      onDismiss={() => (noteWordSel = null)}
    />
  {/if}
</div>

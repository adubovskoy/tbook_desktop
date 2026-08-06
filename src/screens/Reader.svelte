<script lang="ts">
  import { onMount, tick } from "svelte";
  import { app } from "../lib/app.svelte";
  import * as api from "../lib/api";
  import { buildParagraphRender, pairRanges, type ParagraphRender } from "../lib/render";
  import { searchBook, searchParagraphs, type SearchMatch } from "../lib/search";
  import { bookProgress } from "../lib/progress";
  import { FONT_FAMILY } from "../lib/settings";
  import {
    roleAt,
    type Chapter,
    type ParagraphRole,
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
  /** The loaded chapter, kept so a bilingual toggle can rebuild its renders. */
  let chapterData = $state<Chapter | null>(null);
  /** The gloss language the current `renders` were actually built with. */
  let renderedGloss: string | null = null;
  /** The book's cover as a data URL, shown as the first page of chapter one. */
  let coverUrl = $state<string | null>(null);
  /**
   * How many synthetic paragraphs `renders`/`raw` start with before the chapter's
   * own content — the cover page (first chapter only) and/or the chapter-title
   * heading. Search results index the chapter's own paragraphs, so jumps add this
   * shift to land on the right render.
   */
  let titleShift = $state(0);

  let selection = $state<{
    paragraphIndex: number;
    sentenceIndex: number;
    wordIndex: number;
    /** Set when the word lives in a table cell rather than the paragraph. */
    row?: number;
    col?: number;
    /**
     * The click landed on bilingual mode's interleaved translation, so
     * `wordIndex` addresses an align chunk rather than a source word.
     */
    gloss?: boolean;
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
  /** Which result the reader is standing on, for the ‹ › match stepper. */
  let activeMatch = $state(-1);
  let bookTextsCache: string[][] | null = null;

  // Layout
  let viewportEl = $state<HTMLDivElement | null>(null);
  let contentEl = $state<HTMLDivElement | null>(null);
  let pageIndex = $state(0);
  let pages = $state(1);
  let progressFraction = $state(0);

  const isPaged = $derived(app.settings.readMode === "paged");
  const availableLangs = $derived(manifest?.targetLangs ?? []);
  const glossLang = $derived.by(() => {
    const langs = availableLangs;
    const saved = app.settings.glossLang;
    if (saved && langs.includes(saved)) return saved;
    return langs[0] ?? "";
  });
  const chapters = $derived(manifest?.chapters ?? []);
  /** The language interleaved into the text right now, or null when it isn't. */
  const gloss = $derived(app.settings.bilingual && glossLang ? glossLang : null);

  const fontVars = $derived(
    `--reader-font:${FONT_FAMILY[app.settings.font]};` +
      `--reader-size:${app.settings.fontSizeSp}px;` +
      `--reader-align:${app.settings.textAlign === "justify" ? "justify" : "left"};` +
      `--pad-left:${app.settings.padLeft}px;--pad-right:${app.settings.padRight}px;` +
      `--pad-top:${app.settings.padTop}px;--pad-bottom:${app.settings.padBottom}px;` +
      `--popup-word:${app.settings.popupWordSizeSp}px;` +
      `--popup-ipa:${app.settings.popupIpaSizeSp}px;` +
      `--popup-sentence:${app.settings.popupSentenceSizeSp}px;`,
  );

  /** Page counter for the status line: 1-based, and 1/1 in scroll mode. */
  const chapterPage = $derived(isPaged ? pageIndex + 1 : 1);
  const chapterPages = $derived(isPaged ? pages : 1);

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
      // The cover opens the book as a page of its own. A book without one (or
      // with an unreadable entry) simply gets no first page.
      if (ob.manifest.cover) coverUrl = await api.coverDataUrl(bookId).catch(() => null);
      // Stamped only once the book really opened, so a broken file doesn't jump
      // to the top of the library.
      void app.markOpened(bookId);
      const pos = await app.getPosition(bookId);
      const start = Math.max(0, Math.min(pos.chapterIndex, ob.manifest.chapters.length - 1));
      await loadChapter(start, pos.paragraphIndex);
    } catch (e) {
      error = String(e);
      loading = false;
    }
  }

  async function loadChapter(index: number, anchorParagraph = 0, chapterSpaceAnchor = false) {
    if (!manifest || index < 0 || index >= manifest.chapters.length) return;
    loading = true;
    selection = null;
    activeNote = null;
    noteWordSel = null;
    const ref = manifest.chapters[index];
    try {
      const chapter = await api.readChapter(bookId, ref.file);
      chapterData = chapter;
      renderedGloss = gloss;
      renders = buildRenders(chapter, gloss, index);
      const shift = titleShift;
      // Synthetic paragraphs carry no sentence data, so `raw` gets matching empty
      // entries to keep the two lists indexed alike.
      raw = [...Array.from({ length: shift }, () => [] as Sentence[]), ...chapter.paragraphs];

      // Index figures/tables by their anchor paragraph, in render space
      // (skip out-of-range).
      const figs: Record<number, Figure> = {};
      for (const f of chapter.figures ?? []) {
        if (Number.isInteger(f.para) && f.para >= 0 && f.para < chapter.paragraphs.length) {
          figs[f.para + shift] = f;
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
          tbls[t.para + shift] = t;
        }
      }
      tablesByPara = tbls;
      for (const f of Object.values(figs)) void ensureImage(f.image);

      chapterIndex = index;
      loading = false;
      const anchor = chapterSpaceAnchor ? anchorParagraph + shift : anchorParagraph;
      await tick();
      applyLayout();
      restoreToParagraph(anchor);
      savePosition();
    } catch (e) {
      error = String(e);
      loading = false;
    }
  }

  /**
   * Build a chapter's paragraph renders. With `g` set, each sentence's
   * translation is interleaved into the text it belongs to (bilingual mode) —
   * the gloss is part of what paginates, so it has to be baked in here rather
   * than overlaid.
   */
  function buildRenders(chapter: Chapter, g: string | null, index: number): ParagraphRender[] {
    const noteIds = notesData ? new Set(Object.keys(notesData)) : null;
    const content = chapter.paragraphs.map((p, i) =>
      buildParagraphRender(p, roleAt(chapter, i), noteIds, g),
    );
    // The cover (a full page of its own, opening the book) and the chapter title
    // as an in-text heading — the latter unless the content already opens with
    // its own heading. Neither carries sentence data, so their words have no
    // spans and nothing in them is translatable.
    const prefix: ParagraphRender[] = [];
    if (index === 0 && coverUrl) prefix.push(synthetic("", "figure"));
    const title = manifest?.chapters[index]?.title?.trim() ?? "";
    if (title.length > 0 && content[0]?.role !== "heading") prefix.push(synthetic(title, "heading"));
    titleShift = prefix.length;
    return [...prefix, ...content];
  }

  /** A paragraph the book doesn't contain: the cover page, the chapter title. */
  function synthetic(text: string, role: ParagraphRole): ParagraphRender {
    return { text, spans: [], role, emphasis: [], notes: [], glossRuns: [] };
  }

  /** Index of the cover page in `renders`, or -1 when this chapter has none. */
  const coverPageIndex = $derived(chapterIndex === 0 && coverUrl ? 0 : -1);

  /**
   * Bring the loaded chapter's renders in line with the bilingual setting,
   * keeping the reader at the paragraph it was on: word offsets move with the
   * interleaved text, so both the renders and the pagination are rebuilt, while
   * the paragraph indices they are anchored to stay put.
   */
  $effect(() => {
    const g = gloss;
    if (loading || !chapterData || g === renderedGloss) return;
    const anchor = currentTopParagraph();
    renderedGloss = g;
    // A selection on the gloss side has no meaning once the gloss is gone.
    selection = null;
    renders = buildRenders(chapterData, g, chapterIndex);
    tick().then(() => {
      applyLayout();
      restoreToParagraph(anchor);
      savePosition();
    });
  });

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

  // Forward/back one screenful. In scroll mode that is a screen of scrolling,
  // which is what makes the keyboard and tap-to-turn work in both read modes.
  function nextPage() {
    if (isPaged) {
      if (pageIndex + 1 < pages) goToPage(pageIndex + 1);
      else nextChapter();
    } else if (viewportEl) {
      viewportEl.scrollBy({ top: viewportEl.clientHeight * 0.9 });
    }
  }
  function prevPage() {
    if (isPaged) {
      if (pageIndex > 0) goToPage(pageIndex - 1);
      else if (chapterIndex > 0) loadChapter(chapterIndex - 1, Number.MAX_SAFE_INTEGER);
    } else if (viewportEl) {
      viewportEl.scrollBy({ top: -viewportEl.clientHeight * 0.9 });
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
    void app.settings.textAlign;
    // Deliberately not tracking `renders`: every path that replaces them
    // (chapter load, bilingual toggle, image load) re-lays out at its own anchor,
    // and a second effect doing it again would restore to a different one.
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
      const clicked = {
        paragraphIndex: +paraEl.dataset.p!,
        sentenceIndex: +wordEl.dataset.s!,
        wordIndex: +wordEl.dataset.w!,
        row: cellEl ? +cellEl.dataset.r! : undefined,
        col: cellEl ? +cellEl.dataset.c! : undefined,
        gloss: wordEl.dataset.g ? true : undefined,
      };
      // Clicking the same run again clears it: in bilingual mode no sheet opens
      // for a gloss chunk, so this is what puts the highlight away.
      selection = sameRun(selection, clicked) ? null : clicked;
      return;
    }
    // Tap action: with page-turning taps, a click on the left/right half of the
    // reading area turns the page and the chrome is reached from the top bar's
    // own area; otherwise a background click toggles the chrome.
    if (app.settings.tapMode === "pageTurn" && !chromeVisible) {
      const box = viewportEl?.getBoundingClientRect();
      if (box) {
        if (e.clientY < box.top + 64) {
          chromeVisible = true;
        } else if (e.clientX < box.left + box.width / 2) {
          prevPage();
        } else {
          nextPage();
        }
        return;
      }
    }
    chromeVisible = !chromeVisible;
  }

  function sameRun(a: typeof selection, b: NonNullable<typeof selection>): boolean {
    return (
      a != null &&
      a.paragraphIndex === b.paragraphIndex &&
      a.sentenceIndex === b.sentenceIndex &&
      a.wordIndex === b.wordIndex &&
      a.row === b.row &&
      a.col === b.col &&
      !!a.gloss === !!b.gloss
    );
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

  /**
   * What paragraph `pi` highlights for the current selection: the clicked word —
   * so the page echoes the sheet's word highlight — plus, in bilingual mode, the
   * words it aligns with on the other side of the language boundary. Table-cell
   * selections address a sentence outside the paragraph list, so they get none.
   */
  function selectedRangesFor(pi: number): Array<[number, number]> {
    const sel = selection;
    if (!sel || sel.paragraphIndex !== pi || sel.row !== undefined) return [];
    const render = renders[pi];
    if (!render) return [];
    return pairRanges(render, sel.sentenceIndex, sel.wordIndex, sel.gloss === true);
  }

  function highlightRangesFor(pi: number): Array<[number, number]> {
    if (searchQuery.trim().length === 0) return [];
    const text = renders[pi]?.text;
    if (!text) return [];
    return searchParagraphs(searchQuery, [text], chapterIndex).map((m) => [m.start, m.end]);
  }

  /** Jump to a paragraph addressed in chapter-content space (as search indexes it). */
  function jumpToChapterParagraph(chapter: number, paragraph: number) {
    if (chapter === chapterIndex) {
      restoreToParagraph(paragraph + titleShift);
      savePosition();
    } else {
      loadChapter(chapter, paragraph, true);
    }
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
    activeMatch = searchResults.indexOf(m);
    jumpToChapterParagraph(m.chapterIndex, m.paragraphIndex);
  }

  /** Step through the matches without reopening the search panel. */
  function stepMatch(delta: number) {
    if (searchResults.length === 0) return;
    const next = Math.min(searchResults.length - 1, Math.max(0, activeMatch + delta));
    activeMatch = next;
    jumpToChapterParagraph(searchResults[next].chapterIndex, searchResults[next].paragraphIndex);
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
    if (e.key === "ArrowRight" || e.key === "PageDown" || e.key === " ") {
      e.preventDefault();
      nextPage();
    } else if (e.key === "ArrowLeft" || e.key === "PageUp") {
      e.preventDefault();
      prevPage();
    } else if (e.key === "n" && searchResults.length > 0) {
      stepMatch(1);
    } else if (e.key === "p" && searchResults.length > 0) {
      stepMatch(-1);
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
        {#if i === coverPageIndex}
          <!-- The cover: a page of its own, opening the book. -->
          <figure class="para figure full-page" data-p={i}>
            <img src={coverUrl} alt={manifest?.title ?? ""} onload={onImageLoad} />
          </figure>
        {:else}
        <Paragraph
          render={r}
          index={i}
          highlightRanges={highlightRangesFor(i)}
          selectedRanges={selectedRangesFor(i)}
          {gloss}
          figure={figuresByPara[i]}
          imageUrl={figuresByPara[i] ? (imageUrls[figuresByPara[i].image] ?? null) : null}
          table={tablesByPara[i]}
          {onImageLoad}
        />
        {/if}
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
        <div class="reader-chapter">
          {[manifest?.author, chapters[chapterIndex]?.title].filter(Boolean).join(" · ")}
        </div>
      </div>
      {#if availableLangs.length > 0}
        <!-- The slash of "source / translation": bilingual mode interleaves each
             sentence's translation into the page. -->
        <button
          class="icon-btn bilingual"
          class:active={app.settings.bilingual}
          title={app.settings.bilingual ? "Bilingual text: on" : "Bilingual text: off"}
          aria-pressed={app.settings.bilingual}
          onclick={() => app.update({ bilingual: !app.settings.bilingual })}>/</button
        >
      {/if}
      <button class="icon-btn" title="Search" onclick={() => (searchActive = true)}>🔍</button>
      {#if searchResults.length > 0}
        <!-- Step through the matches with the panel closed (also n / p). -->
        <span class="match-nav">
          <button
            class="icon-btn"
            title="Previous match"
            disabled={activeMatch <= 0}
            onclick={() => stepMatch(-1)}>‹</button
          >
          <span class="match-count">
            {activeMatch >= 0 ? activeMatch + 1 : "–"}/{searchResults.length}
          </span>
          <button
            class="icon-btn"
            title="Next match"
            disabled={activeMatch >= searchResults.length - 1}
            onclick={() => stepMatch(1)}>›</button
          >
        </span>
      {/if}
      <button
        class="icon-btn"
        title="Day / night"
        onclick={() =>
          app.update({ theme: app.settings.theme === "dark" ? "light" : "dark" })}>◐</button
      >
      <button class="icon-btn" title="Chapters" onclick={() => (chapterListOpen = true)}>☰</button>
      <button class="icon-btn" title="Settings" onclick={() => app.goSettings()}>⚙</button>
    </header>

    <footer class="reader-bottom">
      <div class="reader-status">
        {#if isPaged}Page {chapterPage} / {chapterPages} ·{/if}
        {chapters[chapterIndex]?.title || "—"} · {Math.round(progressFraction * 100)}%
      </div>
      <div class="reader-controls">
        <button
          class="icon-btn"
          title="Previous chapter"
          disabled={chapterIndex <= 0}
          onclick={prevChapter}>‹</button
        >
        {#if isPaged}
          <!-- Seek within the chapter; the book-wide progress bar is below. -->
          <input
            class="page-slider"
            type="range"
            min="0"
            max={Math.max(0, pages - 1)}
            value={pageIndex}
            aria-label="Page"
            oninput={(e) => goToPage(+e.currentTarget.value)}
          />
        {:else}
          <div class="progress-track">
            <div class="progress-fill" style="width:{progressFraction * 100}%"></div>
          </div>
        {/if}
        <button
          class="icon-btn"
          title="Next chapter"
          disabled={!manifest || chapterIndex >= manifest.chapters.length - 1}
          onclick={nextChapter}>›</button
        >
      </div>
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

  <!-- A click on the interleaved gloss only highlights the pair: its index
       addresses an align chunk, not a word a dictionary could look up. -->
  {#if selection && selectedSentence && !selection.gloss}
    <TranslationSheet
      sentence={selectedSentence}
      wordIndex={selection.wordIndex}
      {glossLang}
      availableLangs={app.settings.hideLangPicker ? [] : availableLangs}
      sourceLang={manifest?.sourceLang ?? "en"}
      glossOnPage={gloss !== null}
      onGlossLangChange={(lang) => app.setGloss(lang)}
      onOpenDictionaries={() => app.goDictionaries()}
      onDismiss={() => (selection = null)}
    />
  {/if}

  {#if activeNote}
    <NoteSheet
      note={activeNote}
      {gloss}
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
      availableLangs={app.settings.hideLangPicker ? [] : availableLangs}
      sourceLang={manifest?.sourceLang ?? "en"}
      onGlossLangChange={(lang) => app.setGloss(lang)}
      onOpenDictionaries={() => app.goDictionaries()}
      onDismiss={() => (noteWordSel = null)}
    />
  {/if}
</div>

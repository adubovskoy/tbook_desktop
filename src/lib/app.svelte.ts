// Global reactive app state (Svelte 5 runes): current view, library, settings,
// and reading-position persistence — all backed by tauri-plugin-store.

import { load, type Store } from "@tauri-apps/plugin-store";
import * as api from "./api";
import type { BookSummary } from "./types";
import {
  type AppSettings,
  DEFAULTS,
  FONT_MAX,
  FONT_MIN,
  KEY,
  PAD_MAX,
  PAD_MIN,
  POPUP_FONT_MAX,
  POPUP_FONT_MIN,
  clamp,
} from "./settings";

export type View = "library" | "reader" | "settings" | "dictionaries" | "help";

/** Key prefix the book id is appended to for the library's ordering. */
const LAST_OPENED_PREFIX = "last_opened_";

export interface ReadingPosition {
  chapterIndex: number;
  /** Anchor paragraph (top of viewport) — robust across re-pagination. */
  paragraphIndex: number;
  /** Within-chapter progress fraction [0,1], for the progress bar. */
  fraction: number;
}

class AppState {
  view = $state<View>("library");
  bookId = $state<string | null>(null);
  books = $state<BookSummary[]>([]);
  settings = $state<AppSettings>({ ...DEFAULTS });
  ready = $state(false);

  #store: Store | null = null;
  /** Reader-supplied getter for the live position, so `flush()` can grab it on close. */
  #positionProvider: (() => (ReadingPosition & { bookId: string }) | null) | null = null;

  async init(): Promise<void> {
    this.#store = await load("settings.json", { autoSave: true, defaults: {} });
    await this.#loadSettings();
    await this.refreshBooks();
    this.ready = true;
  }

  get store(): Store {
    if (!this.#store) throw new Error("store not initialized");
    return this.#store;
  }

  async #loadSettings(): Promise<void> {
    const s = this.store;
    const g = async <T>(key: string, fallback: T): Promise<T> =>
      ((await s.get<T>(key)) ?? fallback) as T;
    this.settings = {
      showCover: await g(KEY.showCover, DEFAULTS.showCover),
      font: await g(KEY.font, DEFAULTS.font),
      fontSizeSp: await g(KEY.fontSizeSp, DEFAULTS.fontSizeSp),
      theme: await g(KEY.theme, DEFAULTS.theme),
      accent: await g(KEY.accent, DEFAULTS.accent),
      glossLang: (await s.get<string>(KEY.glossLang)) ?? DEFAULTS.glossLang,
      bilingual: await g(KEY.bilingual, DEFAULTS.bilingual),
      hideLangPicker: await g(KEY.hideLangPicker, DEFAULTS.hideLangPicker),
      readMode: await g(KEY.readMode, DEFAULTS.readMode),
      tapMode: await g(KEY.tapMode, DEFAULTS.tapMode),
      textAlign: await g(KEY.textAlign, DEFAULTS.textAlign),
      popupWordSizeSp: await g(KEY.popupWordSizeSp, DEFAULTS.popupWordSizeSp),
      popupIpaSizeSp: await g(KEY.popupIpaSizeSp, DEFAULTS.popupIpaSizeSp),
      popupSentenceSizeSp: await g(KEY.popupSentenceSizeSp, DEFAULTS.popupSentenceSizeSp),
      padLeft: await g(KEY.padLeft, DEFAULTS.padLeft),
      padRight: await g(KEY.padRight, DEFAULTS.padRight),
      padTop: await g(KEY.padTop, DEFAULTS.padTop),
      padBottom: await g(KEY.padBottom, DEFAULTS.padBottom),
    };
  }

  /** Apply a settings patch (clamping numeric ranges) and persist it. */
  async update(patch: Partial<AppSettings>): Promise<void> {
    const next = { ...this.settings, ...patch };
    if (patch.fontSizeSp !== undefined) {
      next.fontSizeSp = clamp(patch.fontSizeSp, FONT_MIN, FONT_MAX);
    }
    for (const k of ["padLeft", "padRight", "padTop", "padBottom"] as const) {
      if (patch[k] !== undefined) next[k] = clamp(patch[k]!, PAD_MIN, PAD_MAX);
    }
    for (const k of ["popupWordSizeSp", "popupIpaSizeSp", "popupSentenceSizeSp"] as const) {
      if (patch[k] !== undefined) next[k] = clamp(patch[k]!, POPUP_FONT_MIN, POPUP_FONT_MAX);
    }
    this.settings = next;
    for (const k of Object.keys(patch) as (keyof AppSettings)[]) {
      await this.store.set(KEY[k], next[k]);
    }
  }

  async setGloss(lang: string | null): Promise<void> {
    await this.update({ glossLang: lang });
  }

  /**
   * Reload the library, most recently opened first (books never opened keep the
   * backend's title order, after the ones that were).
   */
  async refreshBooks(): Promise<void> {
    const books = await api.listBooks();
    const opened = await this.#lastOpenedAll();
    this.books = books.sort((a, b) => {
      const diff = (opened[b.id] ?? 0) - (opened[a.id] ?? 0);
      return diff !== 0 ? diff : a.title.localeCompare(b.title);
    });
  }

  /** Stamp a book as just opened — this is what orders the library. */
  async markOpened(id: string): Promise<void> {
    await this.store.set(`${LAST_OPENED_PREFIX}${id}`, Date.now());
  }

  /**
   * Last-opened timestamps by book id. One read of the whole store — cheaper
   * than a lookup per book, and books never opened are simply absent.
   */
  async #lastOpenedAll(): Promise<Record<string, number>> {
    const out: Record<string, number> = {};
    for (const [key, value] of await this.store.entries<number>()) {
      if (key.startsWith(LAST_OPENED_PREFIX) && typeof value === "number") {
        out[key.slice(LAST_OPENED_PREFIX.length)] = value;
      }
    }
    return out;
  }

  openReader(id: string): void {
    this.bookId = id;
    this.view = "reader";
  }

  goLibrary(): void {
    this.view = "library";
    this.bookId = null;
    void this.refreshBooks();
  }

  goSettings(): void {
    this.view = "settings";
  }

  goDictionaries(): void {
    this.view = "dictionaries";
  }

  goHelp(): void {
    this.view = "help";
  }

  async getPosition(id: string): Promise<ReadingPosition> {
    const s = this.store;
    return {
      chapterIndex: (await s.get<number>(`pos_chapter_${id}`)) ?? 0,
      paragraphIndex: (await s.get<number>(`pos_para_${id}`)) ?? 0,
      fraction: (await s.get<number>(`pos_frac_${id}`)) ?? 0,
    };
  }

  async savePosition(id: string, pos: ReadingPosition): Promise<void> {
    const s = this.store;
    await s.set(`pos_chapter_${id}`, pos.chapterIndex);
    await s.set(`pos_para_${id}`, pos.paragraphIndex);
    await s.set(`pos_frac_${id}`, pos.fraction);
  }

  /** Forget everything remembered about a book (called when it is deleted). */
  async clearPosition(id: string): Promise<void> {
    const s = this.store;
    await s.delete(`pos_chapter_${id}`);
    await s.delete(`pos_para_${id}`);
    await s.delete(`pos_frac_${id}`);
    await s.delete(`${LAST_OPENED_PREFIX}${id}`);
  }

  /** The reader registers a getter so `flush()` can read the live position on close. */
  registerPositionProvider(fn: () => (ReadingPosition & { bookId: string }) | null): void {
    this.#positionProvider = fn;
  }

  clearPositionProvider(fn: () => (ReadingPosition & { bookId: string }) | null): void {
    if (this.#positionProvider === fn) this.#positionProvider = null;
  }

  /**
   * Persist the latest reading position and force the store to disk. Called on
   * app close, where the debounced in-session saves may not have flushed yet.
   */
  async flush(): Promise<void> {
    if (!this.#store) return;
    const pos = this.#positionProvider?.();
    if (pos) await this.savePosition(pos.bookId, pos);
    await this.#store.save();
  }
}

export const app = new AppState();

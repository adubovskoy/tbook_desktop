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
  clamp,
} from "./settings";

export type View = "library" | "reader" | "settings";

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
      glossLang: (await s.get<string>(KEY.glossLang)) ?? DEFAULTS.glossLang,
      readMode: await g(KEY.readMode, DEFAULTS.readMode),
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
    this.settings = next;
    for (const k of Object.keys(patch) as (keyof AppSettings)[]) {
      await this.store.set(KEY[k], next[k]);
    }
  }

  async setGloss(lang: string): Promise<void> {
    await this.update({ glossLang: lang });
  }

  async refreshBooks(): Promise<void> {
    this.books = await api.listBooks();
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

  async clearPosition(id: string): Promise<void> {
    const s = this.store;
    await s.delete(`pos_chapter_${id}`);
    await s.delete(`pos_para_${id}`);
    await s.delete(`pos_frac_${id}`);
  }
}

export const app = new AppState();

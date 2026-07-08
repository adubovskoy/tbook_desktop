// Mirrors `data/prefs/SettingsRepository.kt`: settings model, defaults, ranges,
// and the store key names (kept identical to the Android DataStore keys).

export type ThemeMode = "system" | "light" | "dark";
export type ReaderFont = "serif" | "sans" | "mono";
export type ReadMode = "scroll" | "paged";

export interface AppSettings {
  showCover: boolean;
  font: ReaderFont;
  fontSizeSp: number;
  theme: ThemeMode;
  glossLang: string | null;
  readMode: ReadMode;
  padLeft: number;
  padRight: number;
  padTop: number;
  padBottom: number;
}

export const DEFAULTS: AppSettings = {
  showCover: true,
  font: "serif",
  fontSizeSp: 18,
  theme: "system",
  glossLang: null,
  readMode: "scroll",
  padLeft: 20,
  padRight: 20,
  padTop: 16,
  padBottom: 12,
};

export const FONT_MIN = 14;
export const FONT_MAX = 30;
export const PAD_MIN = 0;
export const PAD_MAX = 64;
export const PAD_STEP = 4;

/** Maps each setting to its persisted store key (same as Android's keys). */
export const KEY: Record<keyof AppSettings, string> = {
  showCover: "show_cover",
  font: "font_family",
  fontSizeSp: "font_size_sp",
  theme: "theme_mode",
  glossLang: "gloss_lang",
  readMode: "read_mode",
  padLeft: "pad_left",
  padRight: "pad_right",
  padTop: "pad_top",
  padBottom: "pad_bottom",
};

export function clamp(v: number, lo: number, hi: number): number {
  return Math.max(lo, Math.min(hi, v));
}

export const FONT_FAMILY: Record<ReaderFont, string> = {
  serif: 'Georgia, "Times New Roman", "Noto Serif", serif',
  sans: '-apple-system, "Segoe UI", Roboto, "Noto Sans", system-ui, sans-serif',
  mono: '"SF Mono", "Cascadia Code", "Noto Sans Mono", ui-monospace, monospace',
};

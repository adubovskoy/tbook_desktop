// Mirrors `data/prefs/SettingsRepository.kt`: settings model, defaults, ranges,
// and the store key names (kept identical to the Android DataStore keys).

export type ThemeMode = "system" | "light" | "dark";
export type ReaderFont = "serif" | "sans" | "mono";
export type ReadMode = "scroll" | "paged";

/**
 * Which English the app pronounces: the voice read-aloud picks for a book
 * tagged plain "en" (and, once both dictionaries ship, the transcription shown
 * for a tapped word).
 */
export type Accent = "american" | "british";

/** Body-text alignment. Headings/captions keep their own (centered) alignment. */
export type TextAlignMode = "left" | "justify";

/**
 * What a click on the reading page does. `translate` — the default: a click on a
 * word opens its translation, pages are turned with the keys or the page
 * arrows. `pageTurn` — a click turns the page (left half back, right half
 * forward); words still translate, and the chrome is reached by clicking the top
 * strip of the page.
 */
export type TapMode = "translate" | "pageTurn";

/** Whichever English is taught where the machine lives. */
export function deviceAccent(): Accent {
  let region = "";
  try {
    region = new Intl.Locale(navigator.language || "en-US").region ?? "";
  } catch {
    // An unparseable navigator.language just falls through to the default.
  }
  return ["US", "CA", "PH"].includes(region) ? "american" : "british";
}

export interface AppSettings {
  showCover: boolean;
  font: ReaderFont;
  fontSizeSp: number;
  theme: ThemeMode;
  accent: Accent;
  glossLang: string | null;
  /**
   * Bilingual reading: every sentence is followed by its translation into the
   * resolved gloss language, rendered paler, and a click highlights the aligned
   * words in place. Toggled from the reader's top bar.
   */
  bilingual: boolean;
  /**
   * Multilingual books show a language chip row in the translation popup. Hiding
   * it pins the popup to the resolved gloss language.
   */
  hideLangPicker: boolean;
  readMode: ReadMode;
  tapMode: TapMode;
  textAlign: TextAlignMode;
  /**
   * Translation-popup font sizes. Kept separate from the reading text: the popup
   * is read at arm's length from a different distance than the page.
   */
  popupWordSizeSp: number;
  popupIpaSizeSp: number;
  popupSentenceSizeSp: number;
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
  accent: deviceAccent(),
  glossLang: null,
  bilingual: false,
  hideLangPicker: false,
  readMode: "scroll",
  tapMode: "translate",
  textAlign: "left",
  popupWordSizeSp: 24,
  popupIpaSizeSp: 16,
  popupSentenceSizeSp: 16,
  padLeft: 20,
  padRight: 20,
  padTop: 16,
  padBottom: 12,
};

export const FONT_MIN = 14;
export const FONT_MAX = 30;
export const POPUP_FONT_MIN = 14;
export const POPUP_FONT_MAX = 40;
export const POPUP_FONT_STEP = 2;
export const PAD_MIN = 0;
export const PAD_MAX = 64;
export const PAD_STEP = 4;

/** Maps each setting to its persisted store key (same as Android's keys). */
export const KEY: Record<keyof AppSettings, string> = {
  showCover: "show_cover",
  font: "font_family",
  fontSizeSp: "font_size_sp",
  theme: "theme_mode",
  accent: "accent",
  glossLang: "gloss_lang",
  bilingual: "bilingual",
  hideLangPicker: "hide_lang_picker",
  readMode: "read_mode",
  tapMode: "tap_mode",
  textAlign: "text_align",
  popupWordSizeSp: "popup_word_size_sp",
  popupIpaSizeSp: "popup_ipa_size_sp",
  popupSentenceSizeSp: "popup_sentence_size_sp",
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

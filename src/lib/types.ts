// Mirrors the Android app's `data/model/Models.kt` and the Rust backend structs.

export interface ChapterRef {
  id: string;
  title: string;
  file: string;
}

export interface Manifest {
  formatVersion: number;
  title: string;
  author: string;
  sourceLang: string;
  targetLangs: string[];
  cover: string | null;
  /** Entry name of the footnote-bodies file ("notes.json"), if any. */
  notes?: string | null;
  chapters: ChapterRef[];
}

export interface OpenedBook {
  manifest: Manifest;
  chapterSizes: number[];
}

export interface BookSummary {
  id: string;
  title: string;
  author: string;
  sourceLang: string;
  targetLangs: string[];
  hasCover: boolean;
}

/** A translation chunk: `t` is a [start,end) char range in the translation
 *  `text`; `w` lists the source word indices it translates. */
export interface AlignChunk {
  t: number[];
  w?: number[];
}

export interface Translation {
  text: string;
  align?: AlignChunk[];
  /** Optional alignment-coverage score; informative only — never rendered. */
  q?: number;
}

/** Inline emphasis run over `src`: range [s,e), kind `"i"` italic / `"b"` bold. */
export interface Span {
  s: number;
  e: number;
  k: string;
}

/** Inline footnote marker: insertion point `p` into `src` (0 ≤ p ≤ src.length),
 *  note `id` (a key of notes.json), display `label` ("1", "*", …). */
export interface NoteRef {
  p: number;
  id: string;
  label: string;
}

export interface Sentence {
  src: string;
  /** Each entry is a [start,end) char range of a tappable word in `src`. */
  words?: number[][];
  tr?: Record<string, Translation>;
  spans?: Span[];
  notes?: NoteRef[];
}

/** Book-level footnote body (a value of the notes.json object). */
export interface Note {
  label: string;
  /** "note" (default) or "citation"; unknown values are treated as "note". */
  kind?: string;
  paragraphs: Sentence[][];
}

/** A body image anchored to a `figure`-role paragraph (its caption). */
export interface Figure {
  para: number;
  image: string;
  alt?: string;
}

export interface TableCell {
  sentences: Sentence[];
  header?: boolean;
}

/** A table anchored to an empty `table`-role paragraph. */
export interface Table {
  para: number;
  rows: TableCell[][];
}

export interface Chapter {
  paragraphs: Sentence[][];
  paragraphStyles?: string[];
  figures?: Figure[];
  tables?: Table[];
}

export type ParagraphRole = "body" | "subtitle" | "heading" | "sceneBreak" | "figure" | "table";

/** Role of paragraph `index`, defaulting to body when absent/unknown. */
export function roleAt(chapter: Chapter, index: number): ParagraphRole {
  switch (chapter.paragraphStyles?.[index]) {
    case "subtitle":
      return "subtitle";
    case "heading":
      return "heading";
    case "sceneBreak":
      return "sceneBreak";
    case "figure":
      return "figure";
    case "table":
      return "table";
    default:
      return "body";
  }
}

// ---------------------------------------------------------------------------
// Format version 2 (doc/specs/tbook-format-v2.md)
//
// The manifest reaches the WebView normalized by the Rust side, so `Manifest`
// above serves both versions — `formatVersion` says which. Everything below is
// the version-2 chapter model: a language-free skeleton plus one overlay per
// (chapter, language). **Every offset in these types is a Unicode code-point
// offset** (§7); `tokenize.ts` converts them to UTF-16 before any slicing.
// ---------------------------------------------------------------------------

/**
 * Where a locator resolved to (§3.5.2). `paragraph` null = the chapter's start
 * (a locator whose paragraph is gone, or nothing resolved: the book start).
 * `sentence`/`word` come back as stored, to be checked against the tokenized
 * paragraph; `exact` = chapter and paragraph both resolved by id.
 */
export interface Locator {
  chapter: number;
  paragraph: number | null;
  sentence: number | null;
  word: number | null;
  exact: boolean;
}

/** A chapter skeleton `text/chN.json` (§4). */
export interface SkeletonV2 {
  id: string;
  paragraphs: BlockV2[];
}

/** A paragraph, table cell or footnote paragraph (§4.2). */
export interface BlockV2 {
  id?: string;
  /** `body` (default), `subtitle`, `heading`, `sceneBreak`, `figure`, `table`. */
  role?: string;
  /** The whole block text, displayed verbatim (§4.5). */
  text: string;
  /** Sentence ranges `[a, b)` into `text`. */
  sents: number[][];
  /** Per-sentence source word ranges, replacing the tokenizer (§4.6). */
  words?: number[][][];
  spans?: Span[];
  notes?: NoteRef[];
  units?: UnitV2[];
  figure?: FigureV2;
  table?: TableV2;
  header?: boolean;
}

/** A source-side multi-word expression (§4.9). */
export interface UnitV2 {
  s: number;
  w: number[];
  k?: string;
}

export interface FigureV2 {
  image: string;
  alt?: string;
}

export interface TableV2 {
  rows: BlockV2[][];
}

/** A footnote body of `text/notes.json` (§4.12): paragraphs in skeleton shape. */
export interface NoteV2 {
  label: string;
  /** `note` (default) or `citation`; unknown values read as `note`. */
  kind?: string;
  paragraphs: BlockV2[];
}

/** A footnote overlay `gloss/notes.<lang>.json` (§6.12), bound per note id. */
export interface NotesOverlayV2 {
  lang: string;
  gates: string[];
  alignDigest?: string;
  notes: Record<string, GlossParagraphV2[]>;
}

/** An overlay `gloss/chN.<lang>.json` (§6). */
export interface OverlayV2 {
  lang: string;
  chapter: string;
  /** Quality gates that ran over every translation of this overlay (§6.8). */
  gates: string[];
  alignDigest?: string;
  paragraphs: GlossParagraphV2[];
}

/** `{id, s}` for a text block, `{id, rows}` for a table block (§6.1). */
export interface GlossParagraphV2 {
  id: string;
  s?: (TranslationV2 | null)[];
  rows?: GlossCellV2[][];
}

export interface GlossCellV2 {
  s?: (TranslationV2 | null)[];
}

/** The link of one target token: a source word index, several, or none (§6.4). */
export type LinkV2 = number | number[] | null;

/** One sentence's rendering in one language (§6.2). */
export interface TranslationV2 {
  t?: string;
  /** One link per target token, in token order. */
  a?: LinkV2[];
  /** Escape chunks `[a, b, ...sourceWords]` (§6.5). */
  x?: number[][];
  /** Target token inventory override (§6.6). */
  words?: number[][];
  /** Producer-written status; absent = ok (§6.7). */
  s?: string;
  /** Verdict of a gate that did not pass this cell (§6.8). */
  v?: VerdictV2;
  /** Actual language of `t`, with `s: "wrongLang"` (§6.9). */
  lang?: string;
  /** `[kind, targetTokens, sourceWords]` (§6.10). */
  groups?: [string, number[], number[]][];
  /** `[token, a, b, link]`, offsets relative to the token start (§6.11). */
  parts?: [number, number, number, LinkV2][];
}

export interface VerdictV2 {
  ok: boolean;
  by: string;
  why?: string;
}

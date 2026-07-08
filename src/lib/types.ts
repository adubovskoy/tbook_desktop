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

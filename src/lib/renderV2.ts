// Version-2 paragraphs: a skeleton block plus (optionally) one language's
// overlay, rendered into the same `ParagraphRender` the version-1 path
// produces, so `Paragraph.svelte`, `paragraphHTML` and `wordAt` are shared.
//
// The difference from version 1 is where the coordinates come from: there a
// paragraph is a list of sentences joined with spaces and every word carries
// its own range; here the paragraph text is one string rendered verbatim
// (§4.5), sentences are ranges into it and words come from the tokenizer.

import { flagOf, highlightFor, linkWords, targetSide, type Flag } from "./alignV2.ts";
import {
  SCENE_BREAK_TEXT,
  type EmphasisRun,
  type NoteMarker,
  type ParagraphRender,
  type WordSpan,
} from "./render.ts";
import { codePoints, toRange, wordsBySentence, type Range } from "./tokenize.ts";
import type { BlockV2, ParagraphRole, TranslationV2 } from "./types.ts";

/** A block with its file offsets converted to UTF-16 and its words derived. */
export interface PreparedBlock {
  block: BlockV2;
  role: ParagraphRole;
  /** Sentence ranges into `block.text`, in UTF-16 units. */
  sents: Range[];
  /** Word ranges per sentence: the `words` override, or `tbook-w2` (§4.5/§4.6). */
  words: Range[][];
  /** Inline emphasis, converted (§4.7). */
  emphasis: Array<{ range: Range; bold: boolean }>;
  /** Prepared table cells, or null for a non-table block. */
  cells: PreparedBlock[][] | null;
  /** Footnote markers (§4.8): insertion points into `block.text`, UTF-16, sorted. */
  markers: Array<{ at: number; id: string; label: string }>;
}

function roleOf(role: string | undefined): ParagraphRole {
  switch (role) {
    case "subtitle":
    case "heading":
    case "sceneBreak":
    case "figure":
    case "table":
      return role;
    default:
      // Unknown roles render as body (§4.4).
      return "body";
  }
}

/** Convert one block's offsets once, and derive its word inventory (§5.4). */
export function prepareBlock(block: BlockV2): PreparedBlock {
  const text = block.text ?? "";
  const cps = codePoints(text);
  // Sentence and word indices are what every link refers to, so each entry
  // keeps its slot even when its range is degenerate.
  const sents: Range[] = (block.sents ?? []).map((s) => toRange(cps, s));
  const derived = wordsBySentence(text, sents);
  const words: Range[][] = sents.map((_, k) => {
    const override = block.words?.[k];
    if (!override) return derived[k] ?? [];
    return override.map((w) => toRange(cps, w));
  });
  const emphasis = (block.spans ?? [])
    .filter((sp) => sp.k === "i" || sp.k === "b")
    .map((sp) => ({ range: toRange(cps, [sp.s, sp.e]), bold: sp.k === "b" }))
    .filter((e) => e.range[1] > e.range[0]);
  const cells = block.table
    ? (block.table.rows ?? []).map((row) => row.map((cell) => prepareBlock(cell)))
    : null;
  const markers = (block.notes ?? [])
    .filter((n) => typeof n.label === "string" && n.label.length > 0 && Number.isFinite(n.p))
    .map((n) => ({ at: cps.u16(n.p), id: n.id, label: n.label }))
    .sort((a, b) => a.at - b.at);
  return { block, role: roleOf(block.role), sents, words, emphasis, cells, markers };
}

/**
 * Build a block's render. Without `trs` (or with `interleave` false) the text is
 * the block's own, verbatim — the only thing §4.5 allows. With a language's
 * translations interleaved (bilingual mode) each sentence is followed by its
 * gloss, and every linked target token becomes a tappable run of its own; the
 * source offsets after it shift right, exactly as version 1 shifts them past a
 * spliced footnote label.
 */
export function buildBlockRender(
  p: PreparedBlock,
  trs?: (TranslationV2 | null)[] | null,
  interleave = false,
  knownNotes?: Set<string> | null,
): ParagraphRender {
  const { role } = p;
  if (role === "sceneBreak") {
    return {
      text: SCENE_BREAK_TEXT,
      spans: [],
      role,
      emphasis: [],
      notes: [],
      glossRuns: [],
    };
  }
  const src = p.block.text ?? "";
  const spans: WordSpan[] = [];
  const glossRuns: Range[] = [];
  const flaggedRuns: Range[] = [];
  /** Where a gloss or a footnote label was spliced in, and how much text it added. */
  const inserted: Array<{ at: number; len: number }> = [];
  const notes: NoteMarker[] = [];
  // Footnote markers (§4.8) whose note the book has: the label is not part of
  // `text`, so it is spliced in at its insertion point, like version 1 does.
  const markers = knownNotes ? p.markers.filter((m) => knownNotes.has(m.id)) : [];
  let mi = 0;
  let text = "";
  let cursor = 0;

  /** Copy the source up to `to`, splicing in every marker at or before it. */
  const copyTo = (to: number) => {
    while (mi < markers.length && markers[mi].at <= to) {
      const m = markers[mi++];
      const at = Math.max(cursor, Math.min(m.at, src.length));
      text += src.slice(cursor, at);
      cursor = at;
      const start = text.length;
      text += m.label;
      notes.push({ start, end: text.length, id: m.id, label: m.label });
      inserted.push({ at, len: m.label.length });
    }
    if (to > cursor) {
      text += src.slice(cursor, to);
      cursor = to;
    }
  };

  p.sents.forEach((sent, k) => {
    // A sentence boundary may fall inside a word (§4.5); splicing the gloss at
    // the end of the last word keeps that word in one piece on the page.
    const last = p.words[k]?.[p.words[k].length - 1];
    const cut = Math.max(sent[1], last ? last[1] : 0, cursor);
    copyTo(cut);

    if (!interleave) return;
    const tr = trs?.[k];
    if (!tr) return;
    const t = tr.t ?? "";
    const flag = flagOf(tr);
    // A translation identical to its source (numerals, names, "I.") would only
    // read as a stutter; a hidden status never reaches the page at all (§6.7).
    if (t.trim().length === 0 || flag?.hidden) return;
    if (t.trim() === src.slice(sent[0], sent[1]).trim()) return;

    text += " ";
    const base = text.length;
    text += t;
    glossRuns.push([base, text.length]);
    if (flag) flaggedRuns.push([base, text.length]);
    inserted.push({ at: cut, len: text.length - base + 1 });

    // Each linked token becomes its own tappable run; escape chunks keep their
    // own index space above the tokens so a click can still identify them.
    const { toks } = targetSide(tr);
    const chunks: WordSpan[] = [];
    (tr.a ?? []).forEach((link, j) => {
      const words = linkWords(link);
      if (words.length === 0 || j >= toks.length) return;
      chunks.push({
        start: base + toks[j][0],
        end: base + toks[j][1],
        sentenceIndex: k,
        wordIndex: j,
        gloss: true,
        alignedWords: words,
      });
    });
    const { cps } = targetSide(tr);
    (tr.x ?? []).forEach((ch, xi) => {
      if (ch.length < 3) return;
      const a = cps.u16(ch[0]);
      const b = cps.u16(ch[1]);
      if (b <= a) return;
      chunks.push({
        start: base + a,
        end: base + b,
        sentenceIndex: k,
        wordIndex: toks.length + xi,
        gloss: true,
        alignedWords: ch.slice(2),
      });
    });
    chunks.sort((x, y) => x.start - y.start);
    let taken = base;
    for (const span of chunks) {
      if (span.start < taken) continue; // `wordAt` needs a disjoint, sorted list
      spans.push(span);
      taken = span.end;
    }
  });
  copyTo(src.length);

  // Words and emphasis runs are block coordinates: shift them past every gloss
  // or label spliced in before them (a range's start moves past an insertion at
  // its own offset, its end does not — the insertion follows the range).
  const shift = (o: number, isEnd: boolean): number => {
    let d = 0;
    for (const ins of inserted) if (isEnd ? ins.at < o : ins.at <= o) d += ins.len;
    return o + d;
  };
  p.sents.forEach((_, k) => {
    (p.words[k] ?? []).forEach((w, wi) => {
      if (w[1] <= w[0]) return; // nothing to click on
      spans.push({ start: shift(w[0], false), end: shift(w[1], true), sentenceIndex: k, wordIndex: wi });
    });
  });
  // `wordAt` binary-searches a sorted, disjoint list.
  spans.sort((x, y) => x.start - y.start);
  const emphasis: EmphasisRun[] = [];
  for (const e of p.emphasis) {
    const a = shift(e.range[0], false);
    const b = shift(e.range[1], true);
    if (b > a) emphasis.push({ start: a, end: b, bold: e.bold });
  }

  return { text, spans, role, emphasis, notes, glossRuns, flaggedRuns };
}

/** Everything the translation sheet shows for one tap (§8.1–§8.3). */
export interface TapResult {
  /** The tapped source word. */
  word: string;
  /** Its sentence, for read-aloud and context. */
  sentence: string;
  /** The translation to show, or null when there is none (or it is hidden). */
  translation: string | null;
  /** UTF-16 ranges of `translation` to mark. */
  ranges: Range[];
  /** `run` / `split` / a `groups` kind (§8.3). */
  label: string | null;
  /** A non-ok status or failed verdict, which the UI MUST surface (§6.7). */
  flag: Flag | null;
  /** The gates that ran over this overlay, so absence of a verdict means something. */
  gates: string[];
}

/** Resolve a tap on word `i` of sentence `k` against one language's overlay. */
export function tapResult(
  p: PreparedBlock,
  trs: (TranslationV2 | null)[] | null | undefined,
  k: number,
  i: number,
  gates: string[] = [],
): TapResult | null {
  const sent = p.sents[k];
  if (!sent) return null;
  const w = p.words[k]?.[i];
  const src = p.block.text ?? "";
  const base: TapResult = {
    word: w ? src.slice(w[0], w[1]) : "",
    sentence: src.slice(sent[0], sent[1]),
    translation: null,
    ranges: [],
    label: null,
    flag: null,
    gates,
  };
  const tr = trs?.[k];
  if (!tr) return base;
  base.flag = flagOf(tr);
  const t = tr.t ?? "";
  if (t.trim().length === 0 || base.flag?.hidden) return base;
  const h = highlightFor(tr, i);
  base.translation = t;
  base.ranges = h.ranges;
  base.label = h.label;
  return base;
}

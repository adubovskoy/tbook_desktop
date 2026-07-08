// Port of `ui/reader/ParagraphText.kt` (buildParagraphRender + annotated).
//
// Kotlin String and JS string are both UTF-16, so plain string concatenation
// and `.length`/`.slice` here reproduce the Android offsets exactly.

import { escapeAttr, escapeHtml } from "./html";
import type { ParagraphRole, Sentence } from "./types";

export const SCENE_BREAK_TEXT = "* * *";

/** A tappable source word within a paragraph's flattened text. */
export interface WordSpan {
  start: number;
  end: number;
  sentenceIndex: number;
  wordIndex: number;
}

/** An inline emphasis run in paragraph-flattened coordinates. */
export interface EmphasisRun {
  start: number;
  end: number;
  bold: boolean;
}

/** A footnote marker's label, spliced into the flattened text (§5.6). */
export interface NoteMarker {
  start: number;
  end: number;
  id: string;
  label: string;
}

export interface ParagraphRender {
  text: string;
  /** Sorted by start offset (sentence then word order). */
  spans: WordSpan[];
  role: ParagraphRole;
  emphasis: EmphasisRun[];
  /** Footnote markers in paragraph-flattened coordinates. */
  notes: NoteMarker[];
}

function clamp(v: number, lo: number, hi: number): number {
  return Math.max(lo, Math.min(hi, v));
}

/**
 * Join a paragraph's sentences with single spaces and shift each sentence's
 * word offsets — and inline emphasis spans — into paragraph space. A sceneBreak
 * paragraph carries no sentences and renders the ornament.
 *
 * Footnote markers (§5.6): each marker's label is NOT part of `src`, so it is
 * spliced into the flattened text at its insertion point `p`, shifting every
 * later word/emphasis offset right by the label length. Only markers whose id
 * resolves in `knownNotes` (the notes.json keys) render; unknown ids are
 * ignored and leave clean prose.
 */
export function buildParagraphRender(
  sentences: Sentence[],
  role: ParagraphRole = "body",
  knownNotes?: Set<string> | null,
): ParagraphRender {
  if (role === "sceneBreak") {
    return { text: SCENE_BREAK_TEXT, spans: [], role, emphasis: [], notes: [] };
  }
  let text = "";
  const spans: WordSpan[] = [];
  const emphasis: EmphasisRun[] = [];
  const notes: NoteMarker[] = [];
  sentences.forEach((s, si) => {
    if (text.length > 0) text += " ";
    const base = text.length;
    const srcLen = s.src.length;

    // Resolvable markers, clamped into [0, srcLen] and sorted by insertion point.
    const markers = (s.notes ?? [])
      .filter(
        (n) =>
          Number.isFinite(n.p) &&
          typeof n.label === "string" &&
          n.label.length > 0 &&
          knownNotes != null &&
          knownNotes.has(n.id),
      )
      .map((n) => ({ p: clamp(n.p, 0, srcLen), id: n.id, label: n.label }))
      .sort((a, b) => a.p - b.p);

    // Splice each label into the text, recording its flattened range.
    let cursor = 0;
    for (const m of markers) {
      text += s.src.slice(cursor, m.p);
      const start = text.length;
      text += m.label;
      notes.push({ start, end: text.length, id: m.id, label: m.label });
      cursor = m.p;
    }
    text += s.src.slice(cursor);
    const end = text.length;

    // Shift a src offset right past the labels inserted before it. A range
    // start moves past a marker at the same point (the marker precedes the
    // char at `p`); a range end does not (the marker follows the range).
    const shift = (o: number, isEnd: boolean): number => {
      let d = 0;
      for (const m of markers) {
        if (isEnd ? m.p < o : m.p <= o) d += m.label.length;
        else break;
      }
      return o + d;
    };

    (s.words ?? []).forEach((w, wi) => {
      if (w.length >= 2) {
        const a = base + shift(w[0], false);
        const b = base + shift(w[1], true);
        if (a >= base && a <= end && b >= a && b <= end) {
          spans.push({ start: a, end: b, sentenceIndex: si, wordIndex: wi });
        }
      }
    });
    for (const sp of s.spans ?? []) {
      if (sp.k !== "i" && sp.k !== "b") continue;
      const a = clamp(base + shift(sp.s, false), base, end);
      const b = clamp(base + shift(sp.e, true), a, end);
      if (b > a) emphasis.push({ start: a, end: b, bold: sp.k === "b" });
    }
  });
  return { text, spans, role, emphasis, notes };
}

/** Word whose [start,end) strictly contains `offset`, or null. Binary search. */
export function wordAt(render: ParagraphRender, offset: number): WordSpan | null {
  let lo = 0;
  let hi = render.spans.length - 1;
  while (lo <= hi) {
    const mid = (lo + hi) >>> 1;
    const s = render.spans[mid];
    if (offset < s.start) hi = mid - 1;
    else if (offset >= s.end) lo = mid + 1;
    else return s;
  }
  return null;
}

interface Segment {
  text: string;
  word: WordSpan | null;
  bold: boolean;
  italic: boolean;
  highlight: boolean;
}

/**
 * Build the paragraph as an HTML string: tappable words and emphasis runs become
 * `<span>`s carrying data attributes / classes; plain gaps stay as text nodes
 * (keeps the element count down). Highlight ranges (search) tint their range.
 */
export function paragraphHTML(
  render: ParagraphRender,
  highlightRanges: Array<[number, number]> = [],
): string {
  const text = render.text;
  const n = text.length;

  // All boundary points so each segment is uniform in word/emphasis/highlight.
  const bounds = new Set<number>([0, n]);
  for (const w of render.spans) {
    bounds.add(clamp(w.start, 0, n));
    bounds.add(clamp(w.end, 0, n));
  }
  for (const e of render.emphasis) {
    bounds.add(clamp(e.start, 0, n));
    bounds.add(clamp(e.end, 0, n));
  }
  for (const m of render.notes) {
    bounds.add(clamp(m.start, 0, n));
    bounds.add(clamp(m.end, 0, n));
  }
  for (const [a, b] of highlightRanges) {
    bounds.add(clamp(a, 0, n));
    bounds.add(clamp(b, 0, n));
  }
  const points = [...bounds].sort((a, b) => a - b);

  let html = "";
  for (let i = 0; i + 1 < points.length; i++) {
    const p = points[i];
    const q = points[i + 1];
    if (q <= p) continue;
    // A footnote-marker segment is a tappable superscript, never a word.
    const marker = render.notes.find((m) => m.start <= p && m.end >= q);
    if (marker) {
      html += `<sup class="note" data-note="${escapeAttr(marker.id)}">${escapeHtml(text.slice(p, q))}</sup>`;
      continue;
    }
    const seg: Segment = {
      text: text.slice(p, q),
      word: wordAt(render, p),
      bold: false,
      italic: false,
      highlight: false,
    };
    for (const e of render.emphasis) {
      if (e.start <= p && e.end >= q) {
        if (e.bold) seg.bold = true;
        else seg.italic = true;
      }
    }
    for (const [a, b] of highlightRanges) {
      if (a <= p && b >= q) seg.highlight = true;
    }

    const classes: string[] = [];
    if (seg.word) classes.push("w");
    if (seg.bold) classes.push("b");
    if (seg.italic) classes.push("i");
    if (seg.highlight) classes.push("hl");
    if (classes.length === 0) {
      html += escapeHtml(seg.text);
      continue;
    }
    const attrs = seg.word ? ` data-s="${seg.word.sentenceIndex}" data-w="${seg.word.wordIndex}"` : "";
    html += `<span class="${classes.join(" ")}"${attrs}>${escapeHtml(seg.text)}</span>`;
  }
  return html;
}

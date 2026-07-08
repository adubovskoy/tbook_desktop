// Port of `TranslationSheet.highlightedTranslation`: the translation char ranges
// aligned to the tapped source word.

import { escapeHtml } from "./html";
import type { Translation } from "./types";

function clamp(v: number, lo: number, hi: number): number {
  return Math.max(lo, Math.min(hi, v));
}

/** Render `text` to HTML, wrapping each highlighted [start,end) range in `<mark>`. */
export function highlightedHTML(text: string, ranges: Array<[number, number]>): string {
  const n = text.length;
  if (ranges.length === 0) return escapeHtml(text);
  const bounds = new Set<number>([0, n]);
  for (const [a, b] of ranges) {
    bounds.add(clamp(a, 0, n));
    bounds.add(clamp(b, 0, n));
  }
  const points = [...bounds].sort((x, y) => x - y);
  let html = "";
  for (let i = 0; i + 1 < points.length; i++) {
    const p = points[i];
    const q = points[i + 1];
    if (q <= p) continue;
    const seg = escapeHtml(text.slice(p, q));
    const hot = ranges.some(([a, b]) => a <= p && b >= q);
    html += hot ? `<mark>${seg}</mark>` : seg;
  }
  return html;
}

/** [start,end) ranges in `tr.text` to highlight for the given source wordIndex. */
export function translationHighlightRanges(
  tr: Translation,
  wordIndex: number,
): Array<[number, number]> {
  const n = tr.text.length;
  const out: Array<[number, number]> = [];
  for (const chunk of tr.align ?? []) {
    if (!(chunk.w ?? []).includes(wordIndex)) continue;
    if (!chunk.t || chunk.t.length < 2) continue;
    const a = clamp(chunk.t[0], 0, n);
    const b = clamp(chunk.t[1], a, n);
    if (b > a) out.push([a, b]);
  }
  return out;
}

// Port of `ui/reader/BookSearch.kt`: case-insensitive, non-overlapping substring
// search over joined paragraph texts. Offsets are in the same JS-UTF-16
// coordinate system the renderer uses for highlight ranges.

export interface SearchMatch {
  chapterIndex: number;
  paragraphIndex: number;
  /** [start,end) char offsets into the joined paragraph text. */
  start: number;
  end: number;
  snippet: string;
}

/** Case-insensitive indexOf that returns the offset in the *original* string. */
function indexOfIgnoreCase(haystack: string, needleLower: string, from: number): number {
  const n = haystack.length;
  const m = needleLower.length;
  if (m === 0) return from <= n ? from : -1;
  for (let i = from; i + m <= n; i++) {
    let ok = true;
    for (let j = 0; j < m; j++) {
      if (haystack[i + j].toLowerCase() !== needleLower[j]) {
        ok = false;
        break;
      }
    }
    if (ok) return i;
  }
  return -1;
}

export function searchParagraphs(
  query: string,
  paragraphTexts: string[],
  chapterIndex: number,
  snippetRadius = 30,
): SearchMatch[] {
  const q = query.trim();
  if (q.length === 0) return [];
  const ql = q.toLowerCase();
  const out: SearchMatch[] = [];
  paragraphTexts.forEach((text, pi) => {
    let from = 0;
    while (true) {
      const idx = indexOfIgnoreCase(text, ql, from);
      if (idx < 0) break;
      const end = idx + q.length;
      const s = Math.max(0, idx - snippetRadius);
      const e = Math.min(text.length, end + snippetRadius);
      const prefix = s > 0 ? "…" : "";
      const suffix = e < text.length ? "…" : "";
      out.push({
        chapterIndex,
        paragraphIndex: pi,
        start: idx,
        end,
        snippet: prefix + text.slice(s, e) + suffix,
      });
      from = end; // non-overlapping
    }
  });
  return out;
}

/** Scan a whole book given `[chapter][paragraph] -> text` (from the backend). */
export function searchBook(query: string, bookTexts: string[][]): SearchMatch[] {
  if (query.trim().length === 0) return [];
  const out: SearchMatch[] = [];
  bookTexts.forEach((paras, ci) => {
    out.push(...searchParagraphs(query, paras, ci));
  });
  return out;
}

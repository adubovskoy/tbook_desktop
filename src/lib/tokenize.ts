// The normative tokenizer `tbook-w2` (.tbook v2 spec §5) and the code-point ↔
// UTF-16 conversion of §7.
//
// Version 2 ships no per-word character ranges: the consumer regenerates them
// with this tokenizer, on both sides — source words and target tokens — unless
// the file carries a `words` override (§4.6, §6.6).
//
// Offsets *read from the file* are Unicode code points; JS strings are UTF-16.
// `codePoints()` converts, once per string, and is a no-op fast path for text
// without astral characters (the whole reference corpus).

/** A half-open `[start, end)` range, in UTF-16 units unless said otherwise. */
export type Range = [number, number];

/**
 * §5.1, verbatim: a token starts with a letter or a number and continues with
 * letters, marks and numbers; `'`, `’` or `-` joins two such runs; `.`, `,` and
 * `:` join two runs only when the right-hand one starts with a digit.
 */
const TOKEN_RE =
  /[\p{L}\p{N}][\p{L}\p{M}\p{N}]*(?:['’\-][\p{L}\p{N}][\p{L}\p{M}\p{N}]*|[.,:]\p{N}[\p{L}\p{M}\p{N}]*)*/gu;

/** Every `tbook-w2` token of `text`, as UTF-16 ranges in text order. */
export function tokenize(text: string): Range[] {
  const out: Range[] = [];
  if (!text) return out;
  TOKEN_RE.lastIndex = 0;
  for (const m of text.matchAll(TOKEN_RE)) {
    const start = m.index ?? 0;
    out.push([start, start + m[0].length]);
  }
  return out;
}

/** The token texts of `text` (the tokenizer's own conformance vectors). */
export function tokenStrings(text: string): string[] {
  return tokenize(text).map(([a, b]) => text.slice(a, b));
}

/**
 * Code-point ↔ UTF-16 conversion for one string (§7). Every offset read from a
 * `.tbook` entry passes through `u16()` before the string is sliced; offsets
 * the consumer derived itself are already native and need nothing.
 */
export interface CodePoints {
  /** UTF-16 index of code-point offset `cp`, clamped into the string. */
  u16(cp: number): number;
  /** Code-point offset of UTF-16 index `i`, clamped. */
  cp(i: number): number;
  /** The string's length in UTF-16 units. */
  length: number;
}

function clamp(v: number, lo: number, hi: number): number {
  return v < lo ? lo : v > hi ? hi : v;
}

/** Build the conversion for `text`. O(1) when the text has no astral chars. */
export function codePoints(text: string): CodePoints {
  const n = text.length;
  // A high surrogate is the only way a code point can span two UTF-16 units.
  if (!/[\uD800-\uDBFF]/.test(text)) {
    const identity = (v: number) => clamp(Math.trunc(v) || 0, 0, n);
    return { u16: identity, cp: identity, length: n };
  }
  const toU16: number[] = [];
  const toCp = new Array<number>(n + 1);
  for (let i = 0; i < n; ) {
    toCp[i] = toU16.length;
    const c = text.codePointAt(i) ?? 0;
    const w = c > 0xffff ? 2 : 1;
    if (w === 2) toCp[i + 1] = toU16.length;
    toU16.push(i);
    i += w;
  }
  toCp[n] = toU16.length;
  toU16.push(n);
  return {
    u16: (cp) => toU16[clamp(Math.trunc(cp) || 0, 0, toU16.length - 1)],
    cp: (i) => toCp[clamp(Math.trunc(i) || 0, 0, n)],
    length: n,
  };
}

/**
 * Convert a code-point range read from the file into a UTF-16 range, clamped
 * and never inverted (§7). A degenerate range stays in the list as an empty
 * one: sentences, words and tokens are addressed by index, so dropping one
 * would renumber every later link.
 */
export function toRange(cps: CodePoints, r: number[] | undefined): Range {
  if (!r || r.length < 2) return [0, 0];
  const a = cps.u16(r[0]);
  const b = cps.u16(r[1]);
  return [a, b > a ? b : a];
}

/**
 * The word inventory of every sentence (§4.5): the tokens of `text` grouped by
 * the sentence range containing each token's **first** unit, so a boundary
 * falling mid-word leaves the word with the sentence it starts in. A token in a
 * gap between ranges joins the preceding sentence, which keeps word indices
 * stable (a validator reports the gap; a reader must not shift indices).
 */
export function wordsBySentence(text: string, sents: Range[]): Range[][] {
  const out: Range[][] = sents.map(() => []);
  if (sents.length === 0) return out;
  for (const tok of tokenize(text)) {
    let k = 0;
    // Last sentence whose start is at or before the token's start.
    for (let j = sents.length - 1; j >= 0; j--) {
      if (sents[j][0] <= tok[0]) {
        k = j;
        break;
      }
    }
    out[k].push(tok);
  }
  return out;
}

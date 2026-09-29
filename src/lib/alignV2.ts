// The version-2 tap algorithm: the word rung of §8.2, the derived run/split
// labelling of §8.3, and the status/verdict surfacing §6.7/§6.8 make a MUST.
//
// Version 1's counterpart is `align.ts`, which stays as it is: there a chunk
// carries its own char range, here one link per target token carries a source
// word index and the consumer regenerates the ranges with `tbook-w2`.

import { codePoints, tokenize, type CodePoints, type Range } from "./tokenize.ts";
import type { LinkV2, TranslationV2, VerdictV2 } from "./types.ts";

/** A translation's token inventory, computed on demand and cached (§5.4). */
export interface TargetSide {
  text: string;
  /** Token ranges in UTF-16 units: the `words` override, or `tbook-w2`. */
  toks: Range[];
  cps: CodePoints;
}

// Keyed by the translation object, so a chapter's other languages are never
// tokenized and the cache dies with the overlay it came from (§5.4).
const sides = new WeakMap<TranslationV2, TargetSide>();

/** The token inventory of `tr`, tokenized at most once per translation. */
export function targetSide(tr: TranslationV2): TargetSide {
  const hit = sides.get(tr);
  if (hit) return hit;
  const text = tr.t ?? "";
  const cps = codePoints(text);
  // §6.6: a translation carrying `words` MUST NOT be re-tokenized.
  // `a` indexes this list, so an override keeps every entry (§6.6).
  const toks: Range[] = tr.words
    ? tr.words.map((w) => {
        const a = cps.u16(w[0]);
        const b = cps.u16(w[1]);
        return [a, b > a ? b : a] as Range;
      })
    : tokenize(text);
  const side = { text, toks, cps };
  sides.set(tr, side);
  return side;
}

/** Does this link name source word `i`? `null` and `[]` never do (§6.4). */
function hits(link: LinkV2 | undefined, i: number): boolean {
  if (link === null || link === undefined) return false;
  return Array.isArray(link) ? link.includes(i) : link === i;
}

/** The source words a link names: empty for `null` (unaligned) and `[]`. */
export function linkWords(link: LinkV2 | undefined): number[] {
  if (link === null || link === undefined) return [];
  return Array.isArray(link) ? link : [link];
}

export interface Highlight {
  /** UTF-16 ranges of `tr.t` to mark, sorted and non-degenerate. */
  ranges: Range[];
  /** Indices of the whole tokens highlighted (what the label is derived from). */
  tokens: number[];
  /** `run` / `split` (§8.3), or a `groups` kind when the file asserts one. */
  label: string | null;
}

/**
 * §8.2 — every token whose link names source word `i` (its part spans when a
 * part of it links `i`, smallest containing wins), plus every escape chunk
 * whose indices name `i`. Offsets read from the file are code points and are
 * converted here (§7); ranges are clamped and degenerate ones dropped, because
 * alignment is model-generated and a file may carry rare imperfections.
 */
export function highlightFor(tr: TranslationV2, i: number): Highlight {
  const { toks, cps } = targetSide(tr);
  const ranges: Range[] = [];
  const tokens: number[] = [];
  (tr.a ?? []).forEach((link, j) => {
    if (j >= toks.length || !hits(link, i)) return;
    const [ta, tb] = toks[j];
    // A part's offsets are relative to its token's start, in code points.
    const parts = (tr.parts ?? []).filter((p) => p[0] === j && hits(p[3], i));
    if (parts.length > 0) {
      const base = cps.cp(ta);
      for (const p of parts) ranges.push([cps.u16(base + p[1]), cps.u16(base + p[2])]);
    } else {
      ranges.push([ta, tb]);
      tokens.push(j);
    }
  });
  for (const ch of tr.x ?? []) {
    if (ch.length >= 3 && ch.slice(2).includes(i)) ranges.push([cps.u16(ch[0]), cps.u16(ch[1])]);
  }
  const clean = ranges
    .filter(([a, b]) => b > a)
    .sort((p, q) => p[0] - q[0] || p[1] - q[1]);
  return { ranges: clean, tokens, label: labelFor(tr, i, tokens) };
}

/**
 * §8.3 — a source word rendered by several target tokens is either one run of
 * consecutive tokens or a split rendering, and a learner cannot tell them apart
 * unless the reader says so. A `groups` entry naming the word wins over the
 * derived label (§6.10).
 */
export function labelFor(tr: TranslationV2, i: number, tokens: number[]): string | null {
  for (const g of tr.groups ?? []) {
    if (Array.isArray(g) && Array.isArray(g[2]) && g[2].includes(i)) return g[0];
  }
  return label(tokens);
}

/** `run` for one block of consecutive token indices, `split` for several. */
export function label(tokens: number[]): "run" | "split" | null {
  if (tokens.length < 2) return null;
  const s = [...tokens].sort((a, b) => a - b);
  return s.every((v, k) => k === 0 || v === s[k - 1] + 1) ? "run" : "split";
}

/**
 * A non-ok status or a failed verdict on one translation. §6.7/§6.8 make
 * surfacing it a MUST: a learner must never take «СКАНДАЛ В БОГЕМИИ» under the
 * `tr` key for Turkish. Never derived by the consumer — only read from the file.
 */
export interface Flag {
  /** The producer's status, or `"flagged"` when only a verdict failed. */
  status: string;
  /** A short line for the badge. */
  text: string;
  /** `wrongLang` / `rejected`: the text may be wrong, so it is not shown. */
  hidden: boolean;
  verdict?: VerdictV2;
}

/** Statuses whose text is hidden rather than shown (§6.7 MAY). */
const HIDDEN = new Set(["wrongLang", "rejected"]);

/** The flag of a translation, or `null` when it is ok and unflagged. */
export function flagOf(tr: TranslationV2 | null | undefined): Flag | null {
  if (!tr) return null;
  const status = tr.s;
  const verdict = tr.v && tr.v.ok === false ? tr.v : undefined;
  if (!status && !verdict) return null;
  const parts: string[] = [];
  if (status) parts.push(statusText(status, tr.lang));
  if (verdict) parts.push(verdict.why ? `${verdict.by}: ${verdict.why}` : `flagged by ${verdict.by}`);
  return {
    status: status ?? "flagged",
    text: parts.join(" · "),
    hidden: status !== undefined && HIDDEN.has(status),
    verdict,
  };
}

/** Plain wording for a status. Unknown values read like `unverified` (§6.7). */
function statusText(status: string, lang?: string): string {
  switch (status) {
    case "raw":
      return "not aligned — word taps unavailable";
    case "unaligned":
      return "no word alignment";
    case "skipped":
      return "not translated (skipped)";
    case "rejected":
      return "rejected by a quality gate";
    case "unverified":
      return "unverified";
    case "wrongLang":
      return lang ? `wrong language (detected ${lang})` : "wrong language";
    case "free":
      return "free rendering — word taps are unreliable";
    default:
      return `unverified (${status})`;
  }
}

/**
 * Which quality gates ran over the overlay (§6.8 SHOULD): the context that
 * tells "checked and flagged" from "never checked". Shown with a flag only —
 * a clean cell gets no indicator at all.
 */
export function gatesText(gates: string[] | undefined): string {
  return gates && gates.length > 0 ? `checked by ${gates.join(", ")}` : "no quality checks ran";
}

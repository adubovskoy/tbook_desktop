// `alignDigest` (.tbook v2 spec §9.2): a digest over the texts of the aligned
// pairs, which the WebView recomputes from the overlay it is about to use. A
// permuted or drifted alignment passes every structural check and the entry
// digests too (when they were recomputed); this is what catches it.
//
// It lives here rather than in Rust because the pairs are made of source words
// and target tokens, and the tokenizer (`tokenize.ts`) is the WebView's. The
// entry digests of §9.1 are checked by the Rust core on every read.

import { targetSide } from "./alignV2.ts";
import type { PreparedBlock } from "./renderV2.ts";
import type { GlossParagraphV2, OverlayV2, TranslationV2 } from "./types.ts";

/** A footnote overlay `gloss/notes.<lang>.json` (§6.12). */
export interface NotesOverlayLike {
  alignDigest?: string;
  notes: Record<string, GlossParagraphV2[]>;
}

/**
 * The canonical string of §9.2 for a chapter overlay: paragraphs in entry order
 * (a table's rows, then cells), each sentence with a `t`, one line per
 * (source word, target token) pair — `[]` links as an empty word — then the
 * escape chunks. Source words and target tokens are the file's own
 * inventories: overrides kept entry for entry, the tokenizer otherwise.
 */
export function alignCanonical(ov: OverlayV2, blocks: PreparedBlock[]): string {
  let out = "";
  ov.paragraphs.forEach((gp, p) => {
    const b = blocks[p];
    if (b) out += paragraph(gp, b);
  });
  return out;
}

/** §6.12: a notes overlay digests its notes in code-point order of their ids. */
export function notesAlignCanonical(
  ov: NotesOverlayLike,
  notes: Record<string, PreparedBlock[]>,
): string {
  let out = "";
  for (const id of Object.keys(ov.notes ?? {}).sort(codePointOrder)) {
    const body = notes[id];
    if (!body) continue;
    ov.notes[id].forEach((gp, p) => {
      const b = body[p];
      if (b) out += paragraph(gp, b);
    });
  }
  return out;
}

function paragraph(gp: GlossParagraphV2, b: PreparedBlock): string {
  if (gp.rows) {
    if (!b.cells) return "";
    let out = "";
    gp.rows.forEach((row, r) =>
      row.forEach((cell, c) => {
        const pc = b.cells?.[r]?.[c];
        if (pc) out += sentences(cell.s ?? [], pc);
      }),
    );
    return out;
  }
  return sentences(gp.s ?? [], b);
}

function sentences(s: (TranslationV2 | null)[], b: PreparedBlock): string {
  const src = b.block.text ?? "";
  let out = "";
  s.forEach((tr, k) => {
    if (!tr || !tr.t || k >= b.words.length) return;
    const words = b.words[k];
    const word = (i: number) => {
      const w = words[i];
      return w ? src.slice(w[0], w[1]) : "";
    };
    const { text, toks, cps } = targetSide(tr);
    (tr.a ?? []).forEach((link, j) => {
      if (j >= toks.length || link === null || link === undefined) return;
      const tok = text.slice(toks[j][0], toks[j][1]);
      if (Array.isArray(link) && link.length === 0) {
        out += `\t${tok}\n`;
        return;
      }
      for (const i of ascending(Array.isArray(link) ? link : [link])) out += `${word(i)}\t${tok}\n`;
    });
    for (const x of tr.x ?? []) {
      if (x.length < 2) continue;
      const a = cps.u16(x[0]);
      const frag = text.slice(a, Math.max(a, cps.u16(x[1])));
      if (x.length === 2) out += `\t${frag}\n`;
      else for (const i of ascending(x.slice(2))) out += `${word(i)}\t${frag}\n`;
    }
  });
  return out;
}

function ascending(idx: number[]): number[] {
  return [...idx].sort((a, b) => a - b);
}

/** Code-point order; `<` on JS strings is UTF-16 order, which differs above U+FFFF. */
function codePointOrder(a: string, b: string): number {
  const x = [...a];
  const y = [...b];
  for (let i = 0; i < Math.min(x.length, y.length); i++) {
    const d = (x[i].codePointAt(0) ?? 0) - (y[i].codePointAt(0) ?? 0);
    if (d !== 0) return d;
  }
  return x.length - y.length;
}

/** `"sha256:<hex>"` of the UTF-8 of `text`. */
export async function sha256(text: string): Promise<string> {
  const buf = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  let hex = "";
  for (const byte of new Uint8Array(buf)) hex += byte.toString(16).padStart(2, "0");
  return `sha256:${hex}`;
}

/**
 * Why an overlay must be rejected (§9.2), or null when its links reproduce the
 * declared digest. Nothing declared, nothing to check: the field is REQUIRED
 * and a validator reports it, while verifying is only a consumer MAY.
 */
export async function alignDigestError(
  declared: string | undefined,
  canonical: string,
): Promise<string | null> {
  if (!declared) return null;
  const actual = await sha256(canonical);
  return declared.toLowerCase() === actual
    ? null
    : `its word alignment fails the integrity check (alignDigest ${declared}, links give ${actual})`;
}

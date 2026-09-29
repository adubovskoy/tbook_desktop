// `alignDigest` (§9.2) as the WebView recomputes it: the spec's own vectors
// (Annex C, the swapped S1 links, `[]` and `x` pairs, the notes order), and —
// when the migrated reference books are on this machine — every overlay of
// both, which pins this tokenizer + canonical string to the producer's.
//
// Run with `npm test`; `npm run check` type-checks it with the rest of src/.

import { alignCanonical, alignDigestError, notesAlignCanonical, sha256 } from "./integrityV2.ts";
import { prepareBlock } from "./renderV2.ts";
import { check, checkAsync, report } from "./testing.ts";
import type { LinkV2, OverlayV2, SkeletonV2 } from "./types.ts";

function eq(got: unknown, want: unknown): void {
  const g = JSON.stringify(got);
  const w = JSON.stringify(want);
  if (g !== w) throw new Error(`got ${g}, want ${w}`);
}

const S1 = "“Then I can wait in the next room.”";
const S1_RU = "«Тогда я могу подождать в соседней комнате.»";
const skeleton: SkeletonV2 = {
  id: "cbedb517c",
  paragraphs: [{ id: "04537723", text: S1, sents: [[0, 35]] }],
};
const blocks = skeleton.paragraphs.map(prepareBlock);

function overlay(a: LinkV2[], x?: number[][], alignDigest?: string): OverlayV2 {
  return {
    lang: "ru",
    chapter: "cbedb517c",
    gates: [],
    alignDigest,
    paragraphs: [{ id: "04537723", s: [{ t: S1_RU, a, x }] }],
  };
}

const ANNEX_C = "sha256:4bfa91a5b38848ba41ed5bf471da900413b0acdb6c68928dfaa74c0b6e832918";

check("§9.2 the canonical string of the Annex C overlay", () => {
  eq(
    alignCanonical(overlay([0, 1, 2, 3, 4, [5, 6], 7]), blocks),
    "Then\tТогда\nI\tя\ncan\tмогу\nwait\tподождать\nin\tв\nthe\tсоседней\nnext\tсоседней\nroom\tкомнате\n",
  );
});

check("§9.2 inserted links and escape chunks, as the reference producer writes them", () => {
  eq(alignCanonical(overlay([[], null, 2], [[1, 6, 0], [7, 8]]), blocks), "\tТогда\ncan\tмогу\nThen\tТогда\n\tя\n");
});

check("§6.12 notes digest in code-point order of their ids", () => {
  const body = { n2: blocks, n10: blocks };
  const gp = [{ id: "04537723", s: [{ t: "Тогда", a: [0] as LinkV2[] }] }];
  const gq = [{ id: "04537723", s: [{ t: "я", a: [1] as LinkV2[] }] }];
  // "n10" < "n2" by code point, whatever order the object lists them in.
  eq(notesAlignCanonical({ notes: { n2: gp, n10: gq } }, body), "I\tя\nThen\tТогда\n");
});

await checkAsync("§9.2 the Annex C digest, and the swapped links' digest", async () => {
  eq(await sha256(alignCanonical(overlay([0, 1, 2, 3, 4, [5, 6], 7]), blocks)), ANNEX_C);
  eq(
    await sha256(alignCanonical(overlay([7, 1, 2, 3, 4, [5, 6], 0]), blocks)),
    "sha256:f7552db092fe0c64ff64ab2d69079e20a36e4fb3f9efd8c51dd6ab99e5e24d7b",
  );
});

await checkAsync("B.3 permuted links are rejected; a matching or absent digest passes", async () => {
  const permuted = overlay([7, 1, 2, 3, 4, [5, 6], 0], undefined, ANNEX_C);
  const why = await alignDigestError(permuted.alignDigest, alignCanonical(permuted, blocks));
  if (!why || !why.includes("integrity")) throw new Error(`not rejected: ${why}`);
  const honest = overlay([0, 1, 2, 3, 4, [5, 6], 7], undefined, ANNEX_C);
  eq(await alignDigestError(honest.alignDigest, alignCanonical(honest, blocks)), null);
  eq(await alignDigestError(undefined, "anything"), null);
});

// --- the migrated reference books ----------------------------------------

const FIXTURES =
  "/home/adubovskoy/.claude/projects/-home-adubovskoy-Develop-reader-doc/" +
  "9771d31c-1b9b-41bf-b2fe-cc8ab066d401/panel/out/v2/";

/** Entry name → bytes of a ZIP, just enough of the format for these books. */
async function readZip(path: string): Promise<Map<string, Uint8Array> | null> {
  // Node-only modules, imported by computed name so the WebView type-check
  // (which has no Node types) sees `any`.
  const fs = await import(["node", "fs"].join(":"));
  const zlib = await import(["node", "zlib"].join(":"));
  if (!fs.existsSync(path)) return null;
  const buf: Uint8Array = fs.readFileSync(path);
  const dv = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
  let eocd = buf.length - 22;
  while (eocd >= 0 && dv.getUint32(eocd, true) !== 0x06054b50) eocd--;
  const count = dv.getUint16(eocd + 10, true);
  let at = dv.getUint32(eocd + 16, true);
  const out = new Map<string, Uint8Array>();
  for (let n = 0; n < count; n++) {
    const method = dv.getUint16(at + 10, true);
    const size = dv.getUint32(at + 20, true);
    const nameLen = dv.getUint16(at + 28, true);
    const skip = nameLen + dv.getUint16(at + 30, true) + dv.getUint16(at + 32, true);
    const local = dv.getUint32(at + 42, true);
    const name = new TextDecoder().decode(buf.subarray(at + 46, at + 46 + nameLen));
    const data = local + 30 + dv.getUint16(local + 26, true) + dv.getUint16(local + 28, true);
    const raw = buf.subarray(data, data + size);
    out.set(name, method === 8 ? zlib.inflateRawSync(raw) : raw);
    at += 46 + skip;
  }
  return out;
}

for (const book of ["sample-v2.tbook", "kimi-v2.tbook"]) {
  await checkAsync(`every overlay of ${book} reproduces its alignDigest`, async () => {
    const zip = await readZip(FIXTURES + book);
    if (!zip) return; // not on this machine
    const json = (name: string) => JSON.parse(new TextDecoder().decode(zip.get(name)!));
    const manifest = json("manifest.json");
    let checked = 0;
    for (const ch of manifest.spine) {
      const prepared = (json(ch.text) as SkeletonV2).paragraphs.map(prepareBlock);
      for (const entry of Object.values(ch.gloss) as string[]) {
        const ov = json(entry) as OverlayV2;
        const why = await alignDigestError(ov.alignDigest, alignCanonical(ov, prepared));
        if (why) throw new Error(`${entry}: ${why}`);
        checked++;
      }
    }
    if (checked === 0) throw new Error("no overlays checked");
  });
}

report();

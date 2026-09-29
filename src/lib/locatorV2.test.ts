// Version-2 reading positions as locators (§3.5.2) and version-2 footnote
// markers (§4.8, §4.12) on the page and in bilingual mode.
//
// Run with `npm test`; `npm run check` type-checks it with the rest of src/.

import { legacyLocator, locatorFor, withinParagraph } from "./locatorV2.ts";
import { paragraphHTML, wordAt } from "./render.ts";
import { buildBlockRender, prepareBlock, tapResult } from "./renderV2.ts";
import { check, report } from "./testing.ts";
import type { BlockV2, TranslationV2 } from "./types.ts";

function eq(got: unknown, want: unknown): void {
  const g = JSON.stringify(got);
  const w = JSON.stringify(want);
  if (g !== w) throw new Error(`got ${g}, want ${w}`);
}

const s9: BlockV2 = {
  id: "781304bb",
  text: "“‘Oh, at his new offices. He did tell me the address.",
  sents: [
    [0, 23],
    [23, 53],
  ],
};

check("§3.5.2 a paragraph's locator names its first word", () => {
  eq(locatorFor("cbedb517c", prepareBlock(s9)), "cbedb517c/781304bb/0/0");
});

check("§3.5.2 a wordless paragraph is a paragraph locator; no paragraph, the chapter", () => {
  const sceneBreak = prepareBlock({ id: "aa", role: "sceneBreak", text: "", sents: [] });
  eq(locatorFor("c1", sceneBreak), "c1/aa");
  eq(locatorFor("c1", undefined), "c1");
  eq(locatorFor("c1", prepareBlock({ text: "x", sents: [[0, 1]] })), "c1");
});

check("§3.5.2 a stored word that is not there is dropped, never moved", () => {
  const b = prepareBlock(s9);
  // Sentence 1 is "He did tell me the address." — six words.
  eq(withinParagraph(b, 1, 5), { sentence: 1, word: 5 });
  eq(withinParagraph(b, 1, 6), { sentence: 1, word: null });
  eq(withinParagraph(b, 2, 0), { sentence: null, word: null });
  eq(withinParagraph(b, null, 3), { sentence: null, word: null });
  eq(withinParagraph(undefined, 0, 0), { sentence: null, word: null });
});

check("a position saved as two ids still reads as a locator", () => {
  eq(legacyLocator("c1", "p1"), "c1/p1");
  eq(legacyLocator("c1", undefined), undefined);
});

// --- footnote markers ------------------------------------------------------

const s7: BlockV2 = {
  id: "ea90ea77",
  text: "The two attendants brought up the rear. Then silence.",
  sents: [
    [0, 39],
    [40, 53],
  ],
  notes: [
    { p: 39, id: "n1", label: "1" },
    { p: 3, id: "n9", label: "*" },
  ],
};
const known = new Set(["n1"]);

check("§4.8 a marker's label is spliced in at its point; an unknown note id is ignored", () => {
  const r = buildBlockRender(prepareBlock(s7), null, false, known);
  eq(r.text, "The two attendants brought up the rear.1 Then silence.");
  eq(r.notes, [{ start: 39, end: 40, id: "n1", label: "1" }]);
  // Words after the label shift past it and still resolve to themselves.
  const then = wordAt(r, r.text.indexOf("Then"));
  eq([then?.sentenceIndex, then?.wordIndex], [1, 0]);
  eq(r.text.slice(then!.start, then!.end), "Then");
  if (!paragraphHTML(r).includes('<sup class="note" data-note="n1">1</sup>')) {
    throw new Error("marker is not a tappable superscript");
  }
});

check("§4.8 without known notes the text stays verbatim (§4.5)", () => {
  eq(buildBlockRender(prepareBlock(s7)).text, s7.text);
});

check("§4.8 in bilingual mode the label stays with its sentence, before the gloss", () => {
  const trs: TranslationV2[] = [
    { t: "Двое сопровождающих замыкали шествие.", a: [1, [0, 2], 2] },
    { t: "Затем тишина.", a: [0, 1] },
  ];
  const r = buildBlockRender(prepareBlock(s7), trs, true, known);
  eq(
    r.text,
    "The two attendants brought up the rear.1 Двое сопровождающих замыкали шествие. Then silence. Затем тишина.",
  );
  const rear = wordAt(r, r.text.indexOf("rear"));
  eq(r.text.slice(rear!.start, rear!.end), "rear");
  const silence = wordAt(r, r.text.indexOf("silence"));
  eq([silence?.sentenceIndex, silence?.wordIndex, silence?.gloss ?? false], [1, 1, false]);
  eq(r.notes.map((m) => r.text.slice(m.start, m.end)), ["1"]);
});

check("§6.12 a word in a note body taps like one on the page", () => {
  const note = prepareBlock({
    id: "ea90ea77",
    text: "The two attendants brought up the rear.",
    sents: [[0, 39]],
  });
  const cell = tapResult(note, [{ t: "Двое сопровождающих замыкали шествие.", a: [1, [0, 2], 2], s: "free" }], 0, 2, ["judge"]);
  eq(cell?.word, "attendants");
  eq(cell?.ranges.map(([a, b]) => cell.translation!.slice(a, b)), ["сопровождающих", "замыкали"]);
  eq(cell?.flag?.status, "free");
  eq(cell?.gates, ["judge"]);
});

report();

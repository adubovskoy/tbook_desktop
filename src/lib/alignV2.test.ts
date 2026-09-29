// The version-2 word rung (§8.2), derived labelling (§8.3), status surfacing
// (§6.7/§6.8) and paragraph rendering (§4.5), against the expected taps of
// Annex B.2 and the worked examples of §6.
//
// Run with `npm test`; `npm run check` type-checks it with the rest of src/.

import { flagOf, gatesText, highlightFor } from "./alignV2.ts";
import { buildBlockRender, prepareBlock, tapResult } from "./renderV2.ts";
import { check, report } from "./testing.ts";
import type { BlockV2, TranslationV2 } from "./types.ts";

/** The substrings a tap on source word `i` highlights. */
function marks(tr: TranslationV2, i: number): string[] {
  return highlightFor(tr, i).ranges.map(([a, b]) => (tr.t ?? "").slice(a, b));
}

// --- Annex B.2, one link per target token -------------------------------

// S1 “Then I can wait in the next room.”
//    words: 0 Then, 1 I, 2 can, 3 wait, 4 in, 5 the, 6 next, 7 room
const s1ru: TranslationV2 = {
  t: "«Тогда я могу подождать в соседней комнате.»",
  a: [0, 1, 2, 3, 4, [5, 6], 7],
};

check("B.2 S1 wait → «подождать»", () => {
  eq(marks(s1ru, 3), ["подождать"]);
  eq(highlightFor(s1ru, 3).label, null);
});

check("B.2 S1 one token for two words: the / next → «соседней»", () => {
  eq(marks(s1ru, 5), ["соседней"]);
  eq(marks(s1ru, 6), ["соседней"]);
  eq(marks(s1ru, 7), ["комнате"]);
});

check("an unlinked word highlights nothing", () => {
  eq(marks({ t: "Тогда я", a: [null, []] }, 0), []);
});

// §6.11 — parts refine a fan-in: tapping `I`, `can` and `wait` must not all
// light up the whole agglutinated verb.
const s1tr: TranslationV2 = {
  t: "“O zaman bitişik odada bekleyebilirim.”",
  a: [0, 0, [5, 6], [4, 7], [1, 2, 3]],
  parts: [
    [4, 0, 5, 3],
    [4, 5, 10, 2],
    [4, 10, 12, []],
    [4, 12, 14, 1],
  ],
};

check("§6.11 parts: smallest containing span wins", () => {
  eq(marks(s1tr, 3), ["bekle"]);
  eq(marks(s1tr, 2), ["yebil"]);
  eq(marks(s1tr, 1), ["im"]);
  // A token without parts is highlighted whole.
  eq(marks(s1tr, 5), ["bitişik"]);
});

check("§6.11 without parts the whole token lights up", () => {
  const noParts: TranslationV2 = { t: s1tr.t, a: s1tr.a };
  eq(marks(noParts, 3), ["bekleyebilirim"]);
});

// S2 “That is just my point.” words: 0 That, 1 is, 2 just, 3 my, 4 point
const s2ru: TranslationV2 = {
  t: "В этом-то всё и дело.",
  a: [1, [0, 1, 2], 2, 1, [3, 4]],
};

check("B.2 S2 just → «этом-то» «всё», labelled a run", () => {
  eq(marks(s2ru, 2), ["этом-то", "всё"]);
  eq(highlightFor(s2ru, 2).label, "run");
});

check("B.2 S2 is → «В» «этом-то» «и», labelled a split", () => {
  eq(marks(s2ru, 1), ["В", "этом-то", "и"]);
  eq(highlightFor(s2ru, 1).label, "split");
});

// S5 “He doesn’t look a credit to the Bow Street cells, does he?”
//    words: 0 He, 1 doesn’t, 2 look, … 10 does, 11 he
const s5de: TranslationV2 = {
  t: "„Er sieht nicht gerade wie eine Zierde für die Bow-Street-Zellen aus, was?“",
  a: [0, 2, 1, null, null, 3, 4, 5, 6, [7, 8, 9], 2, [10, 11]],
};

check("B.2 S5 look → «sieht … aus», a split rendering", () => {
  eq(marks(s5de, 2), ["sieht", "aus"]);
  eq(highlightFor(s5de, 2).label, "split");
});

check("§6.10 a groups entry names the split instead", () => {
  const withGroup: TranslationV2 = { ...s5de, groups: [["lexeme", [1, 10], [2]]] };
  eq(highlightFor(withGroup, 2).label, "lexeme");
});

check("§6.4 an unaligned token is never highlighted", () => {
  // «gerade» and «wie» carry null links: no tap on any source word may light
  // them up, and a word index no link names highlights nothing at all.
  for (let i = 0; i < 12; i++) {
    const hit = marks(s5de, i);
    if (hit.includes("gerade") || hit.includes("wie")) {
      throw new Error(`word ${i} lit an unaligned token: ${hit.join(", ")}`);
    }
  }
  eq(marks(s5de, 99), []);
});

// §6.5 — an escape chunk carries a span that is not exactly one token.
const s6ru: TranslationV2 = {
  t: "«Всё же, если бы я вышла замуж за лорда Сент-Саймона, я, конечно, исполнила бы свой долг перед ним.",
  a: [0, 0, 1, 3, 2, 4, 4, 4, 5, null, 10, [8, 9], [10, 12], [10, 11], 13, 14, 15, 16],
  x: [
    [40, 44, 6],
    [45, 52, 7],
  ],
};

check("§6.5 escape chunks highlight sub-token spans", () => {
  eq(marks(s6ru, 6), ["Сент"]);
  eq(marks(s6ru, 7), ["Саймона"]);
});

// §6.6 — a target the tokenizer cannot segment carries its own token list.
check("§6.6 a words override replaces the tokenizer", () => {
  const zh: TranslationV2 = {
    t: "所以基本上你是在给你老爸施压",
    words: [
      [0, 2],
      [2, 5],
      [5, 6],
    ],
    a: [0, 1, 2],
  };
  eq(marks(zh, 1), ["基本上"]);
  eq(marks(zh, 2), ["你"]);
});

// --- Statuses and verdicts (§6.7, §6.8) ---------------------------------

check("§6.7 a free rendering is flagged but still shown", () => {
  const s7ru: TranslationV2 = {
    t: "Двое сопровождающих замыкали шествие.",
    a: [1, [0, 2], 2],
    s: "free",
    v: { ok: false, by: "judge", why: "free rendering; brought/rear have no counterpart" },
  };
  const flag = flagOf(s7ru);
  if (!flag) throw new Error("a free rendering must be flagged");
  eq(flag.status, "free");
  eq(flag.hidden, false);
  if (!flag.text.includes("judge")) throw new Error(`verdict not surfaced: ${flag.text}`);
  // B.2: `attendants` is a run, `brought` and `rear` highlight nothing.
  eq(marks(s7ru, 2), ["сопровождающих", "замыкали"]);
  eq(highlightFor(s7ru, 2).label, "run");
  eq(marks(s7ru, 3), []);
  eq(marks(s7ru, 6), []);
});

check("§6.7 wrongLang hides the text and names the language", () => {
  const s8tr: TranslationV2 = { t: "СКАНДАЛ В БОГЕМИИ", s: "wrongLang", lang: "ru" };
  const flag = flagOf(s8tr);
  if (!flag) throw new Error("wrongLang must be flagged");
  eq(flag.hidden, true);
  if (!flag.text.includes("ru")) throw new Error(`detected language not named: ${flag.text}`);
});

check("§6.7 an unknown status is surfaced like unverified", () => {
  const flag = flagOf({ t: "x", s: "x-weird" });
  if (!flag) throw new Error("an unknown status must still be flagged");
  if (!flag.text.includes("unverified")) throw new Error(flag.text);
  eq(flag.hidden, false);
});

check("§6.7 an ok translation carries no flag", () => {
  eq(flagOf(s1ru), null);
  eq(flagOf({ t: "x", v: { ok: true, by: "judge" } }), null);
});

// --- Paragraph rendering (§4.5) and the tap path (§8.1) ------------------

const s9: BlockV2 = {
  id: "781304bb",
  text: "“‘Oh, at his new offices. He did tell me the address. Yes, 17 King Edward Street, near St. Paul’s.’",
  sents: [
    [0, 23],
    [23, 51],
    [51, 88],
    [88, 96],
    [96, 99],
  ],
};

check("§4.5 a paragraph renders its text verbatim", () => {
  const render = buildBlockRender(prepareBlock(s9));
  eq(render.text, s9.text);
  eq(render.role, "body");
  // Word spans address (sentence, word) exactly as the links do.
  const first = render.spans[0];
  eq(s9.text.slice(first.start, first.end), "Oh");
  eq(first.sentenceIndex, 0);
  eq(first.wordIndex, 0);
});

check("§8.1 a tap resolves word, sentence and translation", () => {
  const prepared = prepareBlock(s9);
  const trs: (TranslationV2 | null)[] = [
    { t: "«О, в его новом офисе", a: [0, 1, 2, 3, 4] },
    { t: ". Он действительно сказал мне адрес", a: [0, 1, [1, 2], 3, [4, 5]] },
    { t: ". Да, Кинг-Эдвард-стрит, 17, рядом с С", a: [0, [2, 3, 4], 1, 5, 5, 6] },
    { t: ". Павлом»", a: [0] },
    { t: ".»" },
  ];
  // Annex A.7: `offices` → «офисе», `He` → «Он», `17` → «17», `Paul’s` → «Павлом».
  const offices = tapResult(prepared, trs, 0, 4);
  eq(offices?.word, "offices");
  eq(offices?.translation, "«О, в его новом офисе");
  eq(marksOf(offices), ["офисе"]);
  eq(marksOf(tapResult(prepared, trs, 1, 0)), ["Он"]);
  eq(marksOf(tapResult(prepared, trs, 2, 1)), ["17"]);
  eq(marksOf(tapResult(prepared, trs, 3, 0)), ["Павлом"]);
  // Sentence 4 has no words at all.
  eq(tapResult(prepared, trs, 4, 0)?.word, "");
});

check("§8.1 a null cell is 'no translation', not an error", () => {
  const prepared = prepareBlock(s9);
  const hit = tapResult(prepared, [null, null, null, null, null], 0, 0);
  eq(hit?.word, "Oh");
  eq(hit?.translation, null);
  eq(hit?.flag, null);
});

check("bilingual mode splices the gloss in and keeps words tappable", () => {
  const block: BlockV2 = {
    id: "04537723",
    text: "“Then I can wait in the next room.”",
    sents: [[0, 35]],
  };
  const render = buildBlockRender(prepareBlock(block), [s1ru], true);
  if (!render.text.startsWith(block.text)) throw new Error(render.text);
  if (!render.text.includes(s1ru.t ?? "")) throw new Error("gloss not interleaved");
  eq(render.glossRuns.length, 1);
  const [a, b] = render.glossRuns[0];
  eq(render.text.slice(a, b), s1ru.t);
  // Both sides are tappable: source words, then the translation's tokens.
  const source = render.spans.filter((s) => !s.gloss);
  eq(source.length, 8);
  eq(render.text.slice(source[3].start, source[3].end), "wait");
  const glossSpans = render.spans.filter((s) => s.gloss);
  eq(render.text.slice(glossSpans[3].start, glossSpans[3].end), "подождать");
  eq(glossSpans[3].alignedWords, [3]);
});

check("§6.7 a hidden status keeps its text off the page", () => {
  const block: BlockV2 = { id: "0f4552b1", text: "A SCANDAL IN BOHEMIA", sents: [[0, 20]] };
  const wrong: TranslationV2 = { t: "СКАНДАЛ В БОГЕМИИ", s: "wrongLang", lang: "ru" };
  const render = buildBlockRender(prepareBlock(block), [wrong], true);
  eq(render.text, block.text);
  eq(render.glossRuns.length, 0);
  // A shown-but-flagged one is marked instead.
  const free: TranslationV2 = { t: "СКАНДАЛ В БОГЕМИИ", s: "free" };
  const marked = buildBlockRender(prepareBlock(block), [free], true);
  eq(marked.flaggedRuns?.length, 1);
  eq(marked.flaggedRuns?.[0], marked.glossRuns[0]);
});

check("§4.4 a sceneBreak block renders the ornament, an unknown role reads as body", () => {
  eq(buildBlockRender(prepareBlock({ text: "", sents: [], role: "sceneBreak" })).text, "* * *");
  eq(buildBlockRender(prepareBlock({ text: "x", sents: [[0, 1]], role: "x-odd" })).role, "body");
});

check("§4.6 a source words override replaces the tokenizer", () => {
  const block: BlockV2 = {
    text: "所以基本上你是",
    sents: [[0, 7]],
    words: [
      [
        [0, 2],
        [2, 5],
      ],
    ],
  };
  const render = buildBlockRender(prepareBlock(block));
  eq(render.spans.length, 2);
  eq(block.text.slice(render.spans[1].start, render.spans[1].end), "基本上");
});

function marksOf(hit: { translation: string | null; ranges: Array<[number, number]> } | null) {
  if (!hit || hit.translation === null) return [];
  const t = hit.translation;
  return hit.ranges.map(([a, b]) => t.slice(a, b));
}

function eq(got: unknown, want: unknown): void {
  const a = JSON.stringify(got);
  const b = JSON.stringify(want);
  if (a !== b) throw new Error(`got ${a}, want ${b}`);
}

check("§6.8 a flag says which gates ran, or that none did", () => {
  eq(gatesText(["langcheck", "judge"]), "checked by langcheck, judge");
  eq(gatesText([]), "no quality checks ran");
  eq(gatesText(undefined), "no quality checks ran");
});

report();

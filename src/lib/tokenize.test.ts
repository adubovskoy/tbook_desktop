// Conformance vectors for the version-2 tokenizer and offset conversion:
// v2 spec §5.3, Annex B.6, §7 and the sentence grouping of §4.5.
//
// Run with `npm test` (Node runs the TypeScript directly); `npm run check`
// type-checks it along with the rest of src/.

import { check, report } from "./testing.ts";
import { codePoints, tokenStrings, wordsBySentence, type Range } from "./tokenize.ts";

// §5.3 — the spec's own table.
check("§5.3 English contractions and abbreviations", () => {
  eq(tokenStrings("“That’s the worst of it, Mr. Holmes, I don’t know.”"), [
    "That’s",
    "the",
    "worst",
    "of",
    "it",
    "Mr",
    "Holmes",
    "I",
    "don’t",
    "know",
  ]);
});

check("§5.3 digits are words, currency signs are not", () => {
  eq(tokenStrings("“‘Is £ 4 a week."), ["Is", "4", "a", "week"]);
});

check("§5.3 Russian hyphen joiner", () => {
  eq(tokenStrings("В этом-то всё и дело."), ["В", "этом-то", "всё", "и", "дело"]);
});

check("§5.3 German compound with hyphens", () => {
  eq(
    tokenStrings("„Er sieht nicht gerade wie eine Zierde für die Bow-Street-Zellen aus, was?“"),
    [
      "Er",
      "sieht",
      "nicht",
      "gerade",
      "wie",
      "eine",
      "Zierde",
      "für",
      "die",
      "Bow-Street-Zellen",
      "aus",
      "was",
    ],
  );
});

check("§5.3 a translated street name", () => {
  eq(tokenStrings(". Да, Кинг-Эдвард-стрит, 17, рядом с С"), [
    "Да",
    "Кинг-Эдвард-стрит",
    "17",
    "рядом",
    "с",
    "С",
  ]);
});

// Annex B.6 — one token each, or none at all.
check("Annex B.6 single-token vectors", () => {
  for (const s of ["Bow-Street-Zellen", "30,000", "5:15", "l'avons", "don’t", "этом-то"]) {
    eq(tokenStrings(s), [s]);
  }
});

check("Annex B.6 punctuation and symbols are never tokens", () => {
  for (const s of ["£", "—", "…", "", " ,. "]) eq(tokenStrings(s), []);
});

check("Annex B.6 an abbreviation's period does not join", () => {
  eq(tokenStrings("St. Paul’s"), ["St", "Paul’s"]);
});

check("a numeric separator joins only before a digit", () => {
  eq(tokenStrings("3.14 and 1888'di, §7"), ["3.14", "and", "1888'di", "7"]);
});

// §7 — offsets in the file are code points; JS strings are UTF-16.
check("§7 code-point offsets convert to UTF-16 indices", () => {
  const plain = codePoints("«Тогда я»");
  eq(plain.u16(1), 1);
  eq(plain.u16(99), 9); // clamped into the string

  const astral = "a𝒳b"; // 𝒳 is one code point, two UTF-16 units
  const cps = codePoints(astral);
  eq(cps.u16(0), 0);
  eq(cps.u16(1), 1);
  eq(cps.u16(2), 3);
  eq(cps.u16(3), 4);
  eq(cps.cp(3), 2);
  eq(astral.slice(cps.u16(1), cps.u16(2)), "𝒳");
});

check("tokens of astral text keep native units", () => {
  eq(tokenStrings("x 𝒳y z"), ["x", "𝒳y", "z"]);
});

// §4.5 — a sentence boundary may fall inside a word; the word stays with the
// sentence it starts in (the S9 paragraph of the spec).
check("§4.5 a mid-word boundary keeps the word in the sentence it starts in", () => {
  const text =
    "“‘Oh, at his new offices. He did tell me the address. Yes, 17 King Edward Street, near St. Paul’s.’";
  const sents: Range[] = [
    [0, 23],
    [23, 51],
    [51, 88],
    [88, 96],
    [96, 99],
  ];
  const words = wordsBySentence(text, sents);
  const str = (k: number) => words[k].map(([a, b]) => text.slice(a, b));
  eq(str(0), ["Oh", "at", "his", "new", "offices"]);
  eq(str(1), ["He", "did", "tell", "me", "the", "address"]);
  eq(str(2), ["Yes", "17", "King", "Edward", "Street", "near", "St"]);
  eq(str(3), ["Paul’s"]);
  eq(str(4), []);
});

function eq(got: unknown, want: unknown): void {
  const a = JSON.stringify(got);
  const b = JSON.stringify(want);
  if (a !== b) throw new Error(`got ${a}, want ${b}`);
}

report();

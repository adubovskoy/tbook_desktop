// Reading positions of version-2 books as locators (.tbook v2 spec §3.5.2):
// `chapterId/paragraphId/sentence/word`, content-derived ids that survive a
// re-conversion renumbering chapters or paragraphs. The Rust core resolves the
// ids (it holds every skeleton); the WebView writes locators and checks the
// sentence/word part against its own tokenization.

import type { PreparedBlock } from "./renderV2.ts";

/**
 * The locator of a paragraph: its first word (`…/k/0`, `k` the first sentence
 * that has words), just `chapterId/paragraphId` when it has none, or the
 * chapter alone when there is no paragraph (the cover page or the chapter
 * title, which the reader puts before the chapter's own text).
 */
export function locatorFor(chapterId: string, block: PreparedBlock | undefined): string {
  const id = block?.block.id;
  if (!block || !id) return chapterId;
  const k = block.words.findIndex((w) => w.length > 0);
  return k < 0 ? `${chapterId}/${id}` : `${chapterId}/${id}/${k}/0`;
}

/**
 * A resolved sentence/word checked against the paragraph's word inventory:
 * an index that is not there is dropped, never mapped to another word.
 */
export function withinParagraph(
  block: PreparedBlock | undefined,
  sentence: number | null | undefined,
  word: number | null | undefined,
): { sentence: number | null; word: number | null } {
  const words = block?.words;
  if (!words || sentence == null || sentence >= words.length) return { sentence: null, word: null };
  return { sentence, word: word != null && word < words[sentence].length ? word : null };
}

/** The locator a position saved before locators existed still names. */
export function legacyLocator(chapterId?: string, paragraphId?: string): string | undefined {
  return chapterId && paragraphId ? `${chapterId}/${paragraphId}` : undefined;
}

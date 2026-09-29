// Typed wrappers over the Rust Tauri commands.

import { invoke } from "@tauri-apps/api/core";
import type {
  BookSummary,
  Chapter,
  Locator,
  Note,
  NotesOverlayV2,
  NoteV2,
  OpenedBook,
  OverlayV2,
  SkeletonV2,
} from "./types";
import type { Accent } from "./settings";

export const listBooks = () => invoke<BookSummary[]>("list_books");
export const openBook = (id: string) => invoke<OpenedBook>("open_book", { id });
export const readChapter = (id: string, file: string) =>
  invoke<Chapter>("read_chapter", { id, file });
/** A version-2 chapter skeleton — the language-free text (v2 spec §4). */
export const readSkeleton = (id: string, chapter: number) =>
  invoke<SkeletonV2>("read_skeleton", { id, chapter });
/**
 * One version-2 overlay, already bound to its skeleton by the Rust side (§6.1).
 * `null` means this chapter has no overlay in that language; a rejection means
 * one exists but does not bind, and the chapter is shown without a translation.
 */
export const readOverlay = (id: string, chapter: number, lang: string) =>
  invoke<OverlayV2 | null>("read_overlay", { id, chapter, lang });
/**
 * Resolve a saved version-2 reading position (§3.5.2): the chapter id, falling
 * back to the paragraph id across the whole book; degrading to the chapter's
 * start, then to the book's start — never to a wrong word.
 */
export const resolveLocator = (id: string, locator: string) =>
  invoke<Locator>("resolve_locator", { id, locator });
/** A version-2 book's footnote bodies (§4.12), or null when it has none. */
export const readNotesV2 = (id: string) =>
  invoke<Record<string, NoteV2> | null>("read_notes_v2", { id });
/**
 * A version-2 footnote overlay (§6.12), bound note by note by the Rust side.
 * `null` = no footnote overlay in that language; a rejection = one that does
 * not bind or fails its digest.
 */
export const readNotesOverlay = (id: string, lang: string) =>
  invoke<NotesOverlayV2 | null>("read_notes_overlay", { id, lang });
export const bookTexts = (id: string) => invoke<string[][]>("book_texts", { id });
export const bookNotes = (id: string) =>
  invoke<Record<string, Note> | null>("book_notes", { id });
export const bookImage = (id: string, entry: string) =>
  invoke<string>("book_image", { id, entry });
export const coverDataUrl = (id: string) => invoke<string | null>("cover_data_url", { id });
export const importBook = (path: string) => invoke<BookSummary>("import_book", { path });
export const deleteBook = (id: string) => invoke<void>("delete_book", { id });
/** English IPA for a word, transcribed in the reader's chosen accent. */
export const ipaFor = (word: string, accent: Accent) =>
  invoke<string | null>("ipa_for", { word, accent });

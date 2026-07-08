// Typed wrappers over the Rust Tauri commands.

import { invoke } from "@tauri-apps/api/core";
import type { BookSummary, Chapter, Note, OpenedBook } from "./types";

export const listBooks = () => invoke<BookSummary[]>("list_books");
export const openBook = (id: string) => invoke<OpenedBook>("open_book", { id });
export const readChapter = (id: string, file: string) =>
  invoke<Chapter>("read_chapter", { id, file });
export const bookTexts = (id: string) => invoke<string[][]>("book_texts", { id });
export const bookNotes = (id: string) =>
  invoke<Record<string, Note> | null>("book_notes", { id });
export const bookImage = (id: string, entry: string) =>
  invoke<string>("book_image", { id, entry });
export const coverDataUrl = (id: string) => invoke<string | null>("cover_data_url", { id });
export const importBook = (path: string) => invoke<BookSummary>("import_book", { path });
export const deleteBook = (id: string) => invoke<void>("delete_book", { id });
export const ipaFor = (word: string) => invoke<string | null>("ipa_for", { word });

// Offline dictionaries: typed wrappers over the Rust `dict_*` commands, plus the
// pure state derivation the Dictionaries screen renders.
//
// Port of `data/dict/DictionaryModels.kt` + `LibraryPairs.kt` +
// `ui/dictionaries/DictionariesUiState.kt`. The row building is kept pure here
// (as it is on Android, for the same reason) so it can be reasoned about and
// tested without a screen.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { BookSummary } from "./types";

export interface RemoteDict {
  id: string;
  source: string;
  target: string;
  name: string;
  version: number;
  minFormatVersion: number;
  /** Decompressed size of the SQLite file. */
  sizeBytes: number;
  /** Size of the gzip file as transferred (the progress denominator). */
  downloadSizeBytes: number;
  sha256: string;
  url: string;
  entryCount: number;
  license: string;
  attribution: string;
}

export interface InstalledDict {
  id: string;
  source: string;
  target: string;
  name: string;
  version: number;
  sizeBytes: number;
  license: string;
  attribution: string;
}

export interface DictSense {
  gloss: string;
  translations: string[];
  examples: string[];
}

export interface DictEntry {
  lemma: string;
  pos: string | null;
  gender: string | null;
  ipa: string | null;
  senses: DictSense[];
}

export interface ManifestResult {
  /** "fresh" (just fetched), "cached" (offline) or "unavailable". */
  status: "fresh" | "cached" | "unavailable";
  message: string;
  dictionaries: RemoteDict[];
}

export type DownloadPhase = "downloading" | "installing" | "failed" | "done" | "cancelled";

export interface Progress {
  id: string;
  phase: DownloadPhase;
  bytesRead: number;
  totalBytes: number;
  message: string;
}

export const dictManifest = () => invoke<ManifestResult>("dict_manifest");
export const dictInstalled = () => invoke<InstalledDict[]>("dict_installed");
export const dictInstalledFor = (source: string, target: string) =>
  invoke<InstalledDict | null>("dict_installed_for", { source, target });
export const dictDownload = (id: string) => invoke<void>("dict_download", { id });
export const dictCancel = (id: string) => invoke<void>("dict_cancel", { id });
export const dictDelete = (id: string) => invoke<void>("dict_delete", { id });

/**
 * Article(s) for a tapped word. `null` means no dictionary is installed for the
 * pair (the reader offers a download); an empty array means one is, but the word
 * isn't in it.
 */
export const dictLookup = (source: string, target: string, word: string) =>
  invoke<DictEntry[] | null>("dict_lookup", { source, target, word });

/** Subscribe to download progress. Downloads outlive the screen, so does this. */
export const onDictProgress = (handler: (p: Progress) => void): Promise<UnlistenFn> =>
  listen<Progress>("dict-progress", (event) => handler(event.payload));

/** "en-US" → "en": dictionaries are keyed by base language subtags. */
export function baseLang(code: string): string {
  return code.split("-")[0].trim().toLowerCase();
}

/**
 * The language pairs the library actually needs dictionaries for: one pair per
 * (book source language → each of its gloss languages). Filters the download
 * list, so a reader of English books isn't offered a German→Russian dictionary.
 */
export function libraryPairs(books: BookSummary[]): Set<string> {
  const pairs = new Set<string>();
  for (const book of books) {
    const src = baseLang(book.sourceLang);
    if (!src) continue;
    for (const target of book.targetLangs) {
      const t = baseLang(target);
      if (t && t !== src) pairs.add(`${src}-${t}`);
    }
  }
  return pairs;
}

/** One row of the Dictionaries screen. */
export interface DictRow {
  id: string;
  source: string;
  target: string;
  name: string;
  /** Null for an installed dictionary the catalog no longer offers. */
  remote: RemoteDict | null;
  /** Null when not yet downloaded. */
  installed: InstalledDict | null;
  download: Progress | null;
  updateAvailable: boolean;
}

/**
 * Rows = catalog dictionaries whose pair occurs in the library, plus every
 * installed dictionary (even orphaned ones — still shown so they can be
 * deleted), joined with download progress.
 */
export function buildRows(
  available: RemoteDict[],
  installed: InstalledDict[],
  pairs: Set<string>,
  downloads: Record<string, Progress>,
): DictRow[] {
  const installedById = new Map(installed.map((d) => [d.id, d]));
  const rows: DictRow[] = [];
  const offeredIds = new Set<string>();
  for (const remote of available) {
    if (!pairs.has(`${baseLang(remote.source)}-${baseLang(remote.target)}`)) continue;
    const own = installedById.get(remote.id) ?? null;
    offeredIds.add(remote.id);
    rows.push({
      id: remote.id,
      source: remote.source,
      target: remote.target,
      name: remote.name,
      remote,
      installed: own,
      download: downloads[remote.id] ?? null,
      updateAvailable: own !== null && remote.version > own.version,
    });
  }
  for (const own of installed) {
    if (offeredIds.has(own.id)) continue;
    rows.push({
      id: own.id,
      source: own.source,
      target: own.target,
      name: own.name,
      remote: null,
      installed: own,
      download: downloads[own.id] ?? null,
      updateAvailable: false,
    });
  }
  return rows.sort((a, b) => a.id.localeCompare(b.id));
}

const MB = 1024 * 1024;

/** Rounded-up megabytes, as the row subtitle and the progress readout show them. */
export function mb(bytes: number): number {
  return Math.ceil(bytes / MB);
}

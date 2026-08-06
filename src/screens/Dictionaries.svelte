<script lang="ts">
  // Settings ▸ Offline dictionaries: download, update and delete the dictionaries
  // the word popup looks words up in. Port of `ui/dictionaries/DictionariesScreen.kt`.
  // The list is filtered to language pairs of books actually in the library.
  import { onMount } from "svelte";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { app } from "../lib/app.svelte";
  import { langName } from "../lib/lang";
  import {
    buildRows,
    dictCancel,
    dictDelete,
    dictDownload,
    dictInstalled,
    dictManifest,
    libraryPairs,
    mb,
    onDictProgress,
    type DictRow,
    type InstalledDict,
    type ManifestResult,
    type Progress,
  } from "../lib/dict";

  /** null while the catalog fetch is in flight (avoids a "nothing available" flash). */
  let manifest = $state<ManifestResult | null>(null);
  let installed = $state<InstalledDict[]>([]);
  let downloads = $state<Record<string, Progress>>({});

  const pairs = $derived(libraryPairs(app.books));
  const rows = $derived(buildRows(manifest?.dictionaries ?? [], installed, pairs, downloads));
  const libraryEmpty = $derived(pairs.size === 0);

  onMount(() => {
    void refresh();
    void reloadInstalled();
    // Downloads are app-scoped: this screen only watches them.
    const unlisten = onDictProgress((p) => {
      if (p.phase === "done" || p.phase === "cancelled") {
        const { [p.id]: _gone, ...rest } = downloads;
        downloads = rest;
        void reloadInstalled();
      } else {
        downloads = { ...downloads, [p.id]: p };
      }
    });
    return () => void unlisten.then((off) => off());
  });

  /** (Re)fetch the catalog; also serves as Retry for the error banner. */
  async function refresh() {
    manifest = null;
    manifest = await dictManifest();
  }

  async function reloadInstalled() {
    installed = await dictInstalled();
  }

  async function onDownload(row: DictRow) {
    if (!row.remote) return;
    // Show the row as busy at once: the first progress event is a network
    // round-trip away.
    downloads = {
      ...downloads,
      [row.id]: { id: row.id, phase: "downloading", bytesRead: 0, totalBytes: 0, message: "" },
    };
    try {
      await dictDownload(row.id);
    } catch (e) {
      downloads = {
        ...downloads,
        [row.id]: {
          id: row.id,
          phase: "failed",
          bytesRead: 0,
          totalBytes: 0,
          message: String(e),
        },
      };
    }
  }

  async function onDelete(row: DictRow) {
    const pair = `${langName(row.source)} → ${langName(row.target)}`;
    const ok = await confirm(`${pair} will need to be downloaded again to use it.`, {
      title: "Delete dictionary?",
      kind: "warning",
    });
    if (!ok) return;
    await dictDelete(row.id);
    await reloadInstalled();
  }

  function subtitle(row: DictRow): string {
    const size = row.remote
      ? row.remote.downloadSizeBytes > 0
        ? row.remote.downloadSizeBytes
        : row.remote.sizeBytes
      : row.installed?.sizeBytes;
    return [
      row.name,
      size ? `${mb(size)} MB` : null,
      row.remote?.license || null,
      row.remote?.attribution || null,
    ]
      .filter(Boolean)
      .join(" · ");
  }
</script>

<div class="screen">
  <header class="appbar">
    <button class="icon-btn" title="Back" onclick={() => app.goSettings()}>←</button>
    <h1 class="appbar-title">Dictionaries</h1>
    <span></span>
  </header>

  <div class="settings">
    <p class="dict-intro">
      Word dictionaries for the translation popup. Downloaded once, they work fully offline.
    </p>

    {#if manifest === null}
      <p class="dict-note">Loading the dictionary list…</p>
    {:else if manifest.status === "cached"}
      <p class="dict-note">Offline — showing the last loaded list.</p>
    {:else if manifest.status === "unavailable"}
      <div class="dict-banner">
        <span class="dict-error">{manifest.message}</span>
        <button class="text-btn" onclick={refresh}>Retry</button>
      </div>
    {/if}

    {#if rows.length === 0 && manifest !== null}
      <p class="dict-note">
        {#if libraryEmpty}
          Add books to your library to see matching dictionaries.
        {:else if manifest.status === "fresh"}
          No dictionaries are available for your books' languages yet.
        {/if}
      </p>
    {/if}

    {#each rows as row (row.id)}
      <div class="dict-row">
        <div class="dict-main">
          <div class="dict-pair">{langName(row.source)} → {langName(row.target)}</div>
          <div class="dict-sub">{subtitle(row)}</div>
        </div>
        <div class="dict-action">
          {#if row.download?.phase === "downloading"}
            <button class="icon-btn" title="Cancel download" onclick={() => dictCancel(row.id)}
              >✕</button
            >
          {:else if row.download?.phase === "installing"}
            <span class="dict-note">Installing…</span>
          {:else if row.download?.phase === "failed"}
            <button class="text-btn" disabled={!row.remote} onclick={() => onDownload(row)}
              >Retry</button
            >
          {:else if row.installed === null}
            <button class="text-btn" onclick={() => onDownload(row)}>Download</button>
          {:else if row.updateAvailable}
            <button class="text-btn" onclick={() => onDownload(row)}>Update</button>
            <button class="icon-btn" title="Delete dictionary" onclick={() => onDelete(row)}>🗑</button>
          {:else}
            <span class="dict-installed">Installed</span>
            <button class="icon-btn" title="Delete dictionary" onclick={() => onDelete(row)}>🗑</button>
          {/if}
        </div>
      </div>

      {#if row.download?.phase === "downloading"}
        <div class="dict-progress">
          <div class="progress-track">
            <div
              class="progress-fill"
              style="width:{row.download.totalBytes > 0
                ? Math.min(100, (row.download.bytesRead / row.download.totalBytes) * 100)
                : 0}%"
            ></div>
          </div>
          <span class="progress-pct">
            {mb(row.download.bytesRead)} / {mb(row.download.totalBytes)} MB
          </span>
        </div>
      {:else if row.download?.phase === "failed"}
        <div class="dict-error">{row.download.message}</div>
      {/if}
    {/each}
  </div>
</div>

<script lang="ts">
  import { onMount } from "svelte";
  import { confirm, open } from "@tauri-apps/plugin-dialog";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { coverDataUrl, deleteBook, importBook } from "../lib/api";
  import { app } from "../lib/app.svelte";

  let covers = $state<Record<string, string | null>>({});
  let error = $state<string | null>(null);
  /** A .tbook is being dragged over the window. */
  let dragging = $state(false);

  // Dropping a .tbook on the window imports it — the desktop counterpart of
  // Android's "share or open a .tbook from any app".
  onMount(() => {
    const unlisten = getCurrentWebview().onDragDropEvent(async (event) => {
      if (event.payload.type === "over") {
        dragging = true;
      } else if (event.payload.type === "drop") {
        dragging = false;
        await importPaths(event.payload.paths);
      } else {
        dragging = false;
      }
    });
    return () => void unlisten.then((off) => off());
  });

  /** Import every .tbook among `paths`, reporting the first failure. */
  async function importPaths(paths: string[]) {
    const books = paths.filter((p) => p.toLowerCase().endsWith(".tbook"));
    if (books.length === 0) {
      if (paths.length > 0) error = "Only .tbook files can be imported.";
      return;
    }
    error = null;
    for (const path of books) {
      try {
        await importBook(path);
      } catch (e) {
        error = String(e);
      }
    }
    await app.refreshBooks();
  }

  // Lazily load cover thumbnails for books that have one.
  $effect(() => {
    if (!app.settings.showCover) return;
    for (const b of app.books) {
      if (b.hasCover && covers[b.id] === undefined) {
        covers[b.id] = null;
        coverDataUrl(b.id)
          .then((url) => (covers[b.id] = url))
          .catch(() => (covers[b.id] = null));
      }
    }
  });

  async function onImport() {
    error = null;
    try {
      const picked = await open({
        multiple: true,
        filters: [{ name: "TBook", extensions: ["tbook"] }],
      });
      if (Array.isArray(picked)) await importPaths(picked);
      else if (typeof picked === "string") await importPaths([picked]);
    } catch (e) {
      error = String(e);
    }
  }

  async function onDelete(id: string, title: string) {
    const ok = await confirm(`Delete “${title}” from your library?`, {
      title: "Delete book",
      kind: "warning",
    });
    if (!ok) return;
    await deleteBook(id);
    await app.clearPosition(id);
    await app.refreshBooks();
  }
</script>

<div class="screen" class:drop-target={dragging}>
  <header class="appbar">
    <h1 class="appbar-title">Library</h1>
    <div class="appbar-actions">
      <button class="text-btn" onclick={onImport}>Import…</button>
      <button class="icon-btn" title="Settings" onclick={() => app.goSettings()}>⚙</button>
    </div>
  </header>

  {#if error}
    <div class="banner error">{error}</div>
  {/if}

  {#if app.books.length === 0}
    <div class="empty">
      No books yet. Use <strong>Import…</strong> — or drop a .tbook file on the window.
    </div>
  {:else}
    <div class="library-grid">
      {#each app.books as b (b.id)}
        <div class="book-card">
          <button class="book-open" onclick={() => app.openReader(b.id)}>
            {#if app.settings.showCover}
              <div class="book-cover">
                {#if covers[b.id]}
                  <img src={covers[b.id]} alt="" />
                {:else}
                  <div class="book-cover-fallback">{b.title.slice(0, 1)}</div>
                {/if}
              </div>
            {/if}
            <div class="book-title">{b.title}</div>
            <div class="book-author">{b.author}</div>
          </button>
          <button
            class="book-delete"
            title="Delete"
            onclick={() => onDelete(b.id, b.title)}>🗑</button
          >
        </div>
      {/each}
    </div>
  {/if}
</div>

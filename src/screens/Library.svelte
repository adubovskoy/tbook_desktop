<script lang="ts">
  import { confirm, open } from "@tauri-apps/plugin-dialog";
  import { coverDataUrl, deleteBook, importBook } from "../lib/api";
  import { app } from "../lib/app.svelte";

  let covers = $state<Record<string, string | null>>({});
  let error = $state<string | null>(null);

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
      const path = await open({
        multiple: false,
        filters: [{ name: "TBook", extensions: ["tbook"] }],
      });
      if (typeof path === "string") {
        await importBook(path);
        await app.refreshBooks();
      }
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

<div class="screen">
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
    <div class="empty">No books yet. Use <strong>Import…</strong> to add a .tbook file.</div>
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

<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { app } from "./lib/app.svelte";
  import Library from "./screens/Library.svelte";
  import Reader from "./screens/Reader.svelte";
  import Settings from "./screens/Settings.svelte";

  onMount(() => {
    void app.init();

    // Save the reading position before the window closes. onCloseRequested
    // holds the window open until this handler resolves, then destroys it — so
    // the try/catch guarantees a flush error can't leave the app un-closeable.
    let unlisten: (() => void) | undefined;
    void getCurrentWindow()
      .onCloseRequested(async () => {
        try {
          await app.flush();
        } catch (e) {
          console.error("Failed to save reading state on close", e);
        }
      })
      .then((u) => (unlisten = u));

    return () => unlisten?.();
  });

  // Drive the theme via a data attribute on <html>; CSS maps it to a palette
  // (with `system` deferring to prefers-color-scheme).
  $effect(() => {
    document.documentElement.dataset.theme = app.settings.theme;
  });
</script>

{#if !app.ready}
  <div class="boot">Loading…</div>
{:else if app.view === "reader" && app.bookId}
  {#key app.bookId}
    <Reader />
  {/key}
{:else if app.view === "settings"}
  <Settings />
{:else}
  <Library />
{/if}

<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "./lib/app.svelte";
  import Library from "./screens/Library.svelte";
  import Reader from "./screens/Reader.svelte";
  import Settings from "./screens/Settings.svelte";

  onMount(() => {
    void app.init();
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

<script lang="ts">
  import { buildParagraphRender, paragraphHTML, type ParagraphRender } from "../lib/render";
  import type { Figure, Table } from "../lib/types";

  let {
    render,
    index,
    highlightRanges = [],
    figure = undefined,
    imageUrl = undefined,
    table = undefined,
    onImageLoad = undefined,
  }: {
    render: ParagraphRender;
    index: number;
    highlightRanges?: Array<[number, number]>;
    /** Figure anchored to this paragraph (role "figure"); sentences are its caption. */
    figure?: Figure;
    /** The figure image as a data: URL, once loaded. */
    imageUrl?: string | null;
    /** Table anchored to this (empty) paragraph (role "table"). */
    table?: Table;
    onImageLoad?: () => void;
  } = $props();

  const html = $derived(paragraphHTML(render, highlightRanges));

  // Table cells are mini-paragraphs of ordinary tappable sentences; data-r/data-c
  // on the cell let the click handler resolve the sentence back to the table.
  const tableRows = $derived(
    table
      ? table.rows.map((row) =>
          row.map((cell) => ({
            header: cell.header === true,
            html: paragraphHTML(buildParagraphRender(cell.sentences ?? [])),
          })),
        )
      : null,
  );
</script>

{#if render.role === "heading"}
  <h2 class="para heading" data-p={index}>{@html html}</h2>
{:else if render.role === "subtitle"}
  <p class="para subtitle" data-p={index}>{@html html}</p>
{:else if render.role === "sceneBreak"}
  <p class="para scene-break" data-p={index}>{render.text}</p>
{:else if render.role === "figure"}
  <figure class="para figure" data-p={index}>
    {#if imageUrl}
      <img src={imageUrl} alt={figure?.alt ?? ""} onload={onImageLoad} />
    {/if}
    {#if render.text.length > 0}
      <figcaption>{@html html}</figcaption>
    {/if}
  </figure>
{:else if render.role === "table"}
  <div class="para table-block" data-p={index}>
    {#if tableRows}
      <table>
        <tbody>
          {#each tableRows as row, ri (ri)}
            <tr>
              {#each row as cell, ci (ci)}
                {#if cell.header}
                  <th data-r={ri} data-c={ci}>{@html cell.html}</th>
                {:else}
                  <td data-r={ri} data-c={ci}>{@html cell.html}</td>
                {/if}
              {/each}
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
{:else}
  <p class="para body" data-p={index}>{@html html}</p>
{/if}

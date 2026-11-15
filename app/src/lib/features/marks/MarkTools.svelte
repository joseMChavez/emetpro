<script lang="ts">
  // «Resaltar» bajo «Preparado» y «En pantalla» (D-023): activa el marcado de palabras en la vista previa,
  // elige estilo y color, y quita las marcas de lo que está a la vista.
  import { t } from "$lib/i18n/index.svelte";
  import type { OutputContent } from "$lib/platform";
  import { canMark, hasMarks, MARK_COLORS, MARK_HEX, MARK_STYLES } from "./marks";
  import { markTool } from "./tool.svelte";

  let {
    content,
    saved,
    onclear,
  }: {
    content: OutputContent;
    /** Las marcas se guardan en un elemento del plan; si no, se pierden al cambiar de contenido. */
    saved: boolean;
    onclear: () => void;
  } = $props();
</script>

{#if canMark(content)}
  <div class="marks">
    <div class="row">
      <button class="btn toggle" aria-pressed={markTool.on} onclick={() => (markTool.on = !markTool.on)}>
        <span class="glyph marker" aria-hidden="true">A</span>{t("marks.toggle")}
      </button>
      {#if markTool.on}
        <div class="group" role="radiogroup" aria-label={t("marks.style")}>
          {#each MARK_STYLES as style (style)}
            <button
              class="tool"
              role="radio"
              aria-checked={markTool.style === style}
              title={t(`marks.style.${style}`)}
              aria-label={t(`marks.style.${style}`)}
              onclick={() => (markTool.style = style)}
            >
              <span class="glyph {style}" style:--mc={MARK_HEX[markTool.color]}>A</span>
            </button>
          {/each}
        </div>
        <div class="group" role="radiogroup" aria-label={t("marks.color")}>
          {#each MARK_COLORS as color (color)}
            <button
              class="swatch"
              role="radio"
              aria-checked={markTool.color === color}
              title={t(`marks.color.${color}`)}
              aria-label={t(`marks.color.${color}`)}
              style:--mc={MARK_HEX[color]}
              onclick={() => (markTool.color = color)}
            ></button>
          {/each}
        </div>
      {/if}
      {#if hasMarks(content)}
        <button class="btn" onclick={onclear}>{t("marks.clear")}</button>
      {/if}
    </div>
    {#if markTool.on}
      <p class="muted hint">{t("marks.hint")} {t(saved ? "marks.saved" : "marks.notSaved")}</p>
    {/if}
  </div>
{/if}

<style>
  .marks { display: flex; flex-direction: column; gap: 6px; }
  .hint { margin: 0; font-size: 12.5px; }
  .row { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .group { display: flex; gap: 4px; }
  .toggle { display: inline-flex; align-items: center; gap: 6px; }
  .toggle[aria-pressed="true"] { background: var(--accent); border-color: var(--accent); color: var(--accent-text); }
  .tool,
  .swatch {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
    cursor: pointer;
  }
  .tool[aria-checked="true"] { border-color: var(--accent); box-shadow: 0 0 0 2px var(--accent); }
  .swatch { background: var(--mc); border-radius: 50%; width: 24px; height: 24px; }
  .swatch[aria-checked="true"] { box-shadow: 0 0 0 2px var(--surface), 0 0 0 4px var(--text); }
  .glyph { font-weight: 700; font-size: 15px; line-height: 1; padding: 1px 3px; border-radius: 3px; }
  .glyph.marker { background: var(--mc, #ffd60a); color: #141414; }
  .glyph.underline { text-decoration: underline 3px var(--mc); text-underline-offset: 3px; }
  .glyph.color { color: var(--mc); -webkit-text-stroke: 0.4px rgb(0 0 0 / 0.35); }
</style>

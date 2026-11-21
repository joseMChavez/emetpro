<script lang="ts">
  // Fondo de una canción o de una sección de Media Studio (D-029): un archivo propio (imagen, video o un SVG
  // animado hecho con SVGator, Figma, Inkscape…) o una animación que genera la app (svgbg.ts).
  import { t, type MessageKey } from "$lib/i18n/index.svelte";
  import { toMessage, type Message } from "$lib/core/messages";
  import { platform } from "$lib/platform";
  import { DEFAULT_GENERATED, generateSvg, svgDataUrl, TEMPLATES, type Generated, type Speed } from "./svgbg";

  let {
    value,
    inherit,
    compact = false,
    onchange,
    onerror,
  }: {
    value: string | null | undefined;
    /** Qué se ve sin fondo propio («Como lo general», «Como la canción»). */
    inherit: string;
    compact?: boolean;
    onchange: (path: string | null) => void;
    onerror: (message: Message) => void;
  } = $props();

  let generating = $state(false);
  let saving = $state(false);
  let draft = $state<Generated>({ ...DEFAULT_GENERATED, colors: [...DEFAULT_GENERATED.colors] });
  let preview = $derived(generating ? svgDataUrl(generateSvg(draft)) : "");

  const SPEEDS: Speed[] = ["slow", "medium", "fast"];

  function nameOf(path: string): string {
    if (/[\\/]fondos[\\/]generados[\\/]/.test(path)) return t("studio.bg.generated");
    return path.split(/[\\/]/).pop() ?? path;
  }

  async function pickFile() {
    try {
      const path = await platform.pickBackground(t("background.dialogTitle"), t("studio.bg.filter"), true);
      if (path) onchange(path);
    } catch (e) {
      onerror(toMessage(e));
    }
  }

  async function useGenerated() {
    if (saving) return;
    saving = true;
    try {
      onchange(await platform.studioSaveSvg(generateSvg($state.snapshot(draft) as Generated)));
      generating = false;
    } catch (e) {
      onerror(toMessage(e));
    } finally {
      saving = false;
    }
  }
</script>

<div class="bg" class:compact>
  <div class="row">
    <span class="label">{t("studio.bg.label")}</span>
    {#if value}
      <img class="thumb" src={platform.mediaUrl(value)} alt="" />
      <span class="name" title={value}>{nameOf(value)}</span>
    {:else}
      <span class="muted small">{inherit}</span>
    {/if}
    <button type="button" class="btn small-btn" onclick={pickFile}>{t("studio.bg.file")}</button>
    <button type="button" class="btn small-btn" aria-expanded={generating} onclick={() => (generating = !generating)}>{t("studio.bg.generate")}</button>
    {#if value}<button type="button" class="btn ghost small-btn" onclick={() => onchange(null)}>{t("studio.bg.remove")}</button>{/if}
  </div>

  {#if generating}
    <div class="generator">
      <img class="preview" src={preview} alt={t("studio.bg.preview")} />
      <div class="controls">
        <label class="field-row">
          <span class="label">{t("studio.bg.template")}</span>
          <select class="field compact-field" bind:value={draft.template}>
            {#each TEMPLATES as template (template)}<option value={template}>{t(`studio.tpl.${template}` as MessageKey)}</option>{/each}
          </select>
        </label>
        <div class="field-row">
          <span class="label">{t("studio.bg.colors")}</span>
          <div class="colors">
            {#each draft.colors as _, i (i)}
              <input type="color" bind:value={draft.colors[i]} aria-label={t("studio.bg.colorN", { n: i + 1 })} />
            {/each}
            <input type="color" bind:value={draft.base} aria-label={t("studio.bg.base")} title={t("studio.bg.base")} />
          </div>
        </div>
        <label class="field-row">
          <span class="label">{t("studio.bg.speed")}</span>
          <select class="field compact-field" bind:value={draft.speed}>
            {#each SPEEDS as speed (speed)}<option value={speed}>{t(`studio.speed.${speed}` as MessageKey)}</option>{/each}
          </select>
        </label>
        <label class="field-row">
          <span class="label">{t("studio.bg.intensity")}</span>
          <input type="range" min="0.2" max="1" step="0.05" bind:value={draft.intensity} />
        </label>
        <div class="row">
          <button type="button" class="btn small-btn" onclick={() => (draft.seed += 1)}>{t("studio.bg.variant")}</button>
          <button type="button" class="btn ghost small-btn" onclick={() => (generating = false)}>{t("studio.cancel")}</button>
          <button type="button" class="btn primary small-btn" onclick={useGenerated} disabled={saving}>{t("studio.bg.use")}</button>
        </div>
      </div>
    </div>
    <p class="muted small">{t("studio.bg.hint")}</p>
  {/if}
</div>

<style>
  .bg { display: flex; flex-direction: column; gap: 6px; }
  .row { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .label { font-size: 13px; font-weight: 600; color: var(--muted); }
  .thumb { width: 64px; aspect-ratio: 16 / 9; object-fit: cover; border-radius: 4px; background: #000; }
  .name { max-width: 12em; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600; font-size: 14px; }
  .small-btn { min-height: 34px; padding: 0 10px; font-size: 13px; }
  .generator { display: grid; grid-template-columns: minmax(0, 1.2fr) minmax(0, 1fr); gap: 10px; align-items: start; }
  .compact .generator { grid-template-columns: 1fr; }
  .preview { width: 100%; aspect-ratio: 16 / 9; border-radius: 8px; background: #000; display: block; }
  .controls { display: flex; flex-direction: column; gap: 8px; }
  .field-row { display: flex; flex-direction: column; gap: 4px; }
  .compact-field { width: auto; min-height: 34px; padding: 4px 8px; }
  .colors { display: flex; gap: 6px; }
  .colors input { width: 40px; height: 32px; padding: 0; border: 1px solid var(--border); border-radius: 6px; background: none; cursor: pointer; }
  .colors input:last-child { margin-left: 8px; }
</style>

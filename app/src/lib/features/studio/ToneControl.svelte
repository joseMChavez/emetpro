<script lang="ts">
  // El tono de la canción (D-029): semitonos de más o de menos, para la voz de quien dirige.
  import { t, tCount } from "$lib/i18n/index.svelte";
  import { player, TRANSPOSE_MAX } from "./player.svelte";

  let {
    value,
    onchange,
    disabled = false,
  }: {
    value: number;
    onchange: (semitones: number) => void;
    disabled?: boolean;
  } = $props();

  let text = $derived(value === 0 ? t("studio.tone.original") : `${value > 0 ? "+" : "−"}${tCount("studio.tone.semitones", Math.abs(value))}`);
</script>

<div class="tone" role="group" aria-label={t("studio.tone")}>
  <span class="label">{t("studio.tone")}</span>
  <button type="button" class="mini" onclick={() => onchange(value - 1)} disabled={disabled || value <= -TRANSPOSE_MAX} aria-label={t("studio.tone.down")} title={t("studio.tone.down")}>−</button>
  <span class="value" aria-live="polite">{text}</span>
  <button type="button" class="mini" onclick={() => onchange(value + 1)} disabled={disabled || value >= TRANSPOSE_MAX} aria-label={t("studio.tone.up")} title={t("studio.tone.up")}>+</button>
  {#if value !== 0}<button type="button" class="btn ghost reset" onclick={() => onchange(0)} {disabled}>{t("studio.tone.reset")}</button>{/if}
  {#if player.shiftError && value !== 0}<span class="warn small" role="alert">{t("studio.tone.unavailable")}</span>{/if}
</div>

<style>
  .tone { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
  .label { font-size: 13px; font-weight: 600; color: var(--muted); }
  .mini { min-width: 40px; min-height: 40px; font: inherit; font-size: 18px; font-weight: 700; color: var(--text); background: var(--surface-2); border: 1px solid var(--border); border-radius: 8px; cursor: pointer; }
  .mini:disabled { opacity: 0.4; cursor: default; }
  .value { min-width: 7.5em; text-align: center; font-weight: 700; font-variant-numeric: tabular-nums; }
  .reset { min-height: 36px; padding: 0 10px; font-size: 13px; }
  .warn { color: var(--warn-text, var(--live)); }
</style>

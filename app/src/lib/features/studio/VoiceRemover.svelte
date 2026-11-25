<script lang="ts">
  // Con la versión sin voz ya sacada («Preparar con la pista»): elegir cuál suena, o borrarla.
  import { t } from "$lib/i18n/index.svelte";

  let {
    voiceless,
    disabled = false,
    onvoice,
    onremove,
  }: {
    voiceless: boolean;
    disabled?: boolean;
    /** Elegir con voz (false) o sin voz (true). */
    onvoice: (voiceless: boolean) => void;
    onremove: () => void;
  } = $props();
</script>

<div class="voice-remover">
  <div class="row">
    <span class="label">{t("separator.plays")}</span>
    <div class="segmented" role="group" aria-label={t("separator.plays")}>
      <button type="button" aria-pressed={!voiceless} onclick={() => onvoice(false)} {disabled}>{t("separator.withVoice")}</button>
      <button type="button" aria-pressed={voiceless} onclick={() => onvoice(true)} {disabled}>{t("separator.withoutVoice")}</button>
    </div>
    <button type="button" class="btn ghost small-btn" onclick={onremove} {disabled}>{t("separator.removeTrack")}</button>
  </div>
</div>

<style>
  .row { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .label { font-size: 13px; font-weight: 600; color: var(--muted); }
  .small-btn { min-height: 34px; padding: 0 10px; font-size: 13px; }
</style>

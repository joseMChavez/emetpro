<script lang="ts">
  // «Quitar la voz» (D-029): saca de la pista una versión instrumental con htdemucs y deja elegir cuál suena.
  // Se hace al preparar la canción (tarda casi lo que dura la pista); en el culto solo se elige con voz o sin voz.
  import { onMount } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import type { AudioTrack } from "$lib/platform";
  import { cancelSeparator, loadSeparator, separate, separator } from "./separator.svelte";
  import SeparatorDownload from "./SeparatorDownload.svelte";

  let {
    audio,
    instrumental,
    voiceless,
    disabled = false,
    onseparated,
    onvoice,
    onremove,
  }: {
    audio: AudioTrack;
    instrumental: AudioTrack | null;
    voiceless: boolean;
    disabled?: boolean;
    onseparated: (track: AudioTrack) => void;
    /** Elegir con voz (false) o sin voz (true). */
    onvoice: (voiceless: boolean) => void;
    onremove: () => void;
  } = $props();

  let open = $state(false);
  let running = $state(false);

  // Casi lo que dura la pista en una computadora normal (medido: 0,8–0,9×), redondeado arriba.
  let minutes = $derived(Math.max(1, Math.ceil(audio.durationMs / 60_000)));
  let percent = $derived.by(() => {
    const step = separator.step;
    if (!step) return 0;
    if (step.phase === "separating") return Math.floor((step.done / Math.max(1, step.total)) * 100);
    return step.phase === "saving" ? 100 : 0;
  });

  onMount(() => {
    loadSeparator().catch(() => {});
  });

  async function start() {
    running = true;
    try {
      const track = await separate(audio.path, `${audio.name} (${t("separator.voicelessName")})`);
      if (track) {
        onseparated(track);
        open = false;
      }
    } finally {
      running = false;
    }
  }
</script>

<div class="voice-remover">
  {#if instrumental}
    <div class="row">
      <span class="label">{t("separator.plays")}</span>
      <div class="segmented" role="group" aria-label={t("separator.plays")}>
        <button type="button" aria-pressed={!voiceless} onclick={() => onvoice(false)} {disabled}>{t("separator.withVoice")}</button>
        <button type="button" aria-pressed={voiceless} onclick={() => onvoice(true)} {disabled}>{t("separator.withoutVoice")}</button>
      </div>
      <button type="button" class="btn ghost small-btn" onclick={onremove} {disabled}>{t("separator.removeTrack")}</button>
    </div>
    <p class="muted small">{t("separator.ready")}</p>
  {:else if !open}
    <button type="button" class="btn" onclick={() => (open = true)} {disabled}>🎤 {t("separator.open")}</button>
  {:else}
    <div class="box" role="group" aria-label={t("separator.open")}>
      <p class="small">{t("separator.intro")}</p>
      {#if separator.status && !separator.status.available}
        <p class="warn small">{t("error.separatorUnavailable")}</p>
      {:else if separator.status && !separator.status.ready}
        <p class="small">{t("separator.needModel")}</p>
        <SeparatorDownload />
      {:else if !running}
        <div class="row">
          <button type="button" class="btn primary" onclick={start} disabled={disabled || separator.status?.busy}>{t("separator.start")}</button>
          <span class="muted small">{t("separator.time", { minutes })}</span>
        </div>
      {:else}
        <div class="progress" role="status">
          <span class="small">
            {#if separator.step?.phase === "separating"}{t("separator.separating", { percent })}{:else if separator.step?.phase === "saving"}{t("separator.saving")}{:else}{t("separator.reading")}{/if}
          </span>
          {#if separator.step?.phase === "separating"}<progress max="100" value={percent}></progress>{:else}<progress></progress>{/if}
          <button type="button" class="btn ghost" onclick={cancelSeparator} disabled={separator.step?.phase === "saving"}>{t("whisper.cancel")}</button>
        </div>
      {/if}
      {#if separator.error}<p class="warn small" role="alert">{t(separator.error.key, separator.error.params)}</p>{/if}
      {#if !running}
        <div class="row end"><button type="button" class="btn ghost" onclick={() => (open = false)}>{t("studio.cancel")}</button></div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .voice-remover p { margin: 0; }
  .box { display: flex; flex-direction: column; gap: 8px; padding: 12px; border: 1px solid var(--border); border-radius: 10px; background: var(--surface-2); }
  .row { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .row.end { justify-content: flex-end; }
  .label { font-size: 13px; font-weight: 600; color: var(--muted); }
  .progress { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; }
  progress { flex: 1; min-width: 120px; accent-color: var(--accent); }
  .warn { color: var(--warn-text, var(--live)); }
  .small-btn { min-height: 34px; padding: 0 10px; font-size: 13px; }
</style>

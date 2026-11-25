<script lang="ts">
  // «Preparar con la pista» (D-029): con la pista cargada, de una vez y en este orden, sacar la letra (si no la
  // tiene), colocar cada diapositiva donde se canta y guardar la versión sin voz. La pista se lee una sola vez y,
  // si hay separador, Whisper oye la voz sola (mucho más fiable que la mezcla). Todo queda en el editor para
  // revisarlo antes de guardar.
  import { onMount } from "svelte";
  import { i18n, t } from "$lib/i18n/index.svelte";
  import { toMessage, type Message } from "$lib/core/messages";
  import { platform, type AudioTrack, type PreparedTrack, type PrepareProgress } from "$lib/platform";
  import { loadRecognizer, recognizer } from "./recognizer.svelte";
  import { loadSeparator, separator } from "./separator.svelte";
  import RecognizerDownload from "./RecognizerDownload.svelte";
  import type { PrepareChoice } from "./sync";
  import SeparatorDownload from "./SeparatorDownload.svelte";

  let {
    audio,
    title,
    hasLyrics,
    hasInstrumental,
    disabled = false,
    prepare,
    onprepared,
  }: {
    audio: AudioTrack;
    title: string;
    /** La canción ya tiene letra: no se saca (se sincroniza la que hay). */
    hasLyrics: boolean;
    hasInstrumental: boolean;
    disabled?: boolean;
    /** Antes de empezar (p. ej. pausar la pista del editor). */
    prepare?: () => void;
    onprepared: (result: PreparedTrack, choice: PrepareChoice) => void;
  } = $props();

  const LANGUAGES = [
    { code: "es", label: "Español" },
    { code: "en", label: "English" },
    { code: "it", label: "Italiano" },
    { code: "pt", label: "Português" },
  ];

  let open = $state(false);
  let lyrics = $state(false);
  let sync = $state(true);
  let voice = $state(false);
  let language = $state(["es", "en", "it"].includes(i18n.locale) ? i18n.locale : "es");
  let running = $state(false);
  let progress = $state<PrepareProgress | null>(null);
  let error = $state<Message | null>(null);

  let separatorReady = $derived(!!separator.status?.ready);
  let separatorAvailable = $derived(separator.status?.available ?? true);
  /** Hace falta oír la letra (para sacarla o sincronizar). */
  let listen = $derived(lyrics || sync);
  let needRecognizer = $derived(listen && recognizer.status !== null && !recognizer.status.ready);
  let needSeparator = $derived(voice && separator.status !== null && !separatorReady);
  let canStart = $derived((lyrics || sync || voice) && !needRecognizer && !needSeparator && (hasLyrics || lyrics || !sync));
  // Medido: separar ~0,85× la duración; oír ~0,3–0,5× (más con la voz sola, que da tramos largos).
  let minutes = $derived.by(() => {
    const separating = voice || (listen && separatorReady);
    const factor = (separating ? 0.9 : 0) + (listen ? 0.5 : 0);
    return Math.max(1, Math.ceil((audio.durationMs * factor) / 60_000));
  });
  let percent = $derived.by(() => {
    const p = progress;
    if (!p) return 0;
    if (p.phase === "separating") return Math.floor((p.done / Math.max(1, p.total)) * 100);
    if (p.phase === "listening") return p.percent;
    return 0;
  });

  onMount(() => {
    loadRecognizer().catch(() => {});
    loadSeparator().catch(() => {});
    const off = platform.onPrepareProgress((p) => {
      if (running) progress = p;
    });
    return () => {
      off.then((f) => f());
    };
  });

  function show() {
    // Lo que tiene sentido con lo que ya hay: sin letra, sacarla; sin versión sin voz, hacerla (si se puede).
    lyrics = !hasLyrics;
    sync = true;
    voice = !hasInstrumental && separatorAvailable;
    error = null;
    open = true;
  }

  async function start() {
    prepare?.();
    running = true;
    error = null;
    progress = { phase: "reading" };
    try {
      const result = await platform.studioPrepare(audio.path, `${audio.name} (${t("separator.voicelessName")})`, language, title.trim(), listen, voice);
      onprepared(result, { lyrics, sync, voice });
      open = false;
    } catch (e) {
      const code = (e as { code?: string })?.code;
      if (code !== "whisperCanceled" && code !== "separatorCanceled") error = toMessage(e);
    } finally {
      running = false;
      progress = null;
    }
  }

  function cancel() {
    platform.studioWhisperCancel();
    platform.studioSeparatorCancel();
  }
</script>

<div class="assistant">
  {#if !open}
    <button type="button" class="btn" onclick={show} {disabled}>✨ {t("prepare.open")}</button>
  {:else}
    <div class="box" role="group" aria-label={t("prepare.open")}>
      <p class="small">{t("prepare.intro")}</p>
      {#if !running}
        <label class="check">
          <input type="checkbox" bind:checked={lyrics} disabled={hasLyrics} />
          <span>{t("prepare.lyrics")} {#if hasLyrics}<span class="muted small">· {t("prepare.hasLyrics")}</span>{/if}</span>
        </label>
        <label class="check">
          <input type="checkbox" bind:checked={sync} />
          <span>{t("prepare.sync")}</span>
        </label>
        <label class="check">
          <input type="checkbox" bind:checked={voice} disabled={!separatorAvailable} />
          <span>{t(hasInstrumental ? "prepare.voiceAgain" : "prepare.voice")} {#if !separatorAvailable}<span class="muted small">· {t("error.separatorUnavailable")}</span>{/if}</span>
        </label>
        {#if listen}
          <label class="field-row">
            <span class="label">{t("whisper.language")}</span>
            <select class="field" bind:value={language}>
              {#each LANGUAGES as l (l.code)}<option value={l.code}>{l.label}</option>{/each}
              <option value="">{t("whisper.autoLanguage")}</option>
            </select>
          </label>
        {/if}
        {#if needRecognizer}
          <p class="small">{t("whisper.needModel")}</p>
          <RecognizerDownload />
        {/if}
        {#if needSeparator}
          <p class="small">{t("separator.needModel")}</p>
          <SeparatorDownload />
        {:else if sync && !voice && separatorAvailable && !separatorReady}
          <p class="muted small">{t("prepare.betterWithSeparator")}</p>
        {/if}
        {#if sync && !hasLyrics && !lyrics}<p class="warn small">{t("prepare.syncNeedsLyrics")}</p>{/if}
        <div class="row">
          <button type="button" class="btn primary" onclick={start} disabled={disabled || !canStart}>{t("prepare.start")}</button>
          <span class="muted small">{t("prepare.time", { minutes })}</span>
          <span class="grow"></span>
          <button type="button" class="btn ghost" onclick={() => (open = false)}>{t("studio.cancel")}</button>
        </div>
      {:else}
        <div class="progress" role="status">
          <span class="small">
            {#if progress?.phase === "separating"}{t("separator.separating", { percent })}{:else if progress?.phase === "saving"}{t("separator.saving")}{:else if progress?.phase === "listening"}{t("whisper.listening", { percent })}{:else}{t("separator.reading")}{/if}
          </span>
          {#if progress?.phase === "separating" || progress?.phase === "listening"}<progress max="100" value={percent}></progress>{:else}<progress></progress>{/if}
          <button type="button" class="btn ghost" onclick={cancel}>{t("whisper.cancel")}</button>
        </div>
      {/if}
      {#if error}<p class="warn small" role="alert">{t(error.key, error.params)}</p>{/if}
    </div>
  {/if}
</div>

<style>
  .box { display: flex; flex-direction: column; gap: 8px; padding: 12px; border: 1px solid var(--border); border-radius: 10px; background: var(--surface-2); }
  .box p { margin: 0; }
  .check { display: flex; align-items: baseline; gap: 8px; font-size: 14px; }
  .field-row { display: flex; flex-direction: column; gap: 4px; max-width: 16em; }
  .label { font-size: 13px; font-weight: 600; color: var(--muted); }
  .row { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .grow { flex: 1; }
  .progress { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; }
  progress { flex: 1; min-width: 120px; accent-color: var(--accent); }
  .warn { color: var(--warn-text, var(--live)); }
</style>

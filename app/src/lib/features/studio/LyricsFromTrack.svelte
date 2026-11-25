<script lang="ts">
  // «Sacar la letra de la pista» (D-029): escucha la pista con Whisper y deja un borrador en el cuadro de pegar para
  // que el usuario lo limpie. Se hace al preparar la canción: tarda unos minutos y no hace falta internet (salvo
  // para descargar el reconocedor la primera vez).
  import { onMount } from "svelte";
  import { i18n, t } from "$lib/i18n/index.svelte";
  import { cancelRecognizer, loadRecognizer, recognizer, transcribe } from "./recognizer.svelte";
  import RecognizerDownload from "./RecognizerDownload.svelte";
  import { heardLines, lyricsDraft, resample } from "./whisper";

  let {
    title,
    durationMs,
    disabled = false,
    prepare,
    ondraft,
  }: {
    /** Contexto para el reconocedor: con el título acierta más palabras. */
    title: string;
    durationMs: number;
    disabled?: boolean;
    /** La pista en mono, lista para analizar; null si no se pudo cargar. */
    prepare: () => Promise<{ samples: Float32Array; rate: number } | null>;
    ondraft: (draft: string, info: { lines: number; loops: number }) => void;
  } = $props();

  const RATE = 16_000;
  const LANGUAGES = [
    { code: "es", label: "Español" },
    { code: "en", label: "English" },
    { code: "it", label: "Italiano" },
    { code: "pt", label: "Português" },
  ];

  let open = $state(false);
  let language = $state(["es", "en", "it"].includes(i18n.locale) ? i18n.locale : "es");
  let step = $state<"idle" | "preparing" | "listening">("idle");

  // Unos 0,4–0,6 s por segundo de canción en una computadora normal (medido con Whisper «small»): se redondea arriba.
  let minutes = $derived(Math.max(1, Math.ceil((durationMs * 0.6) / 60_000)));

  onMount(() => {
    loadRecognizer().catch(() => {});
  });

  async function start() {
    step = "preparing";
    try {
      const audio = await prepare();
      if (!audio) return;
      step = "listening";
      const segments = await transcribe(resample(audio.samples, audio.rate, RATE), language, title.trim());
      if (!segments) return;
      const { lines, loops } = heardLines(segments);
      const draft = lines.length ? lyricsDraft(lines, { verse: t("studio.draft.verse"), chorus: t("studio.draft.chorus") }) : "";
      ondraft(draft, { lines: lines.length, loops });
      open = false;
    } finally {
      step = "idle";
    }
  }
</script>

<div class="lyrics-from-track">
  {#if !open}
    <button type="button" class="btn" onclick={() => (open = true)} {disabled}>🎙 {t("whisper.open")}</button>
  {:else}
    <div class="box" role="group" aria-label={t("whisper.open")}>
      <p class="small">{t("whisper.intro")}</p>
      {#if recognizer.status && !recognizer.status.supported}
        <p class="warn small">{t("error.whisperUnsupported")}</p>
      {:else if recognizer.status && !recognizer.status.ready}
        <p class="small">{t("whisper.needModel")}</p>
        <RecognizerDownload />
      {:else if step === "idle"}
        <div class="row">
          <label class="field-row">
            <span class="label">{t("whisper.language")}</span>
            <select class="field" bind:value={language}>
              {#each LANGUAGES as l (l.code)}<option value={l.code}>{l.label}</option>{/each}
              <option value="">{t("whisper.autoLanguage")}</option>
            </select>
          </label>
          <button type="button" class="btn primary" onclick={start} disabled={disabled || recognizer.status?.busy}>{t("whisper.start")}</button>
        </div>
        <p class="muted small">{t("whisper.time", { minutes })}</p>
      {:else}
        <div class="progress" role="status">
          {#if step === "preparing"}
            <span class="small">{t("whisper.preparing")}</span>
            <progress></progress>
          {:else}
            <span class="small">{t("whisper.listening", { percent: recognizer.percent ?? 0 })}</span>
            <progress max="100" value={recognizer.percent ?? 0}></progress>
          {/if}
          <button type="button" class="btn ghost" onclick={cancelRecognizer} disabled={step === "preparing"}>{t("whisper.cancel")}</button>
        </div>
      {/if}
      {#if recognizer.error}<p class="warn small" role="alert">{t(recognizer.error.key, recognizer.error.params)}</p>{/if}
      {#if step === "idle"}
        <div class="row end"><button type="button" class="btn ghost" onclick={() => (open = false)}>{t("studio.cancel")}</button></div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .box { display: flex; flex-direction: column; gap: 8px; padding: 12px; border: 1px solid var(--border); border-radius: 10px; background: var(--surface-2); }
  .box p { margin: 0; }
  .row { display: flex; align-items: flex-end; gap: 8px; flex-wrap: wrap; }
  .row.end { justify-content: flex-end; }
  .field-row { display: flex; flex-direction: column; gap: 4px; }
  .label { font-size: 13px; font-weight: 600; color: var(--muted); }
  .progress { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; }
  progress { flex: 1; min-width: 120px; accent-color: var(--accent); }
  .warn { color: var(--warn-text, var(--live)); }
</style>

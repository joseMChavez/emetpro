<script lang="ts">
  // Descarga del reconocedor de voz: botón, avance y cancelar. La usan el editor de Media Studio y Ajustes.
  import { t } from "$lib/i18n/index.svelte";
  import { cancelRecognizer, downloadRecognizer, megabytes, recognizer } from "./recognizer.svelte";

  let { onready }: { onready?: () => void } = $props();

  let percent = $derived(recognizer.download && recognizer.download.total > 0 ? Math.floor((recognizer.download.done / recognizer.download.total) * 100) : 0);

  async function start() {
    if (await downloadRecognizer()) onready?.();
  }
</script>

{#if recognizer.download}
  <div class="progress" role="status">
    <span class="small">{t("whisper.downloading", { percent, done: megabytes(recognizer.download.done), total: megabytes(recognizer.download.total) })}</span>
    <progress max="100" value={percent}></progress>
    <button type="button" class="btn ghost" onclick={cancelRecognizer}>{t("whisper.cancel")}</button>
  </div>
{:else if recognizer.status}
  <div class="offer">
    <button type="button" class="btn" onclick={start} disabled={recognizer.status.busy}>{t("whisper.download", { size: megabytes(recognizer.status.size) })}</button>
    <span class="muted small">{t("whisper.downloadHint")}</span>
  </div>
{/if}

<style>
  .progress, .offer { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; }
  progress { flex: 1; min-width: 120px; accent-color: var(--accent); }
</style>

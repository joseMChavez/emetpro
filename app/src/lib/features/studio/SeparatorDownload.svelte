<script lang="ts">
  // Descarga del separador (motor y modelo): botón, avance y cancelar. La usan el editor de Media Studio y Ajustes.
  import { t } from "$lib/i18n/index.svelte";
  import { cancelSeparator, downloadSeparator, separator } from "./separator.svelte";
  import { megabytes } from "./recognizer.svelte";

  let { onready }: { onready?: () => void } = $props();

  let percent = $derived(separator.download && separator.download.total > 0 ? Math.floor((separator.download.done / separator.download.total) * 100) : 0);

  async function start() {
    if (await downloadSeparator()) onready?.();
  }
</script>

{#if separator.download}
  <div class="progress" role="status">
    <span class="small">{t("separator.downloading", { percent, done: megabytes(separator.download.done), total: megabytes(separator.download.total) })}</span>
    <progress max="100" value={percent}></progress>
    <button type="button" class="btn ghost" onclick={cancelSeparator}>{t("whisper.cancel")}</button>
  </div>
{:else if separator.status}
  <div class="offer">
    <button type="button" class="btn" onclick={start} disabled={separator.status.busy}>{t("separator.download", { size: megabytes(separator.status.size) })}</button>
    <span class="muted small">{t("separator.downloadHint")}</span>
  </div>
{/if}

<style>
  .progress, .offer { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; }
  progress { flex: 1; min-width: 120px; accent-color: var(--accent); }
</style>

<script lang="ts">
  // Exportar el plan abierto o una de sus secciones a un .emetplan (D-028): para mandarlo por correo o llevarlo a
  // otro equipo. Imágenes, PDF y presentaciones van dentro; los videos, solo si se marca (el correo suele limitar).
  import { onMount } from "svelte";
  import { i18n, t } from "$lib/i18n/index.svelte";
  import { toMessage, type Message } from "$lib/core/messages";
  import { platform, type PlanExportInfo } from "$lib/platform";
  import { bibles } from "$lib/features/bible/bibles.svelte";
  import { formatBytes } from "./size";

  let {
    planId,
    sectionId,
    label,
    fileName,
    onclose,
    onerror,
  }: {
    planId: string;
    /** null = el plan entero. */
    sectionId: string | null;
    /** «Plan «Culto del domingo»» o «Sección «Predicación»». */
    label: string;
    fileName: string;
    onclose: () => void;
    onerror: (message: Message) => void;
  } = $props();

  /** Más que esto no suele pasar por correo. */
  const MAIL_LIMIT = 25 * 1024 * 1024;

  let info = $state<PlanExportInfo | null>(null);
  let includeVideos = $state(false);
  let busy = $state(false);
  let done = $state<string | null>(null);

  let total = $derived(info ? info.otherBytes + (includeVideos ? info.videoBytes : 0) : 0);

  onMount(async () => {
    try {
      info = await platform.planExportInfo(planId, sectionId);
      // Con videos chicos, se incluyen sin preguntar.
      includeVideos = info.videos > 0 && info.otherBytes + info.videoBytes <= MAIL_LIMIT;
    } catch (e) {
      onerror(toMessage(e));
      onclose();
    }
  });

  async function save() {
    busy = true;
    try {
      const refs = bibles.all.map((b) => ({ id: b.id, abbreviation: b.abbreviation, name: b.name }));
      const result = await platform.planExport(t("planfile.saveTitle"), t("planfile.filter"), fileName, planId, sectionId, includeVideos, refs);
      if (result) done = result.path;
    } catch (e) {
      onerror(toMessage(e));
    } finally {
      busy = false;
    }
  }
</script>

<div class="box">
  <strong>{t("planfile.exportHeading", { what: label })}</strong>
  {#if !info}
    <p class="muted small">…</p>
  {:else if done}
    <p class="small">✓ {t("planfile.exported")}</p>
    <p class="muted small path">{done}</p>
    <div class="row end"><button class="btn primary" onclick={onclose}>{t("plans.done")}</button></div>
  {:else}
    <p class="small">{t("planfile.exportIntro")}</p>
    <p class="muted small">{t("planfile.exportCounts", { items: info.items, size: formatBytes(info.otherBytes, i18n.locale) })}</p>
    {#if info.videos > 0}
      <label class="check">
        <input type="checkbox" bind:checked={includeVideos} />
        <span>{t("planfile.includeVideos", { count: info.videos, size: formatBytes(info.videoBytes, i18n.locale) })}</span>
      </label>
      {#if !includeVideos}<p class="muted small">{t("planfile.videosOut")}</p>{/if}
    {/if}
    <p class="small" class:warn={total > MAIL_LIMIT}>
      {t("planfile.total", { size: formatBytes(total, i18n.locale) })}
      {#if total > MAIL_LIMIT}· {t("planfile.tooBigForMail")}{/if}
    </p>
    <div class="row end">
      <button class="btn ghost" onclick={onclose}>{t("songs.cancel")}</button>
      <button class="btn primary" onclick={save} disabled={busy}>{busy ? t("planfile.saving") : t("planfile.save")}</button>
    </div>
  {/if}
</div>

<style>
  .box { display: flex; flex-direction: column; gap: 8px; padding: 12px; border: 1px solid var(--border); border-radius: 8px; background: var(--surface-2); }
  .box p { margin: 0; }
  .row { display: flex; gap: 8px; flex-wrap: wrap; }
  .row.end { justify-content: flex-end; }
  .check { display: flex; gap: 8px; align-items: flex-start; font-size: 14px; }
  .warn { color: var(--warn-text, #b45309); }
  .path { overflow-wrap: anywhere; }
</style>

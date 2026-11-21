<script lang="ts">
  // Importar un .emetplan (D-028): como plan nuevo o al final de uno que ya existe (el sermón dentro del culto).
  // Antes de guardar se ve qué trae: elementos, archivos, letras (como al importar letras) y versiones de la Biblia.
  import { untrack } from "svelte";
  import { i18n, t } from "$lib/i18n/index.svelte";
  import { toMessage, type Message } from "$lib/core/messages";
  import { platform, type OnConflict, type PlanImportPreview } from "$lib/platform";
  import { bibles } from "$lib/features/bible/bibles.svelte";
  import { plans } from "./plans.svelte";
  import { loadStudio } from "$lib/features/studio/library.svelte";
  import { formatBytes } from "./size";

  let {
    preview,
    appendDefault,
    onclose,
    onimported,
    onerror,
  }: {
    preview: PlanImportPreview;
    /** Plan abierto: se propone añadir ahí. */
    appendDefault: string | null;
    onclose: () => void;
    onimported: (planId: string, text: string) => void;
    onerror: (message: Message) => void;
  } = $props();

  // Valores iniciales del formulario (cada vista previa nueva crea el componente otra vez).
  let target = $state<"new" | "append">(untrack(() => (appendDefault ? "append" : "new")));
  let newName = $state(untrack(() => (preview.section ? `${preview.name} · ${preview.section}` : preview.name)));
  let appendTo = $state(untrack(() => appendDefault ?? plans.all[0]?.id ?? ""));
  let onConflict = $state<OnConflict>("skip");
  let busy = $state(false);

  /** Versiones fijadas en las citas que este equipo no tiene: se verán con la de la consola, sin sus marcas. */
  let missingBibles = $derived(preview.bibles.filter((b) => !bibles.all.some((o) => o.id === b.id)));

  async function apply() {
    busy = true;
    try {
      const result = await platform.planImportApply(
        onConflict,
        t("transfer.importedSuffix"),
        target === "new" ? newName.trim() || preview.name : null,
        target === "append" ? appendTo : null,
      );
      // Las canciones de Media Studio que llegaron, a su biblioteca.
      if (result.studio > 0) await loadStudio();
      onimported(result.planId, t("planfile.imported", { items: result.items, added: result.added }));
    } catch (e) {
      onerror(toMessage(e));
    } finally {
      busy = false;
    }
  }

  function cancel() {
    platform.planImportCancel().catch(() => {});
    onclose();
  }
</script>

<div class="box">
  <strong>{t("planfile.importHeading")}</strong>
  <p>
    <b>{preview.section ? t("planfile.sectionOf", { section: preview.section, plan: preview.name }) : preview.name}</b>
    <span class="muted small"> · {preview.file}</span>
  </p>
  <p class="muted small">{t("planfile.importCounts", { items: preview.items, files: preview.files, size: formatBytes(preview.bytes, i18n.locale) })}</p>
  {#if preview.omitted.length > 0}
    <p class="small warn">{t("planfile.omitted", { list: preview.omitted.join(", ") })}</p>
  {/if}
  {#if missingBibles.length > 0}
    <p class="small warn">{t("planfile.missingBibles", { list: missingBibles.map((b) => `${b.abbreviation} (${b.name})`).join(", ") })}</p>
  {/if}
  {#if preview.studio.length > 0}
    <p class="small">{t("planfile.studio", { list: preview.studio.join(", ") })}</p>
    {#if preview.studioConflicts.length > 0 && preview.conflicts.length === 0}
      <p class="small">{t("planfile.studioConflicts", { list: preview.studioConflicts.join(", ") })}</p>
      <fieldset class="choices">
        <legend class="small">{t("transfer.conflictQuestion")}</legend>
        {#each [["skip", "transfer.conflict.skip"], ["replace", "transfer.conflict.replace"], ["keepBoth", "transfer.conflict.keepBoth"]] as const as [id, label] (id)}
          <label><input type="radio" name="plan-conflict" value={id} bind:group={onConflict} /> {t(label)}</label>
        {/each}
      </fieldset>
    {:else if preview.studioConflicts.length > 0}
      <p class="small">{t("planfile.studioConflicts", { list: preview.studioConflicts.join(", ") })}</p>
    {/if}
  {/if}
  {#if preview.fresh.length + preview.identical + preview.conflicts.length > 0}
    <p class="small">
      {t("planfile.songs", { fresh: preview.fresh.length, identical: preview.identical })}
    </p>
    {#if preview.conflicts.length > 0}
      <p class="small">{t("transfer.conflicts", { count: preview.conflicts.length })}: {preview.conflicts.map((s) => s.title).join(", ")}</p>
      <fieldset class="choices">
        <legend class="small">{t("transfer.conflictQuestion")}</legend>
        {#each [["skip", "transfer.conflict.skip"], ["replace", "transfer.conflict.replace"], ["keepBoth", "transfer.conflict.keepBoth"]] as const as [id, label] (id)}
          <label><input type="radio" name="plan-conflict" value={id} bind:group={onConflict} /> {t(label)}</label>
        {/each}
      </fieldset>
    {/if}
  {/if}

  <fieldset class="choices">
    <legend class="small">{t("planfile.where")}</legend>
    <label class="target">
      <input type="radio" name="plan-target" value="new" bind:group={target} />
      <span>{t("planfile.asNew")}</span>
      <input class="field" bind:value={newName} disabled={target !== "new"} aria-label={t("planfile.asNew")} />
    </label>
    <label class="target">
      <input type="radio" name="plan-target" value="append" bind:group={target} disabled={plans.all.length === 0} />
      <span>{t("planfile.appendTo")}</span>
      <select class="field" bind:value={appendTo} disabled={target !== "append"} aria-label={t("planfile.appendTo")}>
        {#each plans.all as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
      </select>
    </label>
  </fieldset>

  <div class="row end">
    <button class="btn ghost" onclick={cancel}>{t("songs.cancel")}</button>
    <button class="btn primary" onclick={apply} disabled={busy || (target === "append" && !appendTo)}>{busy ? t("planfile.importing") : t("planfile.import")}</button>
  </div>
</div>

<style>
  .box { display: flex; flex-direction: column; gap: 8px; padding: 12px; border: 1px solid var(--border); border-radius: 8px; background: var(--surface-2); }
  .box p { margin: 0; }
  .row { display: flex; gap: 8px; flex-wrap: wrap; }
  .row.end { justify-content: flex-end; }
  .warn { color: var(--warn-text, #b45309); }
  .choices { display: flex; flex-direction: column; gap: 6px; margin: 0; padding: 8px 10px; border: 1px solid var(--border); border-radius: 8px; }
  .choices label { display: flex; align-items: center; gap: 8px; font-size: 14px; }
  .target .field { flex: 1; min-width: 0; }
</style>

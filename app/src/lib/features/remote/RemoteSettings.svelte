<script lang="ts">
  // Sección «Control remoto» de Ajustes: encenderlo, el QR para el teléfono y desconectar a todos.
  import { t } from "$lib/i18n/index.svelte";
  import { setSetting, settings } from "$lib/core/settings.svelte";
  import { remote, resetRemoteKey } from "./console.svelte";
  import { notePresets, phrases, yesNo, type NotePreset } from "./chat.svelte";
  import { MAX_NOTE, MAX_REPLIES } from "./protocol";

  type Kind = "ok" | "yesNo" | "custom";

  /** Filas pasadas a «Opciones» que aún no tienen opciones (si no, se verían como «Solo aviso»). */
  let choosingOptions = $state<number[]>([]);

  function kindOf(note: NotePreset, i = -1): Kind {
    if (choosingOptions.includes(i)) return "custom";
    if (note.replies.length === 0) return "ok";
    const yn = yesNo();
    return note.replies.length === 2 && note.replies[0] === yn[0] && note.replies[1] === yn[1] ? "yesNo" : "custom";
  }

  /** Cambia un mensaje preparado; al tocar uno, la lista entera pasa a guardarse en Ajustes. */
  function editNote(i: number, change: Partial<NotePreset>) {
    setSetting("remoteNotes", notePresets().map((n, j) => (j === i ? { ...n, ...change } : n)));
  }

  function setKind(i: number, kind: Kind) {
    const current = notePresets()[i];
    choosingOptions = choosingOptions.filter((j) => j !== i);
    if (kind === "custom") {
      if (kindOf(current) !== "custom") choosingOptions.push(i);
      return;
    }
    editNote(i, { replies: kind === "ok" ? [] : yesNo() });
  }

  function options(text: string): string[] {
    return text.split(",").map((o) => o.trim()).filter(Boolean).slice(0, MAX_REPLIES);
  }

  function setOptions(i: number, text: string) {
    const replies = options(text);
    if (replies.length > 0) choosingOptions = choosingOptions.filter((j) => j !== i);
    editNote(i, { replies });
  }

  function removeNote(i: number) {
    choosingOptions = [];
    setSetting("remoteNotes", notePresets().filter((_, j) => j !== i));
  }

  function addNote() {
    setSetting("remoteNotes", [...notePresets(), { text: "", replies: [] }]);
  }

  function editPhrase(i: number, text: string) {
    setSetting("remotePhrases", phrases().map((p, j) => (j === i ? text : p)));
  }

  const DONE_MS = 4000;
  let resetDone = $state(false);

  async function reset() {
    await resetRemoteKey();
    resetDone = true;
    setTimeout(() => (resetDone = false), DONE_MS);
  }
</script>

<section class="card">
  <h2>{t("settings.remote")}</h2>
  <p class="muted small">{t("remote.intro")}</p>
  <label class="check">
    <input type="checkbox" checked={settings.remoteEnabled} disabled={remote.busy} onchange={(e) => setSetting("remoteEnabled", e.currentTarget.checked)} />
    {t("remote.enable")}
  </label>
  {#if remote.error}
    <p class="warn" role="alert">{t(remote.error.key, remote.error.params)}</p>
  {/if}
  {#if remote.info?.running}
    <div class="pair">
      {#if remote.info.qr}
        <!-- El SVG lo genera el backend a partir de la dirección: no lleva texto del usuario. -->
        <div class="qr" aria-hidden="true">{@html remote.info.qr}</div>
      {/if}
      <div class="stack">
        <p>{t("remote.scan")}</p>
        <code>{remote.info.url}</code>
        {#if !remote.info.onNetwork}<p class="warn">{t("remote.noNetwork")}</p>{/if}
        <p class="label">{t("remote.clients", { n: remote.info.clients })}</p>
        <p class="muted small">{t("remote.firewall")}</p>
      </div>
    </div>
    <div class="presets">
      <h3>{t("chat.settings.notes")}</h3>
      <p class="muted small">{t("chat.settings.notesHint")}</p>
      {#each notePresets() as note, i (i)}
        {@const kind = kindOf(note, i)}
        <div class="row">
          <input class="field" value={note.text} maxlength={MAX_NOTE} placeholder={t("chat.placeholder")} onchange={(e) => editNote(i, { text: e.currentTarget.value })} />
          <select class="field compact" value={kind} onchange={(e) => setKind(i, e.currentTarget.value as Kind)} aria-label={t("chat.replies")}>
            <option value="ok">{t("chat.kind.ok")}</option>
            <option value="yesNo">{t("chat.kind.yesNo")}</option>
            <option value="custom">{t("chat.kind.custom")}</option>
          </select>
          <button class="btn ghost" onclick={() => removeNote(i)} aria-label={t("chat.settings.remove")}>✕</button>
          {#if kind === "custom"}
            <input class="field options" value={note.replies.join(", ")} placeholder={t("chat.optionsPlaceholder")} onchange={(e) => setOptions(i, e.currentTarget.value)} />
          {/if}
        </div>
      {/each}
      <div class="row-actions">
        <button class="btn" onclick={addNote}>{t("chat.settings.add")}</button>
        {#if settings.remoteNotes !== null}<button class="btn ghost" onclick={() => setSetting("remoteNotes", null)}>{t("chat.settings.defaults")}</button>{/if}
      </div>

      <h3>{t("chat.settings.phrases")}</h3>
      <p class="muted small">{t("chat.settings.phrasesHint")}</p>
      {#each phrases() as phrase, i (i)}
        <div class="row">
          <input class="field" value={phrase} maxlength={MAX_NOTE} onchange={(e) => editPhrase(i, e.currentTarget.value)} />
          <button class="btn ghost" onclick={() => setSetting("remotePhrases", phrases().filter((_, j) => j !== i))} aria-label={t("chat.settings.remove")}>✕</button>
        </div>
      {/each}
      <div class="row-actions">
        <button class="btn" onclick={() => setSetting("remotePhrases", [...phrases(), ""])}>{t("chat.settings.add")}</button>
        {#if settings.remotePhrases !== null}<button class="btn ghost" onclick={() => setSetting("remotePhrases", null)}>{t("chat.settings.defaults")}</button>{/if}
      </div>
    </div>
    <div class="foot">
      {#if resetDone}<span class="ok small">{t("remote.resetDone")}</span>{/if}
      <button class="btn" onclick={reset}>{t("remote.resetKey")}</button>
    </div>
  {/if}
</section>

<style>
  .card { padding: 16px; display: flex; flex-direction: column; gap: 12px; }
  h2 { font-size: 16px; }
  .check { display: flex; align-items: center; gap: 8px; cursor: pointer; }
  .pair { display: flex; gap: 16px; align-items: flex-start; flex-wrap: wrap; }
  /* Fondo blanco siempre: los lectores de QR fallan con el tema oscuro. */
  .qr { flex: none; width: 220px; padding: 8px; background: #fff; border-radius: 10px; }
  .qr :global(svg) { display: block; width: 100%; height: auto; }
  .stack { flex: 1; min-width: 220px; display: flex; flex-direction: column; gap: 8px; }
  .label { font-weight: 600; }
  code { font-size: 12.5px; overflow-wrap: anywhere; }
  .warn { padding: 8px 12px; border-radius: 8px; background: var(--warn-bg); color: var(--warn-text); }
  .foot { display: flex; align-items: center; justify-content: flex-end; flex-wrap: wrap; gap: 10px; border-top: 1px solid var(--border); padding-top: 10px; }
  .ok { color: var(--ok); font-weight: 600; }
  .presets { display: flex; flex-direction: column; gap: 8px; border-top: 1px solid var(--border); padding-top: 12px; }
  .presets h3 { font-size: 14px; margin-top: 6px; }
  .row { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
  .row .field { width: auto; }
  .row > .field:first-child { flex: 1 1 200px; min-width: 0; }
  .row select.field { flex: none; }
  .row .options { flex-basis: 100%; }
  .row-actions { display: flex; gap: 8px; }
</style>

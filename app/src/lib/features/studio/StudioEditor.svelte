<script lang="ts">
  // Editor de una canción de Media Studio (D-029). La vista previa es «Preparado» de la consola, como en Textos.
  import { untrack } from "svelte";
  import { t, tCount } from "$lib/i18n/index.svelte";
  import { toMessage, type Message } from "$lib/core/messages";
  import { platform, type MediaSong, type OutputContent, type SectionKind, type SongIssue } from "$lib/platform";
  import { saveStudioSong, songCredit } from "./library.svelte";
  import { onScreen } from "./live.svelte";
  import { player } from "./player.svelte";
  import { issuePlace, issueText, kindName, sectionName } from "./names";
  import { parseLyrics } from "./parse";
  import { expand, newId, sectionOf, sectionText, SECTION_KINDS, slideText, studioContent, withText } from "./song";
  import { clearStart, formatTime, markStart, slideAt, slideStarts } from "./timeline";

  let {
    initial,
    onclose,
    onprepare,
    onerror,
  }: {
    initial: MediaSong;
    /** saved: la canción guardada, o null si se canceló. */
    onclose: (saved: MediaSong | null) => void;
    onprepare: (content: OutputContent) => void;
    onerror: (message: Message) => void;
  } = $props();

  let song = $state<MediaSong>(untrack(() => structuredClone($state.snapshot(initial)) as MediaSong));
  let issues = $state<SongIssue[] | null>(null);
  let saving = $state(false);
  let pasteOpen = $state(untrack(() => initial.sections.length === 0));
  let pasteText = $state("");
  let syncOpen = $state(false);
  /** Lo escrito en cada sección tal cual (con sus líneas en blanco a medio escribir); el modelo se lee de ahí. */
  let drafts = $state<Record<string, string>>({});
  /** La siguiente diapositiva desplegada que se marcará con Espacio. */
  let next = $state(0);

  let expanded = $derived(expand(song));
  let starts = $derived(slideStarts(expanded));
  let playingRow = $derived(syncOpen ? slideAt(starts, player.positionMs) : null);
  let errors = $derived(issues?.filter((i) => i.severity === "error") ?? []);
  let ownTrack = $derived(song.audio !== null && player.path === song.audio.path);
  /** La pista de la canción en pantalla está sonando: el editor no la interrumpe. */
  let liveBusy = $derived(player.playing && onScreen.song !== null && !ownTrack);

  function snapshot(): MediaSong {
    return $state.snapshot(song) as MediaSong;
  }

  function preview(index = 0) {
    const content = studioContent(snapshot(), songCredit(song));
    onprepare(content.kind === "text" ? { ...content, index: Math.min(index, content.slides.length - 1) } : content);
  }

  function changed() {
    issues = null;
    preview();
  }

  // Mientras se sincroniza, «Preparado» va con la pista.
  $effect(() => {
    const row = playingRow;
    if (row !== null) untrack(() => preview(row));
  });

  // ---- Pista ----

  async function pickAudio() {
    if (liveBusy) return;
    try {
      const picked = await platform.studioPickAudio(t("studio.pickAudio"), t("studio.audioFilter"));
      if (!picked) return;
      const duration = await player.load(picked.path);
      if (duration === null) {
        onerror({ key: "studio.trackError", params: { error: player.error ?? "" } });
        return;
      }
      song.audio = { path: picked.path, name: picked.name, durationMs: duration };
      song.mode = "track";
      changed();
    } catch (e) {
      onerror(toMessage(e));
    }
  }

  async function ensureTrack(): Promise<boolean> {
    if (!song.audio || liveBusy) return false;
    if (ownTrack && player.status === "ready") return true;
    const duration = await player.load(song.audio.path);
    if (duration !== null && duration !== song.audio.durationMs) song.audio.durationMs = duration;
    return duration !== null;
  }

  async function togglePlay() {
    if (await ensureTrack()) player.toggle();
  }

  async function back(seconds: number) {
    if (await ensureTrack()) player.seek(player.now() - seconds * 1000);
  }

  // ---- Letra, secciones y orden ----

  function applyPaste() {
    const parsed = parseLyrics(pasteText, newId);
    if (parsed.sections.length === 0) return;
    // Los tiempos eran de las apariciones anteriores: se empieza de cero.
    song.sections = parsed.sections;
    song.order = parsed.order;
    drafts = {};
    pasteOpen = false;
    pasteText = "";
    next = 0;
    changed();
  }

  function addSection(kind: SectionKind) {
    const section = { id: newId(), kind, label: "", slides: [] };
    song.sections.push(section);
    song.order.push({ id: newId(), section: section.id });
    changed();
  }

  function removeSection(id: string) {
    song.sections = song.sections.filter((s) => s.id !== id);
    song.order = song.order.filter((e) => e.section !== id);
    changed();
  }

  function setText(id: string, text: string) {
    drafts[id] = text;
    const i = song.sections.findIndex((s) => s.id === id);
    if (i >= 0) song.sections[i] = withText(snapshot().sections[i], text, newId);
    changed();
  }

  function addEntry(sectionId: string) {
    if (!sectionId) return;
    song.order.push({ id: newId(), section: sectionId });
    changed();
  }

  function moveEntry(i: number, by: number) {
    const j = i + by;
    if (j < 0 || j >= song.order.length) return;
    const order = snapshot().order;
    [order[i], order[j]] = [order[j], order[i]];
    // Al cambiar el orden, los tiempos dejan de corresponder a la grabación: se quitan los de las dos.
    order[i] = { ...order[i], timing: null };
    order[j] = { ...order[j], timing: null };
    song.order = order;
    changed();
  }

  function removeEntry(i: number) {
    song.order.splice(i, 1);
    changed();
  }

  // ---- Sincronizar: Espacio marca dónde empieza la siguiente diapositiva ----

  let syncBlock = $state<HTMLElement>();

  function toggleSync() {
    syncOpen = !syncOpen;
    // Queda al final de un editor largo: se trae a la vista para marcar viendo las filas.
    if (syncOpen) requestAnimationFrame(() => syncBlock?.scrollIntoView({ block: "start", behavior: "smooth" }));
  }

  function mark() {
    if (!ownTrack || next >= expanded.length) return;
    const updated = markStart(snapshot(), expanded, next, player.now());
    song.order = updated.order;
    next++;
    issues = null;
  }

  function unmark() {
    if (next === 0) return;
    next--;
    song.order = clearStart(snapshot(), expanded, next).order;
    issues = null;
  }

  async function goRow(k: number) {
    next = k;
    const start = starts[k];
    if (start !== null && (await ensureTrack())) player.seek(start);
    preview(k);
  }

  function nudge(k: number, ms: number) {
    const start = starts[k];
    if (start === null) return;
    song.order = markStart(snapshot(), expanded, k, start + ms).order;
    issues = null;
  }

  function isTyping(el: EventTarget | null): boolean {
    return el instanceof HTMLElement && (el.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(el.tagName));
  }

  function onKeydown(event: KeyboardEvent) {
    if (!syncOpen || isTyping(event.target)) return;
    if (event.key === " ") {
      event.preventDefault();
      if (player.playing) mark();
      else togglePlay();
    } else if (event.key === "Backspace") {
      event.preventDefault();
      unmark();
    }
  }

  /** Espacio pulsa el botón con el foco al soltarse: mientras se sincroniza, Espacio solo marca. */
  function onKeyup(event: KeyboardEvent) {
    if (syncOpen && event.key === " " && !isTyping(event.target)) event.preventDefault();
  }

  // ---- Guardar ----

  async function validate() {
    issues = await platform.studioValidate(snapshot());
  }

  async function save() {
    if (saving) return;
    saving = true;
    try {
      const outcome = await saveStudioSong(snapshot());
      issues = outcome.issues;
      if (outcome.saved) close(snapshot());
    } catch (e) {
      onerror(toMessage(e));
    } finally {
      saving = false;
    }
  }

  function close(saved: MediaSong | null) {
    // La pista del editor se para; la de la canción en pantalla sigue sonando.
    if (ownTrack && onScreen.song?.audio?.path !== player.path) player.pause();
    onclose(saved);
  }
</script>

<svelte:window onkeydown={onKeydown} onkeyup={onKeyup} />

<form class="editor" onsubmit={(e) => { e.preventDefault(); save(); }}>
  <div class="pair">
    <label class="field-row">
      <span class="label">{t("studio.title")}</span>
      <!-- svelte-ignore a11y_autofocus -->
      <input class="field" bind:value={song.title} oninput={changed} autofocus={!song.title} />
    </label>
    <label class="field-row">
      <span class="label">{t("studio.artist")}</span>
      <input class="field" bind:value={song.artist} oninput={changed} />
    </label>
  </div>

  <fieldset class="block">
    <legend class="label">{t("studio.mode")}</legend>
    <div class="segmented" role="group">
      <button type="button" aria-pressed={song.mode === "track"} onclick={() => ((song.mode = "track"), changed())}>{t("studio.mode.track")}</button>
      <button type="button" aria-pressed={song.mode === "live"} onclick={() => ((song.mode = "live"), (syncOpen = false), changed())}>{t("studio.mode.live")}</button>
    </div>
    <p class="muted small">{t(song.mode === "track" ? "studio.mode.trackHint" : "studio.mode.liveHint")}</p>
    <div class="row">
      <span class="label">{t("studio.track")}</span>
      {#if song.audio}
        <span class="name" title={song.audio.name}>{song.audio.name}</span>
        <span class="muted small">{formatTime(song.audio.durationMs)}</span>
      {:else}
        <span class="muted small">{t("studio.noTrack")}</span>
      {/if}
      <button type="button" class="btn" onclick={pickAudio} disabled={liveBusy}>{t(song.audio ? "studio.changeTrack" : "studio.pickTrack")}</button>
      {#if song.audio && song.mode === "live"}<span class="muted small">{t("studio.trackKept")}</span>{/if}
    </div>
    {#if liveBusy}<p class="muted small">{t("studio.liveBusy")}</p>{/if}
  </fieldset>

  <section class="block">
    <div class="row">
      <h3>{t("studio.sections")}</h3>
      <button type="button" class="btn ghost small-btn" onclick={() => (pasteOpen = !pasteOpen)}>{t("studio.pasteLyrics")}</button>
    </div>
    {#if pasteOpen}
      <div class="paste">
        <textarea class="field" rows="10" bind:value={pasteText} placeholder={t("studio.pastePlaceholder")}></textarea>
        <p class="muted small">{t("studio.pasteHint")}</p>
        <div class="row end">
          {#if song.sections.length > 0}<span class="muted small">{t("studio.pasteReplaces")}</span>{/if}
          <button type="button" class="btn" onclick={() => (pasteOpen = false)}>{t("studio.cancel")}</button>
          <button type="button" class="btn primary" onclick={applyPaste} disabled={!pasteText.trim()}>{t("studio.pasteApply")}</button>
        </div>
      </div>
    {/if}
    {#each song.sections as section (section.id)}
      <div class="section-card">
        <div class="row">
          <select class="field compact" value={section.kind} aria-label={t("studio.sectionKind")} onchange={(e) => ((section.kind = e.currentTarget.value as SectionKind), changed())}>
            {#each SECTION_KINDS as kind (kind)}<option value={kind}>{kindName(kind)}</option>{/each}
          </select>
          <input class="field compact grow" bind:value={section.label} oninput={changed} placeholder={sectionName(song, { ...section, label: "" })} aria-label={t("studio.sectionLabel")} />
          <span class="muted small">{section.slides.length === 0 ? t("studio.instrumental") : tCount("text.count", section.slides.length)}</span>
          <button type="button" class="icon-btn" onclick={() => removeSection(section.id)} aria-label={t("studio.removeSection")} title={t("studio.removeSection")}>✕</button>
        </div>
        <textarea class="field" rows={Math.min(10, Math.max(2, (drafts[section.id] ?? sectionText(section)).split("\n").length + 1))} value={drafts[section.id] ?? sectionText(section)} oninput={(e) => setText(section.id, e.currentTarget.value)} placeholder={t("studio.sectionPlaceholder")} aria-label={sectionName(song, section)}></textarea>
      </div>
    {/each}
    <div class="row">
      <select class="field compact" value="" aria-label={t("studio.addSection")} onchange={(e) => { addSection(e.currentTarget.value as SectionKind); e.currentTarget.value = ""; }}>
        <option value="" disabled>{t("studio.addSection")}</option>
        {#each SECTION_KINDS as kind (kind)}<option value={kind}>{kindName(kind)}</option>{/each}
      </select>
    </div>
  </section>

  <section class="block">
    <h3>{t("studio.order")}</h3>
    <p class="muted small">{t("studio.orderHint")}</p>
    <ol class="order">
      {#each song.order as entry, i (entry.id)}
        {@const section = sectionOf(song, entry.section)}
        <li class="chip">
          <span class="chip-name">{section ? sectionName(song, section) : "?"}</span>
          <button type="button" class="mini" onclick={() => moveEntry(i, -1)} disabled={i === 0} aria-label={t("studio.moveEarlier")}>◀</button>
          <button type="button" class="mini" onclick={() => moveEntry(i, 1)} disabled={i === song.order.length - 1} aria-label={t("studio.moveLater")}>▶</button>
          <button type="button" class="mini" onclick={() => removeEntry(i)} aria-label={t("studio.removeFromOrder")}>✕</button>
        </li>
      {/each}
    </ol>
    {#if song.sections.length > 0}
      <select class="field compact" value="" aria-label={t("studio.addToOrder")} onchange={(e) => { addEntry(e.currentTarget.value); e.currentTarget.value = ""; }}>
        <option value="" disabled>{t("studio.addToOrder")}</option>
        {#each song.sections as section (section.id)}<option value={section.id}>{sectionName(song, section)}</option>{/each}
      </select>
    {/if}
  </section>

  {#if song.mode === "track" && song.audio}
    <section class="block" bind:this={syncBlock}>
      <div class="row">
        <h3>{t("studio.sync")}</h3>
        <button type="button" class="btn ghost small-btn" onclick={toggleSync} aria-expanded={syncOpen}>{t(syncOpen ? "studio.syncClose" : "studio.syncOpen")}</button>
      </div>
      {#if syncOpen}
        <p class="muted small">{t("studio.syncHint")}</p>
        <div class="row transport">
          <button type="button" class="btn primary" onclick={togglePlay} disabled={liveBusy || (ownTrack && player.status === "loading")}>
            {ownTrack && player.playing ? `⏸ ${t("studio.pause")}` : `▶ ${t("studio.play")}`}
          </button>
          <button type="button" class="btn" onclick={() => back(5)} disabled={liveBusy}>{t("studio.back5")}</button>
          <span class="time">{ownTrack ? formatTime(player.positionMs) : formatTime(0)} / {formatTime(song.audio.durationMs)}</span>
          <button type="button" class="btn" onclick={mark} disabled={!ownTrack || next >= expanded.length}>{t("studio.markNext")}</button>
          <button type="button" class="btn ghost" onclick={unmark} disabled={next === 0}>{t("studio.unmark")}</button>
        </div>
        <ol class="rows">
          {#each expanded as e, k (`${e.entry.id}-${e.slide?.id ?? "inst"}`)}
            <li class:playing={playingRow === k} class:next={next === k}>
              <button type="button" class="row-main" onclick={() => goRow(k)}>
                <span class="when">{starts[k] === null ? "—" : formatTime(starts[k]!)}</span>
                <span class="what">
                  <strong>{sectionName(song, e.section)}</strong>
                  <span class="muted">{e.slide ? slideText(e.slide).split("\n")[0] : t("studio.instrumental")}</span>
                </span>
                {#if next === k}<span class="tag">{t("studio.nextTag")}</span>{/if}
              </button>
              <button type="button" class="mini" onclick={() => nudge(k, -100)} disabled={starts[k] === null} aria-label={t("studio.earlier")}>−0,1</button>
              <button type="button" class="mini" onclick={() => nudge(k, 100)} disabled={starts[k] === null} aria-label={t("studio.later")}>+0,1</button>
            </li>
          {/each}
        </ol>
      {/if}
    </section>
  {/if}

  <details class="block">
    <summary class="label">{t("studio.rights")}</summary>
    <p class="muted small">{t("studio.rightsHint")}</p>
    <div class="pair">
      <label class="field-row"><span class="label">{t("studio.rights.composition")}</span><input class="field" bind:value={song.rights.composition} /></label>
      <label class="field-row"><span class="label">{t("studio.rights.lyrics")}</span><input class="field" bind:value={song.rights.lyrics} /></label>
      <label class="field-row"><span class="label">{t("studio.rights.recording")}</span><input class="field" bind:value={song.rights.recording} /></label>
      <label class="field-row"><span class="label">{t("studio.rights.license")}</span><input class="field" bind:value={song.rights.license} /></label>
    </div>
    <label class="field-row"><span class="label">{t("studio.rights.notes")}</span><input class="field" bind:value={song.rights.notes} /></label>
  </details>

  {#if issues}
    <div class="issues" role="status">
      {#if issues.length === 0}
        <p class="ok">✓ {t("studio.valid")}</p>
      {:else}
        <p class="small">{errors.length > 0 ? t("studio.hasErrors") : t("studio.onlyWarnings")}</p>
        <ul>
          {#each issues as issue, i (i)}
            <li class={issue.severity}>
              <span class="sev">{issue.severity === "error" ? t("studio.error") : t("studio.warning")}</span>
              {issueText(issue.code)}
              {#if issuePlace(song, issue)}<span class="muted"> · {issuePlace(song, issue)}</span>{/if}
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {/if}

  <div class="row end actions">
    <button type="button" class="btn ghost" onclick={() => close(null)}>{t("studio.cancel")}</button>
    <button type="button" class="btn" onclick={validate}>{t("studio.validate")}</button>
    <button type="submit" class="btn primary" disabled={saving}>{t("studio.save")}</button>
  </div>
</form>

<style>
  .editor { display: flex; flex-direction: column; gap: 14px; }
  .block { display: flex; flex-direction: column; gap: 8px; margin: 0; padding: 0; border: 0; min-width: 0; }
  h3 { margin: 0; font-size: 14px; flex: 1; }
  .label { font-size: 13px; font-weight: 600; color: var(--muted); }
  .field-row { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .pair { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
  .row { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .row.end { justify-content: flex-end; }
  .grow { flex: 1; min-width: 8em; }
  .name { max-width: 16em; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-weight: 600; }
  .field.compact { width: auto; min-height: 36px; padding: 4px 8px; }
  .small-btn { min-height: 34px; padding: 0 10px; font-size: 13px; }
  textarea { resize: vertical; font-size: 15px; line-height: 1.45; }
  .paste { display: flex; flex-direction: column; gap: 6px; }
  .section-card { display: flex; flex-direction: column; gap: 6px; padding: 8px; border: 1px solid var(--border); border-radius: 8px; background: var(--surface-2); }
  .icon-btn { min-width: 36px; min-height: 36px; font: inherit; color: var(--muted); background: transparent; border: 0; border-radius: 8px; cursor: pointer; }
  .icon-btn:hover { color: var(--live); background: var(--live-soft); }
  .order { list-style: none; margin: 0; padding: 0; display: flex; flex-wrap: wrap; gap: 6px; }
  .chip { display: flex; align-items: center; gap: 2px; padding: 2px 4px 2px 10px; border: 1px solid var(--border); border-radius: 999px; background: var(--surface-2); }
  .chip-name { font-weight: 600; font-size: 14px; margin-right: 4px; }
  .mini { min-width: 30px; min-height: 30px; font: inherit; font-size: 12px; color: var(--text); background: transparent; border: 0; border-radius: 6px; cursor: pointer; }
  .mini:hover:not(:disabled) { background: var(--surface); }
  .mini:disabled { opacity: 0.35; cursor: default; }
  .transport .time { font-variant-numeric: tabular-nums; }
  .rows { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 2px; max-height: 360px; overflow-y: auto; }
  .rows li { display: flex; align-items: center; gap: 2px; border: 2px solid transparent; border-radius: 8px; }
  .rows li.next { border-color: var(--accent); }
  .rows li.playing { background: var(--live-soft); }
  .row-main { flex: 1; min-width: 0; display: flex; align-items: center; gap: 10px; min-height: 40px; padding: 4px 8px; font: inherit; text-align: left; color: var(--text); background: transparent; border: 0; cursor: pointer; }
  .when { flex: none; width: 5em; font-variant-numeric: tabular-nums; color: var(--muted); }
  .what { flex: 1; min-width: 0; display: flex; gap: 8px; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .what .muted { overflow: hidden; text-overflow: ellipsis; }
  .tag { flex: none; font-size: 12px; font-weight: 700; padding: 1px 6px; border-radius: 5px; background: var(--accent-soft); color: var(--accent); }
  .issues { padding: 8px 10px; border-radius: 8px; background: var(--surface-2); }
  .issues ul { margin: 4px 0 0; padding-left: 0; list-style: none; display: flex; flex-direction: column; gap: 4px; font-size: 14px; }
  .sev { font-weight: 700; margin-right: 6px; }
  .issues li.error .sev { color: var(--live); }
  .issues li.warning .sev { color: var(--warn-text, var(--muted)); }
  .ok { margin: 0; color: var(--ok, var(--accent)); font-weight: 600; }
  .actions { position: sticky; bottom: 0; padding: 8px 0; background: var(--surface); }
  summary { cursor: pointer; }
</style>

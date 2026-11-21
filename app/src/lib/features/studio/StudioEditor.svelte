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
  import BackgroundField from "./BackgroundField.svelte";
  import Waveform, { type Mark } from "./Waveform.svelte";
  import ToneControl from "./ToneControl.svelte";
  import { parseLyrics } from "./parse";
  import { dropTroubledTimings, expand, hasTroubledTimings, newId, sectionOf, sectionText, SECTION_KINDS, slideText, studioContent, withText } from "./song";
  import { clearEntry, clearStart, clearWords, distributeSlides, entryHeads, formatTime, markInSequence, markSection, markStart, markWord, slideAt, slideStarts, slideWords, wordsInSlide } from "./timeline";

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

  // Los campos que una canción más vieja no traiga empiezan vacíos.
  let song = $state<MediaSong>(
    untrack(() => {
      const copy = structuredClone($state.snapshot(initial)) as MediaSong;
      return { ...copy, reference: copy.reference ?? "" };
    }),
  );
  let issues = $state<SongIssue[] | null>(null);
  let saving = $state(false);
  let pasteOpen = $state(untrack(() => initial.sections.length === 0));
  let pasteText = $state("");
  let syncOpen = $state(false);
  /** Lo escrito en cada sección tal cual (con sus líneas en blanco a medio escribir); el modelo se lee de ahí. */
  let drafts = $state<Record<string, string>>({});
  /** La siguiente diapositiva desplegada que se marcará con Espacio. */
  let next = $state(0);
  /**
   * Qué marca Espacio: el comienzo de cada sección (la app reparte sus diapositivas: lo más fácil, sin saberse la
   * letra), de cada diapositiva, o de cada palabra (para iluminar la que suena).
   */
  let markBy = $state<"sections" | "slides" | "words">("sections");
  let byWord = $derived(markBy === "words");
  /** Velocidad al sincronizar (los tiempos siguen siendo los de la pista). */
  const RATES = [1, 0.75, 0.5];
  let syncNotice = $state<string | null>(null);
  /** En modo palabra: la siguiente palabra de la diapositiva `next`. */
  let nextWord = $state(0);

  let expanded = $derived(expand(song));
  let starts = $derived(slideStarts(expanded));
  /** Cuántas palabras de cada diapositiva tienen su tiempo. */
  let timedWords = $derived(slideWords(expanded).map((w) => (w ? w.filter((x) => x !== null).length : 0)));
  let playingRow = $derived(syncOpen ? slideAt(starts, player.positionMs) : null);
  let errors = $derived(issues?.filter((i) => i.severity === "error") ?? []);
  let ownTrack = $derived(song.audio !== null && player.path === song.audio.path);
  /** La pista de la canción en pantalla está sonando: el editor no la interrumpe. */
  let liveBusy = $derived(player.playing && onScreen.song !== null && !ownTrack);
  let heads = $derived(entryHeads(expanded));
  /** La onda de la pista cargada (se calcula una vez por pista). */
  let peaks = $derived(syncOpen && ownTrack && player.status === "ready" ? player.peaks(1200) : null);
  let marks = $derived.by((): Mark[] =>
    expanded.flatMap((e, k): Mark[] => {
      const start = starts[k];
      if (start === null) return [];
      const head = heads.includes(k);
      const auto = e.entry.timing?.slides.find((c) => c.slide === e.slide?.id)?.auto;
      return [{ ms: start, kind: head ? "section" : auto ? "auto" : "slide", label: head ? sectionName(song, e.section) : slideText(e.slide).split("\n")[0] }];
    }),
  );
  let autoRows = $derived(expanded.map((e) => !!e.entry.timing?.slides.find((c) => c.slide === e.slide?.id)?.auto));

  function snapshot(): MediaSong {
    return $state.snapshot(song) as MediaSong;
  }

  function preview(index = 0) {
    const content = studioContent(snapshot(), songCredit(song));
    onprepare(content.kind === "text" ? { ...content, index: Math.min(index, content.slides.length - 1) } : content);
  }

  /** Hay cambios sin guardar (para no perder, por ejemplo, los tiempos marcados al pulsar Cancelar). */
  let dirty = $state(false);
  let confirmDiscard = $state(false);
  let issuesBox = $state<HTMLElement>();

  function changed() {
    issues = null;
    dirty = true;
    confirmDiscard = false;
    preview();
  }

  // Mientras se sincroniza, «Preparado» va con la pista.
  $effect(() => {
    const row = playingRow;
    if (row !== null) untrack(() => preview(row));
  });

  // ---- Pista ----

  /** Mientras se elige la pista (sacar el audio de un video puede tardar unos segundos). */
  let picking = $state(false);

  async function pickAudio() {
    if (liveBusy || picking) return;
    picking = true;
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
    } finally {
      picking = false;
    }
  }

  async function openReference() {
    try {
      await platform.studioOpenReference(song.reference.trim());
    } catch (e) {
      onerror(toMessage(e));
    }
  }

  async function ensureTrack(): Promise<boolean> {
    if (!song.audio || liveBusy) return false;
    if (ownTrack && player.status === "ready") return true;
    const duration = await player.load(song.audio.path);
    if (duration !== null && duration !== song.audio.durationMs) song.audio.durationMs = duration;
    if (duration !== null) player.setTranspose(song.transpose ?? 0);
    return duration !== null;
  }

  /** El tono de la canción: se guarda con ella y, si su pista está cargada, se oye enseguida. */
  function setTone(semitones: number) {
    song.transpose = semitones;
    if (ownTrack) player.setTranspose(semitones);
    changed();
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
    // Los tiempos dicen dónde suena cada aparición en la grabación: se quedan. Con pista, la pista salta para
    // seguir el orden nuevo.
    [order[i], order[j]] = [order[j], order[i]];
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

  /** La primera fila de la sección de la fila k (al marcar por secciones, Espacio marca secciones). */
  function headOf(k: number): number {
    return [...heads].reverse().find((h) => h <= k) ?? 0;
  }

  function setMarkBy(mode: typeof markBy) {
    markBy = mode;
    nextWord = 0;
    if (mode === "sections") next = headOf(Math.min(next, expanded.length - 1));
  }

  /** Reparte con la duración de la pista. Devuelve cuántas repartió. */
  function spread(source: MediaSong): number {
    const { song: out, filled } = distributeSlides(source, song.audio?.durationMs ?? 0);
    song.order = out.order;
    return filled;
  }

  function distribute() {
    const filled = spread(snapshot());
    syncNotice = filled > 0 ? tCount("studio.distributed", filled) : t("studio.nothingToDistribute");
    issues = null;
    dirty = true;
  }

  function mark() {
    if (!ownTrack || next >= expanded.length) return;
    const slide = expanded[next].slide;
    syncNotice = null;
    if (markBy === "sections") {
      const k = headOf(next);
      // Marcar una sección también reparte: las anteriores quedan con su final y esta llega hasta la siguiente.
      spread(markSection(snapshot(), expanded, k, player.now()));
      next = heads.find((h) => h > k) ?? expanded.length;
      nextWord = 0;
    } else if (byWord && slide && wordsInSlide(slide.lines) > 0) {
      song.order = markWord(snapshot(), expanded, next, nextWord, player.now()).order;
      nextWord++;
      if (nextWord >= wordsInSlide(slide.lines)) {
        next++;
        nextWord = 0;
      }
    } else {
      song.order = markInSequence(snapshot(), expanded, next, player.now()).order;
      next++;
      nextWord = 0;
    }
    issues = null;
    dirty = true;
  }

  function unmark() {
    if (markBy === "sections") {
      const previous = [...heads].reverse().find((h) => h < next);
      if (previous === undefined) return;
      next = previous;
      spread(clearEntry(snapshot(), expanded[previous].entryIndex));
      issues = null;
      dirty = true;
      return;
    }
    // Por palabra, a mitad de una diapositiva: se borran sus palabras para marcarla otra vez desde la primera.
    if (byWord && nextWord > 0) {
      song.order = clearWords(snapshot(), expanded, next).order;
      nextWord = 0;
      issues = null;
      dirty = true;
      return;
    }
    if (next === 0) return;
    next--;
    song.order = clearStart(snapshot(), expanded, next).order;
    issues = null;
    dirty = true;
  }

  async function goRow(k: number) {
    next = markBy === "sections" ? headOf(k) : k;
    nextWord = 0;
    const start = starts[k];
    if (start !== null && (await ensureTrack())) player.seek(start);
    preview(k);
  }

  async function seekTo(ms: number) {
    if (await ensureTrack()) player.seek(ms);
  }

  function nudge(k: number, ms: number) {
    const start = starts[k];
    if (start === null) return;
    song.order = markStart(snapshot(), expanded, k, start + ms).order;
    issues = null;
    dirty = true;
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

  /** Quita solo las marcas con errores y vuelve a revisar: el resto de la sincronización se conserva. */
  async function dropTroubled() {
    if (!issues) return;
    song.order = dropTroubledTimings(snapshot(), issues).order;
    dirty = true;
    await validate();
  }

  async function save() {
    if (saving) return;
    saving = true;
    try {
      const outcome = await saveStudioSong(snapshot());
      issues = outcome.issues;
      if (outcome.saved) {
        dirty = false;
        close(snapshot());
      } else {
        // No se guardó: que se vea por qué.
        requestAnimationFrame(() => issuesBox?.scrollIntoView({ block: "center", behavior: "smooth" }));
      }
    } catch (e) {
      onerror(toMessage(e));
    } finally {
      saving = false;
    }
  }

  function cancel() {
    if (dirty && !confirmDiscard) {
      confirmDiscard = true;
      return;
    }
    close(null);
  }

  function close(saved: MediaSong | null) {
    // La pista del editor se para; la de la canción en pantalla sigue sonando.
    if (ownTrack && onScreen.song?.audio?.path !== player.path) player.pause();
    player.setRate(1);
    // Fuera del editor, el tono es el de la canción en pantalla (si es esta pista) o el original.
    if (ownTrack) player.setTranspose(onScreen.song?.audio?.path === player.path ? (onScreen.song?.transpose ?? 0) : 0);
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
      <button type="button" class="btn" onclick={pickAudio} disabled={liveBusy || picking}>{t(song.audio ? "studio.changeTrack" : "studio.pickTrack")}</button>
      {#if song.audio && song.mode === "live"}<span class="muted small">{t("studio.trackKept")}</span>{/if}
    </div>
    {#if song.audio}
      <ToneControl value={song.transpose ?? 0} onchange={setTone} disabled={liveBusy} />
      <p class="muted small">{t("studio.tone.hint")}</p>
    {/if}
    {#if picking}<p class="muted small" role="status">{t("studio.preparingTrack")}</p>{/if}
    {#if liveBusy}<p class="muted small">{t("studio.liveBusy")}</p>{/if}
    <div class="row">
      <label class="field-row grow">
        <span class="label">{t("studio.reference")}</span>
        <input class="field" type="url" bind:value={song.reference} oninput={() => (issues = null)} placeholder="https://www.youtube.com/watch?v=…" />
      </label>
      <button type="button" class="btn reference-open" onclick={openReference} disabled={!/^https?:\/\/\S{4,}$/i.test(song.reference.trim())}>{t("studio.openReference")}</button>
    </div>
    <p class="muted small">{t("studio.referenceHint")}</p>
    <BackgroundField value={song.background} inherit={t("studio.bg.inheritGeneral")} onchange={(path) => ((song.background = path), changed())} {onerror} />
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
        <BackgroundField compact value={section.background} inherit={t("studio.bg.inheritSong")} onchange={(path) => ((section.background = path), changed())} {onerror} />
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
        <div class="row">
          <span class="label">{t("studio.markBy")}</span>
          <div class="segmented" role="group">
            <button type="button" aria-pressed={markBy === "sections"} onclick={() => setMarkBy("sections")}>{t("studio.markBy.sections")}</button>
            <button type="button" aria-pressed={markBy === "slides"} onclick={() => setMarkBy("slides")}>{t("studio.markBy.slides")}</button>
            <button type="button" aria-pressed={markBy === "words"} onclick={() => setMarkBy("words")}>{t("studio.markBy.words")}</button>
          </div>
          <span class="label">{t("studio.speed")}</span>
          <div class="segmented" role="group" title={t("studio.speedHint")}>
            {#each RATES as rate (rate)}
              <button type="button" aria-pressed={player.rate === rate} onclick={() => player.setRate(rate)}>{Math.round(rate * 100)} %</button>
            {/each}
          </div>
        </div>
        <p class="muted small">{t(markBy === "words" ? "studio.syncHintWords" : markBy === "sections" ? "studio.syncHintSections" : "studio.syncHint")}</p>
        <Waveform {peaks} durationMs={song.audio.durationMs} positionMs={ownTrack ? player.positionMs : 0} {marks} onseek={seekTo} />
        <div class="row transport">
          <button type="button" class="btn primary" onclick={togglePlay} disabled={liveBusy || (ownTrack && player.status === "loading")}>
            {ownTrack && player.playing ? `⏸ ${t("studio.pause")}` : `▶ ${t("studio.play")}`}
          </button>
          <button type="button" class="btn" onclick={() => back(5)} disabled={liveBusy}>{t("studio.back5")}</button>
          <span class="time">{ownTrack ? formatTime(player.positionMs) : formatTime(0)} / {formatTime(song.audio.durationMs)}</span>
          <button type="button" class="btn" onclick={mark} disabled={!ownTrack || next >= expanded.length}>{t("studio.markNext")}</button>
          <button type="button" class="btn ghost" onclick={unmark} disabled={next === 0}>{t("studio.unmark")}</button>
          {#if markBy !== "words"}
            <button type="button" class="btn ghost" onclick={distribute} title={t("studio.distributeHint")}>{t("studio.distribute")}</button>
          {/if}
        </div>
        {#if syncNotice}<p class="notice small" role="status">{syncNotice}</p>{/if}
        <ol class="rows">
          {#each expanded as e, k (`${e.entry.id}-${e.slide?.id ?? "inst"}`)}
            <li class:playing={playingRow === k} class:next={next === k}>
              <button type="button" class="row-main" onclick={() => goRow(k)}>
                <span class="when" class:auto={autoRows[k]} title={autoRows[k] ? t("studio.autoTime") : undefined}>{starts[k] === null ? "—" : formatTime(starts[k]!)}</span>
                <span class="what">
                  <strong>{sectionName(song, e.section)}</strong>
                  {#if byWord && next === k && e.slide}
                    <!-- La letra entera, con la palabra que toca marcar destacada. -->
                    <span class="words">{#each e.slide.lines.join(" ").split(/\s+/).filter(Boolean) as w, wi (wi)}<span class:target={wi === nextWord} class:done={wi < nextWord}>{w}</span>{" "}{/each}</span>
                  {:else}
                    <span class="muted">{e.slide ? slideText(e.slide).split("\n")[0] : t("studio.instrumental")}</span>
                  {/if}
                </span>
                {#if e.slide && timedWords[k] > 0}<span class="badge-words" title={t("studio.wordsTimed")}>{timedWords[k]}/{wordsInSlide(e.slide.lines)}</span>{/if}
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
    <div class="issues" role="status" bind:this={issuesBox}>
      {#if issues.length === 0}
        <p class="ok">✓ {t("studio.valid")}</p>
      {:else}
        <p class="small">{errors.length > 0 ? t("studio.hasErrors") : t("studio.onlyWarnings")}</p>
        {#if hasTroubledTimings(issues)}
          <button type="button" class="btn" onclick={dropTroubled}>{t("studio.dropTroubled")}</button>
        {/if}
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
    {#if confirmDiscard}<span class="small discard">{t("studio.discardConfirm")}</span>{/if}
    <button type="button" class="btn ghost" class:danger={confirmDiscard} onclick={cancel}>{t(confirmDiscard ? "studio.discard" : "studio.cancel")}</button>
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
  .when.auto { font-style: italic; opacity: 0.7; }
  .notice { margin: 0; color: var(--accent); font-weight: 600; }
  .when { flex: none; width: 5em; font-variant-numeric: tabular-nums; color: var(--muted); }
  .what { flex: 1; min-width: 0; display: flex; gap: 8px; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .what .muted { overflow: hidden; text-overflow: ellipsis; }
  .words { white-space: normal; line-height: 1.5; }
  .words .done { color: var(--muted); }
  .words .target { font-weight: 800; text-decoration: underline; text-decoration-color: var(--accent); text-decoration-thickness: 3px; }
  .badge-words { flex: none; font-size: 12px; font-variant-numeric: tabular-nums; color: var(--muted); }
  .tag { flex: none; font-size: 12px; font-weight: 700; padding: 1px 6px; border-radius: 5px; background: var(--accent-soft); color: var(--accent); }
  .issues { padding: 8px 10px; border-radius: 8px; background: var(--surface-2); }
  .issues ul { margin: 4px 0 0; padding-left: 0; list-style: none; display: flex; flex-direction: column; gap: 4px; font-size: 14px; }
  .sev { font-weight: 700; margin-right: 6px; }
  .issues li.error .sev { color: var(--live); }
  .issues li.warning .sev { color: var(--warn-text, var(--muted)); }
  .ok { margin: 0; color: var(--ok, var(--accent)); font-weight: 600; }
  .actions { position: sticky; bottom: 0; padding: 8px 0; background: var(--surface); }
  summary { cursor: pointer; }
  .reference-open { align-self: flex-end; }
  .discard { color: var(--live); font-weight: 600; }
</style>

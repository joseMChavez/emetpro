<script lang="ts">
  // Plan: lo que se va a presentar en un culto, preparado de antemano y en orden (D-019).
  // Sin plan abierto se ven todos los planes; con uno abierto, se edita y se usa en el mismo sitio:
  // un clic prepara el elemento, doble clic lo envía a pantalla, «Siguiente» prepara el que sigue.
  import { untrack } from "svelte";
  import { t, tCount, i18n } from "$lib/i18n/index.svelte";
  import { toMessage, type Message } from "$lib/core/messages";
  import { library } from "$lib/features/songs/library.svelte";
  import { bibles, currentBibles, visibleBibles } from "$lib/features/bible/bibles.svelte";
  import { bookLanguage } from "$lib/features/bible/scripture";
  import { parseList, referenceLabel, scriptureContent, type ScriptureItem } from "./scripture";
  import { baseName, contentOf, detail, firstLine, icon, markShown, problem, session, textSlides, title } from "./items.svelte";
  import RichEditor from "$lib/features/richtext/RichEditor.svelte";
  import { hiddenAt, sectionOf, summaries } from "./sections";
  import { folded, setFolded, toggleFold, unfold } from "./fold.svelte";
  import PlanExport from "./PlanExport.svelte";
  import PlanImport from "./PlanImport.svelte";
  import type { PlanImportPreview } from "$lib/platform";
  import { plainSlide, type RichSlide } from "$lib/features/richtext/rich";
  import { platform, type MediaFile, type OutputContent } from "$lib/platform";
  import {
    choosePlan,
    createPlan,
    loadPlans,
    deletePlan,
    duplicatePlan,
    missingFiles,
    openPlan,
    plans,
    savePlan,
    today,
    type Plan,
    type PlanItem,
  } from "./plans.svelte";

  let {
    live,
    onprepare,
    onsend,
    onerror,
  }: {
    live: OutputContent;
    onprepare: (content: OutputContent) => void;
    onsend: (content: OutputContent) => void;
    onerror: (message: Message) => void;
  } = $props();

  const PAUSE_PRESETS = [5, 10, 15, 20, 30];
  const MAX_MINUTES = 600;

  type Pause = Extract<PlanItem, { kind: "pause" }>;
  type TextItem = Extract<PlanItem, { kind: "text" }>;

  let plan = $derived(openPlan.plan);
  let confirmDelete = $state<string | null>(null);
  let showPast = $state(false);

  /** Qué se está añadiendo o editando debajo de la lista. */
  let adding = $state<"song" | "scripture" | null>(null);
  let songFilter = $state("");
  /** La lista de citas que se está escribiendo o pegando, y con qué versión se mostrarán ("" = la de la consola). */
  let scriptureText = $state("");
  let scriptureBible = $state("");
  let addingScripture = $state(false);
  let scriptureLines = $derived(parseList(scriptureText, bookLanguage(currentBibles().bible)));
  let scriptureReady = $derived(scriptureLines.filter((l) => l.reference !== null).length);
  /** Pausa en el formulario: nueva (sin id en el plan) o una que se edita. */
  let pauseDraft = $state<{ id: string; title: string; minutes: number; seconds: number; background: string | null; isNew: boolean } | null>(null);
  /** Texto con formato en el formulario (D-027): nuevo o uno que se edita. */
  let textDraft = $state<{ id: string; title: string; slides: RichSlide[]; marks?: TextItem["marks"]; isNew: boolean } | null>(null);
  /** Diapositiva del texto que se está escribiendo, para la vista previa. */
  let textIndex = $state(0);
  /** Exportando el plan (sectionId null) o una sección a un .emetplan (D-028). */
  let exporting = $state<{ sectionId: string | null; label: string; fileName: string } | null>(null);
  /** Un .emetplan leído, esperando dónde ponerlo. */
  let importing = $state<PlanImportPreview | null>(null);
  /** Sección cuyo título se está escribiendo. */
  let editingSection = $state<string | null>(null);

  /** Copiando medios elegidos a la carpeta de la app. */
  let copying = $state(false);
  /** Aviso breve tras añadir algo («Admirable» añadida al plan). */
  let notice = $state<string | null>(null);
  let noticeTimer: ReturnType<typeof setTimeout> | undefined;

  function notify(text: string) {
    notice = text;
    clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => (notice = null), 3000);
  }

  const now = today();
  let upcoming = $derived(plans.all.filter((p) => p.date === null || p.date >= now));
  let past = $derived(plans.all.filter((p) => p.date !== null && p.date < now).reverse());
  let presentable = $derived(plan ? plan.items.filter((i) => i.kind !== "section") : []);

  // Secciones como acordeón: plegadas, solo se ve su cabecera con un resumen.
  let foldedNow = $derived(plan ? folded(plan.id) : new Set<string>());
  let hidden = $derived(plan ? hiddenAt(plan.items, foldedNow) : []);
  let sectionInfo = $derived.by(() => {
    if (!plan) return new Map();
    const shown = session.shown[plan.id] ?? [];
    return summaries(plan.items, (id) => shown.includes(id), session.current?.id ?? null);
  });
  let sectionIds = $derived(plan ? plan.items.filter((i) => i.kind === "section").map((i) => i.id) : []);
  let allFolded = $derived(sectionIds.length > 0 && sectionIds.every((id) => foldedNow.has(id)));

  function foldAll() {
    if (plan) setFolded(plan.id, allFolded ? [] : sectionIds);
  }

  // Lo que se añade (canciones, citas, medios, importado…) va a la última sección: si estaba plegada, se abre
  // para que se vea lo que acaba de entrar.
  let knownPlan: string | null = null;
  let knownIds = new Set<string>();
  $effect(() => {
    const current = plan;
    if (!current) return;
    const ids = current.items.map((i) => i.id);
    untrack(() => {
      if (knownPlan === current.id) for (const id of ids) if (!knownIds.has(id)) unfold(current.id, sectionOf(current.items, id));
      knownPlan = current.id;
      knownIds = new Set(ids);
    });
  });
  let songMatches = $derived.by(() => {
    const q = songFilter.trim().toLocaleLowerCase();
    const songs = q ? library.songs.filter((s) => [s.title, s.singer, s.body].some((f) => f.toLocaleLowerCase().includes(q))) : library.songs;
    return songs.slice(0, 50);
  });

  function formatDate(date: string | null): string {
    if (!date) return t("plans.noDate");
    const [y, m, d] = date.split("-").map(Number);
    return new Date(y, m - 1, d).toLocaleDateString(i18n.locale, { weekday: "long", day: "numeric", month: "long", year: "numeric" });
  }

  async function prepareItem(item: PlanItem, send = false) {
    if (item.kind === "section" || problem(item)) return;
    try {
      const content = await contentOf(item);
      if (!content && item.kind === "scripture") {
        onerror({ key: "plans.verseNotFound", params: { reference: title(item) } });
        return;
      }
      if (!content || content.kind === "empty") return;
      session.current = { id: item.id, content };
      // «Siguiente» puede entrar en una sección plegada: se abre para que se vea qué se preparó.
      if (plan) unfold(plan.id, sectionOf(plan.items, item.id));
      if (send) onsend(content);
      else onprepare(content);
    } catch (e) {
      onerror(toMessage(e));
    }
  }

  /** Prepara el siguiente elemento que se pueda mostrar. */
  function prepareNext() {
    const current = session.current;
    const start = current ? presentable.findIndex((i) => i.id === current.id) + 1 : 0;
    const next = presentable.slice(start).find((i) => !problem(i));
    if (next) prepareItem(next);
  }

  // Cuando lo preparado desde el plan llega a la pantalla, se marca como mostrado.
  $effect(() => {
    const onScreen = live;
    const id = plan?.id;
    if (id) untrack(() => markShown(id, onScreen));
  });

  // --- Editar el plan ---

  /** Animaciones en el plan y sus secciones (D-024): null = como lo de arriba. */
  const toChoice = (value: boolean | null | undefined) => (value === true ? "on" : value === false ? "off" : "inherit");
  const fromChoice = (value: string) => (value === "on" ? true : value === "off" ? false : null);

  function setSectionAnimate(id: string, value: string) {
    if (!plan) return;
    setItems(plan.items.map((i) => (i.id === id && i.kind === "section" ? { ...i, animate: fromChoice(value) } : i)));
  }

  function update(change: Partial<Plan>) {
    if (plan) savePlan({ ...plan, ...change });
  }

  function setItems(items: PlanItem[]) {
    update({ items });
  }

  function move(i: number, delta: -1 | 1) {
    if (!plan) return;
    const items = [...plan.items];
    const j = i + delta;
    if (j < 0 || j >= items.length) return;
    [items[i], items[j]] = [items[j], items[i]];
    setItems(items);
    // Al cruzar la cabecera de una sección plegada, lo movido no desaparece: se abre donde cae.
    unfold(plan.id, sectionOf(items, items[j].id));
  }

  function removeItem(id: string) {
    if (plan) setItems(plan.items.filter((i) => i.id !== id));
  }

  function addSection() {
    if (!plan) return;
    const id = crypto.randomUUID();
    setItems([...plan.items, { id, kind: "section", title: t("plans.newSection") }]);
    editingSection = id;
  }

  function renameSection(id: string, value: string) {
    if (!plan) return;
    editingSection = null;
    const name = value.trim();
    if (!name) return;
    setItems(plan.items.map((i) => (i.id === id && i.kind === "section" ? { ...i, title: name } : i)));
  }

  function addSong(songId: string) {
    if (!plan) return;
    setItems([...plan.items, { id: crypto.randomUUID(), kind: "song", songId }]);
    const song = library.songs.find((s) => s.id === songId);
    notify(t("plans.addedSong", { title: song?.title ?? "", plan: plan.name }));
  }

  async function addMedia() {
    if (!plan) return;
    copying = true;
    try {
      // Se guardan copias en la carpeta de EmetPro: el plan sigue sirviendo aunque se mueva o borre el original.
      const picked: MediaFile[] = await platform.planPickMedia(t("dialog.title"), t("dialog.filter"));
      // El plan pudo cambiar mientras el diálogo estaba abierto: se añade al que está abierto ahora.
      const target = openPlan.plan;
      if (!target || picked.length === 0) return;
      for (const file of picked) missingFiles.delete(file.path);
      savePlan({
        ...target,
        items: [...target.items, ...picked.map((f): PlanItem => ({ id: crypto.randomUUID(), kind: "media", path: f.path, media: f.kind }))],
      });
      notify(picked.length === 1 ? t("plans.addedMedia", { name: picked[0].name }) : t("plans.addedMediaMany", { count: picked.length }));
    } catch (e) {
      onerror(toMessage(e));
    } finally {
      copying = false;
    }
  }

  /** Añade las citas reconocidas. Las que la versión no tiene (Juan 3:99) no entran y se avisa. */
  async function addScripture() {
    if (!plan) return;
    addingScripture = true;
    try {
      const bible = scriptureBible || null;
      const found: ScriptureItem[] = [];
      const notFound: string[] = [];
      for (const line of scriptureLines) {
        if (!line.reference) continue;
        const { book, chapter, verseStart, verseEnd } = line.reference;
        const item: ScriptureItem = { id: crypto.randomUUID(), kind: "scripture", book, chapter, verseStart, verseEnd, bible };
        const ok = await scriptureContent(item).catch(() => null);
        if (ok) found.push(item);
        else notFound.push(line.text);
      }
      const target = openPlan.plan;
      if (target && found.length > 0) savePlan({ ...target, items: [...target.items, ...found] });
      if (notFound.length > 0) {
        onerror({ key: "plans.versesNotFound", params: { list: notFound.join("; ") } });
        // Quedan escritas las que no entraron, para corregirlas.
        scriptureText = notFound.join("\n");
      } else {
        scriptureText = "";
        adding = null;
      }
      if (found.length > 0) notify(t("plans.addedVerses", { count: found.length }));
    } finally {
      addingScripture = false;
    }
  }

  function newPause(minutes: number) {
    pauseDraft = { id: crypto.randomUUID(), title: t("plans.pauseDefault"), minutes, seconds: 0, background: null, isNew: true };
    adding = null;
    textDraft = null;
  }

  function editPause(item: Pause) {
    textDraft = null;
    const total = Math.round(item.durationMs / 1000);
    pauseDraft = {
      id: item.id,
      title: item.title,
      minutes: Math.floor(total / 60),
      seconds: total % 60,
      background: item.background ?? null,
      isNew: false,
    };
    adding = null;
  }

  let draftMs = $derived(
    pauseDraft
      ? (Math.min(MAX_MINUTES, Math.max(0, Math.floor(pauseDraft.minutes || 0))) * 60 + Math.min(59, Math.max(0, Math.floor(pauseDraft.seconds || 0)))) * 1000
      : 0,
  );

  function savePause() {
    if (!plan || !pauseDraft || draftMs === 0) return;
    const item: PlanItem = {
      id: pauseDraft.id,
      kind: "pause",
      title: pauseDraft.title.trim(),
      durationMs: draftMs,
      background: pauseDraft.background,
    };
    const exists = plan.items.some((i) => i.id === item.id);
    setItems(exists ? plan.items.map((i) => (i.id === item.id ? item : i)) : [...plan.items, item]);
    if (!exists) notify(t("plans.addedPause", { name: item.title || t("plans.pause") }));
    pauseDraft = null;
  }

  async function pickPauseBackground() {
    try {
      const path = await platform.planPickBackground(t("background.dialogTitle"), t("background.dialogFilter"));
      if (path && pauseDraft) {
        missingFiles.delete(path);
        pauseDraft.background = path;
      }
    } catch (e) {
      onerror(toMessage(e));
    }
  }

  function newText() {
    textDraft = { id: crypto.randomUUID(), title: "", slides: [[]], isNew: true };
    textIndex = 0;
    adding = null;
    pauseDraft = null;
  }

  function editText(item: TextItem) {
    textDraft = { id: item.id, title: item.title, slides: $state.snapshot(item.slides) as RichSlide[], marks: item.marks, isNew: false };
    textIndex = 0;
    adding = null;
    pauseDraft = null;
  }

  /** Mientras se escribe, «Preparado» muestra la diapositiva en la que se está. */
  function previewText() {
    if (!textDraft) return;
    const slides = textSlides($state.snapshot(textDraft.slides) as RichSlide[]);
    if (slides.length === 0) return onprepare({ kind: "empty" });
    const before = textSlides(textDraft.slides.slice(0, textIndex + 1)).length;
    onprepare({ kind: "text", slides: slides.map(plainSlide), rich: slides, index: Math.max(0, Math.min(slides.length - 1, before - 1)) });
  }

  function saveText() {
    if (!plan || !textDraft) return;
    const slides = textSlides($state.snapshot(textDraft.slides) as RichSlide[]);
    if (slides.length === 0) return;
    const item: PlanItem = { id: textDraft.id, kind: "text", title: textDraft.title.trim(), slides };
    // Las marcas se guardan por número de diapositiva: si el texto cambió, las de palabras que ya no están no se pintan.
    if (textDraft.marks) item.marks = textDraft.marks;
    const exists = plan.items.some((i) => i.id === item.id);
    setItems(exists ? plan.items.map((i) => (i.id === item.id ? item : i)) : [...plan.items, item]);
    if (!exists) notify(t("plans.addedText", { plan: plan.name }));
    textDraft = null;
  }

  // --- Exportar e importar (D-028) ---

  function exportPlan(sectionId: string | null, sectionTitle?: string) {
    if (!plan) return;
    textDraft = null;
    pauseDraft = null;
    adding = null;
    exporting = sectionId
      ? { sectionId, label: t("planfile.sectionLabel", { section: sectionTitle ?? "" }), fileName: `${plan.name} - ${sectionTitle ?? ""}` }
      : { sectionId: null, label: t("planfile.planLabel", { plan: plan.name }), fileName: plan.name };
  }

  async function pickImport() {
    try {
      const preview = await platform.planImportPick(t("planfile.openTitle"), t("planfile.filter"));
      if (!preview) return;
      exporting = null;
      textDraft = null;
      pauseDraft = null;
      adding = null;
      importing = preview;
    } catch (e) {
      onerror(toMessage(e));
    }
  }

  async function imported(planId: string, text: string) {
    importing = null;
    await loadPlans();
    choosePlan(planId);
    session.current = null;
    notify(text);
  }

  // --- Lista de planes ---

  function newPlan() {
    createPlan(t("plans.untitled"), today());
    session.current = null;
  }

  function open(id: string) {
    choosePlan(id);
    session.current = null;
    adding = null;
    pauseDraft = null;
    textDraft = null;
    exporting = null;
    importing = null;
    confirmDelete = null;
  }

  function remove(id: string) {
    if (confirmDelete !== id) {
      confirmDelete = id;
      return;
    }
    deletePlan(id);
    confirmDelete = null;
  }

  function countLabel(p: Plan): string {
    return t("plans.itemCount", { count: p.items.filter((i) => i.kind !== "section").length });
  }
</script>

{#snippet planRow(p: Plan)}
  <li>
    <button class="item" onclick={() => open(p.id)}>
      <span class="name">
        {p.name}
        <span class="muted small sub">{formatDate(p.date)} · {countLabel(p)}</span>
      </span>
    </button>
    <button class="btn ghost small-btn" onclick={() => duplicatePlan(p, t("plans.copyName", { name: p.name }))}>{t("plans.duplicate")}</button>
    <button class="icon-btn remove" class:confirm={confirmDelete === p.id} onclick={() => remove(p.id)} aria-label={t("plans.delete", { name: p.name })} title={t("plans.delete", { name: p.name })}>
      {confirmDelete === p.id ? t("songs.confirmDelete") : "✕"}
    </button>
  </li>
{/snippet}

<div class="plan-panel">
  {#if !plan}
    <div class="actions">
      <button class="btn primary" onclick={newPlan}>{t("plans.new")}</button>
      <button class="btn" onclick={pickImport}>{t("planfile.importButton")}</button>
      <span class="muted small">{t("plans.intro")}</span>
    </div>
    {#if notice}<p class="notice small" role="status">✓ {notice}</p>{/if}
    {#if importing}
      {#key importing}<PlanImport preview={importing} appendDefault={null} onclose={() => (importing = null)} onimported={imported} {onerror} />{/key}
    {/if}
    {#if plans.all.length === 0}
      <p class="muted">{t("plans.empty")}</p>
    {:else}
      {#if upcoming.length > 0}
        <ul class="items">
          {#each upcoming as p (p.id)}{@render planRow(p)}{/each}
        </ul>
      {:else}
        <p class="muted">{t("plans.noUpcoming")}</p>
      {/if}
      {#if past.length > 0}
        <details bind:open={showPast}>
          <summary class="muted">{t("plans.past", { count: past.length })}</summary>
          <ul class="items">
            {#each past as p (p.id)}{@render planRow(p)}{/each}
          </ul>
        </details>
      {/if}
    {/if}
  {:else}
    <div class="head">
      <button class="btn ghost" onclick={() => choosePlan(null)}>← {t("plans.all")}</button>
      <button class="btn ghost" onclick={() => exportPlan(null)} title={t("planfile.exportHint")}>{t("planfile.exportButton")}</button>
      <button class="btn ghost" onclick={pickImport} title={t("planfile.importHereHint")}>{t("planfile.importHere")}</button>
      <button class="btn primary next" onclick={prepareNext} disabled={presentable.length === 0}>{t("plans.next")} ▶</button>
    </div>

    <div class="meta">
      <input
        class="field plan-name"
        value={plan.name}
        onchange={(e) => update({ name: e.currentTarget.value.trim() || t("plans.untitled") })}
        aria-label={t("plans.name")}
        maxlength="80"
      />
      <input
        class="field"
        type="date"
        value={plan.date ?? ""}
        onchange={(e) => update({ date: e.currentTarget.value || null })}
        aria-label={t("plans.date")}
      />
    </div>
    <label class="anim-row">
      <span class="muted small">{t("anim.planLabel")}</span>
      <select class="field compact" value={toChoice(plan.animate)} onchange={(e) => update({ animate: fromChoice(e.currentTarget.value) })}>
        <option value="inherit">{t("anim.cascade.general")}</option>
        <option value="on">{t("anim.cascade.on")}</option>
        <option value="off">{t("anim.cascade.off")}</option>
      </select>
    </label>
    <details class="notes" open={plan.notes !== ""}>
      <summary class="muted small">{t("plans.notes")}</summary>
      <textarea
        class="field"
        rows="2"
        value={plan.notes}
        onchange={(e) => update({ notes: e.currentTarget.value })}
        placeholder={t("plans.notesPlaceholder")}
      ></textarea>
    </details>

    {#if plan.items.length === 0}
      <p class="muted">{t("plans.emptyPlan")}</p>
    {:else}
      <div class="list-head">
        <p class="muted small">{t("plans.hint")}</p>
        {#if sectionIds.length > 1}
          <button class="btn ghost small-btn" onclick={foldAll}>{allFolded ? t("plans.unfoldAll") : t("plans.foldAll")}</button>
        {/if}
      </div>
      <ol class="items">
        {#each plan.items as item, i (item.id)}
          {@const issue = problem(item)}
          {@const done = (session.shown[plan.id] ?? []).includes(item.id)}
          {#if item.kind === "section"}
            {@const isFolded = foldedNow.has(item.id)}
            {@const info = sectionInfo.get(item.id)}
            <li class="section" class:folded={isFolded}>
              {#if editingSection === item.id}
                <!-- svelte-ignore a11y_autofocus -->
                <input
                  class="field section-input"
                  value={item.title}
                  autofocus
                  onfocus={(e) => e.currentTarget.select()}
                  onblur={(e) => renameSection(item.id, e.currentTarget.value)}
                  onkeydown={(e) => {
                    if (e.key === "Enter") e.currentTarget.blur();
                    if (e.key === "Escape") editingSection = null;
                  }}
                  aria-label={t("plans.sectionName")}
                />
              {:else}
                <button
                  class="section-title"
                  onclick={() => toggleFold(plan.id, item.id)}
                  aria-expanded={!isFolded}
                  title={t(isFolded ? "plans.unfoldSection" : "plans.foldSection", { section: item.title })}
                >
                  <span class="chev" aria-hidden="true"></span>
                  <span class="section-name">{item.title}</span>
                  {#if isFolded && info}
                    <span class="section-sum">
                      {tCount("plans.sectionItems", info.count)}{#if info.shown > 0}<span class="sum-shown" title={t("plans.sectionShown", { shown: info.shown, count: info.count })}>✓ {info.shown}</span>{/if}
                    </span>
                    {#if info.current}<span class="section-dot" title={t("plans.sectionCurrent")} aria-label={t("plans.sectionCurrent")}></span>{/if}
                  {/if}
                </button>
              {/if}
              <!-- Plegada, su sitio es para el resumen: la animación se cambia con la sección abierta. -->
              {#if !isFolded}
                <select
                  class="section-anim"
                  class:set={item.animate != null}
                  value={toChoice(item.animate)}
                  onchange={(e) => setSectionAnimate(item.id, e.currentTarget.value)}
                  aria-label="{t('anim.planLabel')} · {item.title}"
                  title={t("anim.planLabel")}
                >
                  <option value="inherit">✨ {t("anim.cascade.plan")}</option>
                  <option value="on">✨ {t("anim.cascade.on")}</option>
                  <option value="off">⏸ {t("anim.cascade.off")}</option>
                </select>
              {/if}
              <button class="icon-btn" onclick={() => (editingSection = item.id)} aria-label={t("plans.renameSection")} title={t("plans.renameSection")}>✎</button>
              <button class="icon-btn" onclick={() => exportPlan(item.id, item.title)} aria-label={t("planfile.exportSection", { section: item.title })} title={t("planfile.exportSection", { section: item.title })}>⤓</button>
              <button class="icon-btn" onclick={() => move(i, -1)} disabled={i === 0} aria-label={t("songs.moveUp")}>▲</button>
              <button class="icon-btn" onclick={() => move(i, 1)} disabled={i === plan.items.length - 1} aria-label={t("songs.moveDown")}>▼</button>
              <button class="icon-btn remove" onclick={() => removeItem(item.id)} aria-label={t("plans.remove", { name: item.title })}>✕</button>
            </li>
          {:else if !hidden[i]}
            <li class:chosen={session.current?.id === item.id} class:broken={issue !== null}>
              <button
                class="item"
                onclick={() => prepareItem(item)}
                ondblclick={() => prepareItem(item, true)}
                disabled={issue !== null}
                title={item.kind === "media" ? item.path : undefined}
              >
                <span class="icon" aria-hidden="true">{icon(item)}</span>
                <span class="name">
                  {title(item)}
                  <span class="small sub" class:muted={!issue}>
                    {#if session.converting === item.id}{t("media.converting")}{:else}{issue ?? detail(item)}{/if}
                  </span>
                </span>
                {#if done}<span class="done" title={t("plans.shown")} aria-label={t("plans.shown")}>✓</span>{/if}
              </button>
              {#if item.kind === "pause"}
                <button class="icon-btn" onclick={() => editPause(item)} aria-label={t("plans.editPause")} title={t("plans.editPause")}>✎</button>
              {:else if item.kind === "text"}
                <button class="icon-btn" onclick={() => editText(item)} aria-label={t("plans.editText")} title={t("plans.editText")}>✎</button>
              {/if}
              <button class="icon-btn" onclick={() => move(i, -1)} disabled={i === 0} aria-label={t("songs.moveUp")}>▲</button>
              <button class="icon-btn" onclick={() => move(i, 1)} disabled={i === plan.items.length - 1} aria-label={t("songs.moveDown")}>▼</button>
              <button class="icon-btn remove" onclick={() => removeItem(item.id)} aria-label={t("plans.remove", { name: title(item) })}>✕</button>
            </li>
          {/if}
        {/each}
      </ol>
    {/if}

    {#if notice}<p class="notice small" role="status">✓ {notice}</p>{/if}

    {#if importing}
      {#key importing}<PlanImport preview={importing} appendDefault={plan.id} onclose={() => (importing = null)} onimported={imported} {onerror} />{/key}
    {:else if exporting}
      {#key exporting}
        <PlanExport
          planId={plan.id}
          sectionId={exporting.sectionId}
          label={exporting.label}
          fileName={exporting.fileName}
          onclose={() => (exporting = null)}
          {onerror}
        />
      {/key}
    {:else if textDraft}
      <form class="box" onsubmit={(e) => { e.preventDefault(); saveText(); }}>
        <strong>{t(textDraft.isNew ? "plans.addText" : "plans.editText")}</strong>
        <label class="field-row">
          <span class="label">{t("plans.textTitle")}</span>
          <input class="field" bind:value={textDraft.title} maxlength="80" placeholder={firstLine(textDraft.slides) || t("plans.textTitlePlaceholder")} />
        </label>
        {#key textDraft.id}
          <RichEditor
            slides={textDraft.slides}
            onchange={(next) => {
              if (textDraft) textDraft.slides = next;
              previewText();
            }}
            onfocusslide={(i) => {
              textIndex = i;
              previewText();
            }}
          />
        {/key}
        <div class="row end">
          <button type="button" class="btn ghost" onclick={() => (textDraft = null)}>{t("songs.cancel")}</button>
          <button type="submit" class="btn primary" disabled={textSlides(textDraft.slides).length === 0}>{t(textDraft.isNew ? "plans.add" : "songs.save")}</button>
        </div>
      </form>
    {:else if pauseDraft}
      <form class="box" onsubmit={(e) => { e.preventDefault(); savePause(); }}>
        <strong>{t(pauseDraft.isNew ? "plans.addPause" : "plans.editPause")}</strong>
        <label class="field-row">
          <span class="label">{t("clock.label")}</span>
          <input class="field" bind:value={pauseDraft.title} maxlength="60" placeholder={t("clock.labelPlaceholder")} />
        </label>
        <div class="row">
          <label class="field-row">
            <span class="label">{t("clock.min")}</span>
            <input class="field num-field" type="number" min="0" max={MAX_MINUTES} bind:value={pauseDraft.minutes} />
          </label>
          <span class="colon">:</span>
          <label class="field-row">
            <span class="label">{t("clock.sec")}</span>
            <input class="field num-field" type="number" min="0" max="59" bind:value={pauseDraft.seconds} />
          </label>
        </div>
        <div class="row">
          <span class="label">{t("songs.background")}</span>
          {#if pauseDraft.background}
            <span class="muted small bg-name" title={pauseDraft.background}>{baseName(pauseDraft.background)}</span>
            <button type="button" class="btn ghost" onclick={() => pauseDraft && (pauseDraft.background = null)}>{t("background.remove")}</button>
          {:else}
            <span class="muted small">{t("plans.pauseBackgroundDefault")}</span>
          {/if}
          <button type="button" class="btn" onclick={pickPauseBackground}>{t("background.pick")}</button>
        </div>
        <p class="muted small">{t("plans.pauseHint")}</p>
        <div class="row end">
          <button type="button" class="btn ghost" onclick={() => (pauseDraft = null)}>{t("songs.cancel")}</button>
          <button type="submit" class="btn primary" disabled={draftMs === 0}>{t(pauseDraft.isNew ? "plans.add" : "songs.save")}</button>
        </div>
      </form>
    {:else if adding === "scripture"}
      <form class="box" onsubmit={(e) => { e.preventDefault(); addScripture(); }}>
        <strong>{t("plans.addVerses")}</strong>
        <label class="field-row">
          <span class="label">{t("plans.versesLabel")}</span>
          <!-- svelte-ignore a11y_autofocus -->
          <textarea class="field" rows="3" bind:value={scriptureText} placeholder={t("plans.versesPlaceholder")} autofocus></textarea>
        </label>
        {#if scriptureLines.length > 0}
          <ul class="refs">
            {#each scriptureLines as line, i (i)}
              <li class:bad={!line.reference}>
                {#if line.reference}
                  ✓ {referenceLabel({ id: "", kind: "scripture", ...line.reference, bible: scriptureBible || null }, bibles.all.find((b) => b.id === scriptureBible) ?? currentBibles().bible)}
                {:else}
                  ✕ {line.text} <span class="small">— {t("plans.notUnderstood")}</span>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
        <label class="field-row">
          <span class="label">{t("plans.version")}</span>
          <select class="field" bind:value={scriptureBible}>
            <option value="">{t("plans.versionConsole", { version: currentBibles().bible?.abbreviation ?? "" })}</option>
            {#each visibleBibles() as b (b.id)}
              <option value={b.id}>{b.abbreviation} · {b.name}</option>
            {/each}
          </select>
        </label>
        <p class="muted small">{t("plans.versesHint")}</p>
        <div class="row end">
          <button type="button" class="btn ghost" onclick={() => (adding = null)}>{t("songs.cancel")}</button>
          <button type="submit" class="btn primary" disabled={scriptureReady === 0 || addingScripture}>{t("plans.addCount", { count: scriptureReady })}</button>
        </div>
      </form>
    {:else if adding === "song"}
      <div class="box">
        <div class="row">
          <!-- svelte-ignore a11y_autofocus -->
          <input class="field grow" bind:value={songFilter} placeholder={t("songs.search")} aria-label={t("songs.search")} autofocus />
          <button class="btn" onclick={() => (adding = null)}>{t("plans.done")}</button>
        </div>
        {#if library.songs.length === 0}
          <p class="muted small">{t("plans.noSongs")}</p>
        {:else}
          <ul class="items picker">
            {#each songMatches as song (song.id)}
              {@const count = plan.items.filter((i) => i.kind === "song" && i.songId === song.id).length}
              <li>
                <button class="item" onclick={() => addSong(song.id)}>
                  <span class="name">{song.title}<span class="muted small sub">{song.singer}</span></span>
                  {#if count > 0}<span class="done" title={t("plans.inPlan")}>✓</span>{/if}
                  <span class="plus" aria-hidden="true">+</span>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {:else}
      <div class="add">
        <span class="muted small">{t("plans.addLabel")}</span>
        <button class="btn" onclick={addSection}>+ {t("plans.section")}</button>
        <button class="btn" onclick={() => { adding = "song"; songFilter = ""; }}>+ {t("plans.song")}</button>
        <button class="btn" onclick={() => (adding = "scripture")}>+ {t("plans.verses")}</button>
        <button class="btn" onclick={newText}>+ {t("plans.text")}</button>
        <button class="btn" onclick={addMedia} disabled={copying}>{copying ? t("plans.copying") : `+ ${t("plans.media")}`}</button>
        <div class="pause-add">
          <button class="btn" onclick={() => newPause(10)}>+ {t("plans.pause")}</button>
          {#each PAUSE_PRESETS as m (m)}
            <button class="chip" onclick={() => newPause(m)} title={t("plans.addPause")}>{t("clock.minutes", { n: m })}</button>
          {/each}
        </div>
      </div>
    {/if}
  {/if}
</div>

<style>
  .plan-panel { display: flex; flex-direction: column; gap: 10px; }
  .notice {
    margin: 0;
    padding: 6px 10px;
    border-radius: 8px;
    color: var(--accent);
    background: var(--accent-soft);
    font-weight: 600;
  }
  .actions, .head, .add, .row { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .head { justify-content: space-between; }
  .meta { display: grid; grid-template-columns: 1fr auto; gap: 8px; }
  .plan-name { font-size: 18px; font-weight: 650; }
  .notes textarea { width: 100%; resize: vertical; margin-top: 6px; box-sizing: border-box; }
  summary { cursor: pointer; }
  .row.end { justify-content: flex-end; }
  .grow { flex: 1; min-width: 0; }
  .label { font-size: 13px; font-weight: 600; color: var(--muted); }
  .field-row { display: flex; flex-direction: column; gap: 4px; }
  .num-field { width: 6em; font-variant-numeric: tabular-nums; }
  .colon { font-size: 20px; font-weight: 700; align-self: flex-end; padding-bottom: 6px; }
  .bg-name { max-width: 14em; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .box {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
  }
  .pause-add { display: flex; align-items: center; gap: 4px; flex-wrap: wrap; }
  .refs { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 2px; font-weight: 600; color: var(--accent); }
  .refs li { border: 0; display: block; }
  .refs li:hover { background: transparent; }
  .refs li.bad { color: var(--live); font-weight: 400; }
  .box textarea { resize: vertical; font-size: 15px; }
  .chip {
    min-height: 32px;
    padding: 0 10px;
    font: inherit;
    font-size: 13px;
    font-weight: 600;
    color: var(--text);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 999px;
    cursor: pointer;
  }
  .chip:hover { border-color: var(--accent); }
  .chip:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }

  .items { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 4px; }
  .picker { max-height: 320px; overflow-y: auto; }
  li { display: flex; align-items: center; gap: 2px; border: 1px solid transparent; border-radius: 8px; }
  li:hover { background: var(--surface-2); }
  li.chosen { border-color: var(--accent); background: var(--accent-soft); }
  li.broken .name { color: var(--muted); }
  li.broken .sub { color: var(--live); }
  li.section { margin-top: 8px; border-bottom: 1px solid var(--border); border-radius: 0; }
  li.section:hover { background: transparent; }
  .list-head { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  .list-head p { margin: 0; }
  .list-head .btn { flex: none; }
  .section-title {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 40px;
    padding: 6px 4px;
    font: inherit;
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-align: left;
    color: var(--muted);
    background: transparent;
    border: 0;
    border-radius: 6px;
    cursor: pointer;
  }
  .section-title:hover { color: var(--text); background: var(--surface-2); }
  .section-title:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
  /* Triángulo que gira: abajo abierta, a la derecha plegada. */
  .chev {
    flex: none;
    width: 0;
    height: 0;
    margin: 0 2px 0 4px;
    border-left: 5px solid transparent;
    border-right: 5px solid transparent;
    border-top: 6px solid currentColor;
    transition: transform 0.15s ease;
  }
  .folded .chev { transform: rotate(-90deg); }
  .section-name { flex: 0 1 auto; min-width: 3em; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; text-transform: uppercase; }
  .section-sum { flex: 0 0 auto; font-size: 12px; font-weight: 500; letter-spacing: 0; color: var(--muted); }
  .sum-shown { margin-left: 8px; color: var(--accent); font-weight: 650; }
  .section-dot { flex: none; width: 8px; height: 8px; border-radius: 50%; background: var(--accent); box-shadow: 0 0 0 3px var(--accent-soft); }
  li.section.folded { background: var(--surface-2); border-radius: 8px; border-bottom-color: transparent; }
  li.section.folded:hover { background: var(--surface-2); }
  .section-input { flex: 1; min-width: 0; }
  .anim-row { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  .field.compact { width: auto; min-height: 32px; padding: 2px 8px; }
  /* Discreto mientras sigue al plan; destacado si la sección decide. */
  .section-anim { max-width: 150px; font: inherit; font-size: 12px; color: var(--muted); background: transparent; border: 1px solid transparent; border-radius: 6px; padding: 2px 4px; }
  .section-anim:hover, .section-anim:focus-visible { border-color: var(--border); }
  .section-anim.set { color: var(--text); border-color: var(--border); background: var(--surface-2); }
  .item {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 46px;
    padding: 6px 10px;
    font: inherit;
    text-align: left;
    color: var(--text);
    background: transparent;
    border: 0;
    cursor: pointer;
  }
  .item:disabled { cursor: default; }
  .icon { width: 1.4em; text-align: center; font-size: 17px; }
  .name { flex: 1; min-width: 0; display: flex; flex-direction: column; overflow: hidden; font-weight: 600; }
  .name, .sub { text-overflow: ellipsis; white-space: nowrap; }
  .sub { font-weight: 400; overflow: hidden; }
  .done { color: var(--accent); font-weight: 700; }
  .plus { color: var(--accent); font-weight: 700; font-size: 18px; }
  .small-btn { min-height: 34px; padding: 0 10px; font-size: 13px; }
  .icon-btn {
    flex: none;
    min-width: 36px;
    min-height: 36px;
    font: inherit;
    font-size: 13px;
    color: var(--muted);
    background: transparent;
    border: 0;
    border-radius: 8px;
    cursor: pointer;
  }
  .icon-btn:disabled { opacity: 0.35; cursor: default; }
  .icon-btn:not(:disabled):hover { background: var(--surface); color: var(--text); }
  .icon-btn.remove:hover, .icon-btn.confirm { color: var(--live); background: var(--live-soft); }
  .icon-btn.confirm { padding: 0 10px; font-weight: 650; }
</style>

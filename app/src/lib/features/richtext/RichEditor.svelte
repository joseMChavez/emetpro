<script lang="ts">
  // Editor de texto con formato (D-027): una barra de formato y una tarjeta por diapositiva, con fondo oscuro como la
  // pantalla para ver los colores como saldrán. Lo usan «Texto libre» y el elemento «Texto» del plan.
  import { tick } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { splitSlides } from "$lib/features/songs/slides";
  import { NO_COLOR_HEX, readSlide, slideHtml } from "./dom";
  import { BLOCK_TYPES, fromPlain, TEXT_COLORS, TEXT_HEX, type BlockAlign, type BlockType, type RichSlide, type TextColor } from "./rich";

  let {
    slides,
    onchange,
    onfocusslide,
  }: {
    slides: RichSlide[];
    onchange: (slides: RichSlide[]) => void;
    /** Se escribe en esa diapositiva (desde 0): la vista previa puede ir a ella. */
    onfocusslide?: (index: number) => void;
  } = $props();

  /** Una clave por tarjeta, para no rehacer el HTML de las demás al añadir, quitar o mover una. */
  let keys = $state<string[]>([]);
  let cards: (HTMLDivElement | undefined)[] = $state([]);
  let active = $state(0);
  let formats = $state({ bold: false, italic: false, underline: false });

  $effect.pre(() => {
    const n = Math.max(1, slides.length);
    if (keys.length !== n) keys = Array.from({ length: n }, (_, i) => keys[i] ?? crypto.randomUUID());
  });

  let shown = $derived(slides.length > 0 ? slides : [[]]);

  /** Pone el HTML una sola vez: después manda lo que se escribe (rehacerlo movería el cursor). */
  function init(node: HTMLDivElement, slide: RichSlide) {
    node.innerHTML = slideHtml(slide);
  }

  function emit(next: RichSlide[]) {
    onchange(next);
  }

  function sync(i: number) {
    const card = cards[i];
    if (!card) return;
    const next = [...shown];
    next[i] = readSlide(card);
    emit(next);
  }

  function focusCard(i: number) {
    active = i;
    onfocusslide?.(i);
  }

  function selectionIn(i: number): Range | null {
    const card = cards[i];
    const sel = document.getSelection();
    if (!card || !sel || sel.rangeCount === 0) return null;
    const range = sel.getRangeAt(0);
    return card.contains(range.commonAncestorContainer) ? range : null;
  }

  function exec(command: string, value?: string) {
    if (!selectionIn(active)) cards[active]?.focus();
    document.execCommand(command, false, value);
    sync(active);
    refreshFormats();
  }

  function color(c: TextColor | null) {
    if (!selectionIn(active)) return;
    document.execCommand("styleWithCSS", false, "true");
    document.execCommand("foreColor", false, c ? TEXT_HEX[c] : NO_COLOR_HEX);
    document.execCommand("styleWithCSS", false, "false");
    sync(active);
  }

  /** Los bloques de la tarjeta que toca la selección (o el del cursor). */
  function selectedBlocks(): HTMLElement[] {
    const card = cards[active];
    let range = selectionIn(active);
    if (!card || !range) return [];
    // Texto suelto en la tarjeta (sin bloque): se envuelve primero.
    const loose = [...card.childNodes].some((n) => !(n instanceof HTMLElement && n.tagName === "DIV"));
    if (loose) {
      document.execCommand("formatBlock", false, "div");
      range = selectionIn(active);
      if (!range) return [];
    }
    const blocks = [...card.children].filter((el): el is HTMLElement => el instanceof HTMLElement && range!.intersectsNode(el));
    return blocks;
  }

  function setType(type: BlockType) {
    for (const el of selectedBlocks()) el.dataset.type = type;
    sync(active);
  }

  function setAlign(align: BlockAlign) {
    const blocks = selectedBlocks();
    const all = blocks.length > 0 && blocks.every((el) => el.dataset.align === align);
    for (const el of blocks) {
      if (all) {
        delete el.dataset.align;
        el.style.textAlign = "";
      } else {
        el.dataset.align = align;
        el.style.textAlign = align;
      }
    }
    sync(active);
  }

  function refreshFormats() {
    if (!selectionIn(active)) return;
    formats = {
      bold: document.queryCommandState("bold"),
      italic: document.queryCommandState("italic"),
      underline: document.queryCommandState("underline"),
    };
  }

  async function addSlide(after: number, slide: RichSlide = []) {
    const next = [...shown];
    next.splice(after + 1, 0, slide);
    keys.splice(after + 1, 0, crypto.randomUUID());
    emit(next);
    await tick();
    cards[after + 1]?.focus();
    focusCard(after + 1);
  }

  function removeSlide(i: number) {
    if (shown.length <= 1) {
      if (cards[0]) cards[0].innerHTML = slideHtml([]);
      emit([[]]);
      return;
    }
    keys.splice(i, 1);
    emit(shown.filter((_, j) => j !== i));
    active = Math.max(0, Math.min(active, shown.length - 2));
  }

  function moveSlide(i: number, delta: -1 | 1) {
    const j = i + delta;
    if (j < 0 || j >= shown.length) return;
    const next = [...shown];
    [next[i], next[j]] = [next[j], next[i]];
    [keys[i], keys[j]] = [keys[j], keys[i]];
    emit(next);
  }

  /** Como en un procesador de textos: el renglón nuevo después de un título es texto normal. */
  function afterEnter(e: KeyboardEvent, i: number) {
    if (e.key !== "Enter" || e.shiftKey) return;
    requestAnimationFrame(() => {
      const node = document.getSelection()?.anchorNode;
      const card = cards[i];
      if (!node || !card) return;
      let block = node instanceof HTMLElement ? node : node.parentElement;
      while (block && block.parentElement !== card) block = block.parentElement;
      if (block && (block.dataset.type === "title" || block.dataset.type === "subtitle") && !block.textContent) {
        block.dataset.type = "body";
        sync(i);
      }
    });
  }

  /** Se pega solo el texto. Si trae separadores de diapositiva (`**`), cada parte va en su diapositiva. */
  async function paste(e: ClipboardEvent, i: number) {
    e.preventDefault();
    const text = e.clipboardData?.getData("text/plain") ?? "";
    if (!text) return;
    const parts = /^[ \t]*(?:\*[ \t]*){2,}$|\*{3,}/m.test(text) ? splitSlides(text) : [text];
    document.execCommand("insertText", false, parts[0] ?? "");
    sync(i);
    for (let k = 1; k < parts.length; k++) await addSlide(i + k - 1, fromPlain(parts[k]));
  }

  const BLOCK_LABEL = {
    title: "rich.title",
    subtitle: "rich.subtitle",
    body: "rich.body",
    bullet: "rich.bullet",
    number: "rich.number",
  } as const satisfies Record<BlockType, string>;
</script>

<svelte:document onselectionchange={refreshFormats} />

<div class="rich-editor">
  <!-- onmousedown con preventDefault: pulsar la barra no quita la selección del texto. -->
  <div class="toolbar" role="toolbar" aria-label={t("rich.toolbar")}>
    <div class="group">
      {#each BLOCK_TYPES as type (type)}
        <button type="button" class="tb" onmousedown={(e) => e.preventDefault()} onclick={() => setType(type)} title={t(BLOCK_LABEL[type])}>
          {#if type === "title"}<strong>T</strong>{:else if type === "subtitle"}<strong class="sub">T</strong>{:else if type === "body"}¶{:else if type === "bullet"}•≡{:else}1≡{/if}
        </button>
      {/each}
    </div>
    <div class="group">
      <button type="button" class="tb" class:on={formats.bold} onmousedown={(e) => e.preventDefault()} onclick={() => exec("bold")} title={t("rich.bold")}><b>B</b></button>
      <button type="button" class="tb" class:on={formats.italic} onmousedown={(e) => e.preventDefault()} onclick={() => exec("italic")} title={t("rich.italic")}><i>I</i></button>
      <button type="button" class="tb" class:on={formats.underline} onmousedown={(e) => e.preventDefault()} onclick={() => exec("underline")} title={t("rich.underline")}><u>U</u></button>
    </div>
    <div class="group">
      {#each ["left", "center", "right"] as const as align (align)}
        <button type="button" class="tb" onmousedown={(e) => e.preventDefault()} onclick={() => setAlign(align)} title={t(`rich.align.${align}`)} aria-label={t(`rich.align.${align}`)}>
          <svg viewBox="0 0 16 16" aria-hidden="true">
            <path d={align === "left" ? "M2 3h12M2 6.5h8M2 10h12M2 13.5h8" : align === "center" ? "M2 3h12M4 6.5h8M2 10h12M4 13.5h8" : "M2 3h12M6 6.5h8M2 10h12M6 13.5h8"} />
          </svg>
        </button>
      {/each}
    </div>
    <div class="group colors">
      <button type="button" class="swatch none" onmousedown={(e) => e.preventDefault()} onclick={() => color(null)} title={t("rich.noColor")} aria-label={t("rich.noColor")}></button>
      {#each TEXT_COLORS as c (c)}
        <button type="button" class="swatch" style:--c={TEXT_HEX[c]} onmousedown={(e) => e.preventDefault()} onclick={() => color(c)} title={t(`rich.color.${c}`)} aria-label={t(`rich.color.${c}`)}></button>
      {/each}
    </div>
  </div>

  <ol class="cards">
    {#each shown as slide, i (keys[i] ?? i)}
      <li class:active={active === i}>
        <div class="card-head">
          <span class="n">{t("rich.slide", { n: i + 1 })}</span>
          <button type="button" class="icon-btn" onclick={() => moveSlide(i, -1)} disabled={i === 0} aria-label={t("songs.moveUp")}>▲</button>
          <button type="button" class="icon-btn" onclick={() => moveSlide(i, 1)} disabled={i === shown.length - 1} aria-label={t("songs.moveDown")}>▼</button>
          <button type="button" class="icon-btn" onclick={() => removeSlide(i)} aria-label={t("rich.removeSlide", { n: i + 1 })}>✕</button>
        </div>
        <div
          class="page"
          contenteditable="true"
          role="textbox"
          aria-multiline="true"
          tabindex="0"
          aria-label={t("rich.slide", { n: i + 1 })}
          bind:this={cards[i]}
          use:init={slide}
          oninput={() => sync(i)}
          onfocus={() => focusCard(i)}
          onpaste={(e) => paste(e, i)}
          onkeydown={(e) => afterEnter(e, i)}
        ></div>
      </li>
    {/each}
  </ol>
  <button type="button" class="btn add" onclick={() => addSlide(shown.length - 1)}>+ {t("rich.addSlide")}</button>
  <p class="muted small">{t("rich.hint")}</p>
</div>

<style>
  .rich-editor { display: flex; flex-direction: column; gap: 8px; min-width: 0; }
  .toolbar {
    position: sticky;
    top: 0;
    z-index: 1;
    display: flex;
    flex-wrap: wrap;
    gap: 6px 10px;
    padding: 6px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
  }
  .group { display: flex; gap: 2px; align-items: center; }
  .tb {
    min-width: 32px;
    height: 32px;
    padding: 0 6px;
    font: inherit;
    font-size: 15px;
    color: var(--text);
    background: transparent;
    border: 1px solid transparent;
    border-radius: 6px;
    cursor: pointer;
  }
  .tb:hover { background: var(--surface-2); }
  .tb.on { background: var(--accent-soft); border-color: var(--accent); }
  .tb .sub { font-size: 12px; }
  .tb svg { width: 16px; height: 16px; fill: none; stroke: currentColor; stroke-width: 1.6; stroke-linecap: round; display: block; }
  .swatch {
    width: 22px;
    height: 22px;
    padding: 0;
    border-radius: 50%;
    border: 2px solid var(--border);
    background: var(--c);
    cursor: pointer;
  }
  .swatch.none { background: linear-gradient(135deg, transparent 45%, #e5484d 45% 55%, transparent 55%), #fff; }
  .cards { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; }
  .cards li { border: 2px solid var(--border); border-radius: 8px; overflow: hidden; }
  .cards li.active { border-color: var(--accent); }
  .card-head { display: flex; align-items: center; gap: 2px; padding: 2px 6px; background: var(--surface-2); }
  .card-head .n { flex: 1; font-size: 12px; font-weight: 650; color: var(--muted); }
  .icon-btn { width: 28px; height: 26px; padding: 0; font-size: 12px; color: var(--muted); background: none; border: none; border-radius: 5px; cursor: pointer; }
  .icon-btn:hover:not(:disabled) { color: var(--text); background: var(--surface); }
  .icon-btn:disabled { opacity: 0.35; cursor: default; }
  /* Como la pantalla: fondo oscuro y texto claro, así los colores se ven como saldrán. */
  .page {
    min-height: 88px;
    padding: 10px 12px;
    font-size: 16px;
    line-height: 1.35;
    color: #f5f5f0;
    background: #161a24;
    outline: none;
    overflow-wrap: anywhere;
    counter-reset: n;
  }
  .page :global([data-type="title"]) { font-size: 1.5em; font-weight: 800; }
  .page :global([data-type="subtitle"]) { font-size: 1.2em; font-weight: 700; }
  .page :global([data-type="bullet"]), .page :global([data-type="number"]) { position: relative; padding-left: 1.3em; }
  .page :global([data-type="bullet"])::before { content: "•"; position: absolute; left: 0.25em; }
  .page > :global(:not([data-type="number"])) { counter-reset: n; }
  .page :global([data-type="number"]) { counter-increment: n; }
  .page :global([data-type="number"])::before { content: counter(n) "."; position: absolute; left: 0; }
  .add { align-self: flex-start; }
</style>

<script lang="ts" module>
  import type { OutputStyle } from "$lib/platform";

  export const DEFAULT_STYLE: OutputStyle = {
    fontScale: 1,
    align: "center",
    position: "middle",
    background: "black",
    font: "system",
    backgroundImage: null,
    logo: { image: null, corner: "topRight", size: "medium", opacity: 0.6 },
  };

  /** Alto del logo según el tamaño elegido, en % del alto de la pantalla. */
  const LOGO_HEIGHT: Record<OutputStyle["logo"]["size"], number> = { small: 8, medium: 12, large: 18 };

  const BACKGROUNDS: Record<OutputStyle["background"], string> = {
    black: "#000000",
    navy: "#0b1730",
    charcoal: "#1d1f22",
  };

  /** Color de fondo de la pantalla con este estilo. */
  export function backgroundColor(style: OutputStyle): string {
    return BACKGROUNDS[style.background];
  }

  /** Más palabras que esto no se miden: un texto así no cabe en pantalla de todos modos. */
  const MAX_WORDS = 600;

  /** Dónde quedó cada palabra del texto, entre 0 y COORD sobre la pantalla entera (para la pizarra del teléfono). */
  function measureWords(view: HTMLElement, body: HTMLElement): ScreenWord[] {
    const box = view.getBoundingClientRect();
    if (box.width === 0 || box.height === 0) return [];
    const scale = (value: number, size: number) => Math.round((value / size) * COORD);
    const words: ScreenWord[] = [];
    const range = document.createRange();
    const walker = document.createTreeWalker(body, NodeFilter.SHOW_TEXT);
    const count = { text: 0, second: 0 };
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
      const p = node.parentElement?.closest("p");
      const k = p?.classList.contains("reference") ? "ref" : p?.classList.contains("second") ? "second" : "text";
      // Una palabra resaltada (D-023) lleva su marca: el teléfono la pinta igual.
      const m = (node.parentElement?.closest("[data-mark]") as HTMLElement | null)?.dataset.mark;
      for (const match of (node.textContent ?? "").matchAll(/\S+/gu)) {
        range.setStart(node, match.index);
        range.setEnd(node, match.index + match[0].length);
        // Al empezar una línea, el navegador añade una caja vacía al final de la anterior; y una palabra muy larga
        // partida en dos tiene dos cajas. Vale la más ancha.
        let rect: DOMRect | null = null;
        for (const r of range.getClientRects()) if (!rect || r.width > rect.width) rect = r;
        if (!rect || rect.width === 0) continue;
        words.push({
          t: match[0],
          x: scale(rect.left - box.left, box.width),
          y: scale(rect.top - box.top, box.height),
          w: scale(rect.width, box.width),
          h: scale(rect.height, box.height),
          k,
          ...(k === "ref" ? {} : { i: count[k]++ }),
          ...(m ? { m } : {}),
        });
        if (words.length >= MAX_WORDS) return words;
      }
    }
    return words;
  }
</script>

<script lang="ts">
  // Dibuja un contenido de salida. Lo usan la ventana de salida y las miniaturas de la consola,
  // así la vista previa se ve igual que la pantalla real.
  import { platform, type OutputContent, type VideoTime } from "$lib/platform";
  import { renderPage } from "$lib/features/output/pdf";
  import { formatClock, formatCountdown, timerRemaining } from "$lib/features/clock/clock";
  import { fontFamily } from "$lib/features/output/fonts";
  import { i18n, t } from "$lib/i18n/index.svelte";
  import BoardLayer from "$lib/features/remote/BoardLayer.svelte";
  import { COORD, layerKey, PAPER_COLOR, type ScreenWord } from "$lib/features/remote/board";
  import { sheetPaper } from "$lib/features/remote/board.svelte";
  import { currentMarks, MARK_HEX, segments, type MarkPart, type WordMark } from "$lib/features/marks/marks";

  let {
    content,
    style = DEFAULT_STYLE,
    diagnostics = "",
    preview = false,
    onerror,
    onscreen,
    marking = false,
    onmark,
    playing = false,
    follow = null,
    onvideotime,
  }: {
    content: OutputContent;
    style?: OutputStyle;
    diagnostics?: string;
    /** En miniatura el video no suena, y solo se reproduce con `playing` (si no, se ve su primer cuadro). */
    preview?: boolean;
    /** Vista previa a la vista: el video se reproduce sin sonido. Desde el principio y en bucle, salvo con `follow`. */
    playing?: boolean;
    /** Por dónde va el video en la pantalla (y cuándo llegó el aviso, con performance.now()): la vista lo sigue. */
    follow?: (VideoTime & { at: number }) | null;
    /** La ventana de salida avisa por dónde va el video. */
    onvideotime?: (time: VideoTime) => void;
    onerror?: (message: string) => void;
    /** La ventana de salida mide dónde quedó cada palabra, para que el teléfono dibuje justo encima. */
    onscreen?: (words: ScreenWord[]) => void;
    /** Resaltar palabras (D-023): las palabras se pueden tocar, y arrastrar para marcar varias. */
    marking?: boolean;
    onmark?: (part: MarkPart, from: number, to: number) => void;
  } = $props();

  /** Palabras que se están marcando con el dedo o el ratón, antes de soltar. */
  let drag = $state<{ part: MarkPart; from: number; to: number } | null>(null);

  function wordAt(target: EventTarget | null): { part: MarkPart; w: number } | null {
    const el = (target as Element | null)?.closest?.("[data-w]") as HTMLElement | null;
    if (!el?.dataset.w || !el.dataset.part) return null;
    return { part: el.dataset.part as MarkPart, w: Number(el.dataset.w) };
  }

  function markStart(e: PointerEvent) {
    const word = marking ? wordAt(e.target) : null;
    if (!word || !body) return;
    e.preventDefault();
    body.setPointerCapture(e.pointerId);
    drag = { part: word.part, from: word.w, to: word.w };
  }

  function markMove(e: PointerEvent) {
    if (!drag) return;
    const word = wordAt(document.elementFromPoint(e.clientX, e.clientY));
    if (word && word.part === drag.part) drag.to = word.w;
  }

  function markEnd() {
    if (drag) onmark?.(drag.part, drag.from, drag.to);
    drag = null;
  }

  function pending(part: MarkPart, w: number | null): boolean {
    return drag !== null && w !== null && drag.part === part && w >= Math.min(drag.from, drag.to) && w <= Math.max(drag.from, drag.to);
  }

  let video = $state<HTMLVideoElement>();
  let canvas = $state<HTMLCanvasElement>();
  let stage = $state<HTMLDivElement>();
  let body = $state<HTMLDivElement>();
  let view = $state<HTMLDivElement>();
  let width = $state(0);
  let height = $state(0);
  let now = $state(Date.now());

  let src = $derived("path" in content ? platform.mediaUrl(content.path) : "");
  let name = $derived(("path" in content ? content.path.split(/[\\/]/).pop() : "") ?? "");

  let lyric = $derived.by(() => {
    if (content.kind === "text") {
      return {
        text: content.slides[content.index] ?? "",
        reference: "",
        // La autoría de una letra va solo en la primera diapositiva.
        copyright: content.index === 0 ? (content.credit ?? "") : "",
        marks: currentMarks(content, "text"),
        second: null,
      };
    }
    if (content.kind === "scripture") {
      const verse = content.verses[content.index];
      const second = content.second && verse?.second ? verse.second : null;
      return {
        text: verse?.text ?? "",
        reference: verse ? `${verse.reference} · ${content.version}` : "",
        copyright: [content.copyright, content.second?.copyright].filter(Boolean).join(" · "),
        marks: currentMarks(content, "text"),
        second: second && {
          text: second.text,
          reference: `${second.reference} · ${content.second!.version}`,
          marks: currentMarks(content, "second"),
        },
      };
    }
    return null;
  });

  let clock = $derived.by(() => {
    if (content.kind === "clock") return { text: formatClock(new Date(now), i18n.locale, content.hour12, content.seconds), expired: false, label: "" };
    if (content.kind === "timer") {
      const remaining = timerRemaining(content, now);
      return { text: formatCountdown(remaining), expired: remaining <= 0, label: content.label ?? "" };
    }
    return null;
  });

  // La imagen de fondo va detrás del texto, la Biblia y el reloj, y queda sola al quitar el contenido.
  // Una letra o una cuenta atrás (la pausa de un plan) pueden traer la suya.
  // El logo va encima de todo menos de la pantalla en negro y del patrón de prueba.
  let logo = $derived(
    style.logo?.image && content.kind !== "black" && content.kind !== "testPattern" && content.kind !== "board"
      ? platform.mediaUrl(style.logo.image)
      : null,
  );

  // Sobre una hoja de la pizarra, su fondo; si no, se dibuja encima de lo que haya (D-021).
  let paper = $derived(content.kind === "board" ? sheetPaper(content.sheet) : null);
  let layer = $derived(layerKey(content));

  let backdrop = $derived.by(() => {
    if (!lyric && !clock && content.kind !== "empty") return null;
    const path = ((content.kind === "text" || content.kind === "timer") && content.background) || style.backgroundImage;
    return path ? platform.mediaUrl(path) : null;
  });

  function videoError() {
    // 4 = MEDIA_ERR_SRC_NOT_SUPPORTED: suele indicar que falta el códec en el sistema.
    const hint = video?.error?.code === 4 ? ` ${t("error.codec")}` : "";
    onerror?.(t("error.video", { name }) + hint);
  }

  /** Siguiendo la pantalla: la posición del video allí, contando lo que pasó desde el aviso. */
  let following = $derived(follow && content.kind === "video" && follow.path === content.path ? follow : null);

  $effect(() => {
    if (!video || content.kind !== "video" || (preview && !playing)) return;
    // La vista previa de lo preparado se reproduce siempre; la de lo que está en pantalla, como la pantalla.
    const paused = preview && !following ? false : content.paused;
    if (preview) {
      if (paused) video.pause();
      else video.play().catch(() => {});
      return;
    }
    if (content.paused) {
      video.pause();
    } else {
      video.play().catch((e) => onerror?.(t("error.videoStart", { error: String(e) })));
    }
  });

  // Más de un segundo de diferencia con la pantalla: salta a donde va allí.
  $effect(() => {
    if (!video || !following || !playing) return;
    const expected = following.time + (following.paused ? 0 : (performance.now() - following.at) / 1000);
    if (Math.abs(video.currentTime - expected) > 1) video.currentTime = expected;
  });

  // En la pantalla, el volumen y el silencio elegidos en la consola.
  $effect(() => {
    if (!video || content.kind !== "video" || preview) return;
    video.volume = Math.max(0, Math.min(1, content.volume ?? 1));
    video.muted = content.muted ?? false;
  });

  let lastReport = 0;
  /** Avisa por dónde va: al empezar, pausar, saltar y, mientras se reproduce, dos veces por segundo. */
  function reportTime(always: boolean) {
    if (!video || !onvideotime || content.kind !== "video") return;
    const now = performance.now();
    if (!always && now - lastReport < 500) return;
    lastReport = now;
    onvideotime({ path: content.path, time: video.currentTime, duration: Number.isFinite(video.duration) ? video.duration : 0, paused: video.paused });
  }

  $effect(() => {
    if (content.kind !== "clock" && content.kind !== "timer") return;
    now = Date.now();
    const timer = setInterval(() => (now = Date.now()), 200);
    return () => clearInterval(timer);
  });

  $effect(() => {
    if (!canvas || content.kind !== "pdf" || width === 0 || height === 0) return;
    const controller = new AbortController();
    renderPage(src, content.page, canvas, { width, height }, controller.signal).catch(() =>
      onerror?.(t("error.pdf", { name })),
    );
    return () => controller.abort();
  });

  // Ajuste automático: el texto usa el tamaño máximo elegido y se reduce lo justo para caber.
  // Así un versículo largo no se corta y uno corto no queda diminuto.
  $effect(() => {
    void [lyric?.text, lyric?.reference, lyric?.second?.text, style.fontScale, style.position, style.font, width, height];
    if (!stage || !body || height === 0) return;
    const fits = () => body!.scrollHeight <= stage!.clientHeight + 1 && body!.scrollWidth <= stage!.clientWidth + 1;
    let low = height * 0.022;
    let high = height * 0.08 * style.fontScale;
    body.style.fontSize = `${high}px`;
    if (fits()) return;
    for (let i = 0; i < 10; i++) {
      const mid = (low + high) / 2;
      body.style.fontSize = `${mid}px`;
      if (fits()) low = mid;
      else high = mid;
    }
    body.style.fontSize = `${low}px`;
  });

  // Después de ajustar el tamaño: dónde quedó cada palabra. Otra vez cuando cargan los tipos de letra.
  $effect(() => {
    void [lyric?.text, lyric?.reference, lyric?.second?.text, lyric?.marks, lyric?.second?.marks, style.fontScale, style.position, style.align, style.font, width, height];
    if (!onscreen) return;
    if (!lyric || !body || !view) {
      onscreen([]);
      return;
    }
    let cancelled = false;
    const measure = () => {
      if (!cancelled && body && view) onscreen(measureWords(view, body));
    };
    const frame = requestAnimationFrame(measure);
    document.fonts?.ready.then(() => requestAnimationFrame(measure));
    return () => {
      cancelled = true;
      cancelAnimationFrame(frame);
    };
  });
</script>

{#snippet marked(text: string, part: MarkPart, marks: Map<number, WordMark>)}{#each segments(text, marks) as seg}{#if seg.mark || (marking && seg.w !== null)}<span
        class="word {seg.mark ? `wm ${seg.mark.style}` : ''} {seg.w === null ? 'gap' : ''}"
        class:pending={pending(part, seg.w)}
        data-part={seg.w === null ? undefined : part}
        data-w={seg.w ?? undefined}
        data-mark={seg.mark ? `${seg.mark.style}:${seg.mark.color}` : undefined}
        style:--mc={seg.mark ? MARK_HEX[seg.mark.color] : undefined}>{seg.text}</span>{:else}{seg.text}{/if}{/each}{/snippet}

<div
  class="view"
  class:preview
  class:marking
  class:pattern={content.kind === "testPattern"}
  class:expired={clock?.expired}
  class:logo-bottom={logo !== null && style.logo.corner.startsWith("bottom")}
  style:background-color={paper ? PAPER_COLOR[paper] : BACKGROUNDS[style.background]}
  style:font-family={fontFamily(style.font)}
  bind:clientWidth={width}
  bind:clientHeight={height}
  bind:this={view}
>
  {#if backdrop}
    <img class="backdrop" src={backdrop} alt="" />
    <div class="shade"></div>
  {/if}
  {#if lyric}
    <div
      class="stage pos-{style.position}"
      class:with-credit={lyric.copyright !== ""}
      style:text-align={style.align}
      bind:this={stage}
    >
      <!-- svelte-ignore a11y_no_static_element_interactions (resaltar con el ratón; con teclado no hay palabras que elegir) -->
      <div
        class="body"
        bind:this={body}
        onpointerdown={markStart}
        onpointermove={markMove}
        onpointerup={markEnd}
        onpointercancel={() => (drag = null)}
      >
        <p class="text">{@render marked(lyric.text, "text", lyric.marks)}</p>
        {#if lyric.reference}<p class="reference">{lyric.reference}</p>{/if}
        {#if lyric.second}
          <hr class="divider" />
          <p class="text second">{@render marked(lyric.second.text, "second", lyric.second.marks)}</p>
          <p class="reference">{lyric.second.reference}</p>
        {/if}
      </div>
    </div>
    {#if lyric.copyright}<p class="copyright">{lyric.copyright}</p>{/if}
  {:else if clock}
    <!-- Sobre una imagen, la hora va en una placa oscura: tiene que leerse siempre, con cualquier fondo. -->
    <div class="clock-box" class:plate={backdrop !== null}>
      {#if clock.label}<p class="clock-label">{clock.label}</p>{/if}
      <!-- Tan grande como quepa a lo ancho: «10:45:30 p. m.» es mucho más largo que «5:00». -->
      <p class="clock" style:font-size="min({clock.label ? 24 : 30}cqh, {Math.round(130 / clock.text.length)}cqw)">{clock.text}</p>
    </div>
  {:else if content.kind === "image"}
    <img {src} alt="" onerror={() => onerror?.(t("error.image", { name }))} />
  {:else if content.kind === "video"}
    {#key src}
      <!-- svelte-ignore a11y_media_has_caption -->
      <video
        bind:this={video}
        {src}
        muted={preview}
        loop={preview && playing && !follow}
        preload={preview && !playing ? "metadata" : "auto"}
        onerror={videoError}
        ontimeupdate={() => reportTime(false)}
        onplay={() => reportTime(true)}
        onpause={() => reportTime(true)}
        onseeked={() => reportTime(true)}
        onloadedmetadata={() => reportTime(true)}
      ></video>
    {/key}
  {:else if content.kind === "pdf"}
    <canvas bind:this={canvas}></canvas>
  {:else if content.kind === "testPattern"}
    <div class="corner tl"></div>
    <div class="corner tr"></div>
    <div class="corner bl"></div>
    <div class="corner br"></div>
    <p class="pattern-title">{t("pattern.title")}</p>
    {#if diagnostics}<p class="diag">{diagnostics}</p>{/if}
  {/if}
  {#if layer}<BoardLayer {layer} {paper} />{/if}
  {#if logo}
    <img
      class="logo {style.logo.corner}"
      src={logo}
      alt=""
      style:height="{LOGO_HEIGHT[style.logo.size]}cqh"
      style:opacity={style.logo.opacity}
      onerror={() => onerror?.(t("error.image", { name: style.logo.image?.split(/[\\/]/).pop() ?? "" }))}
    />
  {/if}
</div>

<style>
  /* Ocupa exactamente el recuadro de su contenedor (que debe estar posicionado).
     contain: strict impide que el texto se dibuje o se posicione fuera, aunque el contenedor
     tome su alto de aspect-ratio y el navegador no lo considere definido. */
  .view {
    container-type: size;
    contain: strict;
    position: absolute;
    inset: 0;
    color: #f5f5f0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    overflow: hidden;
  }
  /* Las vistas previas nunca reciben el ratón: no pueden tapar los controles de la consola. */
  .view.preview { pointer-events: none; }
  .stage {
    position: absolute;
    inset: 6% 6%;
    display: flex;
    flex-direction: column;
    justify-content: center;
  }
  .stage.with-credit { bottom: 11%; }
  .stage.pos-top { justify-content: flex-start; }
  .stage.pos-bottom { justify-content: flex-end; }
  .body {
    width: 100%;
    line-height: 1.28;
    overflow-wrap: anywhere;
    /* Sombra suave: el texto se lee aunque el fondo cambie. */
    text-shadow: 0 0.04em 0.12em rgb(0 0 0 / 0.55);
  }
  .text {
    margin: 0;
    white-space: pre-wrap;
    font-weight: 500;
  }
  .reference {
    margin: 0.55em 0 0;
    font-size: 0.6em;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: #f2d58a;
  }
  .copyright {
    position: absolute;
    left: 6cqw;
    right: 6cqw;
    bottom: 3cqh;
    margin: 0;
    font-size: 2cqh;
    line-height: 1.3;
    text-align: center;
    color: rgb(245 245 240 / 0.62);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  /* Marca de agua: encima de cualquier contenido, en una esquina, sin tapar el texto (que deja un 6 % de margen). */
  .logo {
    position: absolute;
    z-index: 2;
    width: auto;
    max-width: 22cqw;
    object-fit: contain;
    pointer-events: none;
  }
  /* Con el logo abajo, el copyright se estrecha para no quedar debajo del logo. */
  .view.logo-bottom .copyright { left: 26cqw; right: 26cqw; }
  .logo.topLeft { top: 3cqh; left: 2.5cqw; }
  .logo.topRight { top: 3cqh; right: 2.5cqw; }
  .logo.bottomLeft { bottom: 3cqh; left: 2.5cqw; }
  .logo.bottomRight { bottom: 3cqh; right: 2.5cqw; }
  .backdrop {
    position: absolute;
    inset: 0;
    object-fit: cover;
  }
  /* Oscurece la imagen lo justo para que el texto se lea sobre cualquier fondo. */
  .shade {
    position: absolute;
    inset: 0;
    background: rgb(0 0 0 / 0.4);
  }
  .divider {
    width: 30%;
    margin: 0.7em auto;
    border: 0;
    border-top: 0.06em solid rgb(245 245 240 / 0.45);
  }
  .text.second { color: rgb(245 245 240 / 0.88); }
  /* Palabras resaltadas (D-023). El marcador lleva el texto oscuro, como un resaltador sobre papel; el relleno de los
     lados no mueve el texto (margen negativo). */
  .wm.marker {
    margin: 0 -0.1em;
    padding: 0 0.1em;
    border-radius: 0.14em;
    background: var(--mc);
    color: #141414;
    text-shadow: none;
    -webkit-box-decoration-break: clone;
    box-decoration-break: clone;
  }
  /* El espacio entre dos palabras marcadas solo rellena el hueco: con relleno propio taparía la letra de al lado. */
  .wm.marker.gap { margin: 0; padding: 0; border-radius: 0; }
  /* Subrayado como fondo, no como text-decoration: la sombra del texto no lo corta y sale seguido entre palabras. */
  .wm.underline {
    background: linear-gradient(var(--mc), var(--mc)) left bottom / 100% 0.1em no-repeat;
    -webkit-box-decoration-break: clone;
    box-decoration-break: clone;
  }
  .wm.color { color: var(--mc); }
  /* Resaltando: las palabras se pueden tocar aunque la vista previa no reciba el ratón. */
  .view.marking .word[data-w] {
    pointer-events: auto;
    cursor: pointer;
    border-radius: 0.1em;
  }
  .view.marking .body { touch-action: none; user-select: none; }
  .view.marking .word[data-w]:hover,
  .word.pending { outline: 0.06em solid rgb(255 255 255 / 0.85); outline-offset: 0.04em; }
  .clock {
    position: relative;
    margin: 0;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.02em;
    text-shadow: 0 0.03em 0.1em rgb(0 0 0 / 0.55);
  }
  .clock-box {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    max-width: 92cqw;
  }
  .clock-box.plate {
    padding: 2cqh 4cqw;
    border-radius: 3cqh;
    background: rgb(0 0 0 / 0.55);
  }
  .clock-label {
    position: relative;
    margin: 0 0 1cqh;
    max-width: 88cqw;
    overflow: hidden;
    font-size: 8cqh;
    font-weight: 600;
    text-align: center;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-shadow: 0 0.04em 0.12em rgb(0 0 0 / 0.6);
  }
  /* Tiempo cumplido: un borde rojo que late despacio, visible desde el púlpito, sin tapar el fondo ni el logo.
     La cuenta sigue con «−» y en rojo claro, que se lee sobre la placa oscura. */
  .view.expired::after {
    content: "";
    position: absolute;
    inset: 0;
    border: 2.2cqh solid #e0262e;
    pointer-events: none;
    animation: expired 1.6s ease-in-out infinite alternate;
  }
  .view.expired .clock { color: #ff8a8a; }
  @keyframes expired {
    from { border-color: #9e1219; }
    to { border-color: #ff3b42; }
  }
  img,
  video {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  .pattern-title {
    margin: 0;
    font-size: 7cqh;
  }
  .diag {
    margin-top: 2cqh;
    font-size: 3cqh;
    font-family: monospace;
    opacity: 0.8;
  }
  /* El borde y las esquinas permiten comprobar a ojo que la salida cubre todo el monitor. */
  .pattern {
    box-sizing: border-box;
    border: 0.6cqh solid #ff3b30;
    background:
      linear-gradient(#222 1px, transparent 1px) 0 0 / 10cqw 10cqh,
      linear-gradient(90deg, #222 1px, transparent 1px) 0 0 / 10cqw 10cqh,
      #000 !important;
  }
  .corner {
    position: absolute;
    width: 6cqh;
    height: 6cqh;
    background: #ffcc00;
  }
  .tl { top: 0; left: 0; }
  .tr { top: 0; right: 0; }
  .bl { bottom: 0; left: 0; }
  .br { bottom: 0; right: 0; }
</style>

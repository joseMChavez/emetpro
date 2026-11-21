<script lang="ts" module>
  export interface Mark {
    ms: number;
    kind: "section" | "slide" | "auto";
    label: string;
  }
</script>

<script lang="ts">
  // La forma de la pista en el editor de Media Studio (D-029): las bajadas y subidas de la música dejan ver dónde
  // cambia de sección. Encima, las marcas (secciones, diapositivas, las repartidas por la app) y dónde va la pista.
  // La onda se dibuja una vez en un canvas; las marcas y el cursor son HTML (se mueven sin redibujar).
  import { t } from "$lib/i18n/index.svelte";
  import { formatTime } from "./timeline";

  let {
    peaks,
    durationMs,
    positionMs,
    marks,
    onseek,
  }: {
    peaks: Float32Array | null;
    durationMs: number;
    positionMs: number;
    marks: Mark[];
    onseek: (ms: number) => void;
  } = $props();

  let canvas = $state<HTMLCanvasElement>();
  let width = $state(0);
  const HEIGHT = 72;

  /** Los nombres de sección que caben: uno muy pegado al anterior no se escribe (sigue la raya). */
  let flagged = $derived.by(() => {
    const shown = new Set<number>();
    let last = -Infinity;
    marks.forEach((mark, i) => {
      if (mark.kind !== "section") return;
      const x = (mark.ms / Math.max(1, durationMs)) * width;
      if (x - last >= 64) {
        shown.add(i);
        last = x;
      }
    });
    return shown;
  });

  const pct = (ms: number) => `${Math.max(0, Math.min(100, (ms / Math.max(1, durationMs)) * 100))}%`;

  $effect(() => {
    const el = canvas;
    const data = peaks;
    const w = width;
    if (!el || !data || w <= 0) return;
    const ratio = window.devicePixelRatio || 1;
    el.width = Math.round(w * ratio);
    el.height = Math.round(HEIGHT * ratio);
    const g = el.getContext("2d");
    if (!g) return;
    g.setTransform(ratio, 0, 0, ratio, 0, 0);
    g.clearRect(0, 0, w, HEIGHT);
    g.fillStyle = getComputedStyle(el).color;
    const mid = HEIGHT / 2;
    // Una barra por píxel, con el pico de los tramos que caen en él.
    for (let x = 0; x < w; x++) {
      const from = Math.floor((x / w) * data.length);
      const to = Math.max(from + 1, Math.floor(((x + 1) / w) * data.length));
      let peak = 0;
      for (let i = from; i < to && i < data.length; i++) peak = Math.max(peak, data[i]);
      const h = Math.max(1, peak * (HEIGHT - 6));
      g.fillRect(x, mid - h / 2, 1, h);
    }
  });

  function seekAt(event: MouseEvent) {
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    if (box.width <= 0) return;
    onseek(Math.round(((event.clientX - box.left) / box.width) * durationMs));
  }

  function onKey(event: KeyboardEvent) {
    // Con el teclado: flechas, 5 s. Espacio queda para marcar.
    if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
      event.preventDefault();
      onseek(Math.max(0, Math.min(durationMs, positionMs + (event.key === "ArrowLeft" ? -5000 : 5000))));
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  class="wave"
  bind:clientWidth={width}
  role="slider"
  tabindex="0"
  aria-label={t("studio.waveform")}
  title={t("studio.waveform")}
  aria-valuemin={0}
  aria-valuemax={durationMs}
  aria-valuenow={Math.round(positionMs)}
  aria-valuetext={formatTime(positionMs)}
  onclick={seekAt}
  onkeydown={onKey}
>
  {#if peaks}
    <canvas bind:this={canvas} style:height="{HEIGHT}px"></canvas>
  {:else}
    <div class="empty" style:height="{HEIGHT}px"></div>
  {/if}
  {#each marks as mark, i (i)}
    <span class="mark {mark.kind}" style:left={pct(mark.ms)} title="{formatTime(mark.ms)} · {mark.label}">
      {#if flagged.has(i)}<span class="flag">{mark.label}</span>{/if}
    </span>
  {/each}
  <span class="head" style:left={pct(positionMs)}></span>
</div>

<style>
  .wave { position: relative; width: 100%; padding-top: 18px; cursor: pointer; color: color-mix(in srgb, var(--accent) 55%, var(--muted)); border-radius: 8px; background: var(--surface-2); overflow: hidden; }
  .wave:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  canvas, .empty { display: block; width: 100%; }
  .mark { position: absolute; top: 0; bottom: 0; width: 0; border-left: 1px solid var(--text); opacity: 0.45; pointer-events: none; }
  .mark.section { border-left: 2px solid var(--accent); opacity: 1; }
  .mark.auto { border-left-style: dashed; opacity: 0.35; }
  .flag { position: absolute; top: 1px; left: 2px; padding: 0 4px; font-size: 11px; font-weight: 700; line-height: 15px; white-space: nowrap; color: var(--accent); background: var(--surface); border-radius: 4px; }
  .head { position: absolute; top: 0; bottom: 0; width: 0; border-left: 2px solid var(--live); pointer-events: none; }
</style>

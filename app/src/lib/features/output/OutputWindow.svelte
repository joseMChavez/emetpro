<script lang="ts">
  import { onMount } from "svelte";
  import OutputView, { backgroundColor, DEFAULT_STYLE } from "$lib/features/output/OutputView.svelte";
  import { layerKey, type ScreenLayout, type ScreenWord } from "$lib/features/remote/board";
  import { platform, type OutputContent, type OutputStyle, type StudioTime } from "$lib/platform";
  import { activeWord } from "$lib/features/studio/timeline";
  import { t } from "$lib/i18n/index.svelte";

  let content = $state<OutputContent>({ kind: "empty" });
  let style = $state<OutputStyle>(DEFAULT_STYLE);
  let dpr = $state(1);
  let width = $state(0);
  let height = $state(0);
  // Derivado para que cambie también al cambiar de idioma.
  let size = $derived(`${width}×${height} px · ${t("pattern.scale")} ${dpr}`);

  // Si el teclado quedó en esta ventana (pasa al abrirla a pantalla completa), las flechas siguen funcionando.
  // Las mismas teclas que en la consola; RePág y AvPág son las que envían los presentadores inalámbricos.
  function onKeydown(event: KeyboardEvent) {
    const forward = event.key === "ArrowRight" || event.key === "PageDown";
    const back = event.key === "ArrowLeft" || event.key === "PageUp";
    if (!forward && !back) return;
    event.preventDefault();
    platform.outputNavigate(forward).catch(() => {});
  }

  // ---- Media Studio (D-029): la palabra que suena ----
  // La consola avisa por dónde va la pista (al reproducir, pausar o saltar, y cada segundo); entre avisos, la
  // posición se cuenta aquí con el reloj de esta ventana, en cada cuadro y solo mientras suena.
  let studioTime = $state<(StudioTime & { at: number }) | null>(null);
  let word = $state<number | null>(null);
  let frame = 0;

  function trackWord() {
    cancelAnimationFrame(frame);
    const time = studioTime;
    const words = content.kind === "text" && content.studio === time?.song ? content.words?.[content.index] : null;
    if (!time || !words) {
      word = null;
      return;
    }
    const ms = time.ms + (time.playing ? performance.now() - time.at : 0);
    word = activeWord(words, ms);
    if (time.playing) frame = requestAnimationFrame(trackWord);
  }

  $effect(() => {
    void content;
    void studioTime;
    trackWord();
    return () => cancelAnimationFrame(frame);
  });

  let lastScreen = "";

  /** Para la pizarra del teléfono: cómo es esta pantalla y dónde quedó cada palabra del texto. */
  function reportScreen(words: ScreenWord[]) {
    const screen: ScreenLayout = {
      aspect: innerHeight > 0 ? Math.round((innerWidth / innerHeight) * 1000) / 1000 : 16 / 9,
      layer: layerKey(content),
      paper: backgroundColor(style),
      words,
    };
    const json = JSON.stringify(screen);
    if (json === lastScreen) return;
    lastScreen = json;
    platform.remoteScreen(screen).catch(() => {});
  }

  function measure() {
    dpr = window.devicePixelRatio;
    width = Math.round(innerWidth * dpr);
    height = Math.round(innerHeight * dpr);
  }

  onMount(() => {
    measure();
    // Recupera lo que estaba en pantalla si la ventana se recarga.
    const apply = (state: { content: OutputContent; style: OutputStyle }) => {
      content = state.content;
      style = state.style;
    };
    platform.getOutputState().then(apply);
    const unlisten = platform.onOutputState(apply);
    const unlistenStudio = platform.onStudioTime((time) => (studioTime = { ...time, at: performance.now() }));
    return () => {
      unlisten.then((fn) => fn());
      unlistenStudio.then((fn) => fn());
    };
  });
</script>

<svelte:window onresize={measure} onkeydown={onKeydown} />

<div class="output">
  <OutputView {content} {style} {word} animated diagnostics={size} onerror={(message) => platform.reportOutputError(message)} onscreen={reportScreen} onvideotime={(time) => platform.reportVideoTime(time).catch(() => {})} />
</div>

<style>
  :global(html, body) {
    margin: 0;
    /* Gana a los estilos de la consola: la salida siempre empieza en negro. */
    background: #000 !important;
    overflow: hidden;
  }
  .output {
    position: relative;
    width: 100vw;
    height: 100vh;
    cursor: none;
  }
</style>

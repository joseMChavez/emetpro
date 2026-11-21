// Media Studio en pantalla (D-029): une la pista con lo que está en pantalla.
// Modo híbrido: mientras suena, la diapositiva sale del tiempo de la pista. Si el operador elige otra, la pista salta
// a su comienzo y sigue sonando o en pausa como estaba (seek-preserve), y el avance automático sigue desde ahí.
// En directo no hay pista: la consola avanza como con cualquier letra.
import type { MediaSong, OutputContent } from "$lib/platform";
import { player } from "./player.svelte";
import { expand, type Expanded } from "./song";
import { nextStart, slideAt, slideStarts } from "./timeline";
import { studio } from "./library.svelte";

export const onScreen = $state({
  /** La canción de Media Studio en pantalla, o null. */
  song: null as MediaSong | null,
  /** La diapositiva en pantalla (desde 0). */
  index: 0,
});

let expanded: Expanded[] = [];
let starts: (number | null)[] = [];
/** Diapositiva sin tiempo elegida a mano: se respeta hasta que la pista llegue al siguiente comienzo. */
let hold: { until: number | null } | null = null;
/** La última pedida a la pantalla, para no pedirla otra vez mientras llega. */
let lastShown = -1;
let show: ((n: number) => boolean) | null = null;

/** El que muestra diapositivas en pantalla (n desde 1). Lo registra la consola al ver una canción en pantalla. */
export function attach(onshow: (n: number) => boolean): () => void {
  show = onshow;
  const unsubscribe = player.subscribe(onTime);
  return () => {
    unsubscribe();
    if (show === onshow) show = null;
  };
}

function hasTrack(song: MediaSong | null): song is MediaSong & { audio: NonNullable<MediaSong["audio"]> } {
  return song?.mode === "track" && song.audio !== null;
}

/** La pista de la canción en pantalla está cargada y lista. */
export function trackReady(): boolean {
  const song = onScreen.song;
  return hasTrack(song) && player.path === song.audio.path && player.status === "ready";
}

/** Sigue lo que está en pantalla. Al llegar una canción con pista, la carga en pausa en su diapositiva. */
export function follow(content: OutputContent) {
  if (content.kind !== "text" || !content.studio) {
    onScreen.song = null;
    return;
  }
  const song = studio.songs.find((s) => s.id === content.studio) ?? null;
  const changed = song?.id !== onScreen.song?.id;
  if (song !== onScreen.song) {
    onScreen.song = song;
    expanded = song ? expand(song) : [];
    starts = slideStarts(expanded);
  }
  if (content.index !== onScreen.index) lastShown = content.index;
  onScreen.index = content.index;
  if (!changed || !hasTrack(song)) return;
  hold = null;
  lastShown = content.index;
  // Si ya sonaba esta misma pista (se volvió a enviar), no se toca.
  if (player.path === song.audio.path && player.playing) return;
  const index = content.index;
  player.load(song.audio.path).then((duration) => {
    if (duration === null || onScreen.song?.id !== song.id) return;
    player.seek(starts[index] ?? 0);
  });
}

/** Mostrar la diapositiva n (desde 1). Con pista, la pista salta allí; devuelve false si no es cosa de Media Studio. */
export function goTo(content: OutputContent, n: number): boolean {
  if (content.kind !== "text" || !content.studio || content.studio !== onScreen.song?.id || !trackReady() || !show) return false;
  const i = n - 1;
  if (i < 0 || i >= expanded.length) return true;
  const start = starts[i];
  if (start !== null) {
    hold = null;
    player.seek(start);
  } else {
    hold = { until: nextStart(starts, player.now()) };
  }
  if (show(n)) lastShown = i;
  return true;
}

/** Secciones de la canción en pantalla, en el orden en que aparecen por primera vez. */
export function sectionsOnScreen(): { id: string; current: boolean }[] {
  const current = expanded[onScreen.index]?.section.id;
  const seen = new Set<string>();
  const out: { id: string; current: boolean }[] = [];
  for (const e of expanded) {
    if (seen.has(e.section.id)) continue;
    seen.add(e.section.id);
    out.push({ id: e.section.id, current: e.section.id === current });
  }
  return out;
}

/**
 * Saltar a una sección: la siguiente vez que aparece desde donde se está (la que viene en la canción). Si ya no
 * vuelve a aparecer, la última vez que apareció (repetir el coro al final). Devuelve la diapositiva (desde 1).
 */
export function sectionTarget(sectionId: string): number | null {
  const here = expanded[onScreen.index]?.entryIndex ?? -1;
  const firsts = expanded.map((e, i) => ({ e, i })).filter(({ e }) => e.section.id === sectionId && (e.slide === null || e.slideIndex === 0));
  const ahead = firsts.find(({ e }) => e.entryIndex > here);
  const target = ahead ?? firsts.filter(({ e }) => e.entryIndex <= here).pop();
  return target ? target.i + 1 : null;
}

/** Cada cuadro mientras suena: si la pista entró en otra diapositiva, se muestra. */
function onTime(ms: number) {
  if (!show || !player.playing || !trackReady()) return;
  if (hold) {
    if (hold.until === null || ms < hold.until) return;
    hold = null;
  }
  const i = slideAt(starts, ms);
  if (i === null || i === onScreen.index || i === lastShown) return;
  // Si la consola estaba ocupada enviando, se vuelve a intentar en el próximo cuadro.
  if (show(i + 1)) lastShown = i;
}

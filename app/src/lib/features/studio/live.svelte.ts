// Media Studio en pantalla (D-029): une la pista con lo que está en pantalla.
// Modo híbrido: mientras suena, la diapositiva sale del tiempo de la pista. Si el operador elige otra, la pista salta
// a su comienzo y sigue sonando o en pausa como estaba (seek-preserve), y el avance automático sigue desde ahí.
// En directo no hay pista: la consola avanza como con cualquier letra.
import { platform, type MediaSong, type OutputContent } from "$lib/platform";
import { player } from "./player.svelte";
import { expand, type Expanded } from "./song";
import { activeWord, nextStart, slideRanges, slideStarts, step, type Range } from "./timeline";
import { studio } from "./library.svelte";

export const onScreen = $state({
  /** La canción de Media Studio en pantalla, o null. */
  song: null as MediaSong | null,
  /** La diapositiva en pantalla (desde 0). */
  index: 0,
  /** Repetir la sección en pantalla: al terminarla, la pista vuelve a su comienzo (una banda que se queda en el
   *  coro mientras se ora). Se quita al elegir otra diapositiva o sección. */
  loop: false,
});

/** Canción recién enviada a pantalla: al llegar, su pista empieza a sonar (como un video). */
let autoplay: string | null = null;

/** La consola va a enviar esta canción a pantalla: que suene al llegar. */
export function armAutoplay(content: OutputContent) {
  autoplay = content.kind === "text" && content.studio ? content.studio : null;
}

let expanded: Expanded[] = [];
/** Comienzos propios de cada diapositiva desplegada (para saber cuál suena en un instante). */
let starts: (number | null)[] = [];
/** Su tramo en la grabación, propio o el de otra vez que suena su sección (para saltar y seguir un orden propio). */
let ranges: (Range | null)[] = [];
/** Diapositiva sin tiempo elegida a mano: se respeta hasta que la pista llegue al siguiente comienzo. */
let hold: { until: number | null } | null = null;
/** La diapositiva que sigue la pista: la última pedida a la pantalla (aunque todavía no haya llegado). */
let cursor = 0;
/** La consola estaba ocupada y no la mostró: se vuelve a pedir en el próximo aviso. */
let pending = false;
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
    ranges = slideRanges(expanded, song?.audio?.durationMs ?? 0);
  }
  if (content.index !== onScreen.index && !pending) cursor = content.index;
  onScreen.index = content.index;
  const play = autoplay !== null && autoplay === song?.id;
  if (play) autoplay = null;
  if (!hasTrack(song)) return;
  // La misma canción enviada otra vez (en pausa o ya terminada): suena desde la diapositiva enviada.
  if (!changed) {
    if (play && trackReady() && !player.playing) {
      cursor = content.index;
      player.seek(ranges[content.index]?.start ?? 0);
      player.play();
    }
    return;
  }
  hold = null;
  pending = false;
  onScreen.loop = false;
  cursor = content.index;
  // Si ya sonaba esta misma pista (se volvió a enviar), no se toca.
  if (player.path === song.audio.path && player.playing) return;
  const index = content.index;
  player.load(song.audio.path).then((duration) => {
    if (duration === null || onScreen.song?.id !== song.id) return;
    player.seek(ranges[index]?.start ?? 0);
    // Enviada a pantalla: empieza a sonar sola, como un video. Restaurada al abrir la consola, no.
    if (play) player.play();
  });
}

/** Mostrar la diapositiva n (desde 1). Con pista, la pista salta allí; devuelve false si no es cosa de Media Studio. */
export function goTo(content: OutputContent, n: number): boolean {
  if (content.kind !== "text" || !content.studio || content.studio !== onScreen.song?.id || !trackReady() || !show) return false;
  const i = n - 1;
  if (i < 0 || i >= expanded.length) return true;
  const range = ranges[i];
  // Elegir otra cosa es salir de la repetición.
  if (expanded[i]?.entryIndex !== expanded[cursor]?.entryIndex) onScreen.loop = false;
  display(i);
  // La pista salta a donde suena esa diapositiva (sus tiempos o los de otra vez que suena su sección).
  if (range) {
    hold = null;
    player.seek(range.start);
  } else {
    hold = { until: nextStart(starts, player.now()) };
  }
  return true;
}

/** Primera y última diapositiva desplegada de la aparición de la diapositiva i. */
function occurrence(i: number): { first: number; last: number } | null {
  const entry = expanded[i]?.entryIndex;
  if (entry === undefined) return null;
  const first = expanded.findIndex((e) => e.entryIndex === entry);
  let last = first;
  while (expanded[last + 1]?.entryIndex === entry) last++;
  return { first, last };
}

/** Terminar la canción con el fundido: también deja de repetir. */
export function finishSong() {
  onScreen.loop = false;
  player.finish();
}

/** Repetir (o dejar de repetir) la sección en pantalla. */
export function toggleLoop() {
  onScreen.loop = !onScreen.loop && onScreen.song !== null;
}

/** La canción en pantalla tiene algún tiempo marcado: si no, la pista suena de corrido y no sigue a la pantalla. */
export function synced(): boolean {
  return ranges.some((r) => r !== null);
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

/** Pide a la pantalla la diapositiva i y la deja como la que sigue la pista. */
function display(i: number) {
  cursor = i;
  pending = !show?.(i + 1);
}

/** El último aviso de tiempo a la salida: no se repite mientras la pista vaya por donde se espera. */
let lastSent = { song: "", ms: -1, playing: false, at: 0 };

/**
 * Avisa a la salida por dónde va la pista, para iluminar la palabra que suena: al reproducir, pausar o saltar, si
 * se desvía de lo previsto, y una vez por segundo mientras suena. La salida cuenta el resto con su reloj.
 */
function sendTime(ms: number) {
  const song = onScreen.song;
  if (!song || !trackReady()) return;
  const now = performance.now();
  const expected = lastSent.playing ? lastSent.ms + (now - lastSent.at) : lastSent.ms;
  const same = lastSent.song === song.id && lastSent.playing === player.playing;
  if (same && Math.abs(ms - expected) < 120 && now - lastSent.at < 1000) return;
  lastSent = { song: song.id, ms, playing: player.playing, at: now };
  platform.reportStudioTime({ song: song.id, ms: Math.round(ms), playing: player.playing }).catch(() => {});
}

/** La palabra que suena de lo que está en pantalla (para la vista de la consola); null si no hay. */
export function wordOnScreen(content: OutputContent): number | null {
  if (content.kind !== "text" || !content.studio || content.studio !== onScreen.song?.id || !trackReady()) return null;
  return activeWord(content.words?.[content.index], player.positionMs);
}

/** Cada aviso de la pista mientras suena (cada cuadro, y cada 100 ms aunque la consola esté tapada). */
function onTime(ms: number) {
  sendTime(ms);
  if (!show || !player.playing || !trackReady()) return;
  // La consola estaba ocupada enviando: se vuelve a intentar.
  if (pending) pending = !show(cursor + 1);
  if (hold) {
    if (hold.until === null || ms < hold.until) return;
    hold = null;
  }
  const next = step(ranges, starts, cursor, ms, onScreen.loop ? occurrence(cursor) : null);
  if (next.kind === "stay") return;
  display(next.index);
  // Orden propio o repetir: la siguiente suena en otra parte de la grabación.
  if (next.seek !== undefined) player.seek(next.seek, true);
}

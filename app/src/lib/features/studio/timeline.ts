// Media Studio (D-029): dónde empieza cada diapositiva desplegada en la pista y cuál suena en cada instante.
// Intervalos [inicio, inicio de la siguiente). Puro, para probarlo con vitest.
import type { MediaSong, SlideCue, Timing } from "$lib/platform/types";
import type { Expanded } from "./song";

/**
 * Comienzo de cada diapositiva desplegada (en el mismo orden), o null si no tiene tiempo.
 * Una instrumental empieza con su aparición. La primera diapositiva de una aparición sin tiempo propio también.
 */
export function slideStarts(expanded: Expanded[]): (number | null)[] {
  return expanded.map((e) => {
    const timing = e.entry.timing;
    if (!timing) return null;
    if (!e.slide) return timing.startMs;
    const cue = timing.slides.find((c) => c.slide === e.slide!.id);
    if (cue) return cue.startMs;
    // La primera hereda el comienzo de su aparición, salvo que otra diapositiva ya empiece ahí o antes.
    return e.slideIndex === 0 && !timing.slides.some((c) => c.startMs <= timing.startMs) ? timing.startMs : null;
  });
}

/** La diapositiva que suena en ese instante: la última con tiempo que ya empezó. null antes de la primera. */
export function slideAt(starts: (number | null)[], ms: number): number | null {
  let found: number | null = null;
  let best = -1;
  starts.forEach((start, i) => {
    // Las apariciones van hacia adelante (validado): la última que empezó es la que suena.
    if (start !== null && start <= ms && start >= best) {
      best = start;
      found = i;
    }
  });
  return found;
}

/** El siguiente comienzo después de ms, o null si no hay más. */
export function nextStart(starts: (number | null)[], ms: number): number | null {
  let next: number | null = null;
  for (const start of starts) if (start !== null && start > ms && (next === null || start < next)) next = start;
  return next;
}

/** Dónde suena una diapositiva en la grabación: [start, end). `own` = sus propios tiempos (no los de otra vez). */
export interface Range {
  start: number;
  end: number;
  own: boolean;
}

/**
 * El tramo de la grabación de cada diapositiva desplegada. Cada una dura hasta el siguiente comienzo *en la
 * grabación*. Una sin tiempo propio usa el de la misma diapositiva en otra aparición de su sección (la más cercana
 * antes; si no, la primera después): así un coro o una estrofa repetidos fuera del orden de la grabación suenan
 * igual. null si su sección nunca tiene tiempo.
 */
export function slideRanges(expanded: Expanded[], durationMs: number): (Range | null)[] {
  const starts = slideStarts(expanded);
  const sorted = [...new Set(starts.filter((s): s is number => s !== null && s < durationMs))].sort((a, b) => a - b);
  const own = starts.map((start) => {
    if (start === null || start >= durationMs) return null;
    const end = sorted.find((s) => s > start) ?? durationMs;
    return { start, end, own: true };
  });
  const key = (e: Expanded) => `${e.section.id}\u0001${e.slide?.id ?? ""}`;
  return own.map((range, i) => {
    if (range) return range;
    const same = expanded.map((e, j) => ({ e, j })).filter(({ e, j }) => own[j] && key(e) === key(expanded[i]));
    const source = same.filter(({ j }) => j < i).pop() ?? same[0];
    return source ? { ...own[source.j]!, own: false } : null;
  });
}

/** Lo que hay que hacer en este instante de la pista, estando en pantalla la diapositiva `current`. */
export type Step =
  | { kind: "stay" }
  /** Mostrar otra diapositiva; con `seek`, la pista salta antes allí (un orden distinto al de la grabación). */
  | { kind: "show"; index: number; seek?: number };

/** Margen para dar por seguida una diapositiva (un salto que cae un poco antes, el redondeo del reloj). */
const SLACK = 250;

/**
 * Avance mientras suena (modo híbrido). Si la pista sigue dentro de la diapositiva en pantalla, nada. Si acaba de
 * terminarla, la siguiente del orden: donde siga la grabación, o saltando a donde suena esa (orden propio).
 * Si la pista está en otra parte (se movió la barra), la diapositiva que suena ahí.
 */
export function step(ranges: (Range | null)[], starts: (number | null)[], current: number, ms: number): Step {
  const range = ranges[current];
  if (range && ms >= range.start - SLACK && ms < range.end) return { kind: "stay" };
  if (range && ms >= range.end && ms < range.end + 4 * SLACK) {
    const next = current + 1;
    if (next >= ranges.length) return { kind: "stay" };
    const target = ranges[next];
    if (target && Math.abs(target.start - range.end) > 60) return { kind: "show", index: next, seek: target.start };
    return { kind: "show", index: next };
  }
  const at = slideAt(starts, ms);
  if (at === null || at === current) return { kind: "stay" };
  return { kind: "show", index: at };
}

/**
 * Marca que la diapositiva desplegada k empieza en ms. Si ya tenía tiempo, sus líneas se mueven con ella.
 * Devuelve la canción cambiada (no toca la original).
 */
export function markStart(song: MediaSong, expanded: Expanded[], k: number, ms: number): MediaSong {
  const e = expanded[k];
  if (!e) return song;
  ms = Math.max(0, Math.round(ms));
  const order = song.order.map((entry, i) => {
    if (i !== e.entryIndex) return entry;
    const timing: Timing = entry.timing ? { ...entry.timing, slides: [...entry.timing.slides] } : { startMs: ms, slides: [] };
    if (!e.slide) return { ...entry, timing: { ...timing, startMs: ms } };
    if (e.slideIndex === 0) timing.startMs = ms;
    const at = timing.slides.findIndex((c) => c.slide === e.slide!.id);
    const old = at >= 0 ? timing.slides[at] : null;
    const delta = old ? ms - old.startMs : 0;
    const cue: SlideCue = {
      slide: e.slide.id,
      startMs: ms,
      ...(old?.lines?.length ? { lines: old.lines.map((l) => shiftLine(l, delta)) } : {}),
    };
    if (at >= 0) timing.slides[at] = cue;
    else timing.slides.push(cue);
    // En el orden de la sección.
    const position = new Map(e.section.slides.map((s, i) => [s.id, i]));
    timing.slides.sort((a, b) => (position.get(a.slide) ?? 0) - (position.get(b.slide) ?? 0));
    timing.startMs = Math.min(timing.startMs, ...timing.slides.map((c) => c.startMs));
    return { ...entry, timing };
  });
  return { ...song, order };
}

function shiftLine<T extends { startMs: number; endMs?: number | null; words?: number[] }>(line: T, delta: number): T {
  return {
    ...line,
    startMs: line.startMs + delta,
    ...(line.endMs != null ? { endMs: line.endMs + delta } : {}),
    ...(line.words ? { words: line.words.map((w) => w + delta) } : {}),
  };
}

/** Quita el tiempo de la diapositiva desplegada k. La aparición se queda sin tiempo si no le queda ninguno. */
export function clearStart(song: MediaSong, expanded: Expanded[], k: number): MediaSong {
  const e = expanded[k];
  if (!e?.entry.timing) return song;
  const order = song.order.map((entry, i) => {
    if (i !== e.entryIndex || !entry.timing) return entry;
    if (!e.slide) return { ...entry, timing: null };
    const slides = entry.timing.slides.filter((c) => c.slide !== e.slide!.id);
    if (slides.length === 0) return { ...entry, timing: null };
    return { ...entry, timing: { startMs: slides[0].startMs, slides } };
  });
  return { ...song, order };
}

/**
 * Marcar sincronizando (Espacio): como `markStart`, y además quita las marcas que quedarían desordenadas respecto a
 * esta: las de diapositivas siguientes que empiezan antes o a la vez, y las de anteriores que empiezan después o a
 * la vez. Así volver a marcar una diapositiva que se pasó es «seguir marcando desde aquí» y nunca deja errores.
 */
export function markInSequence(song: MediaSong, expanded: Expanded[], k: number, ms: number): MediaSong {
  let next = markStart(song, expanded, k, ms);
  const marked = Math.max(0, Math.round(ms));
  const starts = slideStarts(expand2(next, expanded));
  for (let j = expanded.length - 1; j >= 0; j--) {
    if (j === k) continue;
    const start = starts[j];
    if (start === null) continue;
    if ((j > k && start <= marked) || (j < k && start >= marked)) next = clearStart(next, expand2(next, expanded), j);
  }
  return next;
}

/** Las mismas diapositivas desplegadas, con las apariciones (y sus tiempos) de la canción cambiada. */
function expand2(song: MediaSong, expanded: Expanded[]): Expanded[] {
  return expanded.map((e) => ({ ...e, entry: song.order[e.entryIndex] }));
}

export function formatTime(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 100));
  const tenths = total % 10;
  const seconds = Math.floor(total / 10);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}.${tenths}`;
}

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
    return e.slideIndex === 0 ? timing.startMs : null;
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

export function formatTime(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 100));
  const tenths = total % 10;
  const seconds = Math.floor(total / 10);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}.${tenths}`;
}

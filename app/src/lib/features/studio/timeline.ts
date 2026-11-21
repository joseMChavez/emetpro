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
/** Repetir una sección salta un poco antes de su final (la pista se acaba sola si era la última). */
const LOOP_EARLY = 60;

/**
 * Avance mientras suena (modo híbrido). Si la pista sigue dentro de la diapositiva en pantalla, nada. Si acaba de
 * terminarla, la siguiente del orden: donde siga la grabación, o saltando a donde suena esa (orden propio).
 * Si la pista está en otra parte (se movió la barra), la diapositiva que suena ahí.
 */
export function step(
  ranges: (Range | null)[],
  starts: (number | null)[],
  current: number,
  ms: number,
  /** Repetir esta aparición: al terminar su última diapositiva, vuelve a la primera. */
  loop: { first: number; last: number } | null = null,
): Step {
  const range = ranges[current];
  // Repetir: un poco antes del final, para que el salto caiga a tiempo aunque sea la última de la pista.
  if (loop && range && current === loop.last && ms >= range.end - LOOP_EARLY && ms < range.end + 4 * SLACK) {
    const first = ranges[loop.first];
    if (first) return { kind: "show", index: loop.first, seek: first.start };
  }
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

// ---- Por secciones: se marca dónde empieza cada parte y la app reparte sus diapositivas ----

/** Peso de una diapositiva al repartir: las de más palabras se ven más tiempo (y ninguna casi nada). */
function weight(lines: string[]): number {
  return wordsInSlide(lines) + 3;
}

/**
 * Reparte las diapositivas de cada aparición con tiempo entre su comienzo y su final (el siguiente comienzo en la
 * grabación), según cuántas palabras tiene cada una. Las marcadas a mano se quedan y hacen de tope; solo se
 * escriben las que faltan y las que ya repartió antes la app (`auto`). Devuelve la canción y cuántas repartió.
 */
export function distributeSlides(song: MediaSong, durationMs: number): { song: MediaSong; filled: number } {
  const entryStarts = song.order.map((e) => e.timing?.startMs).filter((s): s is number => s !== undefined && s < durationMs);
  let filled = 0;
  const order = song.order.map((entry) => {
    const timing = entry.timing;
    const section = song.sections.find((x) => x.id === entry.section);
    if (!timing || !section || section.slides.length < 2 || timing.startMs >= durationMs) return entry;
    const end = Math.min(durationMs, ...entryStarts.filter((s) => s > timing.startMs));
    const manual = new Map(timing.slides.filter((c) => !c.auto).map((c) => [c.slide, c]));
    // Topes: la primera (con su marca o con el comienzo de la aparición) y las marcadas a mano.
    const anchors: { index: number; at: number }[] = [];
    section.slides.forEach((slide, index) => {
      const cue = manual.get(slide.id);
      if (cue) anchors.push({ index, at: cue.startMs });
      else if (index === 0) anchors.push({ index, at: timing.startMs });
    });
    const slides: SlideCue[] = [...manual.values()];
    anchors.forEach((anchor, a) => {
      const to = anchors[a + 1] ?? { index: section.slides.length, at: end };
      // Topes desordenados (una marca a mano fuera de sitio): ese tramo no se toca, la validación lo dice.
      if (to.at <= anchor.at || to.index - anchor.index < 2) return;
      const weights = section.slides.slice(anchor.index, to.index).map((x) => weight(x.lines));
      const total = weights.reduce((x, y) => x + y, 0);
      let sum = weights[0];
      for (let i = anchor.index + 1; i < to.index; i++) {
        slides.push({ slide: section.slides[i].id, startMs: Math.round(anchor.at + ((to.at - anchor.at) * sum) / total), auto: true });
        sum += weights[i - anchor.index];
        filled++;
      }
    });
    const position = new Map(section.slides.map((x, i) => [x.id, i]));
    slides.sort((a, b) => (position.get(a.slide) ?? 0) - (position.get(b.slide) ?? 0));
    return { ...entry, timing: { ...timing, slides } };
  });
  return { song: { ...song, order }, filled };
}

/** La primera diapositiva desplegada (o la instrumental) de cada aparición, en orden. */
export function entryHeads(expanded: Expanded[]): number[] {
  return expanded.map((e, k) => (k === 0 || expanded[k - 1].entryIndex !== e.entryIndex ? k : -1)).filter((k) => k >= 0);
}

/**
 * Marcar por secciones (Espacio): la aparición de la diapositiva desplegada k empieza en ms. Lo que había repartido
 * la app dentro de ella se descarta (se repartirá de nuevo); lo desordenado se quita como en `markInSequence`.
 */
export function markSection(song: MediaSong, expanded: Expanded[], k: number, ms: number): MediaSong {
  const e = expanded[k];
  if (!e) return song;
  const next = markInSequence(song, expanded, k, ms);
  return {
    ...next,
    order: next.order.map((entry, i) =>
      i === e.entryIndex && entry.timing ? { ...entry, timing: { ...entry.timing, slides: entry.timing.slides.filter((c) => !c.auto) } } : entry,
    ),
  };
}

/** Quita todos los tiempos de una aparición (deshacer al marcar por secciones). */
export function clearEntry(song: MediaSong, entryIndex: number): MediaSong {
  return { ...song, order: song.order.map((entry, i) => (i === entryIndex ? { ...entry, timing: null } : entry)) };
}

// ---- Palabra por palabra ----

/** Palabras de una línea: cada tramo sin espacios, como las marcas (D-023). */
export function wordsIn(line: string): number {
  return line.split(/\s+/).filter(Boolean).length;
}

/** Palabras de una diapositiva, contando todas sus líneas. */
export function wordsInSlide(lines: string[]): number {
  return lines.reduce((total, line) => total + wordsIn(line), 0);
}

/** La palabra w de la diapositiva (contando todas sus líneas) → su línea y su número dentro de ella. */
function wordPlace(lines: string[], w: number): { line: number; word: number } | null {
  let rest = w;
  for (let line = 0; line < lines.length; line++) {
    const count = wordsIn(lines[line]);
    if (rest < count) return { line, word: rest };
    rest -= count;
  }
  return null;
}

/**
 * Marca que la palabra w (de toda la diapositiva) de la diapositiva desplegada k empieza en ms. La primera
 * palabra marca también la diapositiva, y la primera de cada línea, la línea. Volver a marcar una palabra quita
 * las marcas de las que venían después en la diapositiva (seguir marcando desde aquí).
 */
export function markWord(song: MediaSong, expanded: Expanded[], k: number, w: number, ms: number): MediaSong {
  const e = expanded[k];
  if (!e?.slide) return song;
  const place = wordPlace(e.slide.lines, w);
  if (!place) return song;
  ms = Math.max(0, Math.round(ms));
  let next = w === 0 ? markInSequence(song, expanded, k, ms) : song;
  const entry = next.order[e.entryIndex];
  const cue = entry.timing?.slides.find((c) => c.slide === e.slide!.id);
  if (!entry.timing || !cue) return next;
  const lines = (cue.lines ?? []).slice(0, place.line + 1).map((l) => ({ ...l, words: [...(l.words ?? [])] }));
  // Líneas anteriores sin marcar (se marcó desde la mitad): empiezan con esta palabra, sin palabras propias.
  while (lines.length <= place.line) lines.push({ startMs: ms, words: [] });
  const line = lines[place.line];
  if (place.word === 0) line.startMs = ms;
  line.words = [...line.words.slice(0, place.word), ms];
  // Un hueco (palabras de antes sin marcar) se rellena con esta marca para que el orden siga valiendo.
  for (let i = 0; i < place.word; i++) line.words[i] ??= ms;
  delete line.endMs;
  const slides = entry.timing.slides.map((c) => (c === cue ? { ...c, lines } : c));
  const order = next.order.map((o, i) => (i === e.entryIndex ? { ...o, timing: { ...entry.timing!, slides } } : o));
  next = { ...next, order };
  return next;
}

/** Quita las marcas de palabras de la diapositiva desplegada k (deja la de la diapositiva). */
export function clearWords(song: MediaSong, expanded: Expanded[], k: number): MediaSong {
  const e = expanded[k];
  const entry = e && song.order[e.entryIndex];
  if (!e?.slide || !entry?.timing) return song;
  const slides = entry.timing.slides.map((c) => (c.slide === e.slide!.id ? { slide: c.slide, startMs: c.startMs } : c));
  return { ...song, order: song.order.map((o, i) => (i === e.entryIndex ? { ...o, timing: { ...entry.timing!, slides } } : o)) };
}

/**
 * Cuándo empieza cada palabra de cada diapositiva desplegada (todas sus líneas seguidas, como las numera la
 * salida), para iluminar la que suena. Una aparición sin tiempos propios usa los de otra vez que suena su sección.
 * null si la diapositiva no tiene palabras marcadas.
 */
export function slideWords(expanded: Expanded[]): ((number | null)[] | null)[] {
  const cueOf = (i: number) => {
    const e = expanded[i];
    return e.slide ? e.entry.timing?.slides.find((c) => c.slide === e.slide!.id && c.lines?.some((l) => l.words?.length)) : undefined;
  };
  return expanded.map((e, i) => {
    if (!e.slide) return null;
    // La propia; si no, la de otra vez que suena (la más cercana antes; si no, la primera después).
    const same = expanded.map((x, j) => j).filter((j) => j !== i && expanded[j].slide?.id === e.slide!.id);
    const source = [i, ...same.filter((j) => j < i).reverse(), ...same.filter((j) => j > i)].find((j) => cueOf(j));
    const cue = source === undefined ? undefined : cueOf(source);
    if (!cue) return null;
    const out: (number | null)[] = [];
    e.slide.lines.forEach((text, li) => {
      const words = cue.lines?.[li]?.words ?? [];
      for (let wi = 0; wi < wordsIn(text); wi++) out.push(words[wi] ?? null);
    });
    return out.some((w) => w !== null) ? out : null;
  });
}

/** La palabra que suena en ese instante: la última que ya empezó. null antes de la primera. */
export function activeWord(words: (number | null)[] | null | undefined, ms: number): number | null {
  if (!words) return null;
  let found: number | null = null;
  words.forEach((start, w) => {
    if (start !== null && start <= ms) found = w;
  });
  return found;
}

export function formatTime(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 100));
  const tenths = total % 10;
  const seconds = Math.floor(total / 10);
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}.${tenths}`;
}

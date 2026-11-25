// Sincronización automática (D-029): cada diapositiva entra cuando se empieza a cantar. Whisper dice qué palabras
// oyó y cuándo; aquí se alinean con la letra de la canción, en el orden en que se canta (coros repetidos incluidos),
// y cada diapositiva toma el tiempo de su primera palabra. Con la voz separada, el tiempo se ajusta al arranque de
// la voz más cercano (Whisper yerra ±0,5 s; el arranque, no). Las que no se reconocen se reparten entre las vecinas
// y quedan «para revisar». Las marcas hechas a mano no se tocan.
import type { MediaSong, SlideCue, WhisperSegment } from "$lib/platform/types";
import type { Expanded } from "./song";
import { normalize } from "./whisper";

export interface HeardWord {
  text: string;
  startMs: number;
  endMs: number;
}

/** Lo que se pidió a «Preparar con la pista». */
export interface PrepareChoice {
  lyrics: boolean;
  sync: boolean;
  voice: boolean;
}

export interface SyncResult {
  /** Comienzo propuesto de cada diapositiva desplegada (null: no se sabe). */
  starts: (number | null)[];
  /** Propuestas dudosas: pocas palabras reconocidas o deducidas de las vecinas. */
  doubtful: boolean[];
  /** Diapositivas (con letra) con tiempo propuesto y cuántas hay. */
  synced: number;
  total: number;
}

/** La diapositiva entra un poco antes de que se cante, para que se alcance a leer. */
export const LEAD_MS = 300;
/** Una palabra cantada dura más o menos esto (para ir hacia atrás desde la primera reconocida). */
const WORD_MS = 450;
const MIN_GAP_MS = 400;
/** Ventana para ajustar al arranque de la voz: hacia atrás se mira más (la estimación desde una palabra posterior
 *  de la diapositiva suele quedar tarde), sin pasar de la diapositiva anterior. */
const SNAP_BEFORE_MS = 3000;
const SNAP_AFTER_MS = 900;
/** Dentro de una diapositiva no se canta con un hueco así (una nota sostenida llega a 9 s entre comienzos, pero
 *  entonces todas sus palabras siguen en un grupo; probado de 4 a 12 s, 6 s dio los menos errores). */
const CLUSTER_GAP_MS = 6000;
const MIN_SIMILARITY = 0.5;
const DOUBT_BELOW = 0.4;
const NOT_LYRICS = /^[[(♪*].*[\])♪*]$/;
const CREDITS = /librivox|amara\.org|subt[ií]tul|suscr[ií]bete|gracias por ver|thanks for watching|performed (for|by)|sung (in|by)|^end of /i;

/** Las palabras oídas, sin las de tramos que no son letra (marcas de música, créditos, música sola). */
export function heardWords(segments: WhisperSegment[]): HeardWord[] {
  return segments
    .filter((s) => !NOT_LYRICS.test(s.text.trim()) && !CREDITS.test(s.text) && s.noSpeech <= 0.6)
    .flatMap((s) => s.words ?? [])
    .filter((w) => normalize(w.text) !== "");
}

function levenshtein(a: string, b: string): number {
  const row = Array.from({ length: b.length + 1 }, (_, j) => j);
  for (let i = 1; i <= a.length; i++) {
    let prev = row[0];
    row[0] = i;
    for (let j = 1; j <= b.length; j++) {
      const keep = row[j];
      row[j] = Math.min(row[j] + 1, row[j - 1] + 1, prev + (a[i - 1] === b[j - 1] ? 0 : 1));
      prev = keep;
    }
  }
  return row[b.length];
}

/** Parecido entre dos palabras ya normalizadas (0 a 1). Las cortas tienen que coincidir. */
export function similarity(a: string, b: string): number {
  if (a === b) return 1;
  if (a.length <= 2 || b.length <= 2) return 0;
  return 1 - levenshtein(a, b) / Math.max(a.length, b.length);
}

/**
 * Alinea las palabras de la letra con las oídas, en orden, como un alineamiento global: se puede saltar una
 * palabra de la letra (no se oyó) o una oída (de más, mal oída o inventada). Devuelve, para cada palabra de la
 * letra, el índice de la oída que le corresponde, o -1.
 */
export function alignWords(expected: string[], heard: string[]): number[] {
  const n = expected.length;
  const m = heard.length;
  const SKIP_EXPECTED = -0.6;
  const SKIP_HEARD = -0.3;
  const score = new Float64Array((n + 1) * (m + 1));
  // 0 = vino en diagonal (pareja), 1 = se saltó una de la letra, 2 = se saltó una oída.
  const from = new Uint8Array((n + 1) * (m + 1));
  const at = (i: number, j: number) => i * (m + 1) + j;
  for (let i = 1; i <= n; i++) {
    score[at(i, 0)] = i * SKIP_EXPECTED;
    from[at(i, 0)] = 1;
  }
  for (let j = 1; j <= m; j++) {
    score[at(0, j)] = j * SKIP_HEARD;
    from[at(0, j)] = 2;
  }
  for (let i = 1; i <= n; i++) {
    for (let j = 1; j <= m; j++) {
      const sim = similarity(expected[i - 1], heard[j - 1]);
      let best = sim >= MIN_SIMILARITY ? score[at(i - 1, j - 1)] + 2 * sim - 0.5 : -Infinity;
      let way = 0;
      const skipE = score[at(i - 1, j)] + SKIP_EXPECTED;
      if (skipE > best) [best, way] = [skipE, 1];
      const skipH = score[at(i, j - 1)] + SKIP_HEARD;
      if (skipH > best) [best, way] = [skipH, 2];
      score[at(i, j)] = best;
      from[at(i, j)] = way;
    }
  }
  const match = new Array<number>(n).fill(-1);
  for (let i = n, j = m; i > 0 || j > 0; ) {
    const way = from[at(i, j)];
    if (i > 0 && j > 0 && way === 0) {
      match[i - 1] = j - 1;
      i--;
      j--;
    } else if (i > 0 && (j === 0 || way === 1)) i--;
    else j--;
  }
  return match;
}

/** Las palabras de una diapositiva, normalizadas. */
function slideWords(lines: string[]): string[] {
  return lines.flatMap((l) => normalize(l).split(" ")).filter(Boolean);
}

/** Propone el comienzo de cada diapositiva desplegada a partir de lo oído (y de los arranques de la voz). */
export function alignSlides(expanded: Expanded[], words: HeardWord[], onsets: number[] = [], leadMs = LEAD_MS): SyncResult {
  const owner: number[] = [];
  const indexInSlide: number[] = [];
  const expectedWords: string[] = [];
  const counts = expanded.map((e, k) => {
    const list = e.slide ? slideWords(e.slide.lines) : [];
    list.forEach((w, i) => {
      expectedWords.push(w);
      owner.push(k);
      indexInSlide.push(i);
    });
    return list.length;
  });
  const match = alignWords(
    expectedWords,
    words.map((w) => normalize(w.text)),
  );

  // Las palabras reconocidas de cada diapositiva, en grupos: un hueco largo separa lo que es de otro sitio (un
  // anuncio hablado que repite el título, un trozo que Whisper se inventó). Vale el grupo más grande.
  // El hueco se mide entre comienzos: Whisper a veces estira el final de una palabra sobre todo un silencio.
  const groups: { start: number; last: number; end: number; count: number }[][] = expanded.map(() => []);
  match.forEach((j, i) => {
    if (j < 0) return;
    const k = owner[i];
    // La palabra, hacia atrás lo que tardan las anteriores de la diapositiva.
    const start = Math.max(0, words[j].startMs - indexInSlide[i] * WORD_MS);
    const list = groups[k];
    const group = list[list.length - 1];
    if (group && words[j].startMs - group.last <= CLUSTER_GAP_MS) {
      group.last = words[j].startMs;
      group.end = words[j].endMs;
      group.count++;
    } else list.push({ start, last: words[j].startMs, end: words[j].endMs, count: 1 });
  });
  const raw: (number | null)[] = expanded.map(() => null);
  const matched = expanded.map(() => 0);
  const lastEnd: (number | null)[] = expanded.map(() => null);
  groups.forEach((list, k) => {
    if (!list.length) return;
    const best = list.reduce((a, b) => (b.count >= a.count ? b : a));
    raw[k] = best.start;
    matched[k] = best.count;
    lastEnd[k] = best.end;
  });
  const doubtful = expanded.map((e, k) => counts[k] > 0 && (raw[k] === null || matched[k] / counts[k] < DOUBT_BELOW || groups[k].length > 1));

  // Fuera de orden (una repetición mal emparejada): se descarta y se deduce de las vecinas.
  let previous = -Infinity;
  raw.forEach((t, k) => {
    if (t === null) return;
    if (t < previous + MIN_GAP_MS) {
      raw[k] = null;
      doubtful[k] = true;
    } else previous = t;
  });

  // Ajuste al arranque de la voz más cercano (sin pasar de la diapositiva anterior).
  if (onsets.length) {
    let floor = -Infinity;
    raw.forEach((t, k) => {
      if (t === null) return;
      let best: number | null = null;
      for (const o of onsets) {
        if (o < t - SNAP_BEFORE_MS || o > t + SNAP_AFTER_MS || o <= floor + MIN_GAP_MS) continue;
        if (best === null || Math.abs(o - t) < Math.abs(best - t)) best = o;
      }
      if (best !== null) raw[k] = best;
      floor = raw[k]!;
    });
  }

  // Las que faltan (con letra), repartidas entre las vecinas conocidas según sus palabras.
  const known = raw.map((t, k) => (t !== null && counts[k] > 0 ? k : -1)).filter((k) => k >= 0);
  for (let a = 0; a < known.length - 1; a++) {
    const [from, to] = [known[a], known[a + 1]];
    const span = raw[to]! - raw[from]!;
    const weight = (k: number) => counts[k] + 3;
    let total = 0;
    for (let k = from; k < to; k++) total += weight(k);
    let sum = weight(from);
    for (let k = from + 1; k < to; k++) {
      if (counts[k] > 0) {
        raw[k] = Math.round(raw[from]! + (span * sum) / total);
        doubtful[k] = true;
      }
      sum += weight(k);
    }
  }

  // Instrumentales: la del comienzo, en 0; las demás, un segundo después de la última palabra cantada antes.
  expanded.forEach((e, k) => {
    if (e.slide && counts[k] > 0) return;
    if (k === 0) raw[k] = 0;
    else {
      const sung = [...Array(k).keys()].reverse().find((p) => lastEnd[p] !== null);
      const next = raw.slice(k + 1).find((t) => t !== null);
      const at = sung !== undefined ? lastEnd[sung]! + 1000 : null;
      raw[k] = at !== null && (next == null || at < next - 500) ? at : null;
    }
  });

  // Adelanto para leer a tiempo, sin pisar la anterior.
  let last = -Infinity;
  const starts = raw.map((t, k) => {
    if (t === null) return null;
    const lead = expanded[k].slide && counts[k] > 0 ? leadMs : 0;
    const at = Math.round(Math.max(0, t - lead, last + 200));
    last = at;
    return at;
  });
  const lyricSlides = counts.filter((c) => c > 0).length;
  const synced = starts.filter((t, k) => t !== null && counts[k] > 0).length;
  return { starts, doubtful, synced, total: lyricSlides };
}

/**
 * Escribe los tiempos propuestos como marcas automáticas (`auto`). Las marcas a mano se quedan, y una propuesta que
 * quedaría desordenada respecto a ellas no se escribe. Devuelve la canción cambiada (no toca la original).
 */
export function applySync(song: MediaSong, expanded: Expanded[], starts: (number | null)[]): MediaSong {
  // Lo marcado a mano, por diapositiva desplegada.
  const manual = expanded.map((e) => {
    const timing = song.order[e.entryIndex]?.timing;
    if (!timing) return null;
    if (!e.slide) return e.section.slides.length === 0 ? timing.startMs : null;
    const cue = timing.slides.find((c) => c.slide === e.slide!.id && !c.auto);
    return cue ? cue.startMs : null;
  });
  const accepted = starts.map((t, k) => {
    if (t === null || manual[k] !== null) return null;
    const before = manual.slice(0, k).filter((x): x is number => x !== null).pop();
    const after = manual.slice(k + 1).find((x): x is number => x !== null);
    return (before === undefined || t > before) && (after === undefined || t < after) ? t : null;
  });
  const order = song.order.map((entry, i) => {
    const ks = expanded.map((e, k) => (e.entryIndex === i ? k : -1)).filter((k) => k >= 0);
    const section = ks.length ? expanded[ks[0]].section : null;
    if (!section) return entry;
    if (section.slides.length === 0) {
      const k = ks[0];
      if (manual[k] !== null || accepted[k] === null) return entry;
      return { ...entry, timing: { startMs: accepted[k]!, slides: [] } };
    }
    const cues: SlideCue[] = (entry.timing?.slides ?? []).filter((c) => !c.auto);
    for (const k of ks) {
      const slide = expanded[k].slide;
      if (slide && accepted[k] !== null) cues.push({ slide: slide.id, startMs: accepted[k]!, auto: true });
    }
    if (cues.length === 0) return { ...entry, timing: null };
    const position = new Map(section.slides.map((s, x) => [s.id, x]));
    cues.sort((a, b) => (position.get(a.slide) ?? 0) - (position.get(b.slide) ?? 0));
    return { ...entry, timing: { startMs: Math.min(...cues.map((c) => c.startMs)), slides: cues } };
  });
  return { ...song, order };
}

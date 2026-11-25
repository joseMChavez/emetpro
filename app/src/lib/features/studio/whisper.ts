// Del texto que oye Whisper en la pista a un borrador de letra (D-029). El borrador va al cuadro «Pegar letra
// completa» para que el usuario lo limpie: nada de esto se guarda ni se proyecta sin su revisión.
//
// 1. Limpiar: quitar lo que no es letra (marcas de música, créditos, tramos sin voz) y los bucles que Whisper se
//    inventa en la música sola (la misma frase decenas de veces seguidas).
// 2. Líneas: un tramo largo con varias frases se corta en sus comas y puntos.
// 3. Secciones: empieza una al haber una pausa o un tramo largo (un interludio) y al volver a sonar la primera
//    línea de una sección anterior (cada estrofa de «Noche de paz» empieza igual).
// 4. Borrador: encabezados («Estrofa 1:», «Coro:») y dos líneas por diapositiva; una sección que se repite igual
//    es el coro y, como en el cuadro de pegar, se escribe una vez y después solo su encabezado.
import type { WhisperSegment } from "$lib/platform/types";

export interface HeardLine {
  text: string;
  startMs: number;
  endMs: number;
}

/** Marcas que no son letra: [MUSIC], (música), ♪ ♪, *aplausos*. */
const NOT_LYRICS = /^[[(♪*].*[\])♪*]$/;
/** Créditos y frases que Whisper suele inventar al final de un audio (aprendidas de subtítulos de videos). */
const CREDITS = /librivox|amara\.org|subt[ií]tul|suscr[ií]bete|gracias por ver|thanks for watching|performed (for|by)|sung (in|by)|^end of /i;
/** Más que esto es casi seguro música sin voz. */
const NO_SPEECH = 0.6;
/** Repetida tantas veces seguidas, una frase es un bucle inventado, no un coro. */
const LOOP = 5;
const LONG_LINE = 42;
const SHORT_PIECE = 12;
const SECTION_GAP_MS = 2000;
const MIN_SECTION_LINES = 3;

/** Para comparar: sin mayúsculas, acentos ni signos. */
export function normalize(text: string): string {
  return text
    .toLocaleLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .replace(/[^\p{L}\p{N}]+/gu, " ")
    .trim();
}

function tidy(text: string): string {
  const t = text
    .trim()
    .replace(/^[-–—"“”«»'‘’\s]+|["“”«»'‘’\s]+$/g, "")
    .replace(/[.,;:]+$/, "")
    .trim();
  return t.charAt(0).toLocaleUpperCase() + t.slice(1);
}

/** Corta un tramo largo en sus frases, repartiendo su tiempo según el largo de cada una. */
function split(segment: WhisperSegment): HeardLine[] {
  const text = segment.text.trim();
  if (text.length <= LONG_LINE) return [{ text, startMs: segment.startMs, endMs: segment.endMs }];
  const pieces: string[] = [];
  for (const piece of text.split(/(?<=[.!?;,])\s+/)) {
    if (pieces.length > 0 && piece.length < SHORT_PIECE) pieces[pieces.length - 1] += ` ${piece}`;
    else pieces.push(piece);
  }
  const span = segment.endMs - segment.startMs;
  let at = 0;
  return pieces.map((piece) => {
    const startMs = segment.startMs + Math.round((span * at) / text.length);
    at += piece.length + 1;
    return { text: piece, startMs, endMs: segment.startMs + Math.round((span * Math.min(at, text.length)) / text.length) };
  });
}

/** Las líneas de letra que se oyeron, limpias; `loops`: bucles inventados que se quitaron (conviene avisar). */
export function heardLines(segments: WhisperSegment[]): { lines: HeardLine[]; loops: number } {
  const lines: HeardLine[] = [];
  for (const segment of segments) {
    const text = segment.text.trim();
    if (!text || NOT_LYRICS.test(text) || CREDITS.test(text) || segment.noSpeech > NO_SPEECH) continue;
    for (const line of split(segment)) {
      const clean = tidy(line.text);
      if (normalize(clean)) lines.push({ ...line, text: clean });
    }
  }
  // Bucles: una racha de LOOP o más iguales seguidas se queda en una.
  const out: HeardLine[] = [];
  let loops = 0;
  for (let i = 0; i < lines.length; ) {
    let j = i + 1;
    while (j < lines.length && normalize(lines[j].text) === normalize(lines[i].text)) j++;
    if (j - i >= LOOP) {
      out.push({ ...lines[i], endMs: lines[j - 1].endMs });
      loops++;
    } else out.push(...lines.slice(i, j));
    i = j;
  }
  return { lines: out, loops };
}

/** Dónde empieza cada sección (índices de línea; siempre el 0 si hay líneas). */
export function sectionStarts(lines: HeardLine[]): number[] {
  if (lines.length === 0) return [];
  const durations = lines.map((l) => l.endMs - l.startMs).sort((a, b) => a - b);
  const median = durations[Math.floor(durations.length / 2)];
  const starts = [0];
  for (let i = 1; i < lines.length; i++) {
    const last = starts[starts.length - 1];
    if (i - last < 1) continue;
    const previous = lines[i - 1];
    const pause = lines[i].startMs - previous.endMs >= SECTION_GAP_MS;
    // Un tramo mucho más largo que los demás suele llevar dentro el interludio entre secciones.
    const interlude = previous.endMs - previous.startMs >= Math.max(9000, median * 1.6);
    const again = i - last >= MIN_SECTION_LINES && starts.some((s) => normalize(lines[s].text) === normalize(lines[i].text));
    if (pause || interlude || again) starts.push(i);
  }
  return starts;
}

/** El borrador para el cuadro de pegar: encabezados en el idioma de la app y `perSlide` líneas por diapositiva. */
export function lyricsDraft(lines: HeardLine[], words: { verse: string; chorus: string }, perSlide = 2): string {
  const starts = sectionStarts(lines);
  const sections = starts.map((s, k) => lines.slice(s, starts[k + 1] ?? lines.length).map((l) => l.text));
  const keyOf = (section: string[]) => section.map(normalize).join("\n");
  const count = new Map<string, number>();
  for (const s of sections) count.set(keyOf(s), (count.get(keyOf(s)) ?? 0) + 1);
  const written = new Set<string>();
  let verse = 0;
  const out: string[] = [];
  for (const section of sections) {
    const key = keyOf(section);
    if ((count.get(key) ?? 0) > 1) {
      out.push(`${words.chorus}:`);
      // El coro se escribe la primera vez; después basta su encabezado.
      if (written.has(key)) continue;
      written.add(key);
    } else out.push(`${words.verse} ${++verse}:`);
    for (let i = 0; i < section.length; i += perSlide) {
      out.push(...section.slice(i, i + perSlide));
      if (i + perSlide < section.length) out.push("");
    }
    out.push("");
  }
  return out.join("\n").trim() + "\n";
}

/** Remuestrea a `rate` por interpolación lineal (cuando el navegador no pudo dar esa frecuencia exacta). */
export function resample(samples: Float32Array, from: number, rate: number): Float32Array {
  if (from === rate) return samples;
  const out = new Float32Array(Math.floor((samples.length * rate) / from));
  const step = from / rate;
  for (let i = 0; i < out.length; i++) {
    const x = i * step;
    const a = Math.floor(x);
    const b = Math.min(a + 1, samples.length - 1);
    out[i] = samples[a] + (samples[b] - samples[a]) * (x - a);
  }
  return out;
}

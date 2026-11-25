// Tonalidad de la pista (D-029): se detecta al cargarla y se muestra junto al tono, para los músicos
// («Sol mayor», y «Sol → La» si se sube un tono). Es una propuesta: quien prepara la corrige si no es.
//
// Cómo: cromagrama (cuánta energía tiene cada una de las 12 notas a lo largo de la canción, por FFT) comparado
// con los perfiles de Albrecht y Shanahan (2013) de las 24 tonalidades; la más parecida gana. El final pesa más:
// casi toda canción termina en su tónica. Probado con 62 grabaciones de tonalidad conocida (Bach y Chopin, que
// modulan mucho más que una canción de alabanza): acierta 53; de los fallos, 7 son la relativa, la dominante o
// la homónima. Con los perfiles de Krumhansl-Kessler acertaba la mitad.
import type { MusicalKey } from "$lib/platform";

/** Frecuencia a la que se analiza: hasta ~5 kHz basta para las notas (la fundamental de una voz va de 80 a 1000 Hz). */
export const ANALYSIS_RATE = 11025;
const FRAME = 4096;
/** Notas entre Do2 (65 Hz) y Do7 (2093 Hz): por debajo el bajo engaña a la FFT, por encima son armónicos. */
const LOW_HZ = 65;
const HIGH_HZ = 2100;

const MAJOR = [0.238, 0.006, 0.111, 0.006, 0.137, 0.094, 0.016, 0.214, 0.009, 0.08, 0.008, 0.081];
const MINOR = [0.22, 0.006, 0.104, 0.123, 0.019, 0.103, 0.012, 0.214, 0.062, 0.022, 0.061, 0.052];
/** El último 15 % de la canción cuenta cuatro veces (1 + 3). */
const ENDING = 0.15;
const ENDING_EXTRA = 3;

export interface KeyGuess {
  key: MusicalKey;
  /** La segunda más parecida: casi siempre la relativa o la dominante, que es donde se equivoca cuando falla. */
  alternative: MusicalKey;
}

/** FFT de raíz 2 sobre re/im (en el sitio). */
function fft(re: Float64Array, im: Float64Array) {
  const n = re.length;
  for (let i = 1, j = 0; i < n; i++) {
    let bit = n >> 1;
    for (; j & bit; bit >>= 1) j ^= bit;
    j ^= bit;
    if (i < j) {
      [re[i], re[j]] = [re[j], re[i]];
      [im[i], im[j]] = [im[j], im[i]];
    }
  }
  for (let len = 2; len <= n; len <<= 1) {
    const angle = (-2 * Math.PI) / len;
    const wr = Math.cos(angle);
    const wi = Math.sin(angle);
    for (let i = 0; i < n; i += len) {
      let cr = 1;
      let ci = 0;
      for (let k = 0; k < len / 2; k++) {
        const a = i + k;
        const b = a + len / 2;
        const tr = re[b] * cr - im[b] * ci;
        const ti = re[b] * ci + im[b] * cr;
        re[b] = re[a] - tr;
        im[b] = im[a] - ti;
        re[a] += tr;
        im[a] += ti;
        const next = cr * wr - ci * wi;
        ci = cr * wi + ci * wr;
        cr = next;
      }
    }
  }
}

/** Energía de cada nota (Do = 0) en toda la señal; cada tramo pesa lo mismo, suene fuerte o suave, salvo el final. */
export function chroma(samples: Float32Array, rate: number): Float64Array {
  const total = new Float64Array(12);
  const endsAt = samples.length * (1 - ENDING);
  const window = new Float64Array(FRAME);
  for (let i = 0; i < FRAME; i++) window[i] = 0.5 - 0.5 * Math.cos((2 * Math.PI * i) / (FRAME - 1));
  // Cada bin de la FFT, a qué nota va y con qué peso (más cuanto más cerca del centro de la nota).
  const bins: { bin: number; pc: number; weight: number }[] = [];
  for (let k = 1; k < FRAME / 2; k++) {
    const hz = (k * rate) / FRAME;
    if (hz < LOW_HZ || hz > HIGH_HZ) continue;
    const midi = 12 * Math.log2(hz / 440) + 69;
    const nearest = Math.round(midi);
    const off = Math.abs(midi - nearest);
    if (off > 0.4) continue;
    bins.push({ bin: k, pc: ((nearest % 12) + 12) % 12, weight: 1 - off / 0.4 });
  }
  const re = new Float64Array(FRAME);
  const im = new Float64Array(FRAME);
  const frame = new Float64Array(12);
  for (let start = 0; start + FRAME <= samples.length; start += FRAME) {
    let energy = 0;
    for (let i = 0; i < FRAME; i++) {
      const v = samples[start + i];
      energy += v * v;
      re[i] = v * window[i];
      im[i] = 0;
    }
    if (energy / FRAME < 1e-6) continue; // silencio
    fft(re, im);
    frame.fill(0);
    for (const { bin, pc, weight } of bins) frame[pc] += weight * Math.sqrt(re[bin] * re[bin] + im[bin] * im[bin]);
    const max = Math.max(...frame);
    if (max <= 0) continue;
    const weight = start >= endsAt ? 1 + ENDING_EXTRA : 1;
    for (let p = 0; p < 12; p++) total[p] += (weight * frame[p]) / max;
  }
  return total;
}

function correlation(a: ArrayLike<number>, b: ArrayLike<number>): number {
  let ma = 0;
  let mb = 0;
  for (let i = 0; i < 12; i++) {
    ma += a[i] / 12;
    mb += b[i] / 12;
  }
  let num = 0;
  let da = 0;
  let db = 0;
  for (let i = 0; i < 12; i++) {
    num += (a[i] - ma) * (b[i] - mb);
    da += (a[i] - ma) ** 2;
    db += (b[i] - mb) ** 2;
  }
  return da && db ? num / Math.sqrt(da * db) : 0;
}

/** La tonalidad cuyo perfil más se parece al cromagrama; null si no hay notas (silencio, solo percusión). */
export function keyFromChroma(c: ArrayLike<number>): KeyGuess | null {
  if (Math.max(...Array.from(c)) <= 0) return null;
  const scores: { key: MusicalKey; r: number }[] = [];
  for (let tonic = 0; tonic < 12; tonic++) {
    // El perfil girado para que su tónica caiga en esta nota.
    const rotate = (profile: number[]) => Array.from({ length: 12 }, (_, p) => profile[(p - tonic + 12) % 12]);
    scores.push({ key: { tonic, minor: false }, r: correlation(c, rotate(MAJOR)) });
    scores.push({ key: { tonic, minor: true }, r: correlation(c, rotate(MINOR)) });
  }
  scores.sort((a, b) => b.r - a.r);
  const [best, second] = scores;
  if (best.r <= 0) return null;
  return { key: best.key, alternative: second.key };
}

export function detectKey(samples: Float32Array, rate: number): KeyGuess | null {
  return keyFromChroma(chroma(samples, rate));
}

// ---- Nombres ----

const LETTERS_MAJOR = ["C", "D♭", "D", "E♭", "E", "F", "F♯", "G", "A♭", "A", "B♭", "B"];
const LETTERS_MINOR = ["C", "C♯", "D", "E♭", "E", "F", "F♯", "G", "G♯", "A", "B♭", "B"];
const SOLFEGE: Record<string, string> = { C: "Do", D: "Re", E: "Mi", F: "Fa", G: "Sol", A: "La", B: "Si" };

/** La tonalidad movida unos semitonos (el tono de la canción). */
export function transposeKey(key: MusicalKey, semitones: number): MusicalKey {
  return { tonic: (((key.tonic + semitones) % 12) + 12) % 12, minor: key.minor };
}

/** La tónica en cifrado americano («B♭», «F♯»), con la grafía habitual de esa tonalidad. */
export function letterName(key: MusicalKey): string {
  return (key.minor ? LETTERS_MINOR : LETTERS_MAJOR)[key.tonic];
}

/** La tónica en do-re-mi («Si♭», «Fa♯»). */
export function solfegeName(key: MusicalKey): string {
  const letter = letterName(key);
  return SOLFEGE[letter[0]] + letter.slice(1);
}

/** Cifrado corto: «G», «Em», «B♭m». */
export function chordSymbol(key: MusicalKey): string {
  return letterName(key) + (key.minor ? "m" : "");
}

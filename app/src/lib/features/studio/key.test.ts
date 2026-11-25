import { describe, expect, it } from "vitest";
import { ANALYSIS_RATE, chordSymbol, detectKey, letterName, solfegeName, transposeKey } from "./key";

/** Acordes con armónicos (como un piano o un coro), un segundo cada uno. Notas en MIDI. */
function progression(chords: number[][], seconds = 1): Float32Array {
  const per = ANALYSIS_RATE * seconds;
  const out = new Float32Array(per * chords.length);
  chords.forEach((notes, c) => {
    for (const midi of notes) {
      const hz = 440 * 2 ** ((midi - 69) / 12);
      for (let h = 1; h <= 4; h++) {
        for (let i = 0; i < per; i++) out[c * per + i] += (0.15 / h) * Math.sin((2 * Math.PI * hz * h * i) / ANALYSIS_RATE);
      }
    }
  });
  return out;
}

// Sol mayor: I IV V vi I (con el bajo una octava abajo).
const G = [43, 55, 59, 62];
const C = [48, 60, 64, 67];
const D = [50, 62, 66, 69];
const Em = [52, 59, 64, 67];
const songInG = [G, C, D, Em, C, D, G, G];
// Mi menor armónica: i iv V i (el Re♯ del V la distingue de Sol mayor).
const Am = [45, 57, 60, 64];
const B = [47, 59, 63, 66];
const songInEm = [Em, Am, B, Em, Am, B, Em, Em];

const shift = (chords: number[][], n: number) => chords.map((c) => c.map((m) => m + n));

describe("tonalidad de la pista", () => {
  it("reconoce una canción en Sol mayor", () => {
    const guess = detectKey(progression(songInG), ANALYSIS_RATE);
    expect(guess?.key).toEqual({ tonic: 7, minor: false });
    expect(guess?.alternative).not.toEqual(guess?.key);
  });

  it("reconoce Mi menor (no la confunde con su relativa Sol mayor)", () => {
    expect(detectKey(progression(songInEm), ANALYSIS_RATE)?.key).toEqual({ tonic: 4, minor: true });
  });

  it("al subir la canción, sube la tonalidad igual", () => {
    for (const n of [-5, -2, 1, 3, 6]) {
      const expected = transposeKey({ tonic: 7, minor: false }, n);
      expect(detectKey(progression(shift(songInG, n)), ANALYSIS_RATE)?.key).toEqual(expected);
    }
  });

  it("sin notas no propone nada", () => {
    expect(detectKey(new Float32Array(ANALYSIS_RATE * 3), ANALYSIS_RATE)).toBeNull();
  });

  it("nombra la tonalidad como la escribe un músico", () => {
    expect(letterName({ tonic: 10, minor: false })).toBe("B♭");
    expect(solfegeName({ tonic: 10, minor: false })).toBe("Si♭");
    expect(chordSymbol({ tonic: 1, minor: true })).toBe("C♯m");
    expect(chordSymbol({ tonic: 6, minor: false })).toBe("F♯");
    expect(solfegeName(transposeKey({ tonic: 7, minor: false }, -2))).toBe("Fa");
    expect(transposeKey({ tonic: 11, minor: true }, 3)).toEqual({ tonic: 2, minor: true });
  });
});

describe("tonalidad a otra frecuencia de muestreo", () => {
  it("con la frecuencia real, una pista a 12 kHz (48 kHz / 4) da la misma tonalidad", () => {
    // Misma progresión en Sol mayor, generada a 12 000 Hz.
    const rate = 12_000;
    const chords = [[43, 55, 59, 62], [48, 60, 64, 67], [50, 62, 66, 69], [43, 55, 59, 62]];
    const out = new Float32Array(rate * chords.length);
    chords.forEach((notes, c) => {
      for (const midi of notes) {
        const hz = 440 * 2 ** ((midi - 69) / 12);
        for (let h = 1; h <= 4; h++) for (let i = 0; i < rate; i++) out[c * rate + i] += (0.15 / h) * Math.sin((2 * Math.PI * hz * h * i) / rate);
      }
    });
    expect(detectKey(out, rate)?.key).toEqual({ tonic: 7, minor: false });
  });
});

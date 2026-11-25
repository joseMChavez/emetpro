// Cómo se escribe una tonalidad en cada idioma: do-re-mi con el cifrado americano al lado (en español e italiano
// los músicos usan los dos), o solo letras (inglés).
import { t } from "$lib/i18n/index.svelte";
import type { MusicalKey } from "$lib/platform";
import { chordSymbol, letterName, solfegeName, transposeKey } from "./key";

/** «Sol mayor (G)», «Mi menor (Em)»; en inglés «G major». */
export function keyLabel(key: MusicalKey): string {
  const solfege = t("studio.key.notation") === "solfege";
  const name = t(key.minor ? "studio.key.minor" : "studio.key.major", { note: solfege ? solfegeName(key) : letterName(key) });
  return solfege ? `${name} (${chordSymbol(key)})` : name;
}

/** Con el tono cambiado: «Sol mayor (G) → La mayor (A)»; sin cambio, solo la tonalidad. */
export function keyWithTone(key: MusicalKey, semitones: number): string {
  return semitones === 0 ? keyLabel(key) : `${keyLabel(key)} → ${keyLabel(transposeKey(key, semitones))}`;
}

/** Valor para un <select>: «7» (Sol mayor), «4m» (Mi menor); "" = sin saber. */
export function keyValue(key: MusicalKey | null | undefined): string {
  return key ? `${key.tonic}${key.minor ? "m" : ""}` : "";
}

export function keyFromValue(value: string): MusicalKey | null {
  const m = /^(\d{1,2})(m?)$/.exec(value);
  return m && Number(m[1]) < 12 ? { tonic: Number(m[1]), minor: m[2] === "m" } : null;
}

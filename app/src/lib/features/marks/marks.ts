// Palabras resaltadas en versículos y letras (D-023). Viajan con el contenido (`marks`), por versículo o diapositiva.
// Este archivo no importa nada de Tauri: lo usan la consola, la ventana de salida y el teléfono.
import type { OutputContent } from "$lib/platform/types";

export type MarkStyle = "marker" | "underline" | "color";
export type MarkColor = "yellow" | "green" | "blue" | "pink" | "orange" | "red";

/** Una palabra resaltada: su número en el texto (desde 0) y la palabra misma, para no pintar otra si el texto cambió. */
export interface WordMark {
  w: number;
  t: string;
  style: MarkStyle;
  color: MarkColor;
}

/** Por versículo («versión|cita») o por diapositiva (su número). */
export type Marks = Record<string, WordMark[]>;

/** El texto principal o, en la Biblia, la segunda versión. */
export type MarkPart = "text" | "second";

export const MARK_STYLES: MarkStyle[] = ["marker", "underline", "color"];
export const MARK_COLORS: MarkColor[] = ["yellow", "green", "blue", "pink", "orange", "red"];

/** Claros y saturados: se leen en un proyector sobre fondo oscuro, y con texto oscuro encima en el marcador. */
export const MARK_HEX: Record<MarkColor, string> = {
  yellow: "#ffd60a",
  green: "#4cd97b",
  blue: "#6aa8ff",
  pink: "#ff7eb6",
  orange: "#ff9f43",
  red: "#ff5a52",
};

/** Las palabras de un texto: cada tramo sin espacios, como las cuenta la ventana de salida al medirlas. */
export function words(text: string): string[] {
  return Array.from(text.matchAll(/\S+/gu), (m) => m[0]);
}

/** Dónde se guardan las marcas de esa parte de lo que está a la vista; null si no hay texto que marcar. */
export function marksKey(content: OutputContent, part: MarkPart): string | null {
  if (content.kind === "text") return part === "text" ? String(content.index) : null;
  if (content.kind !== "scripture") return null;
  const verse = content.verses[content.index];
  if (!verse) return null;
  if (part === "text") return `${content.version}|${verse.reference}`;
  return content.second && verse.second ? `${content.second.version}|${verse.second.reference}` : null;
}

/** El texto de esa parte de lo que está a la vista. */
export function partText(content: OutputContent, part: MarkPart): string {
  if (content.kind === "text") return part === "text" ? (content.slides[content.index] ?? "") : "";
  if (content.kind !== "scripture") return "";
  const verse = content.verses[content.index];
  return (part === "text" ? verse?.text : verse?.second?.text) ?? "";
}

function marksOf(content: OutputContent): Marks | undefined {
  return content.kind === "text" || content.kind === "scripture" ? content.marks : undefined;
}

/** Las marcas que valen para ese texto: las que siguen apuntando a la misma palabra, por número de palabra. */
export function validMarks(text: string, marks: readonly WordMark[] | undefined): Map<number, WordMark> {
  const valid = new Map<number, WordMark>();
  if (!marks?.length) return valid;
  const list = words(text);
  for (const mark of marks) if (list[mark.w] === mark.t) valid.set(mark.w, mark);
  return valid;
}

/** Las marcas de esa parte de lo que está a la vista, por número de palabra. */
export function currentMarks(content: OutputContent, part: MarkPart): Map<number, WordMark> {
  const key = marksKey(content, part);
  return key ? validMarks(partText(content, part), marksOf(content)?.[key]) : new Map();
}

export interface Segment {
  text: string;
  /** Número de palabra; null en los espacios. */
  w: number | null;
  mark: WordMark | null;
}

/** El texto en palabras y espacios, cada uno con su marca. Un espacio entre dos palabras con la misma marca la lleva
 *  también: el marcador y el subrayado de una frase salen seguidos. */
export function segments(text: string, marks: Map<number, WordMark>): Segment[] {
  const out: Segment[] = [];
  let w = 0;
  for (const piece of text.split(/(\S+)/u)) {
    if (piece === "") continue;
    if (/\S/u.test(piece)) out.push({ text: piece, w, mark: marks.get(w++) ?? null });
    else out.push({ text: piece, w: null, mark: null });
  }
  for (let i = 1; i < out.length - 1; i++) {
    const [before, gap, after] = [out[i - 1].mark, out[i], out[i + 1].mark];
    if (gap.w === null && before && after && before.style === after.style && before.color === after.color && before.style !== "color" && !gap.text.includes("\n")) {
      gap.mark = before;
    }
  }
  return out;
}

/** Marca (o desmarca) de la palabra `from` a la `to` de esa parte. Si la primera ya tiene esa misma marca, el gesto
 *  las quita; si no, todas quedan con la marca nueva. null si no hay nada que marcar. */
export function applyMark(content: OutputContent, part: MarkPart, from: number, to: number, style: MarkStyle, color: MarkColor): OutputContent | null {
  if (content.kind !== "text" && content.kind !== "scripture") return null;
  const key = marksKey(content, part);
  const list = words(partText(content, part));
  if (!key || list.length === 0) return null;
  const lo = Math.max(0, Math.min(from, to));
  const hi = Math.min(list.length - 1, Math.max(from, to));
  if (lo > hi) return null;
  const current = currentMarks(content, part);
  const first = current.get(lo);
  const remove = first?.style === style && first.color === color;
  // Las que ya no apuntan a su palabra se descartan al tocar ese versículo.
  const kept = [...current.values()].filter((m) => m.w < lo || m.w > hi);
  if (!remove) for (let w = lo; w <= hi; w++) kept.push({ w, t: list[w], style, color });
  kept.sort((a, b) => a.w - b.w);
  const marks = { ...content.marks };
  if (kept.length) marks[key] = kept;
  else delete marks[key];
  return { ...content, marks };
}

/** Quita las marcas de lo que está a la vista (las dos versiones). null si no tenía. */
export function clearMarks(content: OutputContent): OutputContent | null {
  if (!hasMarks(content)) return null;
  const marks = { ...marksOf(content) };
  for (const part of ["text", "second"] as const) {
    const key = marksKey(content, part);
    if (key) delete marks[key];
  }
  return { ...content, marks } as OutputContent;
}

/** Lo que está a la vista tiene alguna marca. */
export function hasMarks(content: OutputContent): boolean {
  return currentMarks(content, "text").size > 0 || currentMarks(content, "second").size > 0;
}

/** Se puede resaltar: una diapositiva de texto o un versículo. */
export function canMark(content: OutputContent): boolean {
  return marksKey(content, "text") !== null && partText(content, "text").trim() !== "";
}

/** Las mismas marcas en otro contenido igual (lo preparado tras marcar lo que está en pantalla, o al revés). */
export function withMarks(content: OutputContent, marks: Marks | undefined): OutputContent {
  if (content.kind !== "text" && content.kind !== "scripture") return content;
  return { ...content, marks: marks ? { ...marks } : {} };
}

// Texto con formato (D-027): títulos, viñetas, alineación, negrita, cursiva, subrayado y colores.
// Una diapositiva es una lista de bloques; cada bloque, tramos de texto con su formato. El texto plano de la
// diapositiva (los bloques unidos con saltos de línea) sigue mandando para las marcas (D-023), las animaciones
// de entrada (D-024), el teléfono y OBS: las palabras se cuentan sobre él.
import { segments, type WordMark } from "$lib/features/marks/marks";

export type BlockType = "title" | "subtitle" | "body" | "bullet" | "number";
export type BlockAlign = "left" | "center" | "right";
export type TextColor = "white" | "yellow" | "orange" | "red" | "pink" | "violet" | "blue" | "cyan" | "green";

export interface RichRun {
  text: string;
  b?: boolean;
  i?: boolean;
  u?: boolean;
  color?: TextColor;
}

export interface RichBlock {
  type: BlockType;
  /** Sin alineación: la del estilo general. */
  align?: BlockAlign;
  runs: RichRun[];
}

export type RichSlide = RichBlock[];

export const BLOCK_TYPES: BlockType[] = ["title", "subtitle", "body", "bullet", "number"];
export const TEXT_COLORS: TextColor[] = ["white", "yellow", "orange", "red", "pink", "violet", "blue", "cyan", "green"];

/** Claros y saturados, para leerse en un proyector sobre fondo oscuro (como los de la foto del sermón). */
export const TEXT_HEX: Record<TextColor, string> = {
  white: "#ffffff",
  yellow: "#ffd60a",
  orange: "#ff9f43",
  red: "#ff5a52",
  pink: "#ff6ec7",
  violet: "#b48cff",
  blue: "#6aa8ff",
  cyan: "#3ee0f0",
  green: "#4cd97b",
};

/** Texto de un bloque, sin formato. */
export function blockText(block: RichBlock): string {
  return block.runs.map((r) => r.text).join("");
}

/** Texto de la diapositiva, sin formato: los bloques en líneas. */
export function plainSlide(slide: RichSlide): string {
  return slide.map(blockText).join("\n");
}

/** Una diapositiva sin nada escrito. */
export function isEmptySlide(slide: RichSlide): boolean {
  return plainSlide(slide).trim() === "";
}

/** Texto plano a diapositiva: cada línea un bloque de texto normal. */
export function fromPlain(text: string): RichSlide {
  return text.split("\n").map((line) => ({ type: "body", runs: line ? [{ text: line }] : [] }));
}

/** Un trozo para dibujar: parte de un tramo, con su palabra (null en los espacios) y su marca. */
export interface RichPiece extends Omit<RichRun, "text"> {
  text: string;
  w: number | null;
  mark: WordMark | null;
}

export interface RenderBlock {
  type: BlockType;
  align?: BlockAlign;
  pieces: RichPiece[];
}

/**
 * La diapositiva lista para dibujar: cada bloque partido donde cambia el formato, la palabra o la marca. Las palabras
 * se numeran sobre el texto plano, igual que en una diapositiva sin formato.
 */
export function renderBlocks(slide: RichSlide, marks: Map<number, WordMark>): RenderBlock[] {
  const segs = segments(plainSlide(slide), marks);
  // Dónde empieza cada segmento en el texto plano.
  const starts: number[] = [];
  let at = 0;
  for (const seg of segs) {
    starts.push(at);
    at += seg.text.length;
  }
  const out: RenderBlock[] = [];
  let offset = 0;
  let si = 0;
  for (const block of slide) {
    const pieces: RichPiece[] = [];
    for (const run of block.runs) {
      const end = offset + run.text.length;
      let pos = offset;
      while (pos < end) {
        while (si < segs.length - 1 && starts[si] + segs[si].text.length <= pos) si++;
        const seg = segs[si];
        const segEnd = starts[si] + seg.text.length;
        const cut = Math.min(end, segEnd);
        const { text: _t, ...format } = run;
        pieces.push({ ...format, text: run.text.slice(pos - offset, cut - offset), w: seg.w, mark: seg.mark });
        pos = cut;
      }
      offset = end;
    }
    out.push({ type: block.type, align: block.align, pieces });
    offset += 1; // el salto de línea entre bloques
  }
  return out;
}

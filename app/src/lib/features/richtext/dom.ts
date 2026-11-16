// El editor (RichEditor.svelte) es un contenteditable: aquí se pasa del modelo a su HTML y del HTML (lo que dejen
// el usuario y los comandos del navegador) al modelo. Solo se lee lo que el modelo sabe guardar; lo demás se ignora.
import { BLOCK_TYPES, TEXT_COLORS, TEXT_HEX, type BlockAlign, type BlockType, type RichBlock, type RichRun, type RichSlide, type TextColor } from "./rich";

/** Color que pone el editor para «sin color»: se lee como ninguno. */
export const NO_COLOR_HEX = "#010203";

function escape(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function runHtml(run: RichRun): string {
  let html = escape(run.text);
  if (run.color) html = `<span style="color: ${TEXT_HEX[run.color]}">${html}</span>`;
  if (run.u) html = `<u>${html}</u>`;
  if (run.i) html = `<i>${html}</i>`;
  if (run.b) html = `<b>${html}</b>`;
  return html;
}

/** HTML del editor para una diapositiva. Un bloque vacío lleva <br> para poder escribir en él. */
export function slideHtml(slide: RichSlide): string {
  const blocks = slide.length > 0 ? slide : [{ type: "body" as const, runs: [] }];
  return blocks
    .map((b) => {
      const inner = b.runs.map(runHtml).join("") || "<br>";
      const align = b.align ? ` data-align="${b.align}" style="text-align: ${b.align}"` : "";
      return `<div data-type="${b.type}"${align}>${inner}</div>`;
    })
    .join("");
}

/** «rgb(255, 214, 10)», «#ffd60a» o «#fd0» → color de la paleta; null si no es ninguno. */
export function colorOf(value: string | null | undefined): TextColor | null {
  if (!value) return null;
  const hex = toHex(value.trim());
  if (!hex || hex === NO_COLOR_HEX) return null;
  return TEXT_COLORS.find((c) => TEXT_HEX[c] === hex) ?? null;
}

function toHex(value: string): string | null {
  const rgb = value.match(/^rgba?\(\s*(\d+)[\s,]+(\d+)[\s,]+(\d+)/i);
  if (rgb) return "#" + rgb.slice(1, 4).map((n) => Number(n).toString(16).padStart(2, "0")).join("");
  const short = value.match(/^#([0-9a-f])([0-9a-f])([0-9a-f])$/i);
  if (short) return `#${short[1]}${short[1]}${short[2]}${short[2]}${short[3]}${short[3]}`.toLowerCase();
  if (/^#[0-9a-f]{6}$/i.test(value)) return value.toLowerCase();
  return null;
}

interface Format {
  b?: boolean;
  i?: boolean;
  u?: boolean;
  color?: TextColor;
}

const BLOCK_TAGS = new Set(["DIV", "P", "H1", "H2", "H3", "H4", "LI", "UL", "OL", "BLOCKQUOTE"]);

function formatOf(el: HTMLElement, outer: Format): Format {
  const f: Format = { ...outer };
  const tag = el.tagName;
  if (tag === "B" || tag === "STRONG") f.b = true;
  if (tag === "I" || tag === "EM") f.i = true;
  if (tag === "U") f.u = true;
  const style = el.style;
  if (style.fontWeight === "bold" || Number(style.fontWeight) >= 600) f.b = true;
  if (style.fontWeight === "normal" || style.fontWeight === "400") f.b = false;
  if (style.fontStyle === "italic") f.i = true;
  if (style.fontStyle === "normal") f.i = false;
  if (style.textDecorationLine?.includes("underline") || style.textDecoration?.includes("underline")) f.u = true;
  const color = style.color || (tag === "FONT" ? el.getAttribute("color") : null);
  if (color) {
    const c = colorOf(color);
    f.color = c ?? undefined;
  }
  return f;
}

function sameFormat(a: RichRun, b: Format): boolean {
  return !!a.b === !!b.b && !!a.i === !!b.i && !!a.u === !!b.u && a.color === b.color;
}

function pushText(runs: RichRun[], text: string, f: Format) {
  if (!text) return;
  const last = runs[runs.length - 1];
  if (last && sameFormat(last, f)) {
    last.text += text;
    return;
  }
  const run: RichRun = { text };
  if (f.b) run.b = true;
  if (f.i) run.i = true;
  if (f.u) run.u = true;
  if (f.color) run.color = f.color;
  runs.push(run);
}

function blockTypeOf(el: HTMLElement, fallback: BlockType): BlockType {
  const t = el.dataset.type as BlockType | undefined;
  if (t && BLOCK_TYPES.includes(t)) return t;
  if (el.tagName === "H1") return "title";
  if (el.tagName === "H2" || el.tagName === "H3") return "subtitle";
  if (el.tagName === "LI") return el.parentElement?.tagName === "OL" ? "number" : "bullet";
  return fallback;
}

function alignOf(el: HTMLElement): BlockAlign | undefined {
  const a = (el.dataset.align || el.style.textAlign) as BlockAlign | string;
  return a === "left" || a === "center" || a === "right" ? a : undefined;
}

/** Lee el contenido del editor. Cada <div> (o <p>, <li>…) es un bloque; un <br> dentro de uno empieza otro igual. */
export function readSlide(root: HTMLElement): RichSlide {
  const blocks: RichBlock[] = [];
  let current: RichBlock | null = null;

  const open = (type: BlockType, align?: BlockAlign) => {
    current = { type, runs: [] };
    if (align) current.align = align;
    blocks.push(current);
    return current;
  };

  const walk = (node: Node, f: Format, type: BlockType, align: BlockAlign | undefined) => {
    if (node.nodeType === Node.TEXT_NODE) {
      const text = (node.textContent ?? "").replace(/ /g, " ").replace(/[\r\n]+/g, " ");
      if (!text) return;
      const block = current ?? open(type, align);
      pushText(block.runs, text, f);
      return;
    }
    if (!(node instanceof HTMLElement)) return;
    if (node.tagName === "BR") {
      // Un <br> al final de un bloque es solo el relleno del navegador; en medio, un salto: bloque nuevo.
      const isLast = !node.nextSibling || (node.nextSibling.nodeType === Node.TEXT_NODE && !node.nextSibling.textContent);
      if (!isLast) open(type, align);
      else if (!current) open(type, align);
      return;
    }
    if (BLOCK_TAGS.has(node.tagName)) {
      if (node.tagName === "UL" || node.tagName === "OL") {
        for (const child of node.childNodes) walk(child, f, type, align);
        return;
      }
      const t = blockTypeOf(node, type);
      const a = alignOf(node) ?? align;
      open(t, a);
      for (const child of node.childNodes) walk(child, formatOf(node, f), t, a);
      current = null;
      return;
    }
    const inner = formatOf(node, f);
    for (const child of node.childNodes) walk(child, inner, type, align);
  };

  for (const child of root.childNodes) walk(child, {}, "body", undefined);
  // Sin líneas vacías al final: el navegador suele dejar una.
  while (blocks.length > 1 && blockIsEmpty(blocks[blocks.length - 1])) blocks.pop();
  return blocks;
}

function blockIsEmpty(block: RichBlock): boolean {
  return block.runs.every((r) => r.text === "");
}

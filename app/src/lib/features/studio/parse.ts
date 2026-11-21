// Media Studio (D-029): arma secciones y orden a partir de una letra pegada entera.
//
// Con encabezados («Coro:», «[Estrofa 2]», «Puente», «Coro x2»): cada encabezado empieza una sección; dentro, una
// línea en blanco o una línea de asteriscos separa diapositivas. Un encabezado que se repite sin texto (o con el
// mismo texto) vuelve a poner esa sección en el orden; «x2» la pone dos veces.
// Sin encabezados: cada bloque separado por una línea en blanco es una sección de una diapositiva; los bloques
// idénticos son la misma sección (el coro), y los demás, estrofas.
// Puro, para probarlo con vitest.
import type { OrderEntry, SectionKind, SongSection } from "$lib/platform/types";

const KINDS: [SectionKind, RegExp][] = [
  ["preChorus", /^(pre[- ]?coro|pre[- ]?chorus|pre[- ]?ritornello|pre[- ]?estribillo)$/],
  ["chorus", /^(coro|chorus|estribillo|ritornello|refr[aá]n|refrain)$/],
  ["verse", /^(estrofa|verso|verse|strofa|stanza)$/],
  ["bridge", /^(puente|bridge|ponte)$/],
  ["intro", /^(intro|introducci[oó]n|introduzione)$/],
  ["instrumental", /^(instrumental|strumentale|solo)$/],
  ["interlude", /^(interludio|interlude)$/],
  ["tag", /^(tag|coda)$/],
  ["outro", /^(final|outro|finale|ending|cierre)$/],
];

export interface Header {
  kind: SectionKind;
  /** Número escrito («Estrofa 2» → 2); 0 si no tiene. */
  number: number;
  /** Veces que se repite («Coro x2»). */
  times: number;
}

/** Si la línea es un encabezado de sección, cuál. */
export function parseHeader(line: string): Header | null {
  let text = line.trim().toLowerCase();
  const bracketed = /^[[(].*[\])]$/.test(text);
  text = text.replace(/^[[(]\s*|\s*[\])]$/g, "").replace(/:\s*$/, "").trim();
  const match = /^(.+?)\s*(\d+)?\s*(?:[x×]\s*(\d+))?$/.exec(text);
  if (!match) return null;
  const name = match[1].trim();
  const kind = KINDS.find(([, re]) => re.test(name))?.[0];
  if (!kind) return null;
  // «Coro» solo en una línea cuenta; también con corchetes o dos puntos.
  if (!bracketed && line.trim().length > 30) return null;
  return { kind, number: Number(match[2] ?? 0), times: Math.max(1, Math.min(9, Number(match[3] ?? 1))) };
}

const SLIDE_BREAK = /^[ \t]*(?:\*[ \t]*){2,}$/;

/** Bloques de líneas separados por líneas en blanco o de asteriscos. */
function blocks(lines: string[]): string[][] {
  const out: string[][] = [];
  let current: string[] = [];
  for (const raw of lines) {
    const line = raw.trim();
    if (line === "" || SLIDE_BREAK.test(raw)) {
      if (current.length) out.push(current);
      current = [];
    } else current.push(line);
  }
  if (current.length) out.push(current);
  return out;
}

const key = (slides: string[][]) => slides.map((s) => s.join("\n")).join("\n\n").toLowerCase();

export function parseLyrics(text: string, newId: () => string): { sections: SongSection[]; order: OrderEntry[] } {
  const lines = text.replace(/\r\n?/g, "\n").split("\n");
  const sections: SongSection[] = [];
  const order: OrderEntry[] = [];
  const byKey = new Map<string, SongSection>();
  const addSection = (kind: SectionKind, slides: string[][]): SongSection => {
    const section: SongSection = { id: newId(), kind, label: "", slides: slides.map((l) => ({ id: newId(), lines: l })) };
    sections.push(section);
    return section;
  };
  const place = (section: SongSection, times = 1) => {
    for (let i = 0; i < times; i++) order.push({ id: newId(), section: section.id });
  };

  const hasHeaders = lines.some((l) => parseHeader(l) !== null);
  if (!hasHeaders) {
    const all = blocks(lines);
    const count = new Map<string, number>();
    for (const b of all) count.set(key([b]), (count.get(key([b])) ?? 0) + 1);
    for (const b of all) {
      const k = key([b]);
      const section = byKey.get(k) ?? addSection((count.get(k) ?? 0) > 1 ? "chorus" : "verse", [b]);
      byKey.set(k, section);
      place(section);
    }
    return { sections, order };
  }

  // Con encabezados: lo que haya antes del primero es una estrofa sin nombre.
  let header: Header | null = null;
  let body: string[] = [];
  /** Por tipo y número escrito: «Coro» repetido más abajo es el mismo coro. */
  const byName = new Map<string, SongSection>();
  const flush = () => {
    const slides = blocks(body);
    body = [];
    if (!header && slides.length === 0) return;
    const h = header ?? { kind: "verse" as SectionKind, number: 0, times: 1 };
    const name = `${h.kind}#${h.number}`;
    const known = byName.get(name);
    if (known && (slides.length === 0 || key(slides) === key(known.slides.map((s) => s.lines)))) return place(known, h.times);
    // Un encabezado sin texto la primera vez (una intro instrumental) es una sección sin diapositivas.
    const same = byKey.get(`${h.kind}|${key(slides)}`);
    const section = same && slides.length > 0 ? same : addSection(h.kind, slides);
    if (slides.length > 0) byKey.set(`${h.kind}|${key(slides)}`, section);
    if (!known) byName.set(name, section);
    place(section, h.times);
  };
  for (const line of lines) {
    const h = parseHeader(line);
    if (h) {
      flush();
      header = h;
    } else body.push(line);
  }
  flush();
  return { sections, order };
}

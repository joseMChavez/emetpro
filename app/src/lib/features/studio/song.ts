// Media Studio (D-029): la canción desplegada según su orden y el contenido que va a la pantalla.
// Puro: sin Svelte ni plataforma, para probarlo con vitest.
import type { MediaSong, OrderEntry, OutputContent, SectionKind, SongIssue, SongSection, SongSlide } from "$lib/platform/types";

/** Una diapositiva de la canción en el orden en que se canta. `slide: null` = sección sin diapositivas (instrumental). */
export interface Expanded {
  entry: OrderEntry;
  /** Posición de la aparición en el orden. */
  entryIndex: number;
  section: SongSection;
  slide: SongSlide | null;
  /** Posición de la diapositiva en su sección (0 para una instrumental). */
  slideIndex: number;
}

export function sectionOf(song: MediaSong, id: string): SongSection | undefined {
  return song.sections.find((s) => s.id === id);
}

export function expand(song: MediaSong): Expanded[] {
  const out: Expanded[] = [];
  song.order.forEach((entry, entryIndex) => {
    const section = sectionOf(song, entry.section);
    if (!section) return;
    if (section.slides.length === 0) out.push({ entry, entryIndex, section, slide: null, slideIndex: 0 });
    section.slides.forEach((slide, slideIndex) => out.push({ entry, entryIndex, section, slide, slideIndex }));
  });
  return out;
}

export function slideText(slide: SongSlide | null): string {
  return slide ? slide.lines.join("\n") : "";
}

/** La canción lista para «Preparado», desde la primera diapositiva. Una instrumental es una diapositiva vacía. */
export function studioContent(song: MediaSong, credit: string): OutputContent {
  const slides = expand(song).map((e) => slideText(e.slide));
  if (slides.length === 0) return { kind: "empty" };
  return {
    kind: "text",
    slides,
    index: 0,
    credit,
    background: song.background,
    studio: song.id,
    ...(song.animation ? { animation: { ...song.animation } } : {}),
  };
}

/** Número de cada sección entre las de su tipo (Estrofa 1, Estrofa 2); 0 si es la única de su tipo. */
export function kindNumber(song: MediaSong, section: SongSection): number {
  const same = song.sections.filter((s) => s.kind === section.kind && (!s.label || s.id === section.id));
  return same.length > 1 ? same.findIndex((s) => s.id === section.id) + 1 : 0;
}

// ---- Texto de una sección en el editor: una línea en blanco separa las diapositivas ----

export function sectionText(section: SongSection): string {
  return section.slides.map((s) => s.lines.join("\n")).join("\n\n");
}

/** Lee el texto escrito y conserva el id de cada diapositiva por su posición, así sus tiempos siguen valiendo. */
export function withText(section: SongSection, text: string, newId: () => string): SongSection {
  const blocks = text
    .split(/\n[ \t]*\n/)
    .map((b) => b.split("\n").map((l) => l.trim()).filter((l) => l.length > 0))
    .filter((lines) => lines.length > 0);
  return { ...section, slides: blocks.map((lines, i) => ({ id: section.slides[i]?.id ?? newId(), lines })) };
}

export function newId(): string {
  return crypto.randomUUID().slice(0, 8);
}

export function emptySong(id: string): MediaSong {
  return {
    id,
    title: "",
    artist: "",
    mode: "live",
    audio: null,
    sections: [],
    order: [],
    rights: { composition: "", lyrics: "", recording: "", license: "", notes: "" },
    reference: "",
    background: null,
  };
}

export const SECTION_KINDS: SectionKind[] = ["intro", "verse", "preChorus", "chorus", "bridge", "instrumental", "interlude", "tag", "outro", "custom"];

/**
 * Quita los tiempos que tienen errores (los de una diapositiva, o los de toda una aparición si el error es de ella),
 * para poder guardar sin perder el resto de la sincronización.
 */
export function dropTroubledTimings(song: MediaSong, issues: SongIssue[]): MediaSong {
  const troubled = issues.filter((i) => i.severity === "error" && i.at.entry);
  if (troubled.length === 0) return song;
  const order = song.order.map((entry) => {
    const mine = troubled.filter((i) => i.at.entry === entry.id);
    if (mine.length === 0 || !entry.timing) return entry;
    if (mine.some((i) => !i.at.slide)) return { ...entry, timing: null };
    const bad = new Set(mine.map((i) => i.at.slide));
    const slides = entry.timing.slides.filter((c) => !bad.has(c.slide));
    return { ...entry, timing: { ...entry.timing, slides } };
  });
  return { ...song, order };
}

/** Hay errores de tiempos que `dropTroubledTimings` puede quitar. */
export function hasTroubledTimings(issues: SongIssue[]): boolean {
  return issues.some((i) => i.severity === "error" && i.at.entry);
}

// Nombres de las secciones y de los problemas de Media Studio, en el idioma de la consola.
import { t, type MessageKey } from "$lib/i18n/index.svelte";
import type { IssueCode, MediaSong, SectionKind, SongIssue, SongSection } from "$lib/platform";
import { kindNumber, sectionOf } from "./song";

export function kindName(kind: SectionKind): string {
  return t(`studio.kind.${kind}` as MessageKey);
}

/** Su nombre propio, o el de su tipo con número si hay varias («Estrofa 2»). */
export function sectionName(song: MediaSong, section: SongSection): string {
  if (section.label.trim()) return section.label.trim();
  const n = kindNumber(song, section);
  return n ? `${kindName(section.kind)} ${n}` : kindName(section.kind);
}

export function issueText(code: IssueCode): string {
  return t(`studio.issue.${code}` as MessageKey);
}

/** Dónde está el problema, para leerlo: «Coro (2.ª vez) · línea 3». */
export function issuePlace(song: MediaSong, issue: SongIssue): string {
  const parts: string[] = [];
  const at = issue.at;
  const entryIndex = at.entry ? song.order.findIndex((e) => e.id === at.entry) : -1;
  const sectionId = at.section ?? (entryIndex >= 0 ? song.order[entryIndex].section : undefined);
  const section = sectionId ? sectionOf(song, sectionId) : undefined;
  if (section) {
    let name = sectionName(song, section);
    if (entryIndex >= 0) {
      const time = song.order.slice(0, entryIndex + 1).filter((e) => e.section === section.id).length;
      if (song.order.filter((e) => e.section === section.id).length > 1) name += ` (${t("studio.time", { n: time })})`;
    }
    parts.push(name);
  } else if (entryIndex >= 0) parts.push(t("studio.orderPosition", { n: entryIndex + 1 }));
  if (section && at.slide) {
    const n = section.slides.findIndex((s) => s.id === at.slide);
    if (n >= 0) parts.push(t("studio.slideN", { n: n + 1 }));
  }
  if (at.line !== undefined) parts.push(t("studio.lineN", { n: at.line + 1 }));
  if (at.word !== undefined) parts.push(t("studio.wordN", { n: at.word + 1 }));
  return parts.join(" · ");
}

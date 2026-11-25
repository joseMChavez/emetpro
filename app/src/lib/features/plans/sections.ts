// Secciones del plan como acordeón: cada elemento pertenece a la última sección que tiene encima.
// Lo que está antes de la primera sección no pertenece a ninguna y siempre se ve.
// Lo usan la consola y el control remoto, con el estado de qué está plegado en cada uno.

type Item = { id: string; kind: string };

/** Para cada posición, la sección a la que pertenece (la cabecera, a sí misma); null antes de la primera. */
export function owners(items: readonly Item[]): (string | null)[] {
  let owner: string | null = null;
  return items.map((item) => {
    if (item.kind === "section") owner = item.id;
    return owner;
  });
}

export interface SectionSummary {
  /** Elementos de la sección (sin contar la cabecera). */
  count: number;
  /** Cuántos ya pasaron por la pantalla. */
  shown: number;
  /** Tiene el elemento actual: plegada, la cabecera lo avisa. */
  current: boolean;
}

/** Resumen de cada sección, para su cabecera cuando está plegada. */
export function summaries(
  items: readonly Item[],
  shown: (id: string) => boolean,
  current: string | null,
): Map<string, SectionSummary> {
  const result = new Map<string, SectionSummary>();
  const of = owners(items);
  items.forEach((item, i) => {
    const owner = of[i];
    if (owner === null) return;
    if (item.kind === "section") {
      result.set(owner, { count: 0, shown: 0, current: false });
      return;
    }
    const s = result.get(owner)!;
    s.count++;
    if (shown(item.id)) s.shown++;
    if (item.id === current) s.current = true;
  });
  return result;
}

/** Sección de un elemento (null si está antes de la primera o no existe). */
export function sectionOf(items: readonly Item[], id: string): string | null {
  const i = items.findIndex((item) => item.id === id);
  return i < 0 ? null : owners(items)[i];
}

/** Si una posición queda oculta: los elementos de una sección plegada (su cabecera siempre se ve). */
export function hiddenAt(items: readonly Item[], folded: ReadonlySet<string>): boolean[] {
  const of = owners(items);
  return items.map((item, i) => item.kind !== "section" && of[i] !== null && folded.has(of[i]!));
}

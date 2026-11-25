// Qué secciones de cada plan están plegadas. Es una comodidad de quien mira (consola o teléfono), no parte del
// plan: se recuerda en este equipo y, si el navegador no deja guardar, simplemente dura lo que la sesión.
const STORAGE_KEY = "emetpro.planFolds";

function load(): Record<string, string[]> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    const value = raw ? JSON.parse(raw) : null;
    return value && typeof value === "object" ? value : {};
  } catch {
    return {};
  }
}

const folds = $state<Record<string, string[]>>(load());

function save() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(folds));
  } catch {
    // Sin almacenamiento: se queda en memoria.
  }
}

/** Las secciones plegadas del plan. */
export function folded(planId: string): Set<string> {
  return new Set(folds[planId] ?? []);
}

export function setFolded(planId: string, sectionIds: Iterable<string>) {
  const list = [...new Set(sectionIds)];
  if (list.length === 0) delete folds[planId];
  else folds[planId] = list;
  save();
}

export function toggleFold(planId: string, sectionId: string) {
  const set = folded(planId);
  if (set.has(sectionId)) set.delete(sectionId);
  else set.add(sectionId);
  setFolded(planId, set);
}

/** Despliega una sección (si estaba plegada): para que se vea lo que se acaba de preparar o mover. */
export function unfold(planId: string, sectionId: string | null) {
  if (sectionId === null) return;
  const set = folded(planId);
  if (set.delete(sectionId)) setFolded(planId, set);
}

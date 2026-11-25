import { describe, expect, it } from "vitest";
import { hiddenAt, owners, sectionOf, summaries } from "./sections";

const items = [
  { id: "a", kind: "song" },
  { id: "s1", kind: "section" },
  { id: "b", kind: "song" },
  { id: "c", kind: "scripture" },
  { id: "s2", kind: "section" },
  { id: "s3", kind: "section" },
  { id: "d", kind: "media" },
];

describe("secciones del plan como acordeón", () => {
  it("cada elemento pertenece a la sección que tiene encima", () => {
    expect(owners(items)).toEqual([null, "s1", "s1", "s1", "s2", "s3", "s3"]);
    expect(sectionOf(items, "c")).toBe("s1");
    expect(sectionOf(items, "a")).toBeNull();
    expect(sectionOf(items, "nada")).toBeNull();
  });

  it("plegar oculta los elementos de la sección, no su cabecera ni lo de antes", () => {
    expect(hiddenAt(items, new Set(["s1"]))).toEqual([false, false, true, true, false, false, false]);
    expect(hiddenAt(items, new Set(["s2", "s3"]))).toEqual([false, false, false, false, false, false, true]);
    expect(hiddenAt(items, new Set())).toEqual(items.map(() => false));
  });

  it("la cabecera resume cuántos hay, cuántos se mostraron y si tiene el actual", () => {
    const s = summaries(items, (id) => id === "b", "c");
    expect(s.get("s1")).toEqual({ count: 2, shown: 1, current: true });
    expect(s.get("s2")).toEqual({ count: 0, shown: 0, current: false });
    expect(s.get("s3")).toEqual({ count: 1, shown: 0, current: false });
    expect(s.has("a")).toBe(false);
  });
});

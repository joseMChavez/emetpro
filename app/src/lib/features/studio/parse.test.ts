import { describe, expect, it } from "vitest";
import { parseHeader, parseLyrics } from "./parse";

function ids() {
  let n = 0;
  return () => `id${++n}`;
}

/** El orden como nombres de tipo, para leerlo fácil. */
function shape(text: string) {
  const { sections, order } = parseLyrics(text, ids());
  const kindOf = new Map(sections.map((s) => [s.id, s.kind]));
  return { sections, order: order.map((e) => kindOf.get(e.section)), sectionOf: (i: number) => order[i].section };
}

describe("encabezados", () => {
  it("reconoce nombres en español, inglés e italiano, con número, corchetes, dos puntos y repeticiones", () => {
    expect(parseHeader("Coro:")).toEqual({ kind: "chorus", number: 0, times: 1 });
    expect(parseHeader("[Estrofa 2]")).toEqual({ kind: "verse", number: 2, times: 1 });
    expect(parseHeader("(Pre-Coro)")).toEqual({ kind: "preChorus", number: 0, times: 1 });
    expect(parseHeader("Chorus x2")).toEqual({ kind: "chorus", number: 0, times: 2 });
    expect(parseHeader("Ritornello")).toEqual({ kind: "chorus", number: 0, times: 1 });
    expect(parseHeader("PUENTE")).toEqual({ kind: "bridge", number: 0, times: 1 });
    expect(parseHeader("Final")).toEqual({ kind: "outro", number: 0, times: 1 });
  });

  it("no confunde una línea de la letra con un encabezado", () => {
    expect(parseHeader("Coro de ángeles que cantan")).toBeNull();
    expect(parseHeader("Santo, santo, santo")).toBeNull();
  });
});

describe("letra con encabezados", () => {
  const text = `Intro

Estrofa 1
Sublime gracia del Señor
que a un pecador salvó

Coro:
Gloria a Dios
por siempre

Amén, amén

Estrofa 2
En los peligros y aflicción

Coro

Puente
Tú eres fiel

Coro x2
Final`;

  it("el coro se escribe una vez y el orden lo repite", () => {
    const { sections, order, sectionOf } = shape(text);
    expect(order).toEqual(["intro", "verse", "chorus", "verse", "chorus", "bridge", "chorus", "chorus", "outro"]);
    expect(sections.filter((s) => s.kind === "chorus")).toHaveLength(1);
    expect(sectionOf(2)).toBe(sectionOf(4));
    expect(sectionOf(4)).toBe(sectionOf(7));
  });

  it("dentro de una sección, una línea en blanco separa diapositivas", () => {
    const { sections } = shape(text);
    const chorus = sections.find((s) => s.kind === "chorus")!;
    expect(chorus.slides.map((s) => s.lines)).toEqual([["Gloria a Dios", "por siempre"], ["Amén, amén"]]);
  });

  it("intro y final sin texto son secciones sin diapositivas", () => {
    const { sections } = shape(text);
    expect(sections.find((s) => s.kind === "intro")!.slides).toEqual([]);
    expect(sections.find((s) => s.kind === "outro")!.slides).toEqual([]);
  });

  it("estrofas distintas son secciones distintas", () => {
    const { sections } = shape(text);
    expect(sections.filter((s) => s.kind === "verse")).toHaveLength(2);
  });

  it("los asteriscos también separan diapositivas", () => {
    const { sections } = shape("Coro\nUno\n**\nDos");
    expect(sections[0].slides.map((s) => s.lines)).toEqual([["Uno"], ["Dos"]]);
  });

  it("lo que va antes del primer encabezado es una estrofa", () => {
    const { order } = shape("Cuán grande es Él\n\nCoro\nMi corazón");
    expect(order).toEqual(["verse", "chorus"]);
  });
});

describe("letra sin encabezados", () => {
  it("los bloques repetidos son el coro y los demás, estrofas", () => {
    const { sections, order, sectionOf } = shape("A\nB\n\nGloria\nAleluya\n\nC\n\ngloria\naleluya");
    expect(order).toEqual(["verse", "chorus", "verse", "chorus"]);
    expect(sectionOf(1)).toBe(sectionOf(3));
    expect(sections).toHaveLength(3);
  });

  it("texto vacío no arma nada", () => {
    expect(parseLyrics("  \n\n ", ids())).toEqual({ sections: [], order: [] });
  });
});

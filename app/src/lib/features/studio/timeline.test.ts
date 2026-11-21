import { describe, expect, it } from "vitest";
import type { MediaSong } from "$lib/platform/types";
import { emptySong, expand, sectionText, studioContent, withText } from "./song";
import { clearStart, formatTime, markStart, nextStart, slideAt, slideStarts } from "./timeline";

function song(): MediaSong {
  return {
    ...emptySong("s"),
    title: "Sublime gracia",
    mode: "track",
    audio: { path: "/pistas/a.mp3", name: "a.mp3", durationMs: 100_000 },
    sections: [
      { id: "intro", kind: "intro", label: "", slides: [] },
      { id: "v1", kind: "verse", label: "", slides: [{ id: "v1a", lines: ["Sublime gracia"] }] },
      { id: "coro", kind: "chorus", label: "", slides: [{ id: "c1", lines: ["Gloria"] }, { id: "c2", lines: ["Amén"] }] },
    ],
    order: [
      { id: "e0", section: "intro", timing: { startMs: 0, slides: [] } },
      { id: "e1", section: "v1", timing: { startMs: 8_000, slides: [{ slide: "v1a", startMs: 8_000 }] } },
      { id: "e2", section: "coro", timing: { startMs: 20_000, slides: [{ slide: "c1", startMs: 20_000 }, { slide: "c2", startMs: 30_000 }] } },
      { id: "e3", section: "coro", timing: { startMs: 60_000, slides: [{ slide: "c1", startMs: 60_000 }, { slide: "c2", startMs: 70_000 }] } },
    ],
  };
}

describe("desplegar", () => {
  it("el coro aparece cada vez; la instrumental es una diapositiva vacía", () => {
    const content = studioContent(song(), "Sublime gracia");
    expect(content).toMatchObject({ kind: "text", slides: ["", "Sublime gracia", "Gloria", "Amén", "Gloria", "Amén"], index: 0, studio: "s" });
  });

  it("una aparición de una sección que no existe se salta", () => {
    const s = song();
    s.order.push({ id: "e9", section: "nada" });
    expect(expand(s)).toHaveLength(6);
  });

  it("al editar el texto, cada diapositiva conserva su id y con él sus tiempos", () => {
    const coro = song().sections[2];
    expect(sectionText(coro)).toBe("Gloria\n\nAmén");
    const edited = withText(coro, "Gloria a Dios\n\n Amén \n\nAleluya", () => "nueva");
    expect(edited.slides).toEqual([
      { id: "c1", lines: ["Gloria a Dios"] },
      { id: "c2", lines: ["Amén"] },
      { id: "nueva", lines: ["Aleluya"] },
    ]);
  });
});

describe("tiempo → diapositiva, [inicio, fin)", () => {
  const starts = slideStarts(expand(song()));

  it("comienzo de cada diapositiva desplegada", () => {
    expect(starts).toEqual([0, 8_000, 20_000, 30_000, 60_000, 70_000]);
  });

  it("la que suena en cada instante, con el comienzo incluido y el final excluido", () => {
    expect(slideAt(starts, 0)).toBe(0);
    expect(slideAt(starts, 7_999)).toBe(0);
    expect(slideAt(starts, 8_000)).toBe(1);
    expect(slideAt(starts, 29_999)).toBe(2);
    expect(slideAt(starts, 30_000)).toBe(3);
    // El segundo coro es otra aparición: otras posiciones.
    expect(slideAt(starts, 65_000)).toBe(4);
    expect(slideAt(starts, 99_999)).toBe(5);
  });

  it("sin tiempo no se elige; antes de la primera, ninguna", () => {
    expect(slideAt([null, 5_000], 1_000)).toBeNull();
    expect(slideAt([null, 5_000, null], 9_000)).toBe(1);
    expect(nextStart(starts, 20_000)).toBe(30_000);
    expect(nextStart(starts, 70_000)).toBeNull();
  });
});

describe("marcar tiempos", () => {
  it("marcar el primer coro mueve su aparición; el segundo coro no cambia", () => {
    const s = song();
    const ex = expand(s);
    const moved = markStart(s, ex, 2, 21_000);
    expect(moved.order[2].timing).toEqual({ startMs: 21_000, slides: [{ slide: "c1", startMs: 21_000 }, { slide: "c2", startMs: 30_000 }] });
    expect(moved.order[3]).toBe(s.order[3]);
    // La original no se toca.
    expect(s.order[2].timing!.startMs).toBe(20_000);
  });

  it("marcar una aparición sin tiempo la crea; quitar el último tiempo la deja sin tiempo", () => {
    const s = song();
    s.order[3].timing = null;
    const ex = expand(s);
    const second = markStart(s, ex, 5, 71_000);
    expect(second.order[3].timing).toEqual({ startMs: 71_000, slides: [{ slide: "c2", startMs: 71_000 }] });
    const both = markStart(second, expand(second), 4, 60_000);
    expect(both.order[3].timing).toEqual({ startMs: 60_000, slides: [{ slide: "c1", startMs: 60_000 }, { slide: "c2", startMs: 71_000 }] });
    const cleared = clearStart(clearStart(both, expand(both), 4), expand(both), 5);
    expect(cleared.order[3].timing).toBeNull();
  });

  it("al mover una diapositiva, sus líneas y palabras se mueven con ella", () => {
    const s = song();
    s.order[1].timing!.slides[0].lines = [{ startMs: 8_000, endMs: 9_000, words: [8_000, 8_500] }];
    const moved = markStart(s, expand(s), 1, 8_200);
    expect(moved.order[1].timing!.slides[0].lines).toEqual([{ startMs: 8_200, endMs: 9_200, words: [8_200, 8_700] }]);
  });

  it("instrumental: marca el comienzo de su aparición", () => {
    const s = song();
    expect(markStart(s, expand(s), 0, 500).order[0].timing).toEqual({ startMs: 500, slides: [] });
  });

  it("formato de tiempo", () => {
    expect(formatTime(0)).toBe("0:00.0");
    expect(formatTime(83_456)).toBe("1:23.4");
  });
});

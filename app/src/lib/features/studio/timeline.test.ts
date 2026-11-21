import { describe, expect, it } from "vitest";
import type { MediaSong } from "$lib/platform/types";
import { dropTroubledTimings, emptySong, expand, hasTroubledTimings, sectionText, studioContent, withText } from "./song";
import { activeWord, clearStart, clearWords, formatTime, markInSequence, markStart, markWord, nextStart, slideAt, slideRanges, slideStarts, slideWords, step, wordsInSlide } from "./timeline";

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

describe("tramos en la grabación y avance (orden propio)", () => {
  // Desplegada: 0 intro, 1 estrofa, 2-3 coro, 4-5 coro otra vez. Pista de 100 s.
  const ranges = (s: MediaSong) => slideRanges(expand(s), s.audio!.durationMs);

  it("cada diapositiva dura hasta el siguiente comienzo en la grabación", () => {
    expect(ranges(song())).toEqual([
      { start: 0, end: 8_000, own: true },
      { start: 8_000, end: 20_000, own: true },
      { start: 20_000, end: 30_000, own: true },
      { start: 30_000, end: 60_000, own: true },
      { start: 60_000, end: 70_000, own: true },
      { start: 70_000, end: 100_000, own: true },
    ]);
  });

  it("una estrofa repetida al final sin tiempo propio usa el de la primera vez", () => {
    const s = song();
    s.order.push({ id: "e4", section: "v1" });
    expect(ranges(s)[6]).toEqual({ start: 8_000, end: 20_000, own: false });
    // Una sección que nunca suena no tiene tramo.
    s.sections.push({ id: "puente", kind: "bridge", label: "", slides: [{ id: "p1", lines: ["Puente"] }] });
    s.order.push({ id: "e5", section: "puente" });
    expect(ranges(s)[7]).toBeNull();
  });

  it("dentro de la diapositiva, nada; al terminarla, la siguiente donde sigue la grabación", () => {
    const s = song();
    const r = ranges(s);
    const st = slideStarts(expand(s));
    expect(step(r, st, 1, 15_000)).toEqual({ kind: "stay" });
    expect(step(r, st, 1, 20_000)).toEqual({ kind: "show", index: 2 });
    // La última: al final de la pista no hay siguiente.
    expect(step(r, st, 5, 100_000)).toEqual({ kind: "stay" });
  });

  it("orden propio: al terminar una diapositiva, la siguiente del orden salta a donde suena en la grabación", () => {
    // Pista de 200 s: intro 0, estrofa 8, coro 20/30, coro 60/70, y una intro final a los 90 s.
    // Orden propio: después del 2.º coro, otra vez la estrofa 1 (sin tiempo propio: usa el de los 8 s) y la intro final.
    const s = song();
    s.audio = { ...s.audio!, durationMs: 200_000 };
    s.order = [...s.order, { id: "otra", section: "v1" }, { id: "fin", section: "intro", timing: { startMs: 90_000, slides: [] } }];
    const r = slideRanges(expand(s), 200_000);
    const st = slideStarts(expand(s));
    // 0 intro, 1 estrofa, 2-3 coro, 4-5 coro, 6 estrofa repetida, 7 intro final.
    expect(r[5]).toEqual({ start: 70_000, end: 90_000, own: true });
    expect(r[6]).toEqual({ start: 8_000, end: 20_000, own: false });
    expect(step(r, st, 5, 90_000)).toEqual({ kind: "show", index: 6, seek: 8_000 });
    // Sonando la estrofa repetida (los 8-20 s de la grabación) no vuelve a la primera vez.
    expect(step(r, st, 6, 12_000)).toEqual({ kind: "stay" });
    // Al terminarla, la intro final, que suena a los 90 s: otro salto.
    expect(step(r, st, 6, 20_000)).toEqual({ kind: "show", index: 7, seek: 90_000 });
  });

  it("si se movió la barra a otra parte, la diapositiva que suena allí", () => {
    const s = song();
    expect(step(ranges(s), slideStarts(expand(s)), 1, 65_000)).toEqual({ kind: "show", index: 4 });
    expect(step(ranges(s), slideStarts(expand(s)), 4, 2_000)).toEqual({ kind: "show", index: 0 });
  });
});

describe("sincronizar sin dejar errores", () => {
  it("volver a marcar una diapositiva que se pasó quita las siguientes que quedarían antes", () => {
    // Desplegada: 0 intro (0), 1 estrofa (8 s), 2-3 coro (20/30), 4-5 coro (60/70).
    // Se pasó la estrofa: se vuelve a marcar a los 25 s, después del comienzo del coro (20 s).
    const s = song();
    const fixed = markInSequence(s, expand(s), 1, 25_000);
    expect(slideStarts(expand(fixed))).toEqual([0, 25_000, null, 30_000, 60_000, 70_000]);
    // Y la siguiente marca (el coro, a los 27 s) sigue en orden.
    const again = markInSequence(fixed, expand(fixed), 2, 27_000);
    expect(slideStarts(expand(again))).toEqual([0, 25_000, 27_000, 30_000, 60_000, 70_000]);
  });

  it("marcar en orden no quita nada", () => {
    const s = song();
    expect(markInSequence(s, expand(s), 2, 21_000).order[3]).toBe(s.order[3]);
    expect(slideStarts(expand(markInSequence(s, expand(s), 2, 21_000)))).toEqual([0, 8_000, 21_000, 30_000, 60_000, 70_000]);
  });

  it("quitar solo los tiempos con errores", () => {
    const s = song();
    const issues = [
      { code: "slideOutsideEntry" as const, severity: "error" as const, at: { entry: "e2", section: "coro", slide: "c2" } },
      { code: "timeOutOfTrack" as const, severity: "error" as const, at: { entry: "e3" } },
      { code: "untimedEntry" as const, severity: "warning" as const, at: { entry: "e1" } },
    ];
    expect(hasTroubledTimings(issues)).toBe(true);
    const clean = dropTroubledTimings(s, issues);
    expect(clean.order[2].timing!.slides.map((c) => c.slide)).toEqual(["c1"]);
    expect(clean.order[3].timing).toBeNull();
    // Los avisos no se tocan.
    expect(clean.order[1]).toBe(s.order[1]);
    expect(hasTroubledTimings([{ code: "missingTitle", severity: "error", at: {} }])).toBe(false);
  });
});

describe("palabra por palabra", () => {
  // La estrofa tiene una línea de dos palabras: «Sublime gracia». El coro: «Gloria» y «Amén».
  it("marcar cada palabra: la primera marca también la diapositiva; las palabras quedan en su línea", () => {
    let s = song();
    s.sections[1].slides[0].lines = ["Sublime gracia", "del Señor"];
    expect(wordsInSlide(s.sections[1].slides[0].lines)).toBe(4);
    for (const [w, ms] of [[0, 8_100], [1, 8_600], [2, 9_400], [3, 9_900]] as const) s = markWord(s, expand(s), 1, w, ms);
    const cue = s.order[1].timing!.slides[0];
    expect(cue.startMs).toBe(8_100);
    expect(cue.lines).toEqual([
      { startMs: 8_100, words: [8_100, 8_600] },
      { startMs: 9_400, words: [9_400, 9_900] },
    ]);
    expect(slideWords(expand(s))[1]).toEqual([8_100, 8_600, 9_400, 9_900]);
    // Volver a marcar la segunda palabra quita las que venían después.
    s = markWord(s, expand(s), 1, 1, 8_700);
    expect(slideWords(expand(s))[1]).toEqual([8_100, 8_700, null, null]);
    // Borrar las palabras deja la diapositiva marcada.
    s = clearWords(s, expand(s), 1);
    expect(s.order[1].timing!.slides[0]).toEqual({ slide: "v1a", startMs: 8_100 });
    expect(slideWords(expand(s))[1]).toBeNull();
  });

  it("la palabra que suena es la última que empezó", () => {
    const words = [1_000, 1_500, null, 2_500];
    expect(activeWord(words, 999)).toBeNull();
    expect(activeWord(words, 1_000)).toBe(0);
    expect(activeWord(words, 2_000)).toBe(1);
    expect(activeWord(words, 3_000)).toBe(3);
    expect(activeWord(null, 3_000)).toBeNull();
  });

  it("un coro repetido sin palabras propias usa las de la otra vez", () => {
    let s = song();
    s = markWord(s, expand(s), 2, 0, 20_000);
    expect(slideWords(expand(s))[2]).toEqual([20_000]);
    // El segundo coro (índice 4) no tiene palabras propias: usa las del primero (la pista salta allí).
    expect(slideWords(expand(s))[4]).toEqual([20_000]);
  });
});

describe("repetir una sección", () => {
  it("al terminar la última diapositiva de la sección, vuelve a su primera", () => {
    const s = song();
    const r = slideRanges(expand(s), 100_000);
    const st = slideStarts(expand(s));
    // El primer coro: diapositivas 2 (20-30 s) y 3 (30-60 s).
    const loop = { first: 2, last: 3 };
    expect(step(r, st, 3, 45_000, loop)).toEqual({ kind: "stay" });
    expect(step(r, st, 3, 59_950, loop)).toEqual({ kind: "show", index: 2, seek: 20_000 });
    // Sin repetir, sigue con la siguiente.
    expect(step(r, st, 3, 60_000)).toEqual({ kind: "show", index: 4 });
    // En la primera diapositiva de la sección no se repite todavía.
    expect(step(r, st, 2, 29_990, loop)).toEqual({ kind: "stay" });
  });
});

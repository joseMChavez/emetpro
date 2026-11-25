import { describe, expect, it } from "vitest";
import heard from "./fixtures/noche-de-paz.json";
import { alignSlides, alignWords, applySync, heardWords, LEAD_MS, similarity } from "./sync";
import { parseLyrics } from "./parse";
import { expand } from "./song";
import type { MediaSong, WhisperSegment } from "$lib/platform/types";

// «Noche de paz» (Federico Fliedner, dominio público), como la cantan en la grabación: dos líneas por diapositiva.
const LYRICS = `Estrofa 1:
Noche de paz, noche de amor
Todo duerme en derredor

Entre los astros que esparcen su luz
Bella anunciando al niñito Jesús

Brilla la estrella de paz
Brilla la estrella de paz

Estrofa 2:
Noche de paz, noche de amor
Todo duerme en derredor

Sólo velan en la oscuridad
Los pastores que en el campo están

Y la estrella de Belén
Y la estrella de Belén

Estrofa 3:
Noche de paz, noche de amor
Todo duerme en derredor

Sobre el santo niño Jesús
Una estrella esparce su luz

Brilla sobre el Rey
Brilla sobre el Rey

Estrofa 4:
Noche de paz, noche de amor
Todo duerme en derredor

Fieles velando allí en Belén
Los pastores, la madre también

Y la estrella de paz
Y la estrella de paz`;

/** Dónde se empieza a cantar cada diapositiva, medido en la voz sola (null: sin pausa antes, no se mide). */
const TRUTH = [6100, 20960, null, 49880, 64200, 78360, 92350, 106340, null, 134260, 148160, 162190];

function song(): MediaSong {
  let n = 0;
  const { sections, order } = parseLyrics(LYRICS, () => `id${n++}`);
  return { id: "s", title: "Noche de paz", artist: "", mode: "track", audio: { path: "/p.mp3", name: "p.mp3", durationMs: 186_000 }, sections, order, rights: { composition: "", lyrics: "", recording: "", license: "", notes: "" }, reference: "", background: null };
}

function errors(starts: (number | null)[]): number[] {
  return TRUTH.flatMap((t, k) => (t === null || starts[k] === null ? [] : [starts[k]! + LEAD_MS - t]));
}

describe("sincronización automática", () => {
  it("parecido de palabras: las cortas tienen que ser iguales", () => {
    expect(similarity("derredor", "redor")).toBeGreaterThan(0.5);
    expect(similarity("de", "en")).toBe(0);
    expect(similarity("paz", "paz")).toBe(1);
  });

  it("alinea en orden, saltando lo que sobra y lo que falta", () => {
    expect(alignWords(["noche", "de", "paz", "noche", "de", "amor"], ["performed", "noche", "de", "pas", "de", "amor"])).toEqual([1, 2, 3, -1, 4, 5]);
  });

  for (const [name, data, maxError] of [
    ["con la voz separada y sus arranques", heard.voz, 400],
    ["con la mezcla (sin separador)", heard.mezcla, 1400],
  ] as const) {
    it(`pone cada diapositiva donde se canta, ${name}`, () => {
      const expanded = expand(song());
      const result = alignSlides(expanded, heardWords(data.segments as WhisperSegment[]), data.onsets);
      // Lo que importa: las que no quedan «para revisar» están bien (las dudosas las corrige quien prepara).
      const sure = result.starts.map((t, k) => (result.doubtful[k] ? null : t));
      const errs = errors(sure);
      console.log(name, "· errores de las seguras (ms):", errs.join(" "), "· para revisar", result.doubtful.filter(Boolean).length, "de", result.total);
      expect(result.synced).toBe(result.total);
      expect(errs.length).toBeGreaterThanOrEqual(6);
      expect(Math.max(...errs.map(Math.abs))).toBeLessThanOrEqual(maxError);
      // En orden y sin empezar antes de tiempo más de lo previsto.
      const starts = result.starts.filter((t): t is number => t !== null);
      expect(starts).toEqual([...starts].sort((a, b) => a - b));
    });
  }

  it("las marcas a mano se quedan; las automáticas se pueden rehacer", () => {
    const base = song();
    const expanded = expand(base);
    const result = alignSlides(expanded, heardWords(heard.voz.segments as WhisperSegment[]), heard.voz.onsets);
    // A mano: la estrofa 2 empieza en 50 s.
    const manual: MediaSong = { ...base, order: base.order.map((e, i) => (i === 1 ? { ...e, timing: { startMs: 50_000, slides: [{ slide: base.sections[1].slides[0].id, startMs: 50_000 }] } } : e)) };
    const synced = applySync(manual, expand(manual), result.starts);
    const cue = synced.order[1].timing!.slides.find((c) => c.slide === base.sections[1].slides[0].id)!;
    expect(cue).toEqual({ slide: base.sections[1].slides[0].id, startMs: 50_000 });
    expect(synced.order[1].timing!.slides.filter((c) => c.auto)).toHaveLength(2);
    expect(synced.order[0].timing!.slides.every((c) => c.auto)).toBe(true);
    // Volver a sincronizar reemplaza las automáticas, sin duplicarlas.
    const again = applySync(synced, expand(synced), result.starts);
    expect(again.order[0].timing!.slides).toHaveLength(3);
  });
});

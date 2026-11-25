import { describe, expect, it } from "vitest";
import { heardLines, lyricsDraft, normalize, resample, sectionStarts } from "./whisper";
import { parseLyrics } from "./parse";
import type { WhisperSegment } from "$lib/platform/types";

const seg = (from: string, to: string, text: string, noSpeech = 0): WhisperSegment => {
  const ms = (t: string) => t.split(":").reduce((a, n) => a * 60 + Number(n), 0) * 1000;
  return { startMs: ms(from), endMs: ms(to), text, noSpeech };
};

// Lo que dio Whisper «small» con «Noche de paz» (LibriVox, dominio público), voz con poco acompañamiento.
const nocheDePaz: WhisperSegment[] = [
  seg("0:00", "0:05", ' "Noche de Paz", performed for Librevox.org by Karen Savage.'),
  seg("0:05", "0:13", " Noche de paz, noche de amor"),
  seg("0:13", "0:20", " Todo vuelve en derredor"),
  seg("0:20", "0:28", " Entre los astros que esparcen su luz"),
  seg("0:28", "0:35", " Bella anunciando al lignito Jesús"),
  seg("0:35", "0:49", " Brilla la estrella de paz"),
  seg("0:49", "0:57", " Noche de paz, noche de amor"),
  seg("0:57", "1:04", " Todo duerme en derredor"),
  seg("1:04", "1:11", " Solo ve la nena oscuridad"),
  seg("1:11", "1:18", " Los pastores que en el campo están"),
  seg("1:18", "1:31", " Y la estrella de Belén"),
  seg("1:31", "1:39", " Noche de paz, noche de amor"),
  seg("1:39", "1:46", " Todo duerme en derredor"),
  seg("1:46", "1:53", " Sobre el Santo Niño Jesús"),
  seg("1:53", "2:00", " Una estrella de paz a su luz"),
  seg("2:00", "2:14", " Brilla sobre el Rey"),
];

// Con música fuerte: tramos parejos de 7 s y sin pausas; las estrofas solo se notan porque empiezan igual.
const conBanda: WhisperSegment[] = [
  seg("0:05", "0:13", " Noche de paz, Noche de amor,"),
  seg("0:13", "0:20", " Todo duerme en el error,"),
  seg("0:20", "0:28", " Entre los astros que estarsen sus,"),
  seg("0:28", "0:35", " Derunciando al límite de Jesús,"),
  seg("0:35", "0:42", " Grilla la estrella de paz,"),
  seg("0:42", "0:49", " Grilla la estrella de paz."),
  seg("0:49", "0:57", " Noche de paz, Noche de amor,"),
  seg("0:57", "1:04", " Todo duerme en el error,"),
  seg("1:04", "1:11", " Solo verán en la oscuridad,"),
  seg("1:11", "1:18", " Los pasores en la apuesta,"),
  seg("1:18", "1:32", " Grilla la estrella de paz."),
];

describe("letra sacada de la pista", () => {
  it("quita créditos, marcas de música y tramos sin voz; deja la letra limpia", () => {
    const { lines } = heardLines([...nocheDePaz, seg("2:14", "2:20", " [MUSIC]"), seg("2:20", "2:25", " Gracias.", 0.9)]);
    expect(lines[0].text).toBe("Noche de paz, noche de amor");
    expect(lines.map((l) => l.text)).not.toContain("[MUSIC]");
    expect(lines.some((l) => /librevox/i.test(l.text))).toBe(false);
    expect(lines.at(-1)!.text).toBe("Brilla sobre el Rey");
    // Sin puntuación al final ni comillas.
    expect(heardLines(conBanda).lines[0].text).toBe("Noche de paz, Noche de amor");
  });

  it("un bucle inventado en la música sola se queda en una línea y se cuenta", () => {
    const loop = Array.from({ length: 40 }, (_, i) => ({ startMs: i * 2000, endMs: i * 2000 + 2000, text: "It's time to get to the end of the day.", noSpeech: 0 }));
    const { lines, loops } = heardLines(loop);
    expect(lines).toHaveLength(1);
    expect(loops).toBe(1);
    // Un coro que repite una línea dos veces es letra, no un bucle.
    expect(heardLines(conBanda).lines.filter((l) => l.text === "Grilla la estrella de paz")).toHaveLength(3);
  });

  it("corta un tramo largo en sus frases, con su parte del tiempo", () => {
    const { lines } = heardLines([seg("0:18", "0:44", "Go Tell It on the Mountain, over the hills and everywhere. Go Tell It on the Mountain that Jesus Christ is born.")]);
    expect(lines.map((l) => l.text)).toEqual(["Go Tell It on the Mountain", "Over the hills and everywhere", "Go Tell It on the Mountain that Jesus Christ is born"]);
    expect(lines[0].startMs).toBe(18_000);
    expect(lines[1].startMs).toBeGreaterThan(lines[0].startMs);
    expect(lines[2].endMs).toBe(44_000);
  });

  it("encuentra las estrofas por el interludio o porque empiezan igual", () => {
    const clean = heardLines(nocheDePaz).lines;
    expect(sectionStarts(clean).map((i) => clean[i].text)).toEqual(["Noche de paz, noche de amor", "Noche de paz, noche de amor", "Noche de paz, noche de amor"]);
    const noisy = heardLines(conBanda).lines;
    expect(sectionStarts(noisy)).toEqual([0, 6]);
  });

  it("arma un borrador que el cuadro de pegar entiende: secciones, coro y dos líneas por diapositiva", () => {
    const lines = heardLines([...nocheDePaz.slice(1, 6), ...nocheDePaz.slice(1, 6)]).lines;
    const draft = lyricsDraft(lines, { verse: "Estrofa", chorus: "Coro" });
    expect(draft.startsWith("Coro:\nNoche de paz, noche de amor\nTodo vuelve en derredor\n\nEntre los astros")).toBe(true);
    // El coro repetido se escribe una vez.
    expect(draft.match(/Coro:/g)).toHaveLength(2);
    expect(draft.match(/Noche de paz/g)).toHaveLength(1);
    const parsed = parseLyrics(draft, (() => { let n = 0; return () => `id${n++}`; })());
    expect(parsed.sections).toHaveLength(1);
    expect(parsed.sections[0].kind).toBe("chorus");
    expect(parsed.sections[0].slides.map((s) => s.lines.length)).toEqual([2, 2, 1]);
    expect(parsed.order).toHaveLength(2);

    const verses = lyricsDraft(heardLines(nocheDePaz).lines, { verse: "Estrofa", chorus: "Coro" });
    const three = parseLyrics(verses, (() => { let n = 0; return () => `v${n++}`; })());
    expect(three.sections.map((s) => s.kind)).toEqual(["verse", "verse", "verse"]);
  });

  it("compara sin acentos, mayúsculas ni signos", () => {
    expect(normalize("¡Noche de PAZ, niño!")).toBe(normalize("noche de paz nino"));
  });

  it("remuestrea a 16 kHz cuando el navegador dio otra frecuencia", () => {
    const out = resample(new Float32Array([0, 1, 2, 3, 4, 5]), 24_000, 16_000);
    expect(out).toHaveLength(4);
    expect(Array.from(out)).toEqual([0, 1.5, 3, 4.5]);
  });
});

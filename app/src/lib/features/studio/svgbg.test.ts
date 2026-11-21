import { describe, expect, it } from "vitest";
import { DEFAULT_GENERATED, generateSvg, svgDataUrl, TEMPLATES, type Generated } from "./svgbg";

describe("fondos SVG generados", () => {
  it("cada plantilla da un SVG animado, sin scripts ni nada de fuera", () => {
    for (const template of TEMPLATES) {
      const svg = generateSvg({ ...DEFAULT_GENERATED, template });
      expect(svg.startsWith("<svg")).toBe(true);
      expect(svg.endsWith("</svg>")).toBe(true);
      expect(svg).toMatch(/<animate(Transform)?\b/);
      expect(svg).not.toMatch(/<script|on\w+=|href=|<image|<foreignObject/i);
      expect(svg.length).toBeLessThan(64 * 1024);
    }
  });

  it("misma configuración, mismo SVG; otra semilla, otro dibujo", () => {
    expect(generateSvg(DEFAULT_GENERATED)).toBe(generateSvg({ ...DEFAULT_GENERATED }));
    expect(generateSvg({ ...DEFAULT_GENERATED, seed: 8 })).not.toBe(generateSvg(DEFAULT_GENERATED));
  });

  it("usa los colores elegidos; lo que no es un color #rrggbb no entra en el SVG", () => {
    const svg = generateSvg({ ...DEFAULT_GENERATED, colors: ["#FF0000", '"/><script>alert(1)</script>', "#00ff00"] as Generated["colors"] });
    expect(svg).toContain("#ff0000");
    expect(svg).toContain("#00ff00");
    expect(svg).not.toContain("script");
    expect(svg).toContain(DEFAULT_GENERATED.colors[1]);
  });

  it("la velocidad cambia la duración", () => {
    const dur = (svg: string) => Number(/dur="([\d.]+)s"/.exec(svg)![1]);
    const slow = generateSvg({ ...DEFAULT_GENERATED, template: "gradient", speed: "slow" });
    const fast = generateSvg({ ...DEFAULT_GENERATED, template: "gradient", speed: "fast" });
    expect(dur(slow)).toBeGreaterThan(dur(fast));
  });

  it("vista previa como data URL", () => {
    expect(svgDataUrl("<svg/>")).toBe("data:image/svg+xml;charset=utf-8,%3Csvg%2F%3E");
  });
});

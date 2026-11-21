// Fondos animados generados (D-029): la app escribe un SVG con animaciones SMIL a partir de una plantilla, unos
// colores, la velocidad y la intensidad. Se guarda como archivo y se usa como cualquier otro fondo (<img>).
// Solo degradados y movimiento: nada de filtros (el desenfoque es caro en los equipos de proyección modestos).
// Puro, para probarlo con vitest.

export type Template = "aurora" | "galaxy" | "lights" | "waves" | "particles" | "gradient";
export type Speed = "slow" | "medium" | "fast";

export const TEMPLATES: Template[] = ["aurora", "galaxy", "lights", "waves", "particles", "gradient"];

export interface Generated {
  template: Template;
  /** Tres colores #rrggbb. */
  colors: [string, string, string];
  /** Color de fondo, detrás de todo (oscuro, para que se lea la letra). */
  base: string;
  speed: Speed;
  /** 0,2 a 1: cuánto se ven las formas. */
  intensity: number;
  /** Misma semilla, mismo dibujo. */
  seed: number;
}

export const DEFAULT_GENERATED: Generated = {
  template: "aurora",
  colors: ["#3b5bdb", "#9c36b5", "#0ca678"],
  base: "#05070d",
  speed: "slow",
  intensity: 0.7,
  seed: 7,
};

const W = 1920;
const H = 1080;
const SPEED: Record<Speed, number> = { slow: 1.6, medium: 1, fast: 0.6 };

/** Solo colores #rrggbb: lo que se escribe dentro del SVG no puede ser otra cosa. */
function color(value: string, fallback: string): string {
  return /^#[0-9a-f]{6}$/i.test(value) ? value.toLowerCase() : fallback;
}

/** Números pseudoaleatorios con semilla (mulberry32): el mismo fondo cada vez. */
function random(seed: number): () => number {
  let a = seed >>> 0 || 1;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const n = (x: number) => Math.round(x * 10) / 10;

/** Un degradado radial de un color a transparente: una mancha de luz de bordes suaves. */
function glow(id: string, c: string, opacity: number): string {
  return `<radialGradient id="${id}"><stop offset="0" stop-color="${c}" stop-opacity="${n(opacity)}"/><stop offset="1" stop-color="${c}" stop-opacity="0"/></radialGradient>`;
}

function aurora(g: Generated, rnd: () => number, k: number, colors: string[]): string {
  const defs = colors.map((c, i) => glow(`a${i}`, c, g.intensity)).join("");
  const blobs = colors
    .map((_, i) => {
      const [x1, y1, x2, y2] = [rnd() * W, rnd() * H * 0.7, rnd() * W, rnd() * H * 0.7];
      const rx = 700 + rnd() * 500;
      const ry = 380 + rnd() * 260;
      const dur = n((22 + rnd() * 14) * k);
      return `<ellipse cx="0" cy="0" rx="${n(rx)}" ry="${n(ry)}" fill="url(#a${i})"><animateTransform attributeName="transform" type="translate" values="${n(x1)} ${n(y1)};${n(x2)} ${n(y2)};${n(x1)} ${n(y1)}" dur="${dur}s" repeatCount="indefinite" calcMode="spline" keySplines="0.45 0 0.55 1;0.45 0 0.55 1"/></ellipse>`;
    })
    .join("");
  return `<defs>${defs}</defs>${blobs}`;
}

/**
 * Una galaxia en espiral vista de lado que gira despacio: brazos de polvo de colores y estrellas, un núcleo que late,
 * una nebulosa al fondo y estrellas lejanas que titilan. El giro es un solo transform sobre el grupo (barato); la
 * inclinación (achatar el disco) hace que los brazos de delante y de atrás se muevan como en una galaxia de verdad.
 * La semilla cambia el dibujo y si tiene dos o tres brazos.
 */
function galaxy(g: Generated, rnd: () => number, k: number, colors: string[]): string {
  const [c1, c2, c3] = colors;
  const defs =
    glow("gc", "#ffffff", 1) +
    glow("gk", c3, Math.min(1, g.intensity * 1.1)) +
    colors.map((c, i) => glow(`ga${i}`, c, g.intensity * 0.5)).join("") +
    glow("gn", c2, g.intensity * 0.3) +
    glow("gm", c1, g.intensity * 0.25);

  // Cielo: estrellas lejanas, una de cada cuatro titila.
  let sky = "";
  for (let i = 0; i < 90; i++) {
    const op = 0.25 + rnd() * 0.6;
    const r = 0.6 + rnd() * 1.5;
    const twinkle =
      i % 4 === 0
        ? `<animate attributeName="opacity" values="${n(op)};${n(op * 0.15)};${n(op)}" dur="${n(2.5 + rnd() * 5)}s" begin="-${n(rnd() * 6)}s" repeatCount="indefinite"/>`
        : "";
    sky += `<circle cx="${n(rnd() * W)}" cy="${n(rnd() * H)}" r="${n(r)}" fill="#ffffff" opacity="${n(op)}">${twinkle}</circle>`;
  }

  // Nebulosas al fondo, que se desplazan muy despacio.
  const nebula = ["gn", "gm"]
    .map((id) => {
      const [x1, y1] = [rnd() * W, rnd() * H];
      const [x2, y2] = [x1 + (rnd() - 0.5) * 300, y1 + (rnd() - 0.5) * 200];
      return `<ellipse cx="0" cy="0" rx="${n(650 + rnd() * 300)}" ry="${n(360 + rnd() * 200)}" fill="url(#${id})"><animateTransform attributeName="transform" type="translate" values="${n(x1)} ${n(y1)};${n(x2)} ${n(y2)};${n(x1)} ${n(y1)}" dur="${n(60 * k)}s" repeatCount="indefinite" calcMode="spline" keySplines="0.45 0 0.55 1;0.45 0 0.55 1"/></ellipse>`;
    })
    .join("");

  const arms = rnd() < 0.5 ? 2 : 3;
  const R = 600;
  const wind = 4.4;
  let dust = "";
  let stars = "";
  for (let a = 0; a < arms; a++) {
    const base = (a * 2 * Math.PI) / arms;
    // Polvo de colores a lo largo del brazo: manchas de luz cada vez más grandes y tenues hacia afuera.
    for (let j = 0; j < 28; j++) {
      const t = (j + rnd() * 0.5) / 28;
      const r = 60 + t * R;
      const th = base + t * wind + (rnd() - 0.5) * 0.12;
      const size = 50 + t * 80;
      dust += `<ellipse cx="${n(r * Math.cos(th))}" cy="${n(r * Math.sin(th))}" rx="${n(size * 1.4)}" ry="${n(size)}" fill="url(#ga${(a + (j > 8 ? 1 : 0)) % 3})" opacity="${n(0.75 - t * 0.4)}"/>`;
    }
    // Estrellas del brazo: más juntas cerca del centro, algo dispersas a los lados.
    for (let j = 0; j < 110; j++) {
      const t = Math.pow(rnd(), 0.9);
      const r = 35 + t * R + (rnd() - 0.5) * 55;
      const th = base + t * wind + (rnd() - 0.5) * 0.4;
      const fill = rnd() < 0.7 ? "#ffffff" : colors[Math.floor(rnd() * 3)];
      const op = 0.55 + rnd() * 0.45;
      // Algunas, más grandes, brillan y se apagan mientras gira.
      const bright = j % 12 === 0;
      const size = bright ? 3.2 + rnd() * 1.8 : 1.4 + rnd() * 2.6 * (1 - t * 0.4);
      const shine = bright
        ? `<animate attributeName="opacity" values="${n(op)};0.25;${n(op)}" dur="${n(2 + rnd() * 3)}s" begin="-${n(rnd() * 4)}s" repeatCount="indefinite"/>`
        : "";
      stars += `<circle cx="${n(r * Math.cos(th))}" cy="${n(r * Math.sin(th))}" r="${n(size)}" fill="${fill}" opacity="${n(op)}">${shine}</circle>`;
    }
  }
  // Halo: estrellas sueltas por todo el disco.
  for (let i = 0; i < 80; i++) {
    const r = Math.sqrt(rnd()) * R * 1.1;
    const th = rnd() * 2 * Math.PI;
    stars += `<circle cx="${n(r * Math.cos(th))}" cy="${n(r * Math.sin(th))}" r="${n(0.8 + rnd() * 1.4)}" fill="#ffffff" opacity="${n(0.3 + rnd() * 0.5)}"/>`;
  }

  const cx = W * (0.44 + rnd() * 0.12);
  const cy = H * (0.45 + rnd() * 0.1);
  const tilt = -14 - rnd() * 18;
  const spin = `<animateTransform attributeName="transform" type="rotate" from="0" to="-360" dur="${n(160 * k)}s" repeatCount="indefinite"/>`;
  // El núcleo late despacio.
  const core =
    `<circle r="240" fill="url(#gk)"/>` +
    `<circle r="105" fill="url(#gc)" opacity="0.85"><animate attributeName="opacity" values="0.7;1;0.7" dur="${n(7 * k)}s" repeatCount="indefinite"/></circle>`;
  return (
    `<defs>${defs}</defs>${nebula}${sky}` +
    `<g transform="translate(${n(cx)} ${n(cy)}) rotate(${n(tilt)}) scale(1 0.46)"><g>${spin}${dust}${stars}</g>${core}</g>`
  );
}

function lights(g: Generated, rnd: () => number, k: number, colors: string[]): string {
  const defs = colors.map((c, i) => glow(`l${i}`, c, 1)).join("");
  let out = "";
  for (let i = 0; i < 16; i++) {
    const r = 50 + rnd() * 110;
    const x = rnd() * W;
    const y = rnd() * H;
    const top = n(g.intensity * (0.35 + rnd() * 0.5));
    const dur = n((9 + rnd() * 10) * k);
    const drift = n((rnd() - 0.5) * 160);
    out += `<circle cx="${n(x)}" cy="${n(y)}" r="${n(r)}" fill="url(#l${i % 3})" opacity="0"><animate attributeName="opacity" values="0;${top};0" dur="${dur}s" begin="-${n(rnd() * dur)}s" repeatCount="indefinite"/><animateTransform attributeName="transform" type="translate" values="0 0;${drift} ${n(-drift / 2)};0 0" dur="${n(dur * 2)}s" repeatCount="indefinite"/></circle>`;
  }
  return `<defs>${defs}</defs>${out}`;
}

/** Una onda que se repite cada `W` px, de 2·W de ancho: al correrla W px vuelve a empezar sin que se note. */
function wavePath(y: number, amplitude: number): string {
  const half = W / 4;
  let d = `M0 ${n(y)} Q${n(half / 2)} ${n(y - amplitude)} ${n(half)} ${n(y)}`;
  for (let x = half * 2; x <= W * 2; x += half) d += ` T${n(x)} ${n(y)}`;
  return `${d} L${W * 2} ${H} L0 ${H} Z`;
}

function waves(g: Generated, rnd: () => number, k: number, colors: string[]): string {
  return colors
    .map((c, i) => {
      const y = H * (0.55 + i * 0.12);
      const dur = n((18 + i * 7 + rnd() * 4) * k);
      return `<path d="${wavePath(y, 50 + rnd() * 60)}" fill="${c}" opacity="${n(g.intensity * (0.35 - i * 0.07))}"><animateTransform attributeName="transform" type="translate" values="0 0;-${W} 0" dur="${dur}s" repeatCount="indefinite"/></path>`;
    })
    .join("");
}

function particles(g: Generated, rnd: () => number, k: number, colors: string[]): string {
  let out = "";
  for (let i = 0; i < 42; i++) {
    const x = rnd() * W;
    const r = 2 + rnd() * 5;
    const dur = n((14 + rnd() * 16) * k);
    out += `<circle cx="${n(x)}" cy="${H + 20}" r="${n(r)}" fill="${colors[i % 3]}" opacity="${n(g.intensity * (0.4 + rnd() * 0.6))}"><animate attributeName="cy" values="${H + 20};-20" dur="${dur}s" begin="-${n(rnd() * dur)}s" repeatCount="indefinite"/></circle>`;
  }
  return out;
}

function gradient(g: Generated, _rnd: () => number, k: number, colors: string[]): string {
  const dur = n(40 * k);
  const stops = colors.map((c, i) => `<stop offset="${i / 2}" stop-color="${c}"/>`).join("");
  return `<defs><linearGradient id="g" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="${W}" y2="${H}">${stops}<animateTransform attributeName="gradientTransform" type="rotate" values="0 ${W / 2} ${H / 2};360 ${W / 2} ${H / 2}" dur="${dur}s" repeatCount="indefinite"/></linearGradient></defs><rect width="${W}" height="${H}" fill="url(#g)" opacity="${n(g.intensity)}"/>`;
}

const DRAW: Record<Template, (g: Generated, rnd: () => number, k: number, colors: string[]) => string> = { aurora, galaxy, lights, waves, particles, gradient };

export function generateSvg(input: Generated): string {
  const fallback = DEFAULT_GENERATED;
  const g: Generated = {
    ...input,
    template: TEMPLATES.includes(input.template) ? input.template : fallback.template,
    intensity: Math.max(0.2, Math.min(1, Number(input.intensity) || fallback.intensity)),
  };
  const colors = g.colors.map((c, i) => color(c, fallback.colors[i]));
  const base = color(g.base, fallback.base);
  const k = SPEED[g.speed] ?? 1;
  const body = DRAW[g.template](g, random(Math.floor(g.seed)), k, colors);
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${W} ${H}" preserveAspectRatio="xMidYMid slice"><rect width="${W}" height="${H}" fill="${base}"/>${body}</svg>`;
}

/** Para la vista previa del generador, sin guardar archivo. */
export function svgDataUrl(svg: string): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

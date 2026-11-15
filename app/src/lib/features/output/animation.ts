// Animaciones del texto y fondos en movimiento (D-024). Sin Tauri: lo usan la consola y la ventana de salida.
import type { OutputContent, OutputStyle, TextAnimation } from "$lib/platform/types";

export const TRANSITIONS: TextAnimation["transition"][] = ["none", "fade", "zoom"];
export const ENTRANCES: TextAnimation["entrance"][] = ["none", "lines", "words", "typewriter"];
export const AMBIENTS: TextAnimation["ambient"][] = ["none", "breathe", "float", "glow"];
export const BACKGROUND_MOTIONS: TextAnimation["background"][] = ["still", "slow"];

/** Lo de siempre más un fundido suave: igual que TextAnimation::default en output/domain.rs. */
export const DEFAULT_ANIMATION: TextAnimation = { transition: "fade", entrance: "none", ambient: "none", background: "still" };
/** Sin animación: el texto cambia al instante y el fondo no se mueve. */
export const NO_ANIMATION: TextAnimation = { transition: "none", entrance: "none", ambient: "none", background: "still" };

/** Lo que dicen los niveles de la cascada, del más general al más concreto. null = como el de arriba. */
export interface AnimationCascade {
  /** El plan y la sección: activar o desactivar. */
  plan?: boolean | null;
  section?: boolean | null;
  /** Una letra: su propia animación («sin animación» = NO_ANIMATION). */
  own?: TextAnimation | null;
}

/** La animación de un contenido según la cascada, o undefined si es la general (así cambiar el estilo se ve al momento). */
export function resolveAnimation(style: Pick<OutputStyle, "animate" | "animation">, cascade: AnimationCascade): TextAnimation | undefined {
  if (cascade.own) return { ...cascade.own };
  const enabled = cascade.section ?? cascade.plan ?? null;
  if (enabled === null || enabled === style.animate) return undefined;
  return enabled ? { ...style.animation } : { ...NO_ANIMATION };
}

/** La animación con la que se dibuja un contenido. */
export function animationOf(content: OutputContent, style: OutputStyle): TextAnimation {
  if ((content.kind === "text" || content.kind === "scripture") && content.animation) return content.animation;
  return style.animate === false ? NO_ANIMATION : { ...DEFAULT_ANIMATION, ...style.animation };
}

const VIDEO = /\.(mp4|m4v|mov|webm|mkv|ogv)$/i;

/** Un fondo que es un video (en bucle, sin sonido). */
export function isVideo(path: string): boolean {
  return VIDEO.test(path);
}

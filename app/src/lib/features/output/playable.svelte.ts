// Videos que el webview no reproduce (HEVC de iPhone…) se pueden convertir a MP4 una vez (media/video.rs).
// Aquí se recuerda, por ventana, qué archivo cargar para cada video: la copia si existe, si no el original.
// Los datos (planes, biblioteca, teléfono) siguen con la ruta del original.
import { SvelteMap } from "svelte/reactivity";
import { platform } from "$lib/platform";
import { isVideo } from "./animation";

const resolved = new SvelteMap<string, string>();
const asked = new Set<string>();
let listening = false;

async function refresh(path: string) {
  try {
    resolved.set(path, await platform.playableVideo(path));
  } catch {
    resolved.set(path, path);
  }
}

/** La ruta que hay que cargar; null mientras se pregunta (la primera vez de cada video). */
export function playablePath(path: string): string | null {
  if (!isVideo(path)) return path;
  const known = resolved.get(path);
  if (known !== undefined) return known;
  if (!asked.has(path)) {
    asked.add(path);
    void refresh(path);
    if (!listening) {
      listening = true;
      void platform.onVideoConverted((original) => void refresh(original));
    }
  }
  return null;
}

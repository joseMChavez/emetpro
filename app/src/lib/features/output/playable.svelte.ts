// Qué cargar para cada video, por ventana. Los videos van por un servidor HTTP local (core/media_server.rs) y no
// por el protocolo asset: en macOS, un video grande por el protocolo asset podía cerrar la app al buscar o silenciar.
// Si el webview no reproducía el original (HEVC de iPhone…) y se convirtió a MP4 (media/video.rs), se carga la copia.
// Los datos (planes, biblioteca, teléfono) siguen con la ruta del original.
import { SvelteMap } from "svelte/reactivity";
import { platform, type PlayableVideo } from "$lib/platform";
import { isVideo } from "./animation";

const resolved = new SvelteMap<string, PlayableVideo>();
const asked = new Set<string>();
let listening = false;

async function refresh(path: string) {
  try {
    resolved.set(path, await platform.playableVideo(path));
  } catch {
    resolved.set(path, { path, url: null, converted: false });
  }
}

function playable(path: string): PlayableVideo | null {
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

/** La URL que carga el webview; "" mientras se pregunta (la primera vez de cada video). Imágenes y PDF, directas. */
export function playableUrl(path: string): string {
  if (!isVideo(path)) return platform.mediaUrl(path);
  const video = playable(path);
  if (!video) return "";
  return video.url ?? platform.mediaUrl(video.path);
}

/**
 * Falló un video que venía del servidor local. Si el servidor no se puede alcanzar (bloqueado por el sistema),
 * ese video pasa al protocolo asset y devuelve true; si responde, el problema es el archivo (el códec) y
 * devuelve false: así un HEVC no vuelve al protocolo asset, que es justo lo que cerraba la app en macOS.
 */
export async function fallBackToAsset(path: string): Promise<boolean> {
  const video = resolved.get(path);
  if (!video?.url) return false;
  try {
    await fetch(video.url, { method: "HEAD", mode: "no-cors", cache: "no-store" });
    return false;
  } catch {
    resolved.set(path, { ...video, url: null });
    void platform.logError(`servidor local de medios inalcanzable; ${path} va por asset`);
    return true;
  }
}

/** Ya se carga la copia convertida: si tampoco se reproduce, no hay otra cosa que ofrecer. */
export function isConverted(path: string): boolean {
  return playable(path)?.converted ?? false;
}

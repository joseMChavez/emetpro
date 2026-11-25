// El separador para quitar la voz de una pista (htdemucs en ONNX Runtime), compartido por el editor y Ajustes. No
// va en el instalador: se descarga solo cuando el usuario lo pide (D-029).
import { toMessage, type Message } from "$lib/core/messages";
import { platform, type SeparatorProgress, type SeparatorStatus } from "$lib/platform";

export const separator = $state({
  status: null as SeparatorStatus | null,
  /** Descarga en curso (bytes); null si no se descarga. */
  download: null as { done: number; total: number } | null,
  error: null as Message | null,
});

let listening = false;

export async function loadSeparator() {
  separator.status = await platform.studioSeparatorStatus();
  if (!listening) {
    listening = true;
    await platform.onSeparatorProgress((p) => {
      if (separator.download) separator.download = { done: p.done, total: p.total };
    });
  }
}

const canceled = (e: unknown) => (e as { code?: string })?.code === "separatorCanceled";

/** true si quedó listo. */
export async function downloadSeparator(): Promise<boolean> {
  if (separator.download) return false;
  separator.error = null;
  separator.download = { done: 0, total: separator.status?.size ?? 0 };
  try {
    await platform.studioSeparatorDownload();
    return true;
  } catch (e) {
    if (!canceled(e)) separator.error = toMessage(e);
    return false;
  } finally {
    separator.download = null;
    await loadSeparator();
  }
}

export function cancelSeparator() {
  platform.studioSeparatorCancel();
}

export async function removeSeparator() {
  separator.error = null;
  try {
    await platform.studioSeparatorRemove();
  } catch (e) {
    separator.error = toMessage(e);
  } finally {
    await loadSeparator();
  }
}

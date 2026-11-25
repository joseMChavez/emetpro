// El reconocedor de voz (Whisper) para sacar la letra de una pista, compartido por el editor y Ajustes. No va en el
// instalador: se descarga solo cuando el usuario lo pide (D-029, como la copia de LibreOffice de D-015).
import { toMessage, type Message } from "$lib/core/messages";
import { platform, type WhisperSegment, type WhisperStatus } from "$lib/platform";

export const recognizer = $state({
  status: null as WhisperStatus | null,
  /** Descarga en curso (bytes); null si no se descarga. */
  download: null as { done: number; total: number } | null,
  /** Transcripción en curso (0–100); null si no se transcribe. */
  percent: null as number | null,
  error: null as Message | null,
});

let listening = false;

export async function loadRecognizer() {
  recognizer.status = await platform.studioWhisperStatus();
  if (!listening) {
    listening = true;
    await platform.onWhisperProgress((p) => {
      if (p.phase === "download" && recognizer.download) recognizer.download = { done: p.done, total: p.total };
      if (p.phase === "transcribe" && recognizer.percent !== null) recognizer.percent = Math.max(recognizer.percent, p.percent);
    });
  }
}

const canceled = (e: unknown) => (e as { code?: string })?.code === "whisperCanceled";

/** true si quedó listo. */
export async function downloadRecognizer(): Promise<boolean> {
  if (recognizer.download) return false;
  recognizer.error = null;
  recognizer.download = { done: 0, total: recognizer.status?.size ?? 0 };
  try {
    await platform.studioWhisperDownload();
    return true;
  } catch (e) {
    if (!canceled(e)) recognizer.error = toMessage(e);
    return false;
  } finally {
    recognizer.download = null;
    await loadRecognizer();
  }
}

/** Para la descarga o la transcripción en curso. */
export function cancelRecognizer() {
  platform.studioWhisperCancel();
}

export async function removeRecognizer() {
  recognizer.error = null;
  try {
    await platform.studioWhisperRemove();
  } catch (e) {
    recognizer.error = toMessage(e);
  } finally {
    await loadRecognizer();
  }
}

/** Lo que se oye en la pista; null si se canceló o falló (el error queda en `recognizer.error`). */
export async function transcribe(samples: Float32Array, language: string, prompt: string): Promise<WhisperSegment[] | null> {
  recognizer.error = null;
  recognizer.percent = 0;
  try {
    return await platform.studioTranscribe(samples, language, prompt);
  } catch (e) {
    if (!canceled(e)) recognizer.error = toMessage(e);
    return null;
  } finally {
    recognizer.percent = null;
  }
}

/** Bytes en MB, redondeados. */
export const megabytes = (bytes: number) => Math.round(bytes / 1_000_000);

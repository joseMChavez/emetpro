// Mensajes entre la consola y los teléfonos del control remoto, vistos desde la consola (remote/chat.rs).
// El de la consola avisa o pregunta al predicador sin levantarse; el teléfono responde con un toque o manda
// una frase corta. Duran lo que dura el control remoto encendido: no se guardan.
import { settings } from "$lib/core/settings.svelte";
import { toMessage, type Message } from "$lib/core/messages";
import { t, type MessageKey } from "$lib/i18n/index.svelte";
import { platform } from "$lib/platform";
import type { ChatNote, ChatPerson, ConsoleChat } from "./protocol";

export interface NotePreset {
  text: string;
  /** Vacío = solo un aviso («Entendido»). */
  replies: string[];
}

export const chat = $state({
  people: [] as ChatPerson[],
  notes: [] as ChatNote[],
  error: null as Message | null,
  /** Respuestas que ya se vieron en la consola (para no seguir avisando de ellas). */
  acknowledged: [] as string[],
});

function apply(view: ConsoleChat) {
  chat.people = view.people;
  chat.notes = view.notes;
}

/** Mantiene al día los mensajes. Devuelve la función para dejar de escuchar. */
export function watchChat(): () => void {
  const unlisten = platform.onRemoteChat(apply);
  platform.remoteChat().then(apply).catch(() => {});
  return () => void unlisten.then((fn) => fn());
}

export function personName(device: string | null): string {
  return chat.people.find((p) => p.device === device)?.name ?? "?";
}

/** Los conectados primero, y por nombre. */
export function people(): ChatPerson[] {
  return [...chat.people].sort((a, b) => Number(b.online > 0) - Number(a.online > 0) || a.name.localeCompare(b.name));
}

export function online(): number {
  return chat.people.filter((p) => p.online > 0).length;
}

export async function sendNote(to: string | null, text: string, replies: string[]): Promise<boolean> {
  chat.error = null;
  try {
    const id = await platform.remoteChatSend(to, text, replies.map((r) => r.trim()).filter(Boolean));
    if (!id) chat.error = { key: "chat.invalid" };
    return id !== null;
  } catch (e) {
    chat.error = toMessage(e);
    return false;
  }
}

export function closeNote(id: string) {
  platform.remoteChatClose(id).catch(() => {});
}

export function acknowledge(id: string) {
  if (!chat.acknowledged.includes(id)) chat.acknowledged.push(id);
}

/** Lo que la consola tiene que ver ya: frases de los teléfonos sin leer y respuestas nuevas. */
export function incoming(): ChatNote[] {
  return chat.notes.filter((n) => (n.from !== null ? !n.closed : n.answer !== null && !chat.acknowledged.includes(n.id)));
}

/** Lo que respondió, en palabras: la opción elegida o «Entendido». */
export function answerText(note: ChatNote): string {
  const reply = note.answer?.reply;
  return reply === null || reply === undefined ? t("chat.ok") : (note.replies[reply] ?? "?");
}

const DEFAULT_NOTES: { text: MessageKey; replies: "ok" | "yesNo" }[] = [
  { text: "chat.preset.fiveMinutes", replies: "ok" },
  { text: "chat.preset.wrapUp", replies: "ok" },
  { text: "chat.preset.stopStream", replies: "yesNo" },
  { text: "chat.preset.nextSong", replies: "yesNo" },
  { text: "chat.preset.sound", replies: "ok" },
];

const DEFAULT_PHRASES: MessageKey[] = [
  "chat.phrase.louder",
  "chat.phrase.quieter",
  "chat.phrase.again",
  "chat.phrase.water",
  "chat.phrase.ready",
  "chat.phrase.wait",
];

export function yesNo(): string[] {
  return [t("chat.yes"), t("chat.no")];
}

/** Los mensajes preparados: los guardados en Ajustes o los de serie, en el idioma de la consola. */
export function notePresets(): NotePreset[] {
  return settings.remoteNotes ?? defaultNotes();
}

export function defaultNotes(): NotePreset[] {
  return DEFAULT_NOTES.map((n) => ({ text: t(n.text), replies: n.replies === "yesNo" ? yesNo() : [] }));
}

export function phrases(): string[] {
  return settings.remotePhrases ?? defaultPhrases();
}

export function defaultPhrases(): string[] {
  return DEFAULT_PHRASES.map((k) => t(k));
}

/** «hace 2 min», para saber cuánto lleva esperando un mensaje o desde cuándo alguien no está. */
export function ago(at: number, now: number): string {
  const seconds = Math.max(0, Math.round((now - at) / 1000));
  if (seconds < 60) return t("chat.agoSeconds", { n: seconds });
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return t("chat.agoMinutes", { n: minutes });
  return t("chat.agoHours", { n: Math.round(minutes / 60) });
}

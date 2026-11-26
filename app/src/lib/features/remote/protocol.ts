// Lo que se dicen el teléfono y la consola a través del servidor del control remoto (src-tauri/src/features/remote).
// Este archivo no importa nada de Tauri: lo usa también la página del teléfono, que corre en un navegador.
import type { BoardMessage } from "./board";
import type { MarkColor, MarkPart, MarkStyle } from "$lib/features/marks/marks";
import type { MusicalKey } from "$lib/platform/types";

/** Órdenes del teléfono. Deben coincidir con `domain::Command` del backend. */
export type RemoteCommand =
  | { type: "next" }
  | { type: "previous" }
  | { type: "black" }
  | { type: "clear" }
  | { type: "scripture"; book: number; chapter: number; verse: number }
  | { type: "setBible"; id: string }
  | { type: "show"; key: string }
  /** Un elemento del plan abierto en la consola, por su id. */
  | { type: "planItem"; id: string }
  /** Ir a la posición n (desde 1) de lo que está en pantalla: página, diapositiva o versículo. */
  | { type: "goTo"; n: number }
  /** Poner en pantalla una hoja de la pizarra (board.ts). */
  | { type: "showSheet"; id: string }
  /** Resaltar (o desmarcar) de la palabra `from` a la `to` de lo que está en pantalla (D-023). */
  | { type: "mark"; part: MarkPart; from: number; to: number; style: MarkStyle; color: MarkColor }
  /** Quitar las palabras resaltadas del versículo o la diapositiva en pantalla. */
  | { type: "clearMarks" }
  /** Media Studio (D-029): reproducir o pausar la pista de la canción en pantalla. */
  | { type: "trackToggle" }
  /** Repetir (o dejar de repetir) la sección en pantalla. */
  | { type: "trackLoop" }
  /** Terminar la canción con un fundido. */
  | { type: "trackFinish" }
  /** Dejar de fondo, más suave, la canción que sigue sonando tras poner otra cosa en pantalla. */
  | { type: "trackBackground" }
  /** Saltar a una sección de la canción en pantalla (la pista la sigue). */
  | { type: "studioSection"; id: string }
  /** El tono de la canción que suena (semitonos, de −6 a +6). */
  | { type: "trackTone"; semitones: number }
  | { type: "trackVoice"; voiceless: boolean };

/** Algo que se puede volver a poner en pantalla desde el teléfono: lo preparado en la consola o lo reciente. */
export interface RemoteItem {
  key: string;
  /** Cómo lo describe la consola: «Juan 3:16 (LBLA)», «Presentación.pptx, página 5»… */
  label: string;
  /** Lo preparado en la consola (aún no enviado). */
  prepared: boolean;
}

/** El plan abierto en la consola, para tenerlo a un toque (D-019). */
export interface RemotePlan {
  /** Para recordar en el teléfono qué secciones de este plan están plegadas. */
  id: string;
  name: string;
  items: RemotePlanItem[];
}

export type RemotePlanItem =
  | { kind: "section"; id: string; title: string }
  | {
      kind: "item";
      id: string;
      icon: string;
      title: string;
      detail: string;
      /** false si no se puede mostrar (letra borrada, archivo que no está…). */
      available: boolean;
      /** Ya pasó por la pantalla en este culto. */
      shown: boolean;
      /** Es lo último preparado o mostrado desde el plan. */
      current: boolean;
    };

/** Lo que se puede elegir de lo que está en pantalla. La posición actual es `position.n`. */
export type RemoteStrip =
  /** Páginas de un PDF: sus miniaturas están en /api/thumb?doc=…&n=…, mientras siga en pantalla. */
  | { kind: "pages"; doc: string; total: number }
  /** Diapositivas de texto o versículos: una etiqueta corta por cada uno. */
  | { kind: "labels"; labels: string[] };

/** Lo que está en pantalla, contado para el teléfono. */
export type RemoteLive =
  | { kind: "empty" }
  | { kind: "black" }
  | {
      kind: "scripture";
      reference: string;
      text: string;
      version: string;
      /** La Biblia del versículo (id); null si ya no está entre las que se pueden elegir. */
      bible: string | null;
      /** Libro, capítulo y versículo; null si no se pudo saber. */
      place: { book: number; chapter: number; verse: number } | null;
    }
  | { kind: "pdf"; name: string }
  | { kind: "text"; text: string }
  /** Una hoja de la pizarra. */
  | { kind: "board"; sheet: string }
  /** Imagen, video, reloj…: el teléfono muestra la descripción de la consola. */
  | { kind: "other"; description: string };

/** Una canción de Media Studio en pantalla (D-029). */
export interface RemoteStudio {
  /** null = en directo (sin pista): solo secciones. */
  playing: boolean | null;
  /** Terminando con el fundido. */
  finishing: boolean;
  loop: boolean;
  /** El tono con que suena (semitonos; 0 = el original). */
  transpose?: number;
  /** La tonalidad de la pista, si se sabe: el teléfono dice en cuál suena con el tono puesto. */
  key?: MusicalKey | null;
  /** Suena sin voz; null si la canción no tiene versión sin voz (no se puede elegir). */
  voiceless?: boolean | null;
  sections: { id: string; name: string; current: boolean }[];
  /**
   * Se puso otra cosa en pantalla y la canción sigue sonando: su título. `asking`: todavía no se eligió entre
   * terminarla o dejarla de fondo; `soft`: suena de fondo, más suave.
   */
  behind?: { title: string; asking: boolean; soft: boolean } | null;
}

/** Lo que la consola publica cada vez que cambia algo. */
export interface RemoteStatus {
  /** La ventana de salida está abierta. */
  open: boolean;
  live: RemoteLive;
  /** Posición en lo que está en pantalla, si se puede avanzar (versículos, páginas, diapositivas). */
  position: { n: number; total: number | null; unit: "verse" | "page" | "slide" } | null;
  /** En negro con algo a lo que volver. */
  canUnblack: boolean;
  /** Preguntar antes de mostrar algo nuevo (el mismo ajuste de la consola). */
  confirmSend: boolean;
  /** Lo preparado y lo que estuvo en pantalla hace poco, lo más reciente primero (sin lo que está ahora). */
  items: RemoteItem[];
  /** Para ver y elegir dentro de lo que está en pantalla; null si no hay nada que elegir. */
  strip: RemoteStrip | null;
  /** Sobre qué se dibuja en la pizarra ahora; null si no se puede (pantalla vacía o en negro). */
  canvas: RemoteCanvas | null;
  /** El plan abierto en la consola; null si no hay. */
  plan: RemotePlan | null;
  /** La canción de Media Studio en pantalla; null si no hay. Las versiones anteriores del teléfono no lo leen. */
  studio?: RemoteStudio | null;
  /** Las versiones que se pueden elegir. */
  bibles: { id: string; abbreviation: string; name: string }[];
  /** La Biblia elegida en la consola: el teléfono busca en ella. */
  bible: {
    id: string;
    abbreviation: string;
    /** Idioma de los nombres de libro. */
    language: string | null;
    books: { number: number; chapters: number }[];
  } | null;
}

/** Lo que está en pantalla, para dibujar encima: la capa de sus trazos y qué poner de fondo en el teléfono. */
export interface RemoteCanvas {
  /** La capa (board.ts, layerKey). */
  layer: string;
  under:
    /** Una hoja de la pizarra: su fondo está en la pizarra. */
    | { kind: "sheet"; id: string }
    /** Una página de PDF: la miniatura de /api/thumb. */
    | { kind: "page"; doc: string; n: number }
    /** Texto o versículo: dónde quedó cada palabra llega en la pizarra (screen), medido por la ventana de salida. */
    | { kind: "text"; text: string }
    /** Imagen, video, reloj…: solo se describe. */
    | { kind: "other"; description: string };
}

/** Mensajes con la consola (remote/chat.rs), como los ve un teléfono. */
export interface PhoneChat {
  /** El nombre con que lo conoce la consola; null hasta que se presenta. */
  name: string | null;
  /** Lo que la consola le mandó y aún no se respondió, lo más antiguo primero. */
  pending: { id: string; text: string; replies: string[]; at: number; toAll: boolean }[];
  /** Sus últimas frases para la consola, la más reciente primero; `read`: la consola ya la vio. */
  mine: { id: string; text: string; at: number; read: boolean }[];
  /** Las frases que ofrece para mandar a la consola. */
  phrases: string[];
}

/** Lo que dice el teléfono sobre los mensajes. Deben coincidir con `chat::PhoneMessage` del backend. */
export type PhoneMessage =
  | { type: "hello"; device: string; name: string }
  | { type: "seen"; id: string }
  /** null = «Entendido» de un aviso sin respuestas. */
  | { type: "reply"; id: string; reply: number | null }
  | { type: "say"; text: string };

/** Mensaje del servidor al teléfono. `status` es null hasta que la consola publica por primera vez.
 *  Los de la pizarra (board.ts) y los mensajes con la consola llegan por el mismo WebSocket. */
export type ServerMessage = { type: "status"; status: RemoteStatus | null } | { type: "chat"; chat: PhoneChat } | BoardMessage;

/** Alguien conectado desde un teléfono, como lo ve la consola. */
export interface ChatPerson {
  device: string;
  name: string;
  /** Conexiones abiertas: 0 = desconectado. */
  online: number;
  /** Cuándo se conectó o desconectó por última vez (ms). */
  since: number;
}

/** Un mensaje: de la consola (`from` null) a un teléfono o a todos (`to` null), o de un teléfono a la consola. */
export interface ChatNote {
  id: string;
  from: string | null;
  to: string | null;
  text: string;
  /** Vacío = solo un aviso. */
  replies: string[];
  at: number;
  seen: string[];
  answer: { by: string; reply: number | null; at: number } | null;
  closed: boolean;
}

export interface ConsoleChat {
  people: ChatPerson[];
  notes: ChatNote[];
}

/** Hasta cuántas respuestas puede ofrecer un mensaje, y de qué largo (como en chat.rs). */
export const MAX_REPLIES = 4;
export const MAX_REPLY = 40;
export const MAX_NOTE = 300;
export const MAX_NAME = 40;

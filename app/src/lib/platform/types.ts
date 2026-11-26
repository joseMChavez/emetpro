// Tipos compartidos entre la interfaz y el backend. No dependen de Tauri (ver D-010).
import type { ConsoleChat, RemoteCommand } from "$lib/features/remote/protocol";
import type { Board, BoardMessage, BoardOp, ScreenLayout } from "$lib/features/remote/board";
import type { Marks } from "$lib/features/marks/marks";
import type { RichSlide } from "$lib/features/richtext/rich";

export interface MonitorInfo {
  index: number;
  name: string;
  width: number;
  height: number;
  x: number;
  y: number;
  scale: number;
  isPrimary: boolean;
}

export type OutputContent =
  | { kind: "empty" }
  | { kind: "black" }
  | { kind: "testPattern" }
  | {
      kind: "text";
      slides: string[];
      index: number;
      /** Autoría de una letra; se muestra en la primera diapositiva. */
      credit?: string;
      /** Imagen, GIF o video propio de esta letra; tapa el fondo general. */
      background?: string | null;
      /** Palabras resaltadas, por número de diapositiva (D-023). */
      marks?: Marks;
      /** Solo si la cascada (plan, sección, letra) cambia lo general (D-024). */
      animation?: TextAnimation;
      /** Con formato (D-027): una por diapositiva. `slides` lleva su texto plano, que manda para marcas y teléfono. */
      rich?: RichSlide[];
      /** Canción de Media Studio de la que salen (D-029): la consola la sigue con su pista. */
      studio?: string | null;
      /** Fondo propio de cada diapositiva (el de su sección en Media Studio); null = `background`. */
      backgrounds?: (string | null)[];
      /** Media Studio: cuándo empieza (ms de la pista) cada palabra de cada diapositiva; null = sin marcar. */
      words?: ((number | null)[] | null)[];
    }
  | { kind: "image"; path: string }
  | {
      kind: "video";
      path: string;
      paused: boolean;
      /** De 0 a 1; sin valor, entero. */
      volume?: number;
      muted?: boolean;
    }
  | { kind: "pdf"; path: string; page: number }
  | {
      kind: "scripture";
      verses: ScriptureVerse[];
      index: number;
      version: string;
      copyright: string;
      /** Segunda versión, debajo de la primera. Como mucho dos. */
      second?: { version: string; copyright: string } | null;
      /** Palabras resaltadas, por «versión|cita» (D-023). */
      marks?: Marks;
      animation?: TextAnimation;
    }
  | { kind: "clock"; hour12: boolean; seconds: boolean }
  /** En marcha si endsAt (ms desde 1970) tiene valor; en pausa, remainingMs guarda lo que queda. */
  | {
      kind: "timer";
      durationMs: number;
      endsAt: number | null;
      remainingMs: number;
      /** Título sobre la cuenta («Receso»); vacío = solo la cuenta. */
      label?: string;
      /** Imagen o GIF propio de esta cuenta; tapa el fondo general. */
      background?: string | null;
    }
  /** Una hoja de la pizarra del control remoto (features/remote/board.ts). */
  | { kind: "board"; sheet: string };

export interface ScriptureVerse {
  reference: string;
  text: string;
  /** El mismo versículo en la segunda versión; null si esa versión no lo tiene. */
  second?: { reference: string; text: string } | null;
}

export type FontId = "system" | "montserrat" | "oswald" | "bebasNeue" | "lora" | "merriweather";

export interface OutputStyle {
  /** Tamaño máximo relativo (1 = normal). El texto se reduce solo si no cabe. */
  fontScale: number;
  align: "left" | "center" | "right";
  position: "top" | "middle" | "bottom";
  background: "black" | "navy" | "charcoal";
  font: FontId;
  /** Imagen o GIF detrás del texto, la Biblia y el reloj. */
  backgroundImage: string | null;
  logo: LogoStyle;
  /** Animar el texto (D-024); un plan, una sección o una letra pueden decir otra cosa. */
  animate: boolean;
  animation: TextAnimation;
}

/** Cómo se anima el texto en pantalla y si el fondo de imagen se mueve (D-024). */
export interface TextAnimation {
  transition: "none" | "fade" | "zoom";
  entrance: "none" | "lines" | "words" | "typewriter";
  ambient: "none" | "breathe" | "float" | "glow";
  background: "still" | "slow";
}

export type Corner = "topLeft" | "topRight" | "bottomLeft" | "bottomRight";

/** Logo o emblema como marca de agua en una esquina de la pantalla. */
export interface LogoStyle {
  /** null = sin logo. */
  image: string | null;
  corner: Corner;
  size: "small" | "medium" | "large";
  /** 0..1 */
  opacity: number;
}

export interface BibleInfo {
  id: string;
  abbreviation: string;
  name: string;
  copyright: string;
  link: string | null;
  /** Idioma del texto bíblico, independiente del de la interfaz. */
  language: string | null;
  /** Incluida en la instalación (no se puede quitar). */
  builtin: boolean;
  books: { number: number; chapters: number }[];
}

export interface Verse {
  number: number;
  text: string;
}

export interface SearchHit {
  book: number;
  chapter: number;
  verse: number;
  text: string;
}

/** «presentation» (PowerPoint, Impress) se convierte a PDF al prepararla. */
export type MediaKind = "image" | "video" | "pdf" | "presentation";

export interface MediaFile {
  path: string;
  name: string;
  kind: MediaKind;
}

export interface ObsDetection {
  installed: boolean;
  websocketEnabled: boolean;
  running: boolean;
  /** Abierto en modo seguro (sin WebSocket): hay que cerrarlo y abrirlo normal. */
  safeMode: boolean;
}

export interface ObsStatus {
  obsVersion: string;
  programScene: string | null;
  /** Todas las escenas de OBS, en el orden de su ventana: las del usuario y las de EmetPro. */
  scenes: string[];
  /** Fuentes de texto de OBS que EmetPro puede rellenar. */
  textSources: string[];
  /** Las escenas de EmetPro existen en OBS. */
  emetproReady: boolean;
  hasCamera: boolean;
  streaming: {
    active: boolean;
    reconnecting: boolean;
    durationMs: number;
    congestion: number;
    skippedFrames: number;
    totalFrames: number;
  };
  /** La clave de emisión nunca sale de OBS: solo se indica si existe. */
  destination: { kind: string; server: string; hasKey: boolean } | null;
}

/** Dónde quedó visible el texto al ponerlo en la emisión. */
export type ObsTextShown = { kind: "current" } | { kind: "switched"; scene: string } | { kind: "notInScene" };

export interface ObsCamera {
  id: string;
  name: string;
}

// ---- Media Studio (D-029; features/studio/domain.rs) ----
// Tiempos en ms desde el comienzo de la pista; solo comienzos: cada tramo dura hasta que empieza el siguiente.

export type SongMode = "track" | "live";
export type SectionKind = "intro" | "verse" | "preChorus" | "chorus" | "bridge" | "instrumental" | "interlude" | "tag" | "outro" | "custom";

export interface AudioTrack {
  path: string;
  name: string;
  durationMs: number;
}

export interface SongRights {
  composition: string;
  lyrics: string;
  recording: string;
  license: string;
  notes: string;
}

export interface SongSlide {
  id: string;
  lines: string[];
}

export interface SongSection {
  id: string;
  kind: SectionKind;
  /** Vacío = el nombre de su tipo. */
  label: string;
  slides: SongSlide[];
  /** Fondo propio de la sección; null = el de la canción. */
  background?: string | null;
}

export interface LineCue {
  startMs: number;
  endMs?: number | null;
  words?: number[];
}

export interface SlideCue {
  slide: string;
  startMs: number;
  lines?: LineCue[];
  /** La repartió la app («Repartir diapositivas»), no se marcó: se puede volver a repartir. */
  auto?: boolean;
}

export interface Timing {
  startMs: number;
  slides: SlideCue[];
}

export interface OrderEntry {
  id: string;
  section: string;
  timing?: Timing | null;
}

export interface MediaSong {
  id: string;
  title: string;
  artist: string;
  mode: SongMode;
  audio: AudioTrack | null;
  sections: SongSection[];
  order: OrderEntry[];
  rights: SongRights;
  /** Enlace para escucharla al preparar (YouTube…). Solo se abre en el navegador. */
  reference: string;
  background: string | null;
  animation?: TextAnimation | null;
  /** Semitonos de más o de menos al sonar (de −6 a +6); sin él, el original. */
  transpose?: number;
  /** Tonalidad de la pista (detectada y corregible); ausente = sin saber. */
  key?: MusicalKey | null;
  /** La pista sin la voz, sacada de `audio`: dura lo mismo y sirven los mismos tiempos. */
  instrumental?: AudioTrack | null;
  /** Suena la pista sin voz en lugar de la original. */
  voiceless?: boolean;
}

/** Tónica en semitonos desde Do (0 = Do, 7 = Sol) y si es menor. */
export interface MusicalKey {
  tonic: number;
  minor: boolean;
}

export type IssueCode =
  | "missingTitle" | "invalidReference" | "missingId" | "duplicateId" | "emptyOrder" | "unknownSection" | "trackWithoutAudio" | "invalidAudio"
  | "timeOutOfTrack" | "entriesOutOfOrder" | "sameEntryStart" | "unknownSlide" | "duplicateCue" | "slidesOutOfOrder" | "slideOutsideEntry"
  | "linesOutOfOrder" | "lineOutsideSlide" | "lineEndBeforeStart" | "linesOverlap" | "wordsOutOfOrder" | "wordOutsideLine"
  | "unusedSection" | "emptySlide" | "untimedEntry" | "untimedSlide" | "extraLineCues" | "extraWordCues" | "shortSlide" | "invalidTranspose" | "invalidKey" | "instrumentalMissing";

export interface SongIssue {
  code: IssueCode;
  severity: "error" | "warning";
  at: { entry?: string; section?: string; slide?: string; line?: number; word?: number };
}

export interface StudioSaveOutcome {
  saved: boolean;
  issues: SongIssue[];
}

/** Letra guardada en la biblioteca. */
export interface Song {
  id: string;
  title: string;
  /** Quien la canta o interpreta. */
  singer: string;
  /** Quien escribió la letra. */
  writer: string;
  /** Texto completo; **** (o ***) en una línea separa las diapositivas. */
  body: string;
  /** Imagen, GIF o video propio; null = el fondo general. */
  background: string | null;
  /** Su propia animación; null = la del plan o la general (D-024). */
  animation?: TextAnimation | null;
}

/** Ajustes tal como están en la base: clave → valor JSON. Las claves ausentes valen lo de por defecto. */
export type StoredSettings = Record<string, unknown>;

/** Por qué no se pudo abrir la base (core/db.rs). Salvo «damaged», el archivo queda intacto. */
export type StoreProblem = { code: "diskFull" } | { code: "noPermission" } | { code: "damaged" } | { code: "other"; message: string };

export interface StoreStatus {
  /** Archivo de la base; null si se trabaja en memoria. */
  path: string | null;
  problem: StoreProblem | null;
}

/** Error devuelto por el backend; la interfaz lo traduce al idioma elegido. */
export type CommandError =
  | { code: "monitorDisconnected" }
  | { code: "fileMissing"; path: string }
  | { code: "invalidContent" }
  | { code: "invalidBible"; file: string; reason: string }
  | { code: "bibleNotFound" }
  | { code: "obsNotInstalled" }
  | { code: "obsRunning" }
  | { code: "obsSafeMode" }
  | { code: "obsNotConnected" }
  | { code: "obs"; message: string }
  | { code: "storage"; message: string }
  | { code: "officeNotInstalled" }
  | { code: "conversionFailed"; file: string; reason: string }
  | { code: "videoConverterMissing" }
  | { code: "officeBusy" }
  | { code: "officeCanceled" }
  | { code: "officeDownload"; message: string }
  | { code: "whisperBusy" }
  | { code: "whisperUnsupported" }
  | { code: "whisperCanceled" }
  | { code: "whisperModelMissing" }
  | { code: "whisperDownload"; message: string }
  | { code: "transcribeFailed"; reason: string }
  | { code: "separatorBusy" }
  | { code: "separatorCanceled" }
  | { code: "separatorUnavailable" }
  | { code: "separatorMissing" }
  | { code: "separatorDownload"; message: string }
  | { code: "separateFailed"; reason: string }
  | { code: "prepareFailed"; reason: string }
  | { code: "importFailed"; file: string; reason: string }
  | { code: "exportFailed"; reason: string }
  | { code: "copyFailed"; file: string; reason: string }
  | { code: "remoteFailed"; reason: string }
  | { code: "internal"; message: string };

/** LibreOffice para las presentaciones: el del sistema o la copia privada que EmetPro puede descargar. */
export interface OfficeStatus {
  system: boolean;
  privateCopy: boolean;
  /** Hay copia privada para este sistema. */
  downloadable: boolean;
  installing: boolean;
  version: string;
  /** Tamaño de la descarga en bytes. */
  size: number;
}

export interface OfficeProgress {
  downloaded: number;
  total: number;
  /** Ya descargado y comprobado; ahora se extrae. */
  extracting: boolean;
}

/** Una canción en la vista previa de una importación. */
export interface ImportSong {
  title: string;
  singer: string;
  slides: number;
}

export type ImportInvalidReason = "noTitle" | "noLyrics" | "repeated";

export interface ImportPreview {
  file: string;
  format: "emetpro" | "holyrics";
  fresh: ImportSong[];
  /** Ya están con la misma letra: no se tocan. */
  identical: number;
  /** Ya están con otra letra. */
  conflicts: ImportSong[];
  invalid: { title: string; reason: ImportInvalidReason }[];
  /** Canciones que trae como lista del culto; al importar pueden formar un plan. */
  setlist: number;
}

export type OnConflict = "skip" | "replace" | "keepBoth";

/** Planes en un archivo (.emetplan, D-028). */
export interface BibleRef {
  id: string;
  abbreviation: string;
  name: string;
}

export interface PlanExportInfo {
  items: number;
  videos: number;
  videoBytes: number;
  otherBytes: number;
}

export interface PlanExportResult {
  path: string;
  items: number;
  files: number;
  omitted: number;
}

export interface PlanImportPreview {
  file: string;
  name: string;
  /** Si el archivo es una sección: su título. */
  section: string | null;
  date: string | null;
  app: string;
  items: number;
  files: number;
  bytes: number;
  /** Medios que vienen sin su archivo (videos que no se incluyeron). */
  omitted: string[];
  fresh: ImportSong[];
  identical: number;
  conflicts: ImportSong[];
  bibles: BibleRef[];
  /** Canciones de Media Studio que trae (títulos), y las que ya están en este equipo con otro contenido. */
  studio: string[];
  studioConflicts: string[];
}

export interface PlanImportResult {
  planId: string;
  items: number;
  skipped: number;
  added: number;
  replaced: number;
  /** Canciones de Media Studio guardadas. */
  studio: number;
}

export interface ImportResult {
  added: number;
  replaced: number;
  skipped: number;
  invalid: number;
  /** Letras del plan creado con la lista del archivo; 0 si no se creó. */
  planSongs: number;
}

/** Un plan del culto: lo que se va a presentar, en orden (features/plans, D-019). */
export interface Plan {
  id: string;
  name: string;
  /** Día del culto, «AAAA-MM-DD»; null = sin fecha. */
  date: string | null;
  notes: string;
  items: PlanItem[];
  /** Animar el texto: null = como lo general (D-024). */
  animate?: boolean | null;
}

export type PlanItem = { id: string } & (
  /** `animate` vale para lo que sigue hasta la sección siguiente; null = como el plan (D-024). */
  | { kind: "section"; title: string; animate?: boolean | null }
  | { kind: "song"; songId: string; marks?: Marks }
  /** Una canción de Media Studio (D-029), con su pista y sus fondos. */
  | { kind: "studioSong"; studioSongId: string }
  | { kind: "media"; path: string; media: MediaKind }
  | { kind: "pause"; title: string; durationMs: number; background?: string | null }
  /** Texto con formato escrito en el plan (D-027): un sermón, avisos. */
  | { kind: "text"; title: string; slides: RichSlide[]; marks?: Marks }
  | {
      kind: "scripture";
      book: number;
      chapter: number;
      /** null = el capítulo entero. */
      verseStart: number | null;
      verseEnd: number | null;
      /** Versión fija (id de la Biblia); null = la elegida en la consola al mostrarlo. */
      bible?: string | null;
      /** Palabras resaltadas en esta cita, para este plan (D-023). */
      marks?: Marks;
    }
);

/** Por dónde va el video en la pantalla, en segundos. */
/** Un video listo para cargar: `url` es la del servidor local (null = usar mediaUrl(path)). */
export interface PlayableVideo {
  path: string;
  url: string | null;
  /** Es la copia en MP4, no el original. */
  converted: boolean;
}

/** Por dónde va la pista de la canción de Media Studio en pantalla (D-029), de la consola a la salida. */
export interface StudioTime {
  song: string;
  ms: number;
  playing: boolean;
}

export interface VideoTime {
  path: string;
  time: number;
  duration: number;
  paused: boolean;
}

export interface OutputState {
  open: boolean;
  monitorName: string | null;
  content: OutputContent;
  style: OutputStyle;
}

/** El control remoto, como lo muestra la consola en Ajustes. */
export interface RemoteInfo {
  running: boolean;
  /** Dirección para el teléfono, con la clave (la del QR). */
  url: string | null;
  /** El QR de `url` como SVG. */
  qr: string | null;
  /** false si no hay red: el teléfono no podrá llegar. */
  onNetwork: boolean;
  clients: number;
}

export interface Platform {
  listMonitors(): Promise<MonitorInfo[]>;
  openOutput(monitorIndex: number): Promise<OutputState>;
  closeOutput(): Promise<void>;
  setOutputContent(content: OutputContent): Promise<OutputState>;
  getOutputState(): Promise<OutputState>;
  setOutputStyle(style: OutputStyle): Promise<OutputState>;
  /** Devuelve la función para dejar de escuchar. */
  onOutputState(handler: (state: OutputState) => void): Promise<() => void>;
  /** Abre el selector de archivos; devuelve [] si se cancela. */
  pickMedia(title: string, filterName: string): Promise<MediaFile[]>;
  /** Elige una carpeta y devuelve sus archivos compatibles, subcarpetas incluidas (null si se cancela). */
  pickMediaFolder(title: string): Promise<MediaFile[] | null>;
  /** Elige una imagen o GIF de fondo; null si se cancela. */
  /** Con `videos`, un fondo puede ser también un video en bucle (D-024); el logo, no. */
  pickBackground(title: string, filterName: string, videos?: boolean): Promise<string | null>;
  /** Vuelve a permitir fondos guardados tras reiniciar; devuelve los que siguen existiendo. */
  allowBackgrounds(paths: string[]): Promise<string[]>;
  /** Convierte una presentación a PDF (LibreOffice, PowerPoint o Keynote; o la toma de la caché) y devuelve la ruta del PDF. */
  convertPresentation(path: string): Promise<string>;
  /** Hay con qué convertir presentaciones. */
  officeAvailable(): Promise<boolean>;
  officeStatus(): Promise<OfficeStatus>;
  /** Descarga la copia privada de LibreOffice; termina cuando está lista. */
  officeInstall(): Promise<void>;
  officeCancel(): Promise<void>;
  /** Borra la copia privada (no un LibreOffice del sistema). */
  officeRemove(): Promise<void>;
  onOfficeProgress(handler: (progress: OfficeProgress) => void): Promise<() => void>;
  /** Qué cargar para un video (su copia en MP4 si ya se convirtió) y por dónde (el servidor local, si está). */
  playableVideo(path: string): Promise<PlayableVideo>;
  /** Convierte a MP4 H.264 un video que este equipo no reproduce (HEVC de iPhone…); devuelve la copia. */
  convertVideo(path: string): Promise<string>;
  /** Un video ya tiene copia convertida (llega la ruta del original), desde cualquier ventana. */
  onVideoConverted(handler: (path: string) => void): Promise<() => void>;
  /** URL que el webview puede cargar para un archivo local ya permitido. */
  mediaUrl(path: string): string;
  reportOutputError(message: string): Promise<void>;
  /** Escribe un error de la interfaz en el registro de la app. */
  logError(message: string): Promise<void>;
  onOutputError(handler: (message: string) => void): Promise<() => void>;
  /** Avanzar (true) o retroceder lo que está en pantalla; lo pide la ventana de salida al pulsar una flecha. */
  outputNavigate(forward: boolean): Promise<void>;
  onOutputNavigate(handler: (forward: boolean) => void): Promise<() => void>;
  /** La ventana de salida avisa por dónde va el video, para que la consola lo siga en su vista. */
  reportVideoTime(time: VideoTime): Promise<void>;
  /** La consola avisa por dónde va la pista de Media Studio, para que la salida ilumine la palabra que suena. */
  reportStudioTime(time: StudioTime): Promise<void>;
  onStudioTime(handler: (time: StudioTime) => void): Promise<() => void>;
  onVideoTime(handler: (time: VideoTime) => void): Promise<() => void>;
  listBibles(): Promise<BibleInfo[]>;
  /** Abre el selector de archivos XML; devuelve las Biblias importadas ([] si se cancela). */
  importBibles(title: string, filterName: string): Promise<BibleInfo[]>;
  removeBible(id: string): Promise<void>;
  getChapter(bibleId: string, book: number, chapter: number): Promise<Verse[]>;
  searchBible(bibleId: string, query: string): Promise<SearchHit[]>;

  // Base de datos local
  settingsGet(): Promise<StoredSettings>;
  settingsSet(key: string, value: unknown): Promise<void>;
  /** Vuelve al valor inicial esas claves ([] = todas) y devuelve los ajustes que quedan. */
  settingsReset(keys: string[]): Promise<StoredSettings>;
  /** Cada cambio de ajustes, hecho desde cualquier ventana. */
  onSettingsChanged(handler: (settings: StoredSettings) => void): Promise<() => void>;
  /** Archivo de la base, o por qué se trabaja en memoria (path null). */
  storeStatus(): Promise<StoreStatus>;
  songsList(): Promise<Song[]>;
  songSave(song: Song): Promise<void>;
  songDelete(id: string): Promise<void>;
  /** Borra la biblioteca de letras (los planes las marcan como borradas). */
  songsReset(): Promise<void>;

  // Media Studio (D-029)
  studioList(): Promise<MediaSong[]>;
  studioValidate(song: MediaSong): Promise<SongIssue[]>;
  /** Solo guarda si no hay errores. */
  studioSave(song: MediaSong): Promise<StudioSaveOutcome>;
  studioDelete(id: string): Promise<void>;
  /** Elige una pista (audio, o un video del que se saca el audio) y la deja en la carpeta de la app; null si se cancela. */
  studioPickAudio(title: string, filterName: string): Promise<{ path: string; name: string; fromVideo: boolean } | null>;
  /** Exporta canciones de Media Studio a un .emetplan (con su pista y sus fondos); null si se cancela. */
  studioExport(title: string, filterName: string, fileName: string, songIds: string[]): Promise<PlanExportResult | null>;
  /** Guarda un fondo animado generado (SVG) en la carpeta de la app y devuelve su ruta. */
  studioSaveSvg(svg: string): Promise<string>;
  /** Abre el enlace de referencia en el navegador (solo http/https). */
  studioOpenReference(url: string): Promise<void>;
  /** Los bytes de una pista de la carpeta de la app, para decodificarla. */
  studioReadAudio(path: string): Promise<ArrayBuffer>;
  /** Reconocedor de voz para sacar la letra de una pista (descarga aparte, D-029). */
  studioWhisperStatus(): Promise<WhisperStatus>;
  studioWhisperDownload(): Promise<void>;
  studioWhisperCancel(): Promise<void>;
  studioWhisperRemove(): Promise<void>;
  /** Lo que se oye en la pista (mono, 16 kHz); `language` "" = que lo adivine; `prompt`: contexto (el título). */
  studioTranscribe(samples: Float32Array, language: string, prompt: string): Promise<WhisperSegment[]>;
  onWhisperProgress(handler: (progress: WhisperProgress) => void): Promise<() => void>;
  /** Separador para quitar la voz de una pista (descarga aparte, D-029). */
  studioSeparatorStatus(): Promise<SeparatorStatus>;
  studioSeparatorDownload(): Promise<void>;
  studioSeparatorCancel(): Promise<void>;
  studioSeparatorRemove(): Promise<void>;
  onSeparatorProgress(handler: (progress: SeparatorProgress) => void): Promise<() => void>;
  /**
   * Prepara una canción con su pista de una vez (D-029): lee la pista, separa la voz si hay separador (y guarda la
   * versión sin voz si `voiceless`), y si `listen` oye la letra con Whisper. `language` "" = que lo adivine.
   */
  studioPrepare(path: string, name: string, language: string, prompt: string, listen: boolean, voiceless: boolean): Promise<PreparedTrack>;
  onPrepareProgress(handler: (progress: PrepareProgress) => void): Promise<() => void>;

  // Planes del culto (features/plans)
  /** Con fecha primero, del más próximo al más lejano; después los que no tienen fecha. */
  plansList(): Promise<Plan[]>;
  planSave(plan: Plan): Promise<void>;
  planDelete(id: string): Promise<void>;
  plansReset(): Promise<void>;
  /** Vuelve a permitir los archivos del plan guardado; devuelve los que ya no están en este equipo. */
  planAllowFiles(id: string): Promise<string[]>;
  /** Elige medios y los copia a la carpeta de la app; devuelve las copias, con el nombre original. [] si se cancela. */
  planPickMedia(title: string, filterName: string): Promise<MediaFile[]>;
  /** Elige el fondo de una pausa y lo copia a la carpeta de la app; null si se cancela. */
  planPickBackground(title: string, filterName: string): Promise<string | null>;
  /** Borra las copias que ya no usa ningún plan; devuelve lo que ocupan las que quedan, en bytes. */
  plansCleanFiles(): Promise<number>;

  // Control remoto desde el teléfono (features/remote)
  remoteInfo(): Promise<RemoteInfo>;
  remoteStart(): Promise<RemoteInfo>;
  remoteStop(): Promise<RemoteInfo>;
  /** Clave nueva: se desconectan los teléfonos y hay que volver a escanear el QR. */
  remoteResetKey(): Promise<RemoteInfo>;
  /** Lo que verán los teléfonos (features/remote/protocol.ts). */
  remotePublish(status: unknown): Promise<void>;
  /** Otro PDF en pantalla, de `total` páginas: se olvidan las miniaturas del anterior. */
  remoteThumbsStart(doc: string, total: number): Promise<boolean>;
  /** Una miniatura (JPEG) de la página `page` de `doc`, para los teléfonos. false si se descartó. */
  remoteThumb(doc: string, page: number, jpeg: Uint8Array): Promise<boolean>;
  onRemoteCommand(handler: (command: RemoteCommand) => void): Promise<() => void>;
  onRemoteClients(handler: (count: number) => void): Promise<() => void>;
  /** La pizarra entera: hojas y lo dibujado (features/remote/board.ts). */
  remoteBoard(): Promise<Board>;
  /** Un cambio en la pizarra desde la consola. false si no es válido. */
  remoteBoardOp(op: BoardOp): Promise<boolean>;
  /** La ventana de salida cuenta cómo es la pantalla y dónde quedó el texto, para dibujar encima desde el teléfono. */
  remoteScreen(screen: ScreenLayout | null): Promise<void>;
  onRemoteBoard(handler: (message: BoardMessage) => void): Promise<() => void>;
  /** Los mensajes con los teléfonos y quién está conectado (features/remote/chat.svelte.ts). */
  remoteChat(): Promise<ConsoleChat>;
  /** Un mensaje para un teléfono (`to`) o para todos (null). Devuelve su id; null si no vale. */
  remoteChatSend(to: string | null, text: string, replies: string[]): Promise<string | null>;
  /** Retirar un mensaje de la consola, o dar por leído uno de un teléfono. */
  remoteChatClose(id: string): Promise<boolean>;
  /** Las frases que el teléfono ofrece para mandar a la consola. */
  remoteChatPhrases(phrases: string[]): Promise<void>;
  onRemoteChat(handler: (chat: ConsoleChat) => void): Promise<() => void>;

  // Importar y exportar letras (features/transfer)
  /** Elige un .emetpro, .selah o .hpls y devuelve la vista previa; null si se cancela. No guarda nada. */
  transferImportPick(title: string, filterName: string): Promise<ImportPreview | null>;
  /** Guarda lo de la última vista previa. Con planName, la lista del archivo se guarda además como un plan. */
  transferImportApply(onConflict: OnConflict, planName: string | null, importedSuffix: string): Promise<ImportResult>;
  transferImportCancel(): Promise<void>;
  planExportInfo(planId: string, sectionId: string | null): Promise<PlanExportInfo>;
  /** Guarda el plan (o una sección) en un .emetplan; null si se cancela el diálogo. */
  planExport(
    title: string,
    filterName: string,
    fileName: string,
    planId: string,
    sectionId: string | null,
    includeVideos: boolean,
    bibles: BibleRef[],
  ): Promise<PlanExportResult | null>;
  planImportPick(title: string, filterName: string): Promise<PlanImportPreview | null>;
  /** Como plan nuevo (`newName`) o al final de `appendTo`. */
  planImportApply(onConflict: OnConflict, importedSuffix: string, newName: string | null, appendTo: string | null): Promise<PlanImportResult>;
  planImportCancel(): Promise<void>;
  /** Guarda la biblioteca (o solo las letras de un plan) en un .emetpro; null si se cancela. */
  transferExport(title: string, filterName: string, fileName: string, planId: string | null): Promise<{ path: string; songs: number } | null>;

  // Transmisión con OBS
  obsDetect(): Promise<ObsDetection>;
  /** Activa el WebSocket de OBS en su configuración. Solo con OBS cerrado. */
  obsEnableWebsocket(): Promise<void>;
  obsLaunch(): Promise<void>;
  obsConnect(): Promise<void>;
  obsDisconnect(): Promise<void>;
  obsStatus(): Promise<ObsStatus>;
  /** Crea o completa las escenas de EmetPro en OBS. */
  obsSetup(waitText: string): Promise<void>;
  obsSetScene(scene: string): Promise<void>;
  /** Escribe el texto en esa fuente de OBS y la hace visible en la emisión. */
  obsShowText(source: string, text: string, credit: string): Promise<ObsTextShown>;
  /** Oculta esa fuente de texto en la escena al aire. */
  obsHideText(source: string): Promise<void>;
  obsCameras(): Promise<ObsCamera[]>;
  obsSetCamera(device: string): Promise<void>;
  /** Imagen (data URL) de lo que sale en la emisión. */
  obsPreview(): Promise<string | null>;
  /** Clave vacía = conservar la que ya tiene OBS. */
  obsSetDestination(server: string, key: string): Promise<void>;
  obsStartStream(): Promise<void>;
  obsStopStream(): Promise<void>;
}

export interface WhisperStatus {
  ready: boolean;
  busy: boolean;
  /** false si el procesador es demasiado antiguo: no se ofrece. */
  supported: boolean;
  /** Lo que ocupa la descarga, en bytes. */
  size: number;
}

export type WhisperProgress = { phase: "download"; done: number; total: number } | { phase: "transcribe"; percent: number };

/** Un tramo de lo que se oyó en la pista, tal cual lo da el reconocedor. */
export interface WhisperSegment {
  startMs: number;
  endMs: number;
  text: string;
  /** Probabilidad de que no haya voz (0 a 1). */
  noSpeech: number;
  /** Sus palabras con el tiempo de cada una (para sincronizar). */
  words?: { text: string; startMs: number; endMs: number }[];
}

export interface SeparatorStatus {
  /** Hay separador para este sistema (no lo hay para Mac con Intel). */
  available: boolean;
  ready: boolean;
  busy: boolean;
  /** Lo que ocupa la descarga, en bytes. */
  size: number;
}

export type SeparatorProgress = { phase: "download"; done: number; total: number };

export type PrepareProgress =
  | { phase: "reading" }
  | { phase: "separating"; done: number; total: number }
  | { phase: "saving" }
  | { phase: "listening"; percent: number };

export interface PreparedTrack {
  /** Lo que se oyó, con el tiempo de cada palabra (vacío si no se pidió oír). */
  segments: WhisperSegment[];
  /** La versión sin voz, si se pidió. */
  instrumental: AudioTrack | null;
  /** Se oyó la voz separada (más fiable que la mezcla). */
  cleanVoice: boolean;
  /** Dónde arranca a cantar la voz separada (ms). */
  onsets: number[];
}

// Media Studio (D-029): el único responsable de la pista. Suena en la consola, que está conectada al sonido de la
// iglesia, con Web Audio: la pista se decodifica entera y cada salto empieza un trozo nuevo en la muestra exacta,
// con un fundido corto para que no se oiga un golpe. El reloj es el del audio (AudioContext.currentTime); mientras
// suena se lee en cada cuadro con requestAnimationFrame, nunca con un setInterval.
import { platform } from "$lib/platform";

/** Fundido de cada salto, pausa o comienzo, en segundos: lo justo para que no chasquee. */
const FADE = 0.03;

export type PlayerStatus = "empty" | "loading" | "ready" | "error";

type Listener = (ms: number) => void;

class Player {
  status = $state<PlayerStatus>("empty");
  /** La pista cargada (su ruta en la carpeta de la app). */
  path = $state<string | null>(null);
  playing = $state(false);
  positionMs = $state(0);
  durationMs = $state(0);
  error = $state<string | null>(null);

  private ctx: AudioContext | null = null;
  private buffer: AudioBuffer | null = null;
  private source: AudioBufferSourceNode | null = null;
  private gain: GainNode | null = null;
  /** Dónde empezó a sonar el trozo actual: segundo de la pista y momento del reloj del audio. */
  private offset = 0;
  private startedAt = 0;
  private loading = 0;
  private frame = 0;
  private pulses = 0;
  private listeners = new Set<Listener>();

  /** Carga la pista (si no era la cargada). Devuelve su duración en ms, o null si falló o la reemplazó otra. */
  async load(path: string): Promise<number | null> {
    if (this.path === path && this.status === "ready") return this.durationMs;
    const token = ++this.loading;
    this.stopSource();
    this.playing = false;
    this.path = path;
    this.status = "loading";
    this.error = null;
    this.buffer = null;
    this.offset = 0;
    this.positionMs = 0;
    try {
      const bytes = await platform.studioReadAudio(path);
      this.ctx ??= new AudioContext();
      const buffer = await this.ctx.decodeAudioData(bytes);
      // Mientras se decodificaba se pidió otra pista: esta ya no sirve.
      if (token !== this.loading) return null;
      this.buffer = buffer;
      this.durationMs = Math.round(buffer.duration * 1000);
      this.status = "ready";
      return this.durationMs;
    } catch (e) {
      if (token !== this.loading) return null;
      this.status = "error";
      this.error = (e as { message?: string })?.message ?? String(e);
      platform.logError(`media studio: pista ${path}: ${this.error}`).catch(() => {});
      return null;
    }
  }

  /** Deja de sonar y libera la memoria de la pista (unos 80 MB por cada 4 minutos). */
  unload() {
    this.loading++;
    this.stopSource();
    this.playing = false;
    this.buffer = null;
    this.path = null;
    this.status = "empty";
    this.positionMs = 0;
    this.durationMs = 0;
    cancelAnimationFrame(this.frame);
  }

  /** Posición actual, del reloj del audio. */
  now(): number {
    if (!this.playing || !this.ctx) return this.offset * 1000;
    return Math.min(this.durationMs, (this.offset + this.ctx.currentTime - this.startedAt) * 1000);
  }

  async play() {
    if (!this.buffer || !this.ctx || this.playing) return;
    if (this.offset * 1000 >= this.durationMs - 50) this.offset = 0;
    await this.ctx.resume();
    this.startSource(this.offset);
    this.playing = true;
    this.tick();
    this.pulse(++this.pulses);
  }

  pause() {
    if (!this.playing) return;
    this.offset = this.now() / 1000;
    this.stopSource();
    this.playing = false;
    this.positionMs = this.offset * 1000;
    this.emit();
  }

  toggle() {
    if (this.playing) this.pause();
    else this.play();
  }

  /** Salta a ms. Si sonaba, sigue sonando desde ahí; si estaba en pausa, sigue en pausa (seek-preserve). */
  seek(ms: number) {
    if (!this.buffer) return;
    this.offset = Math.max(0, Math.min(ms, this.durationMs)) / 1000;
    this.positionMs = this.offset * 1000;
    if (this.playing) this.startSource(this.offset);
    this.emit();
  }

  /** Avisa de la posición en cada cuadro mientras suena (y tras cada salto o pausa). */
  subscribe(listener: Listener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private emit() {
    for (const listener of this.listeners) listener(this.positionMs);
  }

  private tick = () => {
    cancelAnimationFrame(this.frame);
    if (!this.playing) return;
    this.positionMs = this.now();
    this.emit();
    this.frame = requestAnimationFrame(this.tick);
  };

  /**
   * Un aviso cada 100 ms de pista que da el propio audio: un nodo silencioso que termina y se vuelve a programar.
   * requestAnimationFrame se detiene si la ventana de la consola queda tapada (la salida a pantalla completa en el
   * mismo monitor) o minimizada; el audio no, así el avance automático sigue.
   */
  private pulse(chain: number) {
    const ctx = this.ctx;
    // Solo una cadena a la vez: pausar y reanudar enseguida no deja la anterior viva.
    if (!ctx || !this.playing || chain !== this.pulses) return;
    const node = ctx.createConstantSource();
    const silent = ctx.createGain();
    silent.gain.value = 0;
    node.connect(silent).connect(ctx.destination);
    node.onended = () => {
      node.disconnect();
      silent.disconnect();
      if (!this.playing || chain !== this.pulses) return;
      this.positionMs = this.now();
      this.emit();
      this.pulse(chain);
    };
    node.start();
    node.stop(ctx.currentTime + 0.1);
  }

  /** Empieza un trozo nuevo en ese segundo, con fundido de entrada; el anterior se apaga con fundido de salida. */
  private startSource(offset: number) {
    const ctx = this.ctx!;
    this.stopSource();
    const gain = ctx.createGain();
    gain.gain.setValueAtTime(0, ctx.currentTime);
    gain.gain.linearRampToValueAtTime(1, ctx.currentTime + FADE);
    gain.connect(ctx.destination);
    const source = ctx.createBufferSource();
    source.buffer = this.buffer;
    source.connect(gain);
    source.onended = () => {
      // Solo cuenta el final del trozo que sigue sonando (los saltos paran el anterior, que también avisa).
      if (source !== this.source) return;
      this.source = null;
      this.playing = false;
      this.offset = this.durationMs / 1000;
      this.positionMs = this.durationMs;
      this.emit();
    };
    source.start(0, offset);
    this.source = source;
    this.gain = gain;
    this.offset = offset;
    this.startedAt = ctx.currentTime;
  }

  private stopSource() {
    const source = this.source;
    const gain = this.gain;
    this.source = null;
    this.gain = null;
    if (!source || !gain || !this.ctx) return;
    const at = this.ctx.currentTime;
    gain.gain.cancelScheduledValues(at);
    gain.gain.setValueAtTime(gain.gain.value, at);
    gain.gain.linearRampToValueAtTime(0, at + FADE);
    try {
      source.stop(at + FADE);
    } catch {
      // Ya había terminado.
    }
  }
}

export const player = new Player();

// Media Studio (D-029): el único responsable de la pista. Suena en la consola, que está conectada al sonido de la
// iglesia, con Web Audio: la pista se decodifica entera y cada salto empieza un trozo nuevo en la muestra exacta,
// con un fundido corto para que no se oiga un golpe. El reloj es el del audio (AudioContext.currentTime); mientras
// suena se lee en cada cuadro con requestAnimationFrame, nunca con un setInterval.
import { platform } from "$lib/platform";

/** Fundido de cada salto, en segundos: lo justo para que no chasquee. */
const FADE = 0.03;
/** Al reproducir y al pausar: la música entra y sale como con un músico, no de golpe. */
const SOFT = 0.25;
/** De fondo: el volumen mientras en pantalla hay otra cosa (la cita, un anuncio), como una banda que acompaña bajito. */
const BACKGROUND = 0.3;
/** Al pasar a fondo y al volver: un cambio de volumen que no se note como un corte. */
const SOFTEN = 1.5;
/** «Terminar»: el final de la canción baja despacio, como cuando la banda termina. */
const FINISH = 4;

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

  /** Terminando con el fundido largo. */
  finishing = $state(false);
  /**
   * Velocidad (1 = la real). Solo para sincronizar con calma: el editor la baja y la vuelve a 1 al cerrarse, y la
   * canción en pantalla siempre suena a 1. Los tiempos siguen siendo los de la pista.
   */
  rate = $state(1);
  /** Suena de fondo, más suave, mientras en pantalla hay otra cosa. */
  background = $state(false);

  private ctx: AudioContext | null = null;
  /** Volumen general: los fundidos de reproducir, pausar y terminar (cada trozo tiene el suyo para los saltos). */
  private master: GainNode | null = null;
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
      const buffer = await this.context().decodeAudioData(bytes);
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

  private context(): AudioContext {
    if (!this.ctx) {
      this.ctx = new AudioContext();
      this.master = this.ctx.createGain();
      this.master.connect(this.ctx.destination);
    }
    return this.ctx;
  }

  /**
   * Con un clic del operador: el sistema solo deja sonar audio que se pidió con un gesto. Después sigue
   * habilitado, así la pista puede arrancar sola al enviarla o desde el teléfono.
   */
  unlock() {
    this.context()
      .resume()
      .catch(() => {});
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
    return Math.min(this.durationMs, (this.offset + (this.ctx.currentTime - this.startedAt) * this.rate) * 1000);
  }

  async play() {
    if (!this.buffer || !this.ctx || this.playing) return;
    if (this.offset * 1000 >= this.durationMs - 50) this.offset = 0;
    await this.ctx.resume();
    this.ramp(this.level(), SOFT, 0);
    this.startSource(this.offset);
    this.playing = true;
    this.tick();
    this.pulse(++this.pulses);
  }

  pause() {
    if (!this.playing) return;
    this.offset = this.now() / 1000;
    this.finishing = false;
    // Baja en un cuarto de segundo y para: el trozo se detiene cuando ya no se oye.
    this.ramp(0, SOFT);
    this.stopSource(SOFT);
    this.playing = false;
    this.positionMs = this.offset * 1000;
    this.emit();
  }

  /** Termina la canción con un fundido de unos segundos y queda en pausa allí. */
  finish() {
    const ctx = this.ctx;
    if (!ctx || !this.playing || this.finishing) return;
    this.finishing = true;
    this.ramp(0, FINISH);
    // Lo avisa el propio audio al acabar el fundido (un temporizador se frenaría con la consola tapada).
    const done = ctx.createConstantSource();
    const silent = ctx.createGain();
    silent.gain.value = 0;
    done.connect(silent).connect(ctx.destination);
    done.onended = () => {
      done.disconnect();
      silent.disconnect();
      if (this.finishing) this.pause();
    };
    done.start();
    done.stop(ctx.currentTime + FINISH);
  }

  /** El volumen general va a `to` en `seconds`, empezando en `from` si se da. */
  private ramp(to: number, seconds: number, from?: number) {
    const ctx = this.ctx;
    const gain = this.master?.gain;
    if (!ctx || !gain) return;
    const at = ctx.currentTime;
    gain.cancelScheduledValues(at);
    gain.setValueAtTime(from ?? gain.value, at);
    gain.linearRampToValueAtTime(to, at + seconds);
  }

  /** El volumen normal: el de fondo o el completo. */
  private level(): number {
    return this.background ? BACKGROUND : 1;
  }

  /** Más suave (de fondo) o al volumen completo, con un cambio gradual. */
  setBackground(on: boolean) {
    if (on === this.background) return;
    this.background = on;
    if (this.playing && !this.finishing) this.ramp(this.level(), SOFTEN);
  }

  /** Cambia la velocidad sin cortar: sigue desde donde iba. */
  setRate(rate: number) {
    rate = Math.max(0.5, Math.min(1, rate));
    if (rate === this.rate) return;
    if (this.playing) {
      const at = this.now() / 1000;
      this.rate = rate;
      this.startSource(at);
    } else {
      this.rate = rate;
    }
  }

  /**
   * La forma de la pista: el volumen medio (RMS) de cada uno de `bins` tramos, de 0 a 1, para dibujar la onda en
   * el editor. El medio y no el pico: en la música grabada hoy el pico toca el máximo casi siempre y la onda sería
   * un bloque; el medio deja ver dónde baja y sube (los cambios de sección). null si no hay pista cargada.
   */
  peaks(bins: number): Float32Array | null {
    const buffer = this.buffer;
    if (!buffer || bins < 1) return null;
    const out = new Float32Array(bins);
    const per = buffer.length / bins;
    // Saltear muestras: una pista de 4 minutos tiene 10 millones; con unas 2000 por tramo la forma no cambia.
    const stride = Math.max(1, Math.floor(per / 2000));
    for (let c = 0; c < buffer.numberOfChannels; c++) {
      const data = buffer.getChannelData(c);
      for (let b = 0; b < bins; b++) {
        const end = Math.min(data.length, Math.floor((b + 1) * per));
        let sum = 0;
        let count = 0;
        for (let i = Math.floor(b * per); i < end; i += stride) {
          sum += data[i] * data[i];
          count++;
        }
        out[b] += count ? sum / count : 0;
      }
    }
    for (let b = 0; b < bins; b++) out[b] = Math.sqrt(out[b] / buffer.numberOfChannels);
    const max = out.reduce((m, v) => Math.max(m, v), 0) || 1;
    return out.map((v) => v / max);
  }

  toggle() {
    if (this.playing) this.pause();
    else this.play();
  }

  /**
   * Salta a ms. Si sonaba, sigue sonando desde ahí; si estaba en pausa, sigue en pausa (seek-preserve).
   * `automatic`: un salto de la propia canción (repetir, orden propio), no del operador.
   */
  seek(ms: number, automatic = false) {
    if (!this.buffer) return;
    this.offset = Math.max(0, Math.min(ms, this.durationMs)) / 1000;
    this.positionMs = this.offset * 1000;
    // Que el operador salte durante el fundido final es seguir cantando: vuelve el volumen.
    if (this.finishing && !automatic) {
      this.finishing = false;
      this.ramp(this.level(), SOFT);
    }
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
    source.playbackRate.value = this.rate;
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

  /** Apaga el trozo que suena con un fundido (el de los saltos, o `fade` segundos). */
  private stopSource(fade = FADE) {
    const source = this.source;
    const gain = this.gain;
    this.source = null;
    this.gain = null;
    if (!source || !gain || !this.ctx) return;
    const at = this.ctx.currentTime;
    gain.gain.cancelScheduledValues(at);
    gain.gain.setValueAtTime(gain.gain.value, at);
    gain.gain.linearRampToValueAtTime(0, at + fade);
    try {
      source.stop(at + fade);
    } catch {
      // Ya había terminado.
    }
  }
}

export const player = new Player();

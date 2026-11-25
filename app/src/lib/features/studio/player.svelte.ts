// Media Studio (D-029): el único responsable de la pista. Suena en la consola, que está conectada al sonido de la
// iglesia, con Web Audio: la pista se decodifica entera y cada salto empieza un trozo nuevo en la muestra exacta,
// con un fundido corto para que no se oiga un golpe. El reloj es el del audio (AudioContext.currentTime); mientras
// suena se lee en cada cuadro con requestAnimationFrame, nunca con un setInterval.
import { platform } from "$lib/platform";
import type { StretchNode } from "signalsmith-stretch";

/** Fundido mínimo, en segundos: lo justo para que no chasquee (arrastrar la barra, cambiar la velocidad). */
const FADE = 0.03;
/**
 * Saltos que pide el operador (otra diapositiva, otra sección, la onda): fundido cruzado como el de un músico que
 * pasa de una parte a otra. Con 30 ms se oía como un corte y volver a empezar.
 */
const JUMP = 0.6;
/** Saltos automáticos del orden propio o de repetir la sección: corto, para no correr el ritmo. */
const JUMP_AUTO = 0.08;
/** Con voz ↔ sin voz: el mismo punto de la misma canción, así que se funden sin notarse. */
const SWITCH = 0.25;

/** Cómo se salta: lo pide el operador, lo hace la app sola, o se arrastra la barra (muchos saltos seguidos). */
export type SeekKind = "jump" | "auto" | "scrub";

/** Curva de igual potencia (seno/coseno): al cruzar dos fundidos el volumen no se hunde a la mitad. */
function powerCurve(from: number, to: number, steps = 64): Float32Array {
  return Float32Array.from({ length: steps }, (_, i) => {
    const x = i / (steps - 1);
    return from + (to - from) * (to > from ? Math.sin((x * Math.PI) / 2) : 1 - Math.cos((x * Math.PI) / 2));
  });
}

/**
 * Lleva el volumen de un trozo a `to` en `seconds` desde ahora, con la curva de igual potencia. Si el navegador no
 * acepta la curva (otra recién programada que se solapa, al saltar dos veces seguidas), lo hace en línea recta.
 */
export function fadeTo(param: AudioParam, to: number, at: number, seconds: number) {
  const from = param.value;
  param.cancelScheduledValues(at);
  try {
    param.setValueCurveAtTime(powerCurve(from, to), at, seconds);
  } catch {
    param.cancelScheduledValues(at);
    param.setValueAtTime(from, at);
    param.linearRampToValueAtTime(to, at + seconds);
  }
}
/** Al reproducir y al pausar: la música entra y sale como con un músico, no de golpe. */
const SOFT = 0.25;
/** De fondo: el volumen mientras en pantalla hay otra cosa (la cita, un anuncio), como una banda que acompaña bajito. */
const BACKGROUND = 0.3;
/** Al pasar a fondo y al volver: un cambio de volumen que no se note como un corte. */
const SOFTEN = 1.5;
/** «Terminar»: el final de la canción baja despacio, como cuando la banda termina. */
const FINISH = 4;

/** Cuánto se puede subir o bajar el tono: más se oye artificial. */
export const TRANSPOSE_MAX = 6;

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
  /** Semitonos de más o de menos (el tono de la canción, para la voz de quien dirige). 0 = el original. */
  transpose = $state(0);
  /** El cambio de tono no está disponible en este equipo (sin AudioWorklet o WebAssembly): suena el original. */
  shiftError = $state<string | null>(null);

  private ctx: AudioContext | null = null;
  /** Volumen general: los fundidos de reproducir, pausar y terminar (cada trozo tiene el suyo para los saltos). */
  private master: GainNode | null = null;
  /**
   * Por donde entran los trozos: va directo al volumen general o, si hay que cambiar el tono (o compensar el de
   * marcar más despacio), pasa antes por el cambiador. Sin cambio no lo atraviesa: el sonido original, sin retraso.
   */
  private bus: GainNode | null = null;
  private shifter: StretchNode | null = null;
  private shifterLoad: Promise<StretchNode | null> | null = null;
  /** Lo que retrasa el cambiador, en segundos (para que el reloj diga lo que se oye). */
  private latency = 0;
  private routed = false;
  private buffer: AudioBuffer | null = null;
  /**
   * La otra versión de la canción ya decodificada (con o sin voz), para cambiar en el mismo punto sin silencio.
   * Solo una: cada pista decodificada ocupa unos 80 MB cada 4 minutos.
   */
  private spare = new Map<string, AudioBuffer>();
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
    const kept = this.spare.get(path);
    this.spare.clear();
    try {
      const buffer = kept ?? (await this.decode(path));
      // Mientras se decodificaba se pidió otra pista: esta ya no sirve.
      if (token !== this.loading) return null;
      this.buffer = buffer;
      this.durationMs = Math.round(buffer.duration * 1000);
      this.status = "ready";
      return this.durationMs;
    } catch (e) {
      if (token !== this.loading) return null;
      this.status = "error";
      // Los errores de los comandos son objetos con su código (fileMissing…): se registran enteros.
      const err = e as { message?: string; code?: string };
      this.error = err?.message ?? (err?.code ? JSON.stringify(e) : String(e));
      platform.logError(`media studio: pista ${path}: ${this.error}`).catch(() => {});
      return null;
    }
  }

  private async decode(path: string): Promise<AudioBuffer> {
    const bytes = await platform.studioReadAudio(path);
    return this.context().decodeAudioData(bytes);
  }

  /** Decodifica de antemano la otra versión de la canción cargada (con o sin voz). */
  async preload(path: string) {
    if (this.path === path || this.spare.has(path)) return;
    const token = this.loading;
    try {
      const buffer = await this.decode(path);
      // Si mientras tanto se cargó otra canción, ya no sirve.
      if (token !== this.loading) return;
      this.spare.clear();
      this.spare.set(path, buffer);
    } catch (e) {
      platform.logError(`media studio: pista ${path}: ${(e as { message?: string })?.message ?? e}`).catch(() => {});
    }
  }

  /**
   * Cambia a la otra versión de la misma canción (con o sin voz) en el mismo punto: si suena, con un fundido
   * cruzado corto, sin silencio. Devuelve false si no se pudo (y sigue la que sonaba).
   */
  async switchTo(path: string): Promise<boolean> {
    if (this.path === path) return true;
    if (!this.buffer || !this.path || this.status !== "ready") return (await this.load(path)) !== null;
    const token = this.loading;
    let buffer = this.spare.get(path);
    if (!buffer) {
      try {
        buffer = await this.decode(path);
      } catch (e) {
        platform.logError(`media studio: pista ${path}: ${(e as { message?: string })?.message ?? e}`).catch(() => {});
        return false;
      }
      if (token !== this.loading) return false;
    }
    // La que sonaba queda de reserva para volver a ella al momento.
    this.spare.clear();
    this.spare.set(this.path, this.buffer);
    const at = this.now() / 1000;
    this.buffer = buffer;
    this.path = path;
    this.durationMs = Math.round(buffer.duration * 1000);
    if (this.playing) this.startSource(Math.min(at, buffer.duration), SWITCH);
    else this.offset = Math.min(at, buffer.duration);
    return true;
  }

  private context(): AudioContext {
    if (!this.ctx) {
      this.ctx = new AudioContext();
      this.master = this.ctx.createGain();
      this.master.connect(this.ctx.destination);
      this.bus = this.ctx.createGain();
      this.bus.connect(this.master);
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
    this.spare.clear();
    this.path = null;
    this.status = "empty";
    this.positionMs = 0;
    this.durationMs = 0;
    cancelAnimationFrame(this.frame);
  }

  /** Posición actual, del reloj del audio. */
  now(): number {
    if (!this.playing || !this.ctx) return this.offset * 1000;
    // Con el cambiador, lo que se oye va un poco por detrás de lo que se le da.
    const delay = this.routed ? this.latency : 0;
    return Math.min(this.durationMs, (this.offset + Math.max(0, this.ctx.currentTime - this.startedAt - delay) * this.rate) * 1000);
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
    this.applyShift();
  }

  /** Sube o baja el tono (semitonos, de −6 a +6), sin cambiar la velocidad. */
  setTranspose(semitones: number) {
    const n = Math.max(-TRANSPOSE_MAX, Math.min(TRANSPOSE_MAX, Math.round(semitones) || 0));
    if (n === this.transpose) return;
    this.transpose = n;
    this.applyShift();
  }

  /** Semitonos que aplica el cambiador: el tono elegido, y lo que baja la música al ir más despacio. */
  private shift(): number {
    return this.transpose - (this.rate === 1 ? 0 : 12 * Math.log2(this.rate));
  }

  /** Pone o quita el cambiador según haga falta. La primera vez lo carga (unos cientos de KB). */
  private applyShift() {
    const semitones = this.shift();
    const need = Math.abs(semitones) > 0.01;
    if (need && !this.shifter && !this.shiftError) {
      // Cargado (o fallido, y entonces suena el original), se vuelve a aplicar con lo que se pida para entonces.
      this.loadShifter().then(() => this.applyShift());
      return;
    }
    const shifter = this.shifter;
    if (shifter) {
      // Formantes sin compensar: sobre una mezcla entera la compensación emborrona los instrumentos.
      shifter.schedule({ active: need, semitones, formantCompensation: false });
    }
    this.route(need && shifter !== null);
  }

  private route(through: boolean) {
    if (!this.bus || !this.master || through === this.routed) return;
    // El reloj (now) ya cuenta el retraso del cambiador mientras se pasa por él.
    this.bus.disconnect();
    this.bus.connect(through && this.shifter ? this.shifter : this.master);
    this.routed = through;
  }

  private loadShifter(): Promise<StretchNode | null> {
    this.shifterLoad ??= (async () => {
      const ctx = this.context();
      try {
        const { default: SignalsmithStretch } = await import("signalsmith-stretch");
        // Si el equipo no deja cargar el procesador, la promesa no termina nunca: se le pone un límite.
        const node = await Promise.race([
          SignalsmithStretch(ctx),
          new Promise<never>((_, reject) => setTimeout(() => reject(new Error("tiempo agotado")), 8000)),
        ]);
        this.latency = await node.latency();
        node.connect(this.master!);
        this.shifter = node;
        this.shiftError = null;
        return node;
      } catch (e) {
        this.shiftError = (e as { message?: string })?.message ?? String(e);
        platform.logError(`media studio: cambio de tono: ${this.shiftError}`).catch(() => {});
        return null;
      }
    })();
    return this.shifterLoad;
  }

  /**
   * La forma de la pista: el volumen medio (RMS) de cada uno de `bins` tramos, de 0 a 1, para dibujar la onda en
   * el editor. El medio y no el pico: en la música grabada hoy el pico toca el máximo casi siempre y la onda sería
   * un bloque; el medio deja ver dónde baja y sube (los cambios de sección). null si no hay pista cargada.
   */
  /** La pista en mono a unas `rate` muestras por segundo, para analizarla (la tonalidad), con la frecuencia que
   *  quedó de verdad (el camino de reserva no da exactamente `rate`: 48 kHz / 4 = 12 kHz). null si no hay pista. */
  async mono(rate: number): Promise<{ samples: Float32Array; rate: number } | null> {
    const buffer = this.buffer;
    if (!buffer) return null;
    try {
      // El navegador remuestrea con su filtro (sin los armónicos falsos de quitar muestras sin más).
      const offline = new OfflineAudioContext(1, Math.ceil(buffer.duration * rate), rate);
      const source = offline.createBufferSource();
      source.buffer = buffer;
      source.connect(offline.destination);
      source.start();
      const out = (await offline.startRendering()).getChannelData(0);
      if (out.some((v) => v !== 0)) return { samples: out, rate };
    } catch {
      // Abajo, a mano.
    }
    // Si el render no sirvió (algún WebKit devuelve silencio): media de cada grupo de muestras.
    const step = Math.max(1, Math.round(buffer.sampleRate / rate));
    const out = new Float32Array(Math.floor(buffer.length / step));
    for (let c = 0; c < buffer.numberOfChannels; c++) {
      const data = buffer.getChannelData(c);
      for (let i = 0; i < out.length; i++) {
        let sum = 0;
        for (let j = 0; j < step; j++) sum += data[i * step + j];
        out[i] += sum / step / buffer.numberOfChannels;
      }
    }
    return { samples: out, rate: buffer.sampleRate / step };
  }

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
  seek(ms: number, kind: SeekKind = "jump") {
    if (!this.buffer) return;
    this.offset = Math.max(0, Math.min(ms, this.durationMs)) / 1000;
    this.positionMs = this.offset * 1000;
    // Que el operador salte durante el fundido final es seguir cantando: vuelve el volumen.
    if (this.finishing && kind !== "auto") {
      this.finishing = false;
      this.ramp(this.level(), SOFT);
    }
    if (this.playing) this.startSource(this.offset, kind === "jump" ? JUMP : kind === "auto" ? JUMP_AUTO : FADE);
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

  /**
   * Empieza un trozo nuevo en ese segundo; el anterior se apaga a la vez (fundido cruzado de `fade` segundos, con
   * curvas de igual potencia).
   */
  private startSource(offset: number, fade = FADE) {
    const ctx = this.ctx!;
    this.stopSource(fade);
    const gain = ctx.createGain();
    gain.gain.value = 0;
    fadeTo(gain.gain, 1, ctx.currentTime, fade);
    // Al bus, y de ahí al volumen general: así suenan los fundidos, «de fondo» y el cambio de tono.
    gain.connect(this.bus!);
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
    fadeTo(gain.gain, 0, at, fade);
    try {
      source.stop(at + fade);
    } catch {
      // Ya había terminado.
    }
  }
}

export const player = new Player();

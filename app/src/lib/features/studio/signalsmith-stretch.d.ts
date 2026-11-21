// Tipos mínimos de signalsmith-stretch (MIT), que no trae los suyos: solo lo que usa player.svelte.ts.
declare module "signalsmith-stretch" {
  export interface StretchNode extends AudioNode {
    schedule(options: { active?: boolean; semitones?: number; formantCompensation?: boolean; output?: number }): void;
    latency(): Promise<number>;
  }
  export default function SignalsmithStretch(context: BaseAudioContext, options?: AudioWorkletNodeOptions): Promise<StretchNode>;
}

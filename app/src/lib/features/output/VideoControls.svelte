<script lang="ts">
  // Lo que se puede hacer con un video en pantalla desde la consola: pausar, silenciar y bajar el volumen.
  // Por dónde va lo dice la ventana de salida.
  import { t } from "$lib/i18n/index.svelte";
  import type { OutputContent, VideoTime } from "$lib/platform";

  type Video = Extract<OutputContent, { kind: "video" }>;

  let {
    content,
    time,
    disabled = false,
    onchange,
  }: {
    content: Video;
    time: (VideoTime & { at: number }) | null;
    disabled?: boolean;
    onchange: (content: Video) => void;
  } = $props();

  let volume = $derived(Math.round((content.volume ?? 1) * 100));
  let muted = $derived(content.muted ?? false);
  let current = $derived(time && time.path === content.path ? time : null);

  function clock(seconds: number): string {
    const s = Math.max(0, Math.floor(seconds));
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const rest = String(s % 60).padStart(2, "0");
    return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${rest}` : `${m}:${rest}`;
  }

  function setVolume(value: number) {
    // Subir el volumen de un video silenciado le devuelve el sonido.
    onchange({ ...content, volume: value / 100, muted: value === 0 });
  }
</script>

<div class="video">
  {#if current && current.duration > 0}
    <div class="progress" role="progressbar" aria-label={t("live.videoPosition")} aria-valuemin={0} aria-valuemax={Math.round(current.duration)} aria-valuenow={Math.round(current.time)}>
      <span style:width="{Math.min(100, (current.time / current.duration) * 100)}%"></span>
    </div>
    <span class="time">{clock(current.time)} / {clock(current.duration)}</span>
  {/if}
  <div class="row">
    <button class="btn" onclick={() => onchange({ ...content, paused: !content.paused })} {disabled}>
      {t(content.paused ? "live.resume" : "live.pause")}
    </button>
    <button class="btn" aria-pressed={muted} onclick={() => onchange({ ...content, muted: !muted })} {disabled}>
      {muted ? "🔇" : "🔊"} {t(muted ? "live.unmute" : "live.mute")}
    </button>
    <label class="volume">
      <span class="muted">{t("live.volume")}</span>
      <input type="range" min="0" max="100" step="5" value={muted ? 0 : volume} oninput={(e) => setVolume(Number(e.currentTarget.value))} {disabled} />
      <span class="pct">{muted ? 0 : volume}%</span>
    </label>
  </div>
</div>

<style>
  .video { display: flex; flex-direction: column; gap: 6px; width: 100%; }
  .row { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }
  .progress { height: 6px; border-radius: 3px; background: var(--border); overflow: hidden; }
  .progress span { display: block; height: 100%; background: var(--live); }
  .time { font-size: 13px; font-variant-numeric: tabular-nums; color: var(--muted); }
  .volume { display: inline-flex; align-items: center; gap: 8px; flex: 1; min-width: 200px; }
  .volume input { flex: 1; accent-color: var(--accent); }
  .pct { width: 4ch; text-align: right; font-size: 13px; font-variant-numeric: tabular-nums; }
</style>

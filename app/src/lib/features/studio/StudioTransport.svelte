<script lang="ts">
  // «En pantalla» con una canción de Media Studio (D-029): la pista (▶/⏸ y por dónde va) y una fila de secciones
  // para saltar con un clic siguiendo de oído a los cantantes. Con pista, la música va detrás de lo que se elija.
  import { onMount, untrack } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import type { OutputContent } from "$lib/platform";
  import { studio, loadStudio } from "./library.svelte";
  import { attach, finishSong, follow, goTo, onScreen, sectionTarget, sectionsOnScreen, synced, toggleLoop, trackReady } from "./live.svelte";
  import { player } from "./player.svelte";
  import { sectionName } from "./names";
  import BehindTrack from "./BehindTrack.svelte";
  import ToneControl from "./ToneControl.svelte";
  import { sectionOf } from "./song";
  import { formatTime } from "./timeline";

  let {
    content,
    disabled = false,
    onshow,
  }: {
    content: OutputContent;
    disabled?: boolean;
    /** Muestra la diapositiva n (desde 1) tal cual, sin tocar la pista. */
    onshow: (n: number) => boolean;
  } = $props();

  onMount(() => {
    if (!studio.loaded) loadStudio();
    return attach((n) => onshow(n));
  });

  // Sigue lo que está en pantalla, y la canción misma si se editó o acaba de cargarse la biblioteca.
  $effect(() => {
    const current = content;
    if (current.kind === "text" && current.studio) void studio.songs.find((s) => s.id === current.studio);
    untrack(() => follow(current));
  });

  let song = $derived(onScreen.song);
  let sections = $derived(song ? (void onScreen.index, sectionsOnScreen()) : []);
  let withTrack = $derived(song?.mode === "track" && song.audio !== null);

  function jump(sectionId: string) {
    const n = sectionTarget(sectionId);
    if (n === null) return;
    if (!goTo(content, n)) onshow(n);
  }
</script>

{#if song}
  <div class="studio-live">
    {#if withTrack}
      <div class="transport">
        {#if player.status === "loading" && player.path === song.audio?.path}
          <span class="muted small">{t("studio.loadingTrack")}</span>
        {:else if player.status === "error" && player.path === song.audio?.path}
          <span class="error small" role="alert">{t("studio.trackError", { error: player.error ?? "" })}</span>
        {:else if trackReady()}
          <button class="btn primary play" onclick={() => player.toggle()} aria-label={player.playing ? t("studio.pause") : t("studio.play")}>
            {player.playing ? "⏸" : "▶"} <span>{player.playing ? t("studio.pause") : t("studio.play")}</span>
          </button>
          <span class="time" aria-live="off">{formatTime(player.positionMs)} / {formatTime(player.durationMs)}</span>
          <input
            class="bar"
            type="range"
            min="0"
            max={player.durationMs}
            step="100"
            value={player.positionMs}
            aria-label={t("studio.position")}
            onchange={(e) => player.seek(Number(e.currentTarget.value))}
          />
        {/if}
      </div>
      {#if trackReady()}
        <div class="transport">
          <!-- Como una banda que se queda en el coro mientras se ora: al terminar la sección vuelve a su comienzo. -->
          <button class="btn" aria-pressed={onScreen.loop} onclick={toggleLoop} title={t("studio.loopHint")}>🔁 {t("studio.loop")}</button>
          <button class="btn" onclick={finishSong} disabled={!player.playing || player.finishing} title={t("studio.finishHint")}>
            {player.finishing ? t("studio.finishing") : t("studio.finish")}
          </button>
        </div>
        <!-- Si quien dirige pide otro tono en el momento: dura hasta la próxima canción (el guardado, en el editor). -->
        <ToneControl value={player.transpose} songKey={onScreen.song?.key} onchange={(n) => player.setTranspose(n)} />
      {/if}
      {#if trackReady() && !synced()}
        <!-- Sin tiempos la pista no tiene a dónde saltar: que se note, no que parezca que falla. -->
        <p class="notice small" role="status">{t("studio.notSynced")}</p>
      {/if}
    {:else}
      <p class="muted small">{t("studio.liveHint")}</p>
    {/if}
    <div class="sections" role="group" aria-label={t("studio.sections")}>
      {#each sections as s (s.id)}
        {@const section = sectionOf(song, s.id)}
        {#if section}
          <button class="btn section" aria-pressed={s.current} {disabled} onclick={() => jump(s.id)}>
            {#if s.current}<span class="dot" aria-hidden="true"></span>{/if}{sectionName(song, section)}
          </button>
        {/if}
      {/each}
    </div>
  </div>
{:else}
  <!-- Se puso otra cosa en pantalla mientras sonaba la canción: terminarla o dejarla de fondo. -->
  <BehindTrack />
{/if}

<style>
  .studio-live { display: flex; flex-direction: column; gap: 8px; }
  .transport { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
  .play { min-height: 48px; min-width: 120px; font-size: 16px; gap: 8px; }
  .time { font-variant-numeric: tabular-nums; font-size: 14px; }
  .bar { flex: 1; min-width: 160px; accent-color: var(--live); }
  .sections { display: flex; flex-wrap: wrap; gap: 6px; }
  .section { min-height: 44px; padding: 0 14px; gap: 6px; }
  .section[aria-pressed="true"], .transport .btn[aria-pressed="true"] { border-color: var(--live); background: var(--live-soft); font-weight: 700; }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--live); }
  .error { color: var(--live); }
  .notice { margin: 0; padding: 6px 10px; border-radius: 8px; background: var(--warn-bg); color: var(--warn-text); }
</style>

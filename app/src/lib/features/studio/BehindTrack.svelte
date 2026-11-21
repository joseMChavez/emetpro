<script lang="ts">
  // La canción de Media Studio que siguió sonando al poner otra cosa en pantalla (D-029). Primero pregunta:
  // terminarla con el fundido o dejarla de fondo, más suave. Después quedan sus controles, también en la barra de
  // arriba (`compact`), para pararla desde cualquier pestaña.
  import { t } from "$lib/i18n/index.svelte";
  import { behind, dismissBehind, finishSong, keepInBackground, toggleLoop } from "./live.svelte";
  import { player } from "./player.svelte";

  let { compact = false }: { compact?: boolean } = $props();
</script>

{#if behind.title !== null}
  {#if compact}
    <div class="pill" class:asking={behind.asking} role="status">
      <span class="note" aria-hidden="true">♪</span>
      <span class="title">{t(player.background ? "studio.behind.soft" : "studio.behind.playing", { title: behind.title })}</span>
      <button class="mini" onclick={() => player.toggle()} aria-label={player.playing ? t("studio.pause") : t("studio.play")} title={player.playing ? t("studio.pause") : t("studio.play")}>{player.playing ? "⏸" : "▶"}</button>
      <button class="mini" onclick={finishSong} disabled={!player.playing || player.finishing} title={t("studio.finishHint")}>{player.finishing ? t("studio.finishing") : t("studio.finish")}</button>
    </div>
  {:else}
    <div class="behind" role="status">
      {#if behind.asking}
        <p class="question">{t("studio.behind.question", { title: behind.title })}</p>
        <div class="row">
          <button class="btn primary" onclick={finishSong}>{t("studio.behind.finish")}</button>
          <button class="btn" onclick={keepInBackground}>{t("studio.behind.background")}</button>
        </div>
      {:else}
        <p class="question">{t(player.background ? "studio.behind.soft" : "studio.behind.playing", { title: behind.title })}</p>
        <div class="row">
          <button class="btn" onclick={() => player.toggle()}>{player.playing ? `⏸ ${t("studio.pause")}` : `▶ ${t("studio.play")}`}</button>
          <button class="btn" onclick={finishSong} disabled={!player.playing || player.finishing} title={t("studio.finishHint")}>{player.finishing ? t("studio.finishing") : t("studio.finish")}</button>
          <button class="btn" onclick={() => player.setBackground(!player.background)}>{t(player.background ? "studio.behind.louder" : "studio.behind.softer")}</button>
          {#if behind.range}
            <button class="btn" aria-pressed={behind.loop} onclick={toggleLoop} title={t("studio.loopHint")}>🔁 {t("studio.loop")}</button>
          {/if}
          {#if !player.playing}<button class="btn ghost" onclick={dismissBehind}>{t("studio.behind.dismiss")}</button>{/if}
        </div>
      {/if}
      <p class="muted small">{t("studio.behind.hint")}</p>
    </div>
  {/if}
{/if}

<style>
  .behind { display: flex; flex-direction: column; gap: 8px; padding: 10px 12px; border-radius: 10px; border: 2px solid var(--live); background: var(--live-soft); }
  .question { margin: 0; font-weight: 700; }
  .row { display: flex; flex-wrap: wrap; gap: 8px; }
  .row .btn { min-height: 44px; }
  .row .btn[aria-pressed="true"] { border-color: var(--live); font-weight: 700; }
  .small { margin: 0; }
  .pill { display: flex; align-items: center; gap: 4px; min-width: 0; max-width: 26em; padding: 2px 4px 2px 10px; border-radius: 999px; border: 1px solid var(--live); background: var(--live-soft); font-size: 13px; font-weight: 600; }
  .pill.asking { animation: pulse 1.6s ease-in-out infinite; }
  .note { color: var(--live); }
  .title { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .mini { flex: none; min-height: 28px; padding: 0 8px; font: inherit; font-size: 12px; color: var(--text); background: var(--surface); border: 1px solid var(--border); border-radius: 999px; cursor: pointer; }
  .mini:disabled { opacity: 0.5; cursor: default; }
  @keyframes pulse { 50% { box-shadow: 0 0 0 4px var(--live-soft); } }
  @media (prefers-reduced-motion: reduce) { .pill.asking { animation: none; } }
</style>

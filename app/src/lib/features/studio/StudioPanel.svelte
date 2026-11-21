<script lang="ts">
  // Media Studio (D-029): canciones con pista sincronizada o para interpretar en directo.
  // Clic prepara, doble clic envía a pantalla; «Editar» abre el editor en este mismo panel.
  import { onMount } from "svelte";
  import { t, tCount } from "$lib/i18n/index.svelte";
  import type { Message } from "$lib/core/messages";
  import type { MediaSong, OutputContent } from "$lib/platform";
  import { deleteStudioSong, loadStudio, songCredit, studio } from "./library.svelte";
  import { emptySong, expand, studioContent } from "./song";
  import StudioEditor from "./StudioEditor.svelte";

  let {
    onprepare,
    onsend,
    onerror,
  }: {
    onprepare: (content: OutputContent) => void;
    onsend: (content: OutputContent) => void;
    onerror: (message: Message) => void;
  } = $props();

  let editing = $state<MediaSong | null>(null);
  let current = $state<string | null>(null);
  let filter = $state("");
  let confirmDelete = $state<string | null>(null);

  let shown = $derived.by(() => {
    const q = filter.trim().toLocaleLowerCase();
    if (!q) return studio.songs;
    return studio.songs.filter((s) =>
      [s.title, s.artist, ...s.sections.flatMap((x) => x.slides.flatMap((y) => y.lines))].some((f) => f.toLocaleLowerCase().includes(q)),
    );
  });

  onMount(() => {
    if (!studio.loaded) loadStudio();
  });

  function content(song: MediaSong): OutputContent {
    return studioContent(song, songCredit(song));
  }

  function prepare(song: MediaSong) {
    current = song.id;
    onprepare(content(song));
  }

  function startNew() {
    editing = emptySong(crypto.randomUUID());
    confirmDelete = null;
  }

  function startEdit(song: MediaSong) {
    editing = song;
    confirmDelete = null;
    prepare(song);
  }

  function closed(saved: MediaSong | null) {
    editing = null;
    if (saved) prepare(saved);
  }

  function remove(id: string) {
    if (confirmDelete !== id) {
      confirmDelete = id;
      return;
    }
    deleteStudioSong(id);
    confirmDelete = null;
  }
</script>

<div class="studio-panel">
  {#if editing}
    <StudioEditor initial={editing} onclose={closed} {onprepare} {onerror} />
  {:else}
    <div class="actions">
      <button class="btn primary" onclick={startNew}>{t("studio.new")}</button>
      <input class="field search" bind:value={filter} placeholder={t("studio.search")} aria-label={t("studio.search")} />
    </div>
    {#if !studio.loaded}
      <p class="muted">{t("studio.loading")}</p>
    {:else if studio.songs.length === 0}
      <p class="muted">{t("studio.empty")}</p>
    {:else if shown.length === 0}
      <p class="muted">{t("studio.noMatches", { query: filter })}</p>
    {:else}
      <ul class="items">
        {#each shown as song (song.id)}
          <li class:chosen={song.id === current}>
            <button class="item" onclick={() => prepare(song)} ondblclick={() => onsend(content(song))}>
              <span class="name">
                {song.title}
                <span class="muted small sub">{[song.artist, tCount("text.count", expand(song).length)].filter(Boolean).join(" · ")}</span>
              </span>
              <!-- Con texto, no solo color: se distingue sin ver colores. -->
              <span class="badge" class:track={song.mode === "track"}>{t(song.mode === "track" ? "studio.badge.track" : "studio.badge.live")}</span>
              {#if song.mode === "track" && !song.order.some((e) => e.timing)}
                <span class="badge warn">{t("studio.badge.unsynced")}</span>
              {/if}
            </button>
            <button class="btn ghost small-btn" onclick={() => startEdit(song)}>{t("studio.edit")}</button>
            <button class="icon-btn remove" class:confirm={confirmDelete === song.id} onclick={() => remove(song.id)} aria-label={t("studio.delete", { title: song.title })} title={t("studio.delete", { title: song.title })}>
              {confirmDelete === song.id ? t("studio.confirmDelete") : "✕"}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</div>

<style>
  .studio-panel { display: flex; flex-direction: column; gap: 10px; }
  .actions { display: flex; gap: 8px; }
  .search { flex: 1; min-width: 0; }
  .items { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 4px; }
  li { display: flex; align-items: center; gap: 2px; border: 1px solid transparent; border-radius: 8px; }
  li:hover { background: var(--surface-2); }
  li.chosen { border-color: var(--accent); background: var(--accent-soft); }
  .item { flex: 1; min-width: 0; display: flex; align-items: center; gap: 10px; min-height: 46px; padding: 6px 10px; font: inherit; text-align: left; color: var(--text); background: transparent; border: 0; cursor: pointer; }
  .name { flex: 1; min-width: 0; display: flex; flex-direction: column; overflow: hidden; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
  .sub { font-weight: 400; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .badge { flex: none; font-size: 12px; font-weight: 700; padding: 2px 8px; border-radius: 999px; border: 1px solid var(--border); color: var(--muted); }
  .badge.track { color: var(--accent); border-color: var(--accent); }
  .badge.warn { color: var(--warn-text); background: var(--warn-bg); border-color: transparent; }
  .small-btn { min-height: 34px; padding: 0 10px; font-size: 13px; }
  .icon-btn { flex: none; min-width: 36px; min-height: 36px; font: inherit; font-size: 13px; color: var(--muted); background: transparent; border: 0; border-radius: 8px; cursor: pointer; }
  .icon-btn.remove:hover, .icon-btn.confirm { color: var(--live); background: var(--live-soft); }
  .icon-btn.confirm { padding: 0 10px; font-weight: 650; }
</style>

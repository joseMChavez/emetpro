// Biblioteca de Media Studio (D-029): las canciones guardadas en la base; aquí, una copia para la interfaz.
import { platform, type MediaSong, type StudioSaveOutcome } from "$lib/platform";
import { songBackgrounds } from "./song";

/** El permiso para cargar los fondos no sobrevive a un reinicio: se pide para los de las canciones. */
function allowBackgrounds(songs: MediaSong[]) {
  const paths = [...new Set(songs.flatMap(songBackgrounds))];
  if (paths.length > 0) platform.allowBackgrounds(paths).catch(() => {});
}

export const studio = $state({
  songs: [] as MediaSong[],
  loaded: false,
});

function failed(action: string, e: unknown) {
  platform.logError(`media studio: ${action}: ${JSON.stringify(e)}`).catch(() => {});
}

export async function loadStudio() {
  try {
    studio.songs = await platform.studioList();
    allowBackgrounds(studio.songs);
  } catch (e) {
    failed("leer", e);
  } finally {
    studio.loaded = true;
  }
}

/** Guarda si no hay errores (los revisa el backend) y devuelve lo que haya que mostrar. */
export async function saveStudioSong(song: MediaSong): Promise<StudioSaveOutcome> {
  const outcome = await platform.studioSave(song);
  if (outcome.saved) {
    allowBackgrounds([song]);
    const i = studio.songs.findIndex((s) => s.id === song.id);
    if (i >= 0) studio.songs[i] = song;
    else studio.songs.push(song);
    studio.songs.sort((a, b) => a.title.localeCompare(b.title));
  }
  return outcome;
}

export function deleteStudioSong(id: string) {
  studio.songs = studio.songs.filter((s) => s.id !== id);
  platform.studioDelete(id).catch((e) => failed("borrar", e));
}

export function songCredit(song: MediaSong): string {
  return [song.title, song.artist].filter(Boolean).join(" · ");
}

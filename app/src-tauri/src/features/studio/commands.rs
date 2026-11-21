use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Manager, State};

use super::domain::{Issue, MediaSong};
use super::AUDIO_DIR;
use crate::core::db::{with_store, Db};
use crate::core::error::{internal, CommandError};
use crate::core::picker::{self, dialog_extensions};
use crate::features::media::domain::AUDIO_EXTENSIONS;
use crate::features::plans::files;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveOutcome {
    /// false si hay errores: no se guardó nada.
    pub saved: bool,
    pub issues: Vec<Issue>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PickedAudio {
    /// La copia en la carpeta de la app.
    pub path: String,
    /// El nombre del archivo elegido.
    pub name: String,
}

fn audio_dir(app: &AppHandle) -> Result<PathBuf, CommandError> {
    Ok(app.path().app_data_dir().map_err(internal)?.join(AUDIO_DIR))
}

fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO_EXTENSIONS.contains(&e.to_lowercase().as_str()))
}

#[tauri::command]
pub fn studio_list(state: State<'_, Db>) -> Result<Vec<MediaSong>, CommandError> {
    with_store(&state, |store| store.studio_songs())
}

#[tauri::command]
pub fn studio_validate(song: MediaSong) -> Vec<Issue> {
    song.validate()
}

/// Solo guarda si no hay errores; los avisos vuelven igual para mostrarlos.
#[tauri::command]
pub fn studio_save(state: State<'_, Db>, song: MediaSong) -> Result<SaveOutcome, CommandError> {
    let issues = song.validate();
    if MediaSong::has_errors(&issues) {
        return Ok(SaveOutcome { saved: false, issues });
    }
    with_store(&state, |store| store.save_studio_song(&song))?;
    Ok(SaveOutcome { saved: true, issues })
}

#[tauri::command]
pub fn studio_delete(state: State<'_, Db>, id: String) -> Result<(), CommandError> {
    with_store(&state, |store| store.delete_studio_song(&id))
}

/// Elige una pista y la copia a la carpeta de la app (como los medios de un plan, D-019). None si se cancela.
#[tauri::command]
pub async fn studio_pick_audio(app: AppHandle, title: String, filter_name: String) -> Result<Option<PickedAudio>, CommandError> {
    let request = picker::Request {
        title,
        filter: Some((filter_name, dialog_extensions(&[AUDIO_EXTENSIONS]))),
        mode: picker::Mode::File,
    };
    let Some(source) = picker::pick(&app, request).await.map_err(internal)?.and_then(|p| p.into_iter().next()) else {
        return Ok(None);
    };
    if !is_audio(&source) {
        return Err(CommandError::InvalidContent);
    }
    let name = source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let dir = audio_dir(&app)?;
    let file = source.display().to_string();
    let copy = tauri::async_runtime::spawn_blocking(move || files::store_copy(&dir, &source))
        .await
        .map_err(internal)?
        .map_err(|e| CommandError::CopyFailed { file, reason: e.to_string() })?;
    Ok(Some(PickedAudio { path: copy.to_string_lossy().into_owned(), name }))
}

/// Los bytes de una pista, para decodificarla en la consola (Web Audio). Solo pistas de la carpeta de la app:
/// la ruta viene de la interfaz y no puede servir para leer otro archivo del equipo.
#[tauri::command]
pub async fn studio_read_audio(app: AppHandle, path: String) -> Result<Response, CommandError> {
    let missing = || CommandError::FileMissing { path: path.clone() };
    let root = audio_dir(&app)?.canonicalize().map_err(|_| missing())?;
    let file = Path::new(&path).canonicalize().map_err(|_| missing())?;
    if !file.starts_with(&root) || !is_audio(&file) || !file.is_file() {
        return Err(CommandError::InvalidContent);
    }
    let bytes = tauri::async_runtime::spawn_blocking(move || std::fs::read(file))
        .await
        .map_err(internal)?
        .map_err(|_| missing())?;
    Ok(Response::new(bytes))
}

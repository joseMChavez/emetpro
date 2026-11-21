use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

use super::domain::{is_web_link, Issue, MediaSong};
use super::AUDIO_DIR;
use sha2::Digest;

use crate::core::assets::allow_media;
use crate::core::db::{with_store, Db};
use crate::core::error::{internal, CommandError};
use crate::core::picker::{self, dialog_extensions};
use crate::features::media::domain::{AUDIO_EXTENSIONS, VIDEO_EXTENSIONS};
use crate::features::media::extract_audio;
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
    /// Se eligió un video: la pista es su audio (o el video mismo si no hay con qué sacarlo).
    pub from_video: bool,
}

fn audio_dir(app: &AppHandle) -> Result<PathBuf, CommandError> {
    Ok(app.path().app_data_dir().map_err(internal)?.join(AUDIO_DIR))
}

fn has_extension(path: &Path, list: &[&str]) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| list.contains(&e.to_lowercase().as_str()))
}

fn is_audio(path: &Path) -> bool {
    has_extension(path, AUDIO_EXTENSIONS)
}

/// Lo que puede leer la consola como pista: audio, o un video si no hubo con qué sacarle el audio.
fn is_track(path: &Path) -> bool {
    is_audio(path) || has_extension(path, VIDEO_EXTENSIONS)
}

/// Copia `source` a la carpeta de pistas (por su contenido, como los medios de un plan).
async fn copy_track(dir: PathBuf, source: PathBuf) -> Result<PathBuf, CommandError> {
    let file = source.display().to_string();
    tauri::async_runtime::spawn_blocking(move || files::store_copy(&dir, &source))
        .await
        .map_err(internal)?
        .map_err(|e| CommandError::CopyFailed { file, reason: e.to_string() })
}

/// El audio de un video, en `pistas/video-<huella>/<nombre>.m4a`. Si ya se sacó antes, se reusa.
/// Sin avconvert ni ffmpeg, se copia el video entero y la consola decodifica su audio (más lento y con más memoria).
async fn track_from_video(dir: PathBuf, source: PathBuf) -> Result<PathBuf, CommandError> {
    let file = source.display().to_string();
    let stem = source.file_stem().map(|s| s.to_string_lossy().into_owned()).filter(|s| !s.is_empty()).unwrap_or_else(|| "pista".into());
    let meta = std::fs::metadata(&source).map_err(|_| CommandError::FileMissing { path: file.clone() })?;
    let modified = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs());
    let mut hasher = std::hash::DefaultHasher::new();
    std::hash::Hash::hash(&(&source, meta.len(), modified), &mut hasher);
    let folder = dir.join(format!("video-{:016x}", std::hash::Hasher::finish(&hasher)));
    let target = folder.join(format!("{stem}.m4a"));
    if target.is_file() {
        return Ok(target);
    }
    let partial = folder.join(".extrayendo.m4a");
    let (from, to) = (source.clone(), partial.clone());
    let extracted = tauri::async_runtime::spawn_blocking(move || {
        std::fs::create_dir_all(to.parent().unwrap()).map_err(|e| Some(e.to_string()))?;
        extract_audio(&from, &to)
    })
    .await
    .map_err(internal)?;
    match extracted {
        Ok(()) => {
            std::fs::rename(&partial, &target).map_err(|e| CommandError::CopyFailed { file, reason: e.to_string() })?;
            Ok(target)
        }
        Err(None) => copy_track(dir, source).await,
        Err(Some(reason)) => Err(CommandError::ConversionFailed { file, reason }),
    }
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

/// Elige una pista (audio, o un video del que se saca el audio) y la deja en la carpeta de la app (como los
/// medios de un plan, D-019). None si se cancela.
#[tauri::command]
pub async fn studio_pick_audio(app: AppHandle, title: String, filter_name: String) -> Result<Option<PickedAudio>, CommandError> {
    let request = picker::Request {
        title,
        filter: Some((filter_name, dialog_extensions(&[AUDIO_EXTENSIONS, VIDEO_EXTENSIONS]))),
        mode: picker::Mode::File,
    };
    let Some(source) = picker::pick(&app, request).await.map_err(internal)?.and_then(|p| p.into_iter().next()) else {
        return Ok(None);
    };
    let name = source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let dir = audio_dir(&app)?;
    let from_video = has_extension(&source, VIDEO_EXTENSIONS);
    let track = if from_video {
        track_from_video(dir, source).await?
    } else if is_audio(&source) {
        copy_track(dir, source).await?
    } else {
        return Err(CommandError::InvalidContent);
    };
    Ok(Some(PickedAudio { path: track.to_string_lossy().into_owned(), name, from_video }))
}

/// Lo más grande que puede pesar un fondo generado: las plantillas ocupan unos pocos KB.
const MAX_SVG: usize = 512 * 1024;

/// Guarda un fondo animado generado por la interfaz (studio/svgbg.ts) en `fondos/generados/<huella>.svg` y lo
/// permite para la salida. Mismo SVG, mismo archivo. Se muestra con <img>, donde un SVG no ejecuta scripts.
#[tauri::command]
pub fn studio_save_svg(app: AppHandle, svg: String) -> Result<String, CommandError> {
    let text = svg.trim_start();
    if svg.len() > MAX_SVG || !(text.starts_with("<svg") || text.starts_with("<?xml")) || !svg.contains("</svg>") {
        return Err(CommandError::InvalidContent);
    }
    let dir = app.path().app_data_dir().map_err(internal)?.join("fondos").join("generados");
    let hash = format!("{:x}", sha2::Sha256::digest(svg.as_bytes()));
    let path = dir.join(format!("{}.svg", &hash[..16]));
    if !path.is_file() {
        std::fs::create_dir_all(&dir).map_err(|e| CommandError::CopyFailed { file: path.display().to_string(), reason: e.to_string() })?;
        let partial = path.with_extension("svg.part");
        std::fs::write(&partial, svg.as_bytes())
            .and_then(|_| std::fs::rename(&partial, &path))
            .map_err(|e| CommandError::CopyFailed { file: path.display().to_string(), reason: e.to_string() })?;
    }
    allow_media(&app, &path)?;
    Ok(path.to_string_lossy().into_owned())
}

/// Abre el enlace de referencia de una canción en el navegador. Solo direcciones web.
#[tauri::command]
pub fn studio_open_reference(app: AppHandle, url: String) -> Result<(), CommandError> {
    if !is_web_link(&url) {
        return Err(CommandError::InvalidContent);
    }
    app.opener().open_url(url.trim(), None::<&str>).map_err(internal)
}

/// Los bytes de una pista, para decodificarla en la consola (Web Audio). Solo pistas de la carpeta de la app:
/// la ruta viene de la interfaz y no puede servir para leer otro archivo del equipo.
#[tauri::command]
pub async fn studio_read_audio(app: AppHandle, path: String) -> Result<Response, CommandError> {
    let missing = || CommandError::FileMissing { path: path.clone() };
    let root = audio_dir(&app)?.canonicalize().map_err(|_| missing())?;
    let file = Path::new(&path).canonicalize().map_err(|_| missing())?;
    if !file.starts_with(&root) || !is_track(&file) || !file.is_file() {
        return Err(CommandError::InvalidContent);
    }
    let bytes = tauri::async_runtime::spawn_blocking(move || std::fs::read(file))
        .await
        .map_err(internal)?
        .map_err(|_| missing())?;
    Ok(Response::new(bytes))
}

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use super::domain::{is_web_link, Issue, MediaSong};
use super::separate::runtime as separator;
use super::{prepare, whisper, AUDIO_DIR};
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

/// Una pista de la carpeta de la app (la ruta viene de la interfaz: no puede servir para leer otro archivo).
fn app_track(app: &AppHandle, path: &str) -> Result<PathBuf, CommandError> {
    let missing = || CommandError::FileMissing { path: path.to_string() };
    let root = audio_dir(app)?.canonicalize().map_err(|_| missing())?;
    let file = Path::new(path).canonicalize().map_err(|_| missing())?;
    if !file.starts_with(&root) || !is_track(&file) || !file.is_file() {
        return Err(CommandError::InvalidContent);
    }
    Ok(file)
}

/// Los bytes de una pista, para decodificarla en la consola (Web Audio). Solo pistas de la carpeta de la app:
/// la ruta viene de la interfaz y no puede servir para leer otro archivo del equipo.
#[tauri::command]
pub async fn studio_read_audio(app: AppHandle, path: String) -> Result<Response, CommandError> {
    let missing = || CommandError::FileMissing { path: path.clone() };
    let file = app_track(&app, &path)?;
    let bytes = tauri::async_runtime::spawn_blocking(move || std::fs::read(file))
        .await
        .map_err(internal)?
        .map_err(|_| missing())?;
    Ok(Response::new(bytes))
}

// ---- Sacar la letra de la pista (whisper.rs) ----

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WhisperStatus {
    /// El modelo está descargado.
    pub ready: bool,
    /// Se está descargando o sacando una letra.
    pub busy: bool,
    /// Este procesador puede usarlo (ver `whisper::cpu_supported`).
    pub supported: bool,
    /// Lo que ocupa la descarga, en bytes.
    pub size: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "phase")]
pub enum WhisperProgress {
    Download { done: u64, total: u64 },
    Transcribe { percent: i32 },
}

#[tauri::command]
pub fn studio_whisper_status(app: AppHandle) -> Result<WhisperStatus, CommandError> {
    let data = app.path().app_data_dir().map_err(internal)?;
    Ok(WhisperStatus { ready: whisper::model(&data).is_some(), busy: whisper::busy(), supported: whisper::cpu_supported(), size: whisper::MODEL_SIZE })
}

/// Descarga el reconocedor de voz (solo cuando el usuario lo pide). Avisa del avance con «whisper-progress».
#[tauri::command]
pub async fn studio_whisper_download(app: AppHandle) -> Result<(), CommandError> {
    let data = app.path().app_data_dir().map_err(internal)?;
    let emitter = app.clone();
    whisper::download(&data, move |done| {
        let _ = emitter.emit("whisper-progress", WhisperProgress::Download { done, total: whisper::MODEL_SIZE });
    })
    .await?;
    Ok(())
}

#[tauri::command]
pub fn studio_whisper_cancel() {
    whisper::cancel();
}

#[tauri::command]
pub fn studio_whisper_remove(app: AppHandle) -> Result<(), CommandError> {
    let data = app.path().app_data_dir().map_err(internal)?;
    whisper::remove(&data)?;
    Ok(())
}

/// Saca la letra de una pista. El cuerpo son las muestras (f32 little-endian, mono, 16 kHz, decodificadas en la
/// consola); el idioma y el título (contexto para Whisper) van en las cabeceras «language» y «prompt»
/// (codificado como URL). Avisa del avance con «whisper-progress».
#[tauri::command]
pub async fn studio_transcribe(app: AppHandle, request: tauri::ipc::Request<'_>) -> Result<Vec<whisper::Segment>, CommandError> {
    let data = app.path().app_data_dir().map_err(internal)?;
    let model = whisper::model(&data).ok_or(CommandError::WhisperModelMissing)?;
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else { return Err(CommandError::InvalidContent) };
    let samples = whisper::samples_from_bytes(bytes).ok_or(CommandError::InvalidContent)?;
    let header = |name: &str| request.headers().get(name).and_then(|v| v.to_str().ok()).unwrap_or_default().to_string();
    let language = whisper::language(&header("language"));
    let prompt: String = percent_decode(&header("prompt")).chars().take(200).collect();
    let emitter = app.clone();
    let segments = tauri::async_runtime::spawn_blocking(move || {
        whisper::transcribe(&model, &samples, language, &prompt, move |percent| {
            let _ = emitter.emit("whisper-progress", WhisperProgress::Transcribe { percent });
        })
    })
    .await
    .map_err(internal)??;
    Ok(segments)
}

/// «%C3%B1» → «ñ». Lo que no se entiende se deja tal cual.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if let (b'%', Some(h), Some(l)) = (bytes[i], bytes.get(i + 1).and_then(|b| hex(*b)), bytes.get(i + 2).and_then(|b| hex(*b))) {
            out.push((h * 16 + l) as u8);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod transcribe_tests {
    use super::percent_decode;

    #[test]
    fn titulo_codificado_como_url() {
        assert_eq!(percent_decode("Noche%20de%20paz%2C%20ni%C3%B1o"), "Noche de paz, niño");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("a%zzb"), "a%zzb");
    }
}

// ---- Quitar la voz (separate) ----

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeparatorStatus {
    /// Hay separador para este sistema.
    pub available: bool,
    pub ready: bool,
    pub busy: bool,
    /// Lo que ocupa la descarga (motor y modelo), en bytes.
    pub size: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "phase")]
pub enum SeparatorProgress {
    Download { done: u64, total: u64 },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeparatedTrack {
    pub path: String,
    pub name: String,
    pub duration_ms: u32,
}

#[tauri::command]
pub fn studio_separator_status(app: AppHandle) -> Result<SeparatorStatus, CommandError> {
    let data = app.path().app_data_dir().map_err(internal)?;
    Ok(SeparatorStatus {
        available: separator::available(),
        ready: separator::installed(&data).is_some(),
        busy: separator::busy(),
        size: separator::download_size(),
    })
}

/// Descarga el motor y el modelo para quitar la voz (solo cuando el usuario lo pide). Avance: «separator-progress».
#[tauri::command]
pub async fn studio_separator_download(app: AppHandle) -> Result<(), CommandError> {
    let data = app.path().app_data_dir().map_err(internal)?;
    let emitter = app.clone();
    let total = separator::download_size();
    separator::download(&data, move |done| {
        let _ = emitter.emit("separator-progress", SeparatorProgress::Download { done, total });
    })
    .await?;
    Ok(())
}

#[tauri::command]
pub fn studio_separator_cancel() {
    separator::cancel();
}

#[tauri::command]
pub fn studio_separator_remove(app: AppHandle) -> Result<(), CommandError> {
    let data = app.path().app_data_dir().map_err(internal)?;
    separator::remove(&data)?;
    Ok(())
}

// ---- Preparar con la pista: letra, sincronización y sin voz de una vez (prepare.rs) ----

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "phase")]
pub enum PrepareProgress {
    Reading,
    Separating { done: usize, total: usize },
    Saving,
    Listening { percent: i32 },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedTrack {
    pub segments: Vec<whisper::Segment>,
    pub instrumental: Option<SeparatedTrack>,
    /// Se oyó la voz separada (más fiable que la mezcla).
    pub clean_voice: bool,
    /// Dónde arranca a cantar la voz separada (ms), para ajustar las diapositivas; vacío sin separar.
    pub onsets: Vec<u32>,
}

/// `listen`: oír la letra (para sacarla o sincronizar). `voiceless`: guardar la versión sin voz con el nombre `name`.
/// Avance: «prepare-progress».
#[tauri::command]
pub async fn studio_prepare(
    app: AppHandle,
    path: String,
    name: String,
    language: String,
    prompt: String,
    listen: bool,
    voiceless: bool,
) -> Result<PreparedTrack, CommandError> {
    let data = app.path().app_data_dir().map_err(internal)?;
    let source = app_track(&app, &path)?;
    let model = if listen { Some(whisper::model(&data).ok_or(CommandError::WhisperModelMissing)?) } else { None };
    if voiceless && separator::installed(&data).is_none() {
        return Err(if separator::available() { CommandError::SeparatorMissing } else { CommandError::SeparatorUnavailable });
    }
    let target = voiceless.then(|| {
        let stem = source.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "pista".into());
        let short = uuid::Uuid::new_v4().simple().to_string()[..8].to_string();
        source.with_file_name(format!("{stem}-sin-voz-{short}.flac"))
    });
    let language = whisper::language(&language);
    let prompt: String = prompt.chars().take(200).collect();
    let emitter = app.clone();
    let out = target.clone();
    let prepared = tauri::async_runtime::spawn_blocking(move || {
        let request = prepare::Request {
            track: &source,
            instrumental: out.as_deref(),
            whisper_model: model.as_deref(),
            language,
            prompt: &prompt,
        };
        prepare::prepare(&data, request, move |step| {
            let progress = match step {
                prepare::Step::Reading => PrepareProgress::Reading,
                prepare::Step::Separating(done, total) => PrepareProgress::Separating { done, total },
                prepare::Step::Saving => PrepareProgress::Saving,
                prepare::Step::Listening(percent) => PrepareProgress::Listening { percent },
            };
            let _ = emitter.emit("prepare-progress", progress);
        })
    })
    .await
    .map_err(internal)?;
    let prepared = match prepared {
        Ok(p) => p,
        Err(e) => {
            if let Some(target) = &target {
                let _ = std::fs::remove_file(target);
            }
            return Err(e.into());
        }
    };
    let instrumental = target.zip(prepared.instrumental_ms).map(|(path, duration_ms)| SeparatedTrack {
        path: path.to_string_lossy().into_owned(),
        name,
        duration_ms,
    });
    Ok(PreparedTrack { segments: prepared.segments, instrumental, clean_voice: prepared.clean_voice, onsets: prepared.onsets })
}

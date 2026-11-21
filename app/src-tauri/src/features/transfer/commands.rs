use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use super::domain::{plan, resolve, Incoming, Invalid, OnConflict};
use super::formats::emetpro::{self, Background, Manifest, SongEntry, EXTENSION};
use super::formats::holyrics;
use super::formats::plan::{self as plan_file, BibleRef, PlanManifest};
use super::plan_transfer;
use crate::features::media::domain::{media_kind, MediaKind, AUDIO_EXTENSIONS, VIDEO_EXTENSIONS};
use crate::features::studio::domain::MediaSong;
use super::{Pending, PendingPlan, PlanImportState, TransferState};
use crate::core::assets::allow_media;
use crate::core::db::{now_secs, with_store, Db};
use crate::core::error::{internal, CommandError};
use crate::core::picker::{self, dialog_extensions};
use crate::features::plans::domain::{ItemContent, Plan, PlanItem};
use crate::features::transfer::domain::split_slides;

/// Un archivo de letras no pesa más que esto, ni con fondos.
const MAX_FILE: u64 = 512 * 1024 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SongSummary {
    title: String,
    singer: String,
    slides: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvalidSong {
    title: String,
    reason: Invalid,
}

/// Lo que se va a importar, para que el usuario lo vea antes de confirmar.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    file: String,
    /// "emetpro" u "holyrics".
    format: &'static str,
    fresh: Vec<SongSummary>,
    /// Ya están en la biblioteca con la misma letra: no se tocan.
    identical: usize,
    /// Ya están con otra letra: el usuario elige qué hacer.
    conflicts: Vec<SongSummary>,
    invalid: Vec<InvalidSong>,
    /// Canciones que trae como lista del culto (en .hpls, todas); al importar pueden formar un plan.
    setlist: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    added: usize,
    replaced: usize,
    skipped: usize,
    invalid: usize,
    /// Letras del plan creado con la lista del archivo; 0 si no se creó.
    plan_songs: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    path: String,
    songs: usize,
}

fn summary(song: &Incoming) -> SongSummary {
    SongSummary { title: song.title.trim().to_string(), singer: song.singer.trim().to_string(), slides: song.slides.len() }
}

/// Lee un archivo de letras en cualquiera de los formatos que se reconocen.
fn read_file(path: &Path) -> Result<Pending, String> {
    if plan_file::is_plan(path) {
        return Err("es un plan de EmetPro (.emetplan): se importa desde el panel Plan".into());
    }
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > MAX_FILE {
        return Err("el archivo es demasiado grande".into());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if emetpro::is_package(&bytes) {
        let package = emetpro::read(&bytes)?;
        Ok(Pending { file, format: "emetpro", songs: package.songs, setlist: package.setlist, backgrounds: package.backgrounds })
    } else if holyrics::is_hpls(&bytes) {
        // Una lista de Holyrics es una lista del culto: todas sus canciones, en su orden.
        let songs = holyrics::read(&bytes)?;
        let setlist = (0..songs.len()).collect();
        Ok(Pending { file, format: "holyrics", songs, setlist, backgrounds: Vec::new() })
    } else {
        Err("no es un archivo de letras de EmetPro (.emetpro, .selah) ni una lista de Holyrics (.hpls)".into())
    }
}

/// Elige un archivo, lo lee y devuelve la vista previa. No guarda nada: eso lo hace `transfer_import_apply`.
#[tauri::command]
pub async fn transfer_import_pick(
    app: AppHandle,
    state: State<'_, TransferState>,
    db: State<'_, Db>,
    title: String,
    filter_name: String,
) -> Result<Option<ImportPreview>, CommandError> {
    let request = picker::Request {
        title,
        filter: Some((filter_name, dialog_extensions(&[&["emetpro", "selah", "hpls"]]))),
        mode: picker::Mode::File,
    };
    let Some(path) = picker::pick(&app, request).await.map_err(internal)?.and_then(|p| p.into_iter().next()) else {
        return Ok(None);
    };
    let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let pending = tauri::async_runtime::spawn_blocking(move || read_file(&path))
        .await
        .map_err(internal)?
        .map_err(|reason| CommandError::ImportFailed { file, reason })?;

    let existing = with_store(&db, |store| store.songs())?;
    let plan = plan(&existing, &pending.songs);
    let preview = ImportPreview {
        file: pending.file.clone(),
        format: pending.format,
        fresh: plan.fresh.iter().map(|&i| summary(&pending.songs[i])).collect(),
        identical: plan.identical.len(),
        conflicts: plan.conflicts.iter().map(|(i, _)| summary(&pending.songs[*i])).collect(),
        invalid: plan
            .invalid
            .iter()
            .map(|&(i, reason)| InvalidSong { title: pending.songs[i].title.trim().to_string(), reason })
            .collect(),
        setlist: pending.setlist.len(),
    };
    *state.0.lock().unwrap() = Some(pending);
    Ok(Some(preview))
}

/// Guarda lo leído por `transfer_import_pick`. `imported_suffix` es «(importada)» en el idioma de la interfaz.
/// Con `plan_name`, la lista del archivo se guarda además como un plan con ese nombre.
#[tauri::command]
pub fn transfer_import_apply(
    app: AppHandle,
    state: State<'_, TransferState>,
    db: State<'_, Db>,
    on_conflict: OnConflict,
    plan_name: Option<String>,
    imported_suffix: String,
) -> Result<ImportResult, CommandError> {
    let pending = state.0.lock().unwrap().take().ok_or_else(|| internal("no hay nada leído para importar"))?;
    let file = pending.file.clone();
    // La biblioteca puede haber cambiado desde la vista previa: se vuelve a comparar.
    let existing = with_store(&db, |store| store.songs())?;
    let plan = plan(&existing, &pending.songs);

    // Solo se guardan los fondos de las canciones que de verdad se van a guardar.
    let used: HashSet<usize> = plan
        .fresh
        .iter()
        .chain(plan.conflicts.iter().map(|(i, _)| i))
        .filter_map(|&i| pending.songs[i].background)
        .collect();
    let dir = app.path().app_data_dir().map_err(internal)?.join("fondos");
    let mut saved = std::collections::HashMap::new();
    for i in used {
        let path = save_background(&dir, &pending.backgrounds[i])
            .map_err(|reason| CommandError::ImportFailed { file: file.clone(), reason })?;
        allow_media(&app, &path)?;
        saved.insert(i, path.to_string_lossy().into_owned());
    }

    let resolution = resolve(
        &existing,
        &pending.songs,
        &plan,
        on_conflict,
        &imported_suffix,
        |i| saved.get(&i).cloned(),
        || uuid::Uuid::new_v4().to_string(),
    );
    let new_plan = plan_name.filter(|name| !name.trim().is_empty()).map(|name| Plan {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.trim().to_string(),
        date: None,
        notes: String::new(),
        animate: None,
        items: pending
            .setlist
            .iter()
            .filter_map(|i| resolution.ids.get(i))
            .map(|id| PlanItem { id: uuid::Uuid::new_v4().to_string(), content: ItemContent::Song { song_id: id.clone(), marks: Default::default() } })
            .collect(),
    });
    let new_plan = new_plan.filter(|p| !p.items.is_empty());
    with_store(&db, |store| store.import_songs(&resolution.save, new_plan.as_ref()))?;
    Ok(ImportResult {
        added: resolution.added,
        replaced: resolution.replaced,
        skipped: resolution.skipped,
        invalid: plan.invalid.len(),
        plan_songs: new_plan.map_or(0, |p| p.items.len()),
    })
}

/// El usuario cerró la vista previa sin importar.
#[tauri::command]
pub fn transfer_import_cancel(state: State<'_, TransferState>) {
    state.0.lock().unwrap().take();
}

/// Guarda el fondo con el nombre de su contenido: importar dos veces el mismo no lo duplica.
fn save_background(dir: &Path, background: &Background) -> Result<PathBuf, String> {
    use sha2::{Digest, Sha256};
    let hash = format!("{:x}", Sha256::digest(&background.bytes));
    let ext = Path::new(&background.name)
        .extension()
        .and_then(|e| e.to_str())
        .filter(|e| e.len() <= 5 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or("img")
        .to_ascii_lowercase();
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.{ext}", &hash[..16]));
    if !path.is_file() {
        std::fs::write(&path, &background.bytes).map_err(|e| e.to_string())?;
    }
    Ok(path)
}

/// Exporta la biblioteca entera (o solo las letras de un plan, como lista del culto) a un .emetpro, con sus fondos.
#[tauri::command]
pub async fn transfer_export(
    app: AppHandle,
    db: State<'_, Db>,
    title: String,
    filter_name: String,
    file_name: String,
    plan_id: Option<String>,
) -> Result<Option<ExportResult>, CommandError> {
    let request = picker::Request {
        title,
        filter: Some((filter_name, dialog_extensions(&[&[EXTENSION]]))),
        mode: picker::Mode::Save { name: file_name },
    };
    let Some(mut path) = picker::pick(&app, request).await.map_err(internal)?.and_then(|p| p.into_iter().next()) else {
        return Ok(None);
    };
    if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case(EXTENSION)) {
        path.as_mut_os_string().push(format!(".{EXTENSION}"));
    }

    let mut songs = with_store(&db, |store| store.songs())?;
    // La lista del archivo: las letras del plan, en su orden y sin repetir.
    let mut setlist: Vec<String> = Vec::new();
    if let Some(id) = plan_id {
        let plan = with_store(&db, |store| store.plan(&id))?.ok_or_else(|| internal("el plan ya no existe"))?;
        for item in plan.items {
            match item.content {
                ItemContent::Song { song_id, .. } if !setlist.contains(&song_id) => setlist.push(song_id),
                _ => {}
            }
        }
        songs = setlist.iter().filter_map(|id| songs.iter().find(|s| &s.id == id).cloned()).collect();
    }
    let export = |reason: String| CommandError::ExportFailed { reason };

    let mut backgrounds: Vec<Background> = Vec::new();
    let mut index_of = std::collections::HashMap::<String, usize>::new();
    let mut entries = Vec::with_capacity(songs.len());
    for song in &songs {
        // Un fondo que ya no existe en el disco se omite: la letra se exporta igual.
        let background = song.background.as_ref().and_then(|p| {
            if let Some(&i) = index_of.get(p) {
                return Some(i);
            }
            let bytes = std::fs::read(p).ok()?;
            let name = Path::new(p).file_name()?.to_string_lossy().into_owned();
            backgrounds.push(Background { name, bytes });
            index_of.insert(p.clone(), backgrounds.len() - 1);
            Some(backgrounds.len() - 1)
        });
        entries.push(SongEntry {
            id: song.id.clone(),
            title: song.title.clone(),
            singer: song.singer.clone(),
            writer: song.writer.clone(),
            slides: split_slides(&song.body),
            background,
        });
    }
    let manifest = Manifest {
        format: String::new(),
        version: 0,
        exported_at: now_secs(),
        app: format!("EmetPro {}", env!("CARGO_PKG_VERSION")),
        songs: entries,
        setlist: setlist.into_iter().filter(|id| songs.iter().any(|s| &s.id == id)).collect(),
        backgrounds: Vec::new(),
    };
    let bytes = emetpro::write(manifest, &backgrounds).map_err(export)?;
    // Se escribe aparte y se renombra al final: un corte a mitad no deja un archivo roto con el nombre bueno.
    let partial = path.with_extension(format!("{EXTENSION}.part"));
    std::fs::write(&partial, &bytes).map_err(|e| export(e.to_string()))?;
    std::fs::rename(&partial, &path).map_err(|e| export(e.to_string()))?;
    Ok(Some(ExportResult { path: path.to_string_lossy().into_owned(), songs: songs.len() }))
}

// ---- Planes (.emetplan, D-028) ----

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExportInfo {
    /// Elementos que van (con la sección).
    items: usize,
    videos: usize,
    video_bytes: u64,
    /// Lo que pesan los demás archivos (imágenes, PDF, presentaciones, fondos).
    other_bytes: u64,
}

fn size_of(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn load_items(db: &Db, plan_id: &str, section_id: Option<&str>) -> Result<(Plan, Vec<PlanItem>), CommandError> {
    let plan = with_store(db, |store| store.plan(plan_id))?.ok_or_else(|| internal("el plan ya no existe"))?;
    let items = plan_transfer::select(&plan.items, section_id);
    Ok((plan, items))
}

/// Cuánto pesaría exportar el plan o una sección, con y sin videos (para la casilla «Incluir videos»).
#[tauri::command]
pub fn plan_export_info(db: State<'_, Db>, plan_id: String, section_id: Option<String>) -> Result<PlanExportInfo, CommandError> {
    let (_, items) = load_items(&db, &plan_id, section_id.as_deref())?;
    let all = plan_transfer::pack(&items, true, Path::is_file);
    let without = plan_transfer::pack(&items, false, Path::is_file);
    let studio = with_store(&db, |store| store.studio_songs())?;
    let studio_bytes: u64 = studio_files(&all.studio_songs, &studio).iter().map(|p| size_of(p)).sum();
    let other_bytes: u64 = without.files.iter().map(|(_, p)| size_of(p)).sum::<u64>() + studio_bytes;
    let all_bytes: u64 = all.files.iter().map(|(_, p)| size_of(p)).sum::<u64>() + studio_bytes;
    Ok(PlanExportInfo { items: items.len(), videos: without.omitted, video_bytes: all_bytes.saturating_sub(other_bytes), other_bytes })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanExportResult {
    path: String,
    items: usize,
    files: usize,
    omitted: usize,
}

/// Exporta el plan (o una sección) a un .emetplan con sus archivos y las letras que usa. `bibles` son las versiones
/// de la Biblia que la interfaz conoce, para nombrar las que fijan las citas.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn plan_export(
    app: AppHandle,
    db: State<'_, Db>,
    title: String,
    filter_name: String,
    file_name: String,
    plan_id: String,
    section_id: Option<String>,
    include_videos: bool,
    bibles: Vec<BibleRef>,
) -> Result<Option<PlanExportResult>, CommandError> {
    let (plan, items) = load_items(&db, &plan_id, section_id.as_deref())?;
    let export = |reason: String| CommandError::ExportFailed { reason };
    if items.is_empty() {
        return Err(export("no hay nada que exportar".into()));
    }
    let Some(path) = pick_export_path(&app, title, filter_name, file_name).await? else {
        return Ok(None);
    };
    let library = with_store(&db, |store| store.songs())?;
    let studio = with_store(&db, |store| store.studio_songs())?;
    let (manifest, files, omitted) = build_export(&plan, &items, &library, &studio, include_videos, bibles, section_id.as_deref());
    write_export(path, manifest, files, omitted).await.map(Some)
}

/// Dónde guardar el archivo (con la extensión .emetplan aunque no se escriba). None si se cancela.
async fn pick_export_path(app: &AppHandle, title: String, filter_name: String, file_name: String) -> Result<Option<PathBuf>, CommandError> {
    let request = picker::Request {
        title,
        filter: Some((filter_name, dialog_extensions(&[&[plan_file::EXTENSION]]))),
        mode: picker::Mode::Save { name: file_name },
    };
    let Some(mut path) = picker::pick(app, request).await.map_err(internal)?.and_then(|p| p.into_iter().next()) else {
        return Ok(None);
    };
    if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case(plan_file::EXTENSION)) {
        path.as_mut_os_string().push(format!(".{}", plan_file::EXTENSION));
    }
    Ok(Some(path))
}

/// Escribe el archivo. Puede tardar (videos, pistas): fuera del hilo de los comandos; aparte y renombrado al final.
async fn write_export(path: PathBuf, manifest: PlanManifest, files: Vec<(String, PathBuf)>, omitted: usize) -> Result<PlanExportResult, CommandError> {
    let result = PlanExportResult { path: path.to_string_lossy().into_owned(), items: manifest.items.len(), files: files.len(), omitted };
    tauri::async_runtime::spawn_blocking(move || {
        let partial = path.with_extension(format!("{}.part", plan_file::EXTENSION));
        let written = plan_file::write(&partial, manifest, &files).and_then(|()| std::fs::rename(&partial, &path));
        if written.is_err() {
            let _ = std::fs::remove_file(&partial);
        }
        written
    })
    .await
    .map_err(internal)?
    .map_err(|e| CommandError::ExportFailed { reason: e.to_string() })?;
    Ok(result)
}

/// Canciones de Media Studio sueltas (D-029), en el mismo formato que un plan: un .emetplan con un elemento por
/// canción, con su pista y sus fondos. En la otra computadora se importa como un plan: nuevo o al final de uno.
#[tauri::command]
pub async fn studio_export(
    app: AppHandle,
    db: State<'_, Db>,
    title: String,
    filter_name: String,
    file_name: String,
    song_ids: Vec<String>,
) -> Result<Option<PlanExportResult>, CommandError> {
    let studio = with_store(&db, |store| store.studio_songs())?;
    let chosen: Vec<&MediaSong> = song_ids.iter().filter_map(|id| studio.iter().find(|s| &s.id == id)).collect();
    if chosen.is_empty() {
        return Err(CommandError::ExportFailed { reason: "no hay nada que exportar".into() });
    }
    let plan = Plan {
        id: String::new(),
        name: if chosen.len() == 1 { chosen[0].title.clone() } else { "Media Studio".into() },
        date: None,
        notes: String::new(),
        animate: None,
        items: chosen
            .iter()
            .map(|s| PlanItem { id: uuid::Uuid::new_v4().to_string(), content: ItemContent::StudioSong { studio_song_id: s.id.clone() } })
            .collect(),
    };
    let Some(path) = pick_export_path(&app, title, filter_name, file_name).await? else {
        return Ok(None);
    };
    let (manifest, files, omitted) = build_export(&plan, &plan.items, &[], &studio, true, Vec::new(), None);
    write_export(path, manifest, files, omitted).await.map(Some)
}

/// El manifiesto y los archivos de un plan exportado (sin escribir nada). Devuelve también los videos que no van.
/// Una pista de Media Studio: audio, o el video del que se saca (D-029). Solo por su extensión.
fn is_track_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .is_some_and(|e| AUDIO_EXTENSIONS.contains(&e.as_str()) || VIDEO_EXTENSIONS.contains(&e.as_str()))
}

fn is_background_file(path: &Path) -> bool {
    matches!(media_kind(path), Some(MediaKind::Image | MediaKind::Video))
}

/// Los archivos de las canciones de Media Studio que viajarían (pista y fondos), sin repetir.
fn studio_files(ids: &[String], studio: &[MediaSong]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for song in studio.iter().filter(|s| ids.contains(&s.id)) {
        plan_transfer::pack_studio(song, |path, audio| {
            let p = Path::new(path);
            if p.is_file() && (if audio { is_track_file(p) } else { is_background_file(p) }) && !out.iter().any(|o| o == p) {
                out.push(p.to_path_buf());
            }
            None
        });
    }
    out
}

fn build_export(
    plan: &Plan,
    items: &[PlanItem],
    library: &[crate::features::songs::domain::Song],
    studio: &[MediaSong],
    include_videos: bool,
    bibles: Vec<BibleRef>,
    section_id: Option<&str>,
) -> (PlanManifest, Vec<(String, PathBuf)>, usize) {
    let mut packed = plan_transfer::pack(items, include_videos, Path::is_file);
    // Las letras, con su fondo como un archivo más.
    let mut songs = Vec::new();
    for id in &packed.songs {
        let Some(song) = library.iter().find(|s| &s.id == id) else { continue };
        let background = song.background.as_ref().filter(|p| Path::new(p).is_file() && media_kind(Path::new(p)).is_some()).map(|p| {
            let name = Path::new(p).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            packed.files.push((name, PathBuf::from(p)));
            packed.files.len() - 1
        });
        songs.push(SongEntry {
            id: song.id.clone(),
            title: song.title.clone(),
            singer: song.singer.clone(),
            writer: song.writer.clone(),
            slides: split_slides(&song.body),
            background,
        });
    }
    // Canciones de Media Studio, con su pista y sus fondos como archivos (sin repetir los que ya van).
    let mut studio_songs = Vec::new();
    for id in &packed.studio_songs {
        let Some(song) = studio.iter().find(|s| &s.id == id) else { continue };
        let files = &mut packed.files;
        let packed_song = plan_transfer::pack_studio(song, |path, audio| {
            let p = Path::new(path);
            if !p.is_file() || !(if audio { is_track_file(p) } else { is_background_file(p) }) {
                return None;
            }
            if let Some(i) = files.iter().position(|(_, f)| f == p) {
                return Some(i);
            }
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            files.push((name, p.to_path_buf()));
            Some(files.len() - 1)
        });
        studio_songs.push(serde_json::to_value(packed_song).unwrap_or_default());
    }
    let used_bibles: Vec<BibleRef> = bibles
        .into_iter()
        .filter(|b| packed.items.iter().any(|i| matches!(&i.content, ItemContent::Scripture { bible: Some(id), .. } if id == &b.id)))
        .collect();
    let section = section_id.and_then(|id| match items.first().map(|i| (&i.id, &i.content)) {
        Some((first, ItemContent::Section { title, .. })) if first == id => Some(title.clone()),
        _ => None,
    });
    let manifest = PlanManifest {
        format: String::new(),
        version: 0,
        exported_at: now_secs(),
        app: format!("EmetPro {}", env!("CARGO_PKG_VERSION")),
        name: plan.name.clone(),
        date: plan.date.clone(),
        notes: plan.notes.clone(),
        animate: plan.animate,
        section,
        items: packed.items.iter().map(|i| serde_json::to_value(i).unwrap_or_default()).collect(),
        songs,
        studio_songs,
        bibles: used_bibles,
        files: Vec::new(),
    };
    (manifest, packed.files, packed.omitted)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanImportPreview {
    file: String,
    name: String,
    section: Option<String>,
    date: Option<String>,
    app: String,
    items: usize,
    files: usize,
    bytes: u64,
    /// Medios que vienen sin su archivo (videos que no se incluyeron).
    omitted: Vec<String>,
    fresh: Vec<SongSummary>,
    identical: usize,
    conflicts: Vec<SongSummary>,
    /// Versiones de la Biblia que fijan las citas; la interfaz avisa de las que no están en este equipo.
    bibles: Vec<BibleRef>,
    /// Canciones de Media Studio que trae (títulos), y las que ya están en este equipo con otro contenido.
    studio: Vec<String>,
    studio_conflicts: Vec<String>,
}

fn incoming_songs(manifest: &PlanManifest) -> Vec<Incoming> {
    manifest
        .songs
        .iter()
        .map(|s| Incoming {
            title: s.title.clone(),
            singer: s.singer.clone(),
            writer: s.writer.clone(),
            slides: s.slides.clone(),
            background: s.background.filter(|&i| i < manifest.files.len()),
        })
        .collect()
}

/// Elige un .emetplan y devuelve lo que trae. No guarda nada: eso lo hace `plan_import_apply`.
#[tauri::command]
pub async fn plan_import_pick(
    app: AppHandle,
    state: State<'_, PlanImportState>,
    db: State<'_, Db>,
    title: String,
    filter_name: String,
) -> Result<Option<PlanImportPreview>, CommandError> {
    let request = picker::Request {
        title,
        filter: Some((filter_name, dialog_extensions(&[&[plan_file::EXTENSION]]))),
        mode: picker::Mode::File,
    };
    let Some(path) = picker::pick(&app, request).await.map_err(internal)?.and_then(|p| p.into_iter().next()) else {
        return Ok(None);
    };
    let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let manifest = plan_file::open(&path).map(|(m, _)| m).map_err(|reason| CommandError::ImportFailed { file: file.clone(), reason })?;

    let songs = incoming_songs(&manifest);
    let existing = with_store(&db, |store| store.songs())?;
    let sorted = plan(&existing, &songs);
    let omitted = manifest
        .items
        .iter()
        .filter_map(|v| serde_json::from_value::<PlanItem>(v.clone()).ok())
        .filter_map(|i| match i.content {
            ItemContent::Media { path, .. } if plan_file::parse_ref(&path).is_none() => Some(path),
            _ => None,
        })
        .collect();
    let preview = PlanImportPreview {
        file: file.clone(),
        name: manifest.name.clone(),
        section: manifest.section.clone(),
        date: manifest.date.clone(),
        app: manifest.app.clone(),
        items: manifest.items.len(),
        files: manifest.files.len(),
        bytes: manifest.files.iter().map(|f| f.size).sum(),
        omitted,
        fresh: sorted.fresh.iter().map(|&i| summary(&songs[i])).collect(),
        identical: sorted.identical.len(),
        conflicts: sorted.conflicts.iter().map(|(i, _)| summary(&songs[*i])).collect(),
        bibles: manifest.bibles.clone(),
        studio: incoming_studio(&manifest).into_iter().map(|s| s.title).collect(),
        studio_conflicts: {
            let existing = with_store(&db, |store| store.studio_songs())?;
            incoming_studio(&manifest)
                .into_iter()
                .filter(|s| existing.iter().any(|e| e.id == s.id && !same_text(e, s)))
                .map(|s| s.title)
                .collect()
        },
    };
    *state.0.lock().unwrap() = Some(PendingPlan { path, file, manifest });
    Ok(Some(preview))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanImportResult {
    plan_id: String,
    items: usize,
    skipped: usize,
    added: usize,
    replaced: usize,
    /// Canciones de Media Studio guardadas (nuevas o sustituidas).
    studio: usize,
}

/// Guarda el plan leído: como plan nuevo con `new_name`, o al final del plan `append_to`.
#[tauri::command]
pub async fn plan_import_apply(
    app: AppHandle,
    state: State<'_, PlanImportState>,
    db: State<'_, Db>,
    on_conflict: OnConflict,
    imported_suffix: String,
    new_name: Option<String>,
    append_to: Option<String>,
) -> Result<PlanImportResult, CommandError> {
    let pending = state.0.lock().unwrap().take().ok_or_else(|| internal("no hay ningún plan leído para importar"))?;
    let file = pending.file.clone();
    let failed = |reason: String| CommandError::ImportFailed { file: file.clone(), reason };
    let target = match &append_to {
        Some(id) => Some(with_store(&db, |store| store.plan(id))?.ok_or_else(|| internal("el plan ya no existe"))?),
        None => None,
    };

    // Los archivos se sacan a una carpeta temporal (comprobando cada uno) y de ahí van a su sitio.
    let staging = app.path().app_cache_dir().map_err(internal)?.join(format!("importando-{}", uuid::Uuid::new_v4()));
    let path = pending.path.clone();
    let manifest = pending.manifest;
    let entries = manifest.files.clone();
    let stage = staging.clone();
    let extracted = tauri::async_runtime::spawn_blocking(move || {
        let (_, mut reader) = plan_file::open(&path)?;
        plan_file::extract(&mut reader, &entries, &stage)
    })
    .await
    .map_err(internal)?
    .map_err(&failed);
    let extracted = match extracted {
        Ok(paths) => paths,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(e);
        }
    };
    let data = app.path().app_data_dir().map_err(internal)?;
    let dirs = Dirs {
        backgrounds: data.join("fondos"),
        media: data.join(crate::features::plans::files::DIR),
        tracks: data.join(crate::features::studio::AUDIO_DIR),
    };
    let allow = |path: &Path| allow_media(&app, path);
    let result = place(&db, &manifest, &extracted, on_conflict, &imported_suffix, new_name, target, &dirs, allow).map_err(|e| match e {
        PlaceError::Command(e) => e,
        PlaceError::Io(reason) => failed(reason),
    });
    let _ = std::fs::remove_dir_all(&staging);
    result
}

enum PlaceError {
    Command(CommandError),
    Io(String),
}

impl From<CommandError> for PlaceError {
    fn from(e: CommandError) -> Self {
        Self::Command(e)
    }
}

/// Dónde van los fondos de las letras y los archivos de los elementos.
struct Dirs {
    backgrounds: PathBuf,
    media: PathBuf,
    /// Pistas de Media Studio (D-029).
    tracks: PathBuf,
}

/// Mismo título, secciones y orden (sin mirar archivos ni tiempos): para la vista previa, antes de copiar nada.
fn same_text(a: &MediaSong, b: &MediaSong) -> bool {
    let key = |s: &MediaSong| (s.title.clone(), s.sections.iter().map(|x| (x.id.clone(), x.slides.clone())).collect::<Vec<_>>(), s.order.iter().map(|e| e.section.clone()).collect::<Vec<_>>());
    key(a) == key(b)
}

/// Las canciones de Media Studio que trae el archivo (las que esta versión entiende).
fn incoming_studio(manifest: &PlanManifest) -> Vec<MediaSong> {
    manifest.studio_songs.iter().filter_map(|v| serde_json::from_value(v.clone()).ok()).collect()
}

/// Guarda letras, archivos y plan. Sin Tauri: `allow` da permiso a cada archivo guardado.
#[allow(clippy::too_many_arguments)]
fn place(
    db: &Db,
    manifest: &PlanManifest,
    extracted: &[PathBuf],
    on_conflict: OnConflict,
    imported_suffix: &str,
    new_name: Option<String>,
    target: Option<Plan>,
    dirs: &Dirs,
    allow: impl Fn(&Path) -> Result<(), CommandError>,
) -> Result<PlanImportResult, PlaceError> {
    // Letras: como al importar letras (iguales se reusan, distintas según `on_conflict`).
    let songs = incoming_songs(manifest);
    let existing = with_store(db, |store| store.songs())?;
    let sorted = plan(&existing, &songs);
    let mut song_backgrounds = std::collections::HashMap::new();
    for &i in sorted.fresh.iter().chain(sorted.conflicts.iter().map(|(i, _)| i)) {
        let Some(f) = songs[i].background else { continue };
        if song_backgrounds.contains_key(&f) {
            continue;
        }
        let bytes = std::fs::read(&extracted[f]).map_err(|e| PlaceError::Io(e.to_string()))?;
        let saved = save_background(&dirs.backgrounds, &Background { name: manifest.files[f].name.clone(), bytes }).map_err(PlaceError::Io)?;
        allow(&saved)?;
        song_backgrounds.insert(f, saved.to_string_lossy().into_owned());
    }
    let resolution = resolve(
        &existing,
        &songs,
        &sorted,
        on_conflict,
        imported_suffix,
        |f| song_backgrounds.get(&f).cloned(),
        || uuid::Uuid::new_v4().to_string(),
    );
    let song_ids: std::collections::HashMap<String, String> = manifest
        .songs
        .iter()
        .enumerate()
        .filter_map(|(i, s)| resolution.ids.get(&i).map(|id| (s.id.clone(), id.clone())))
        .collect();

    // Archivos de los elementos: a la carpeta de medios de los planes, con su nombre.
    let mut stored = std::collections::HashMap::new();
    for value in &manifest.items {
        let Ok(item) = serde_json::from_value::<PlanItem>(value.clone()) else { continue };
        let reference = match &item.content {
            ItemContent::Media { path, .. } => plan_file::parse_ref(path),
            ItemContent::Pause { background: Some(path), .. } => plan_file::parse_ref(path),
            _ => None,
        };
        let Some(f) = reference.filter(|f| *f < extracted.len() && !stored.contains_key(f)) else { continue };
        let name = std::ffi::OsString::from(&manifest.files[f].name);
        let copy = crate::features::plans::files::store_copy_named(&dirs.media, &extracted[f], &name).map_err(|e| PlaceError::Io(e.to_string()))?;
        allow(&copy)?;
        stored.insert(f, copy.to_string_lossy().into_owned());
    }

    // Canciones de Media Studio: la pista a `pistas`, los fondos a `fondos`; solo archivos de su tipo.
    let existing_studio = with_store(db, |store| store.studio_songs())?;
    let mut tracks = std::collections::HashMap::new();
    let mut studio_backgrounds = std::collections::HashMap::new();
    let mut studio_ids = std::collections::HashMap::new();
    let mut studio_added = 0;
    for song in incoming_studio(manifest) {
        let mut copy_ref = |path: &str, audio: bool| -> Result<(), PlaceError> {
            let Some(f) = plan_file::parse_ref(path).filter(|f| *f < extracted.len()) else { return Ok(()) };
            let name = std::ffi::OsString::from(&manifest.files[f].name);
            let named = Path::new(&name);
            let (map, dir, ok) = if audio {
                (&mut tracks, &dirs.tracks, is_track_file(named))
            } else {
                (&mut studio_backgrounds, &dirs.backgrounds, is_background_file(named))
            };
            if !ok || map.contains_key(&f) {
                return Ok(());
            }
            let copy = crate::features::plans::files::store_copy_named(dir, &extracted[f], &name).map_err(|e| PlaceError::Io(e.to_string()))?;
            if !audio {
                allow(&copy)?;
            }
            map.insert(f, copy.to_string_lossy().into_owned());
            Ok(())
        };
        if let Some(audio) = &song.audio {
            copy_ref(&audio.path, true)?;
        }
        for path in song.background.iter().chain(song.sections.iter().filter_map(|s| s.background.as_ref())) {
            copy_ref(path, false)?;
        }
        let mut song = plan_transfer::unpack_studio(song, &tracks, &studio_backgrounds);
        let original = song.id.clone();
        match existing_studio.iter().find(|e| e.id == song.id) {
            // La misma canción (se importa otra vez): se usa la que ya está.
            Some(e) if plan_transfer::same_studio_song(e, &song) => {}
            Some(_) if on_conflict == OnConflict::Skip => {}
            Some(_) if on_conflict == OnConflict::KeepBoth => {
                song.id = uuid::Uuid::new_v4().to_string();
                song.title = format!("{} {imported_suffix}", song.title);
                with_store(db, |store| store.save_studio_song(&song))?;
                studio_added += 1;
            }
            _ => {
                with_store(db, |store| store.save_studio_song(&song))?;
                studio_added += 1;
            }
        }
        studio_ids.insert(original, song.id.clone());
    }

    let (items, skipped) = plan_transfer::unpack(&manifest.items, &stored, &song_ids, &studio_ids, || uuid::Uuid::new_v4().to_string());
    let count = items.len();
    let plan = match target {
        Some(mut plan) => {
            plan.items.extend(items);
            plan
        }
        None => Plan {
            id: uuid::Uuid::new_v4().to_string(),
            name: new_name.filter(|n| !n.trim().is_empty()).map(|n| n.trim().to_string()).unwrap_or_else(|| manifest.name.clone()),
            date: manifest.date.clone(),
            notes: manifest.notes.clone(),
            animate: manifest.animate,
            items,
        },
    };
    with_store(db, |store| store.import_songs(&resolution.save, Some(&plan)))?;
    Ok(PlanImportResult { plan_id: plan.id, items: count, skipped, added: resolution.added, replaced: resolution.replaced, studio: studio_added })
}

/// El usuario cerró la vista previa del plan sin importar.
#[tauri::command]
pub fn plan_import_cancel(state: State<'_, PlanImportState>) {
    state.0.lock().unwrap().take();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::db::Store;
    use crate::features::media::domain::MediaKind;
    use crate::features::output::domain::{Marks, RichBlock, RichRun, BlockType};
    use crate::features::songs::domain::Song;
    use std::sync::Mutex;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("emetpro-plantransfer-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn item(id: &str, content: ItemContent) -> PlanItem {
        PlanItem { id: id.into(), content }
    }

    #[test]
    fn de_un_equipo_a_otro() {
        // Equipo A: un plan con una sección «Alabanza» y otra «Final».
        let a = temp("a");
        let fondo = a.join("fondo.jpg");
        let anuncio = a.join("anuncio.png");
        let video = a.join("IMG_0001.MOV");
        std::fs::write(&fondo, b"jpg").unwrap();
        std::fs::write(&anuncio, b"png").unwrap();
        std::fs::write(&video, vec![1u8; 1000]).unwrap();
        let song = Song {
            id: "s-a".into(),
            title: "Admirable".into(),
            singer: "Danilo".into(),
            writer: String::new(),
            body: "Uno\n**\nDos".into(),
            background: Some(fondo.to_string_lossy().into_owned()),
            animation: None,
        };
        // Una canción de Media Studio con su pista y un fondo generado (D-029).
        let pista = a.join("pista.mp3");
        let aurora = a.join("aurora.svg");
        std::fs::write(&pista, vec![7u8; 500]).unwrap();
        std::fs::write(&aurora, b"<svg></svg>").unwrap();
        let media_song: MediaSong = serde_json::from_value(serde_json::json!({
            "id": "ms-a", "title": "Sublime gracia", "mode": "track",
            "audio": {"path": pista.to_string_lossy(), "name": "pista.mp3", "durationMs": 1000},
            "sections": [{"id": "c", "kind": "chorus", "slides": [{"id": "x", "lines": ["Gloria"]}], "background": aurora.to_string_lossy()}],
            "order": [{"id": "e", "section": "c", "timing": {"startMs": 0, "slides": [{"slide": "x", "startMs": 0}]}}]
        }))
        .unwrap();
        let text = vec![vec![RichBlock { kind: BlockType::Title, align: None, runs: vec![RichRun { text: "La Reforma".into(), b: true, i: false, u: false, color: None }] }]];
        let plan = Plan {
            id: "p".into(),
            name: "Culto".into(),
            date: Some("2026-11-22".into()),
            notes: String::new(),
            animate: None,
            items: vec![
                item("s1", ItemContent::Section { title: "Alabanza".into(), animate: None }),
                item("c", ItemContent::Song { song_id: "s-a".into(), marks: Marks::new() }),
                item("m", ItemContent::StudioSong { studio_song_id: "ms-a".into() }),
                item("i", ItemContent::Media { path: anuncio.to_string_lossy().into_owned(), media: MediaKind::Image }),
                item("v", ItemContent::Media { path: video.to_string_lossy().into_owned(), media: MediaKind::Video }),
                item("t", ItemContent::Text { title: "Sermón".into(), slides: text, marks: Marks::new() }),
                item("b", ItemContent::Scripture { book: 43, chapter: 3, verse_start: Some(16), verse_end: None, bible: Some("NTV".into()), marks: Marks::new() }),
                item("s2", ItemContent::Section { title: "Final".into(), animate: None }),
                item("p", ItemContent::Pause { title: "Receso".into(), duration_ms: 1000, background: None }),
            ],
        };
        let bibles = vec![
            BibleRef { id: "NTV".into(), abbreviation: "NTV".into(), name: "Nueva Traducción Viviente".into() },
            BibleRef { id: "RVR".into(), abbreviation: "RVR1960".into(), name: "Reina-Valera".into() },
        ];
        let items = plan_transfer::select(&plan.items, Some("s1"));
        let (manifest, files, omitted) =
            build_export(&plan, &items, std::slice::from_ref(&song), std::slice::from_ref(&media_song), false, bibles, Some("s1"));
        assert_eq!(omitted, 1, "el video no va");
        assert_eq!(manifest.section.as_deref(), Some("Alabanza"));
        assert_eq!(manifest.bibles.len(), 1, "solo la versión que usan las citas");
        assert_eq!(files.len(), 4, "el anuncio, el fondo de la letra, y la pista y el fondo de Media Studio");
        assert_eq!(manifest.studio_songs.len(), 1);
        assert_eq!(manifest.studio_songs[0]["audio"]["path"], "@2");
        let file = a.join("culto.emetplan");
        plan_file::write(&file, manifest, &files).unwrap();

        // Equipo B: ya tiene «Admirable» con otra letra, y un plan del culto general.
        let b = temp("b");
        let db = Db(Mutex::new(Some(Store::open(&b))));
        let own = Song { id: "s-b".into(), title: "Admirable".into(), singer: "Danilo".into(), writer: String::new(), body: "Otra".into(), background: None, animation: None };
        let general = Plan { id: "g".into(), name: "General".into(), date: None, notes: String::new(), animate: None, items: vec![item("x", ItemContent::Section { title: "Bienvenida".into(), animate: None })] };
        with_store(&db, |s| s.import_songs(std::slice::from_ref(&own), Some(&general))).unwrap();

        let (manifest, mut reader) = plan_file::open(&file).unwrap();
        let extracted = plan_file::extract(&mut reader, &manifest.files, &b.join("staging")).unwrap();
        let dirs = Dirs { backgrounds: b.join("fondos"), media: b.join("medios"), tracks: b.join("pistas") };
        let result = place(&db, &manifest, &extracted, OnConflict::KeepBoth, "(importada)", Some("Culto recibido".into()), None, &dirs, |_| Ok(()))
            .unwrap_or_else(|_| panic!("no se pudo importar"));
        assert_eq!(result.items, 7);
        assert_eq!(result.added, 1);
        assert_eq!(result.studio, 1);
        let studio = with_store(&db, |s| s.studio_songs()).unwrap();
        let received = studio.iter().find(|s| s.id == "ms-a").expect("la canción de Media Studio entra");
        let track = &received.audio.as_ref().unwrap().path;
        assert!(track.contains("pistas") && Path::new(track).is_file(), "la pista, en la carpeta de pistas: {track}");
        let bg = received.sections[0].background.as_deref().unwrap();
        assert!(bg.contains("fondos") && bg.ends_with("aurora.svg") && Path::new(bg).is_file());
        assert_eq!(received.order, media_song.order, "con sus tiempos");

        let saved = with_store(&db, |s| s.plan(&result.plan_id)).unwrap().unwrap();
        assert_eq!(saved.name, "Culto recibido");
        assert_eq!(saved.date.as_deref(), Some("2026-11-22"));
        let songs = with_store(&db, |s| s.songs()).unwrap();
        let imported = songs.iter().find(|s| s.title == "Admirable (importada)").expect("la letra entra aparte");
        assert!(imported.background.as_ref().is_some_and(|p| Path::new(p).is_file() && p.contains("fondos")));
        for i in &saved.items {
            match &i.content {
                ItemContent::Song { song_id, .. } => assert_eq!(song_id, &imported.id),
                ItemContent::Media { path, media: MediaKind::Image } => {
                    assert!(path.ends_with("anuncio.png") && Path::new(path).is_file() && path.contains("medios"));
                }
                ItemContent::Media { path, media: MediaKind::Video } => assert_eq!(path, "IMG_0001.MOV"),
                ItemContent::Text { slides, .. } => assert_eq!(slides[0][0].runs[0].text, "La Reforma"),
                ItemContent::Scripture { bible, .. } => assert_eq!(bible.as_deref(), Some("NTV")),
                _ => {}
            }
            if let ItemContent::StudioSong { studio_song_id } = &i.content {
                assert_eq!(studio_song_id, "ms-a");
            }
            assert!(!["s1", "c", "m", "i", "v", "t", "b"].contains(&i.id.as_str()), "ids nuevos");
        }

        // Otra vez, al final del plan general.
        let (manifest, mut reader) = plan_file::open(&file).unwrap();
        let extracted = plan_file::extract(&mut reader, &manifest.files, &b.join("staging2")).unwrap();
        let result = place(&db, &manifest, &extracted, OnConflict::Skip, "(importada)", None, Some(general), &dirs, |_| Ok(()))
            .unwrap_or_else(|_| panic!("no se pudo añadir"));
        assert_eq!(result.plan_id, "g");
        assert_eq!(result.studio, 0, "la misma canción de Media Studio no se duplica");
        assert_eq!(with_store(&db, |s| s.studio_songs()).unwrap().len(), 1);
        let general = with_store(&db, |s| s.plan("g")).unwrap().unwrap();
        assert_eq!(general.items.len(), 8, "la bienvenida y la sección importada");
        assert!(matches!(&general.items[1].content, ItemContent::Section { title, .. } if title == "Alabanza"));

        std::fs::remove_dir_all(&a).unwrap();
        std::fs::remove_dir_all(&b).unwrap();
    }
}

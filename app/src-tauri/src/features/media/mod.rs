// Medios: imágenes, videos, PDF y presentaciones elegidos del equipo, y fondos.
// Plugin interno «media»: la interfaz llama a sus comandos como `plugin:media|<comando>` (platform/tauri.ts).
mod commands;
pub mod domain;
mod scan;
mod video;

use tauri::plugin::{Builder, TauriPlugin};
use tauri::Wry;

pub const NAME: &str = "media";

/// El audio de un video, para otra funcionalidad (Media Studio, D-029). Err(None) = no hay avconvert ni ffmpeg.
pub fn extract_audio(source: &std::path::Path, target: &std::path::Path) -> Result<(), Option<String>> {
    video::extract_audio(source, target).map_err(|e| match e {
        video::VideoError::NoConverter => None,
        video::VideoError::Failed(reason) => Some(reason),
    })
}

pub fn plugin() -> TauriPlugin<Wry> {
    Builder::new(NAME)
        .invoke_handler(tauri::generate_handler![
            commands::pick_media,
            commands::pick_media_folder,
            commands::pick_background,
            commands::allow_backgrounds,
            commands::playable_video,
            commands::convert_video,
        ])
        .build()
}

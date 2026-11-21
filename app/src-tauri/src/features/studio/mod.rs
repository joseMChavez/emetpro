// Media Studio (D-029): canciones con pista sincronizada o para interpretar en directo, con secciones que se
// repiten según un orden. Plugin interno «studio»: la interfaz llama a sus comandos como `plugin:studio|<comando>`.
mod commands;
pub mod domain;
pub mod repo;

use tauri::plugin::{Builder, TauriPlugin};
use tauri::Wry;

pub const NAME: &str = "studio";

/// Carpeta de las pistas dentro de la de la app. Aparte de `medios`: la limpieza de los planes no la toca.
pub const AUDIO_DIR: &str = "pistas";

pub fn plugin() -> TauriPlugin<Wry> {
    Builder::new(NAME)
        .invoke_handler(tauri::generate_handler![
            commands::studio_list,
            commands::studio_validate,
            commands::studio_save,
            commands::studio_delete,
            commands::studio_pick_audio,
            commands::studio_read_audio,
        ])
        .build()
}

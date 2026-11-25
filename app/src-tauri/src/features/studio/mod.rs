// Media Studio (D-029): canciones con pista sincronizada o para interpretar en directo, con secciones que se
// repiten según un orden. Plugin interno «studio»: la interfaz llama a sus comandos como `plugin:studio|<comando>`.
mod commands;
pub mod domain;
pub mod repo;
pub mod separate;
pub mod whisper;

use tauri::plugin::{Builder, TauriPlugin};
use tauri::Wry;

pub const NAME: &str = "studio";

/// Carpeta de las pistas dentro de la de la app. Aparte de `medios`: la limpieza de los planes no la toca.
pub const AUDIO_DIR: &str = "pistas";

impl From<whisper::WhisperError> for crate::core::error::CommandError {
    fn from(e: whisper::WhisperError) -> Self {
        use crate::core::error::CommandError;
        match e {
            whisper::WhisperError::Busy => CommandError::WhisperBusy,
            whisper::WhisperError::Unsupported => CommandError::WhisperUnsupported,
            whisper::WhisperError::Canceled => CommandError::WhisperCanceled,
            whisper::WhisperError::Download(message) => CommandError::WhisperDownload { message },
            whisper::WhisperError::Failed(reason) => CommandError::TranscribeFailed { reason },
        }
    }
}

impl From<separate::runtime::SeparatorError> for crate::core::error::CommandError {
    fn from(e: separate::runtime::SeparatorError) -> Self {
        use crate::core::error::CommandError;
        use separate::runtime::SeparatorError;
        match e {
            SeparatorError::Busy => CommandError::SeparatorBusy,
            SeparatorError::Canceled => CommandError::SeparatorCanceled,
            SeparatorError::Unavailable => CommandError::SeparatorUnavailable,
            SeparatorError::Missing => CommandError::SeparatorMissing,
            SeparatorError::Download(message) => CommandError::SeparatorDownload { message },
            SeparatorError::Failed(reason) => CommandError::SeparateFailed { reason },
        }
    }
}

pub fn plugin() -> TauriPlugin<Wry> {
    Builder::new(NAME)
        .invoke_handler(tauri::generate_handler![
            commands::studio_list,
            commands::studio_validate,
            commands::studio_save,
            commands::studio_delete,
            commands::studio_pick_audio,
            commands::studio_read_audio,
            commands::studio_open_reference,
            commands::studio_save_svg,
            commands::studio_whisper_status,
            commands::studio_whisper_download,
            commands::studio_whisper_cancel,
            commands::studio_whisper_remove,
            commands::studio_transcribe,
            commands::studio_separator_status,
            commands::studio_separator_download,
            commands::studio_separator_cancel,
            commands::studio_separator_remove,
            commands::studio_separate,
        ])
        .build()
}

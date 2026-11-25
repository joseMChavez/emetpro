// Errores que devuelven los comandos a la interfaz (campo «code»; la interfaz los traduce en messages.ts).
// Cada funcionalidad convierte sus propios errores con un `impl From<…> for CommandError` en su módulo.
use serde::Serialize;

// Los errores viajan como códigos para que la interfaz los muestre en el idioma elegido.
#[derive(Debug, Serialize)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum CommandError {
    MonitorDisconnected,
    FileMissing { path: String },
    InvalidContent,
    InvalidBible { file: String, reason: String },
    BibleNotFound,
    ObsNotInstalled,
    /// Hay que cerrar OBS para cambiar su configuración.
    ObsRunning,
    /// OBS está abierto en modo seguro (sin WebSocket).
    ObsSafeMode,
    ObsNotConnected,
    Obs { message: String },
    /// Para mostrar presentaciones hace falta LibreOffice.
    OfficeNotInstalled,
    ConversionFailed { file: String, reason: String },
    /// Para convertir un video que este equipo no reproduce hace falta avconvert (macOS) o ffmpeg.
    VideoConverterMissing,
    /// Ya se está descargando LibreOffice (o borrando la copia).
    OfficeBusy,
    /// El usuario canceló la descarga.
    OfficeCanceled,
    OfficeDownload { message: String },
    /// El archivo de letras no se pudo leer (formato desconocido, dañado…).
    ImportFailed { file: String, reason: String },
    ExportFailed { reason: String },
    /// No se pudo copiar un medio a la carpeta de la app (disco lleno, sin permiso…).
    CopyFailed { file: String, reason: String },
    /// No se pudo encender el control remoto (puertos ocupados, permisos…).
    RemoteFailed { reason: String },
    /// Ya se está descargando el reconocedor de voz o sacando una letra.
    WhisperBusy,
    /// Este procesador es demasiado antiguo para el reconocedor de voz.
    WhisperUnsupported,
    /// El usuario canceló la descarga del reconocedor o la transcripción.
    WhisperCanceled,
    /// Para sacar la letra hace falta descargar antes el reconocedor de voz.
    WhisperModelMissing,
    WhisperDownload { message: String },
    TranscribeFailed { reason: String },
    /// Ya se está descargando el separador o quitando la voz de otra pista.
    SeparatorBusy,
    SeparatorCanceled,
    /// No hay separador para este sistema (Mac con Intel, procesadores ARM en Linux…).
    SeparatorUnavailable,
    /// Hace falta descargar antes el separador.
    SeparatorMissing,
    SeparatorDownload { message: String },
    SeparateFailed { reason: String },
    /// No se pudo leer la pista para prepararla.
    PrepareFailed { reason: String },
    /// No se pudo leer o escribir la base de datos local.
    Storage { message: String },
    Internal { message: String },
}

pub fn internal(e: impl std::fmt::Display) -> CommandError {
    CommandError::Internal {
        message: e.to_string(),
    }
}

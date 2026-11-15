// Videos que el webview no puede reproducir (por ejemplo HEVC/H.265 de un iPhone, a menudo con HDR):
// se convierten una vez a MP4 H.264, que todos los webviews reproducen, con lo que haya en el equipo:
// `avconvert` (viene con macOS) o `ffmpeg` si está instalado. La copia se guarda en caché y se rehace si el
// original cambia; los planes, la biblioteca y el teléfono siguen guardando la ruta del original.
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;

/// De una en una: convertir video ocupa todo el procesador.
static CONVERTING: Mutex<()> = Mutex::new(());

pub enum VideoError {
    /// No hay avconvert ni ffmpeg.
    NoConverter,
    Failed(String),
}

enum Converter {
    AvConvert(PathBuf),
    Ffmpeg(PathBuf),
}

impl Converter {
    fn name(&self) -> &'static str {
        match self {
            Self::AvConvert(_) => "avconvert",
            Self::Ffmpeg(_) => "ffmpeg",
        }
    }

    fn command(&self, source: &Path, target: &Path) -> Command {
        match self {
            Self::AvConvert(exe) => {
                // El tipo de archivo sale de la extensión (.mp4); los presets que no son HEVC graban en H.264.
                let mut command = Command::new(exe);
                command.args(["-p", "Preset1920x1080", "-s"]).arg(source).arg("-o").arg(target);
                command
            }
            Self::Ffmpeg(exe) => {
                let mut command = Command::new(exe);
                command
                    .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-i"])
                    .arg(source)
                    .args([
                        "-map", "0:v:0", "-map", "0:a:0?",
                        // Cabe en 1920×1920: un video vertical sigue vertical; nunca se agranda.
                        "-vf", "scale='if(gte(iw,ih),min(1920,iw),-2)':'if(gte(iw,ih),-2,min(1920,ih))'",
                        "-c:v", "libx264", "-preset", "veryfast", "-crf", "20", "-pix_fmt", "yuv420p",
                        "-c:a", "aac", "-b:a", "160k",
                        "-movflags", "+faststart",
                    ])
                    .arg(target);
                command
            }
        }
    }
}

/// Los convertidores de este equipo: en macOS primero avconvert, que viene con el sistema.
fn converters() -> Vec<Converter> {
    let mut list = Vec::new();
    let avconvert = Path::new("/usr/bin/avconvert");
    if cfg!(target_os = "macos") && avconvert.is_file() {
        list.push(Converter::AvConvert(avconvert.into()));
    }
    if let Some(ffmpeg) = find_ffmpeg() {
        list.push(Converter::Ffmpeg(ffmpeg));
    }
    list
}

/// ffmpeg del PATH o de Homebrew: una app abierta desde el Finder no hereda el PATH de la terminal.
fn find_ffmpeg() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let mut dirs: Vec<PathBuf> =
        std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    if cfg!(target_os = "macos") {
        dirs.push("/opt/homebrew/bin".into());
        dirs.push("/usr/local/bin".into());
    }
    dirs.into_iter().map(|dir| dir.join(exe)).find(|p| p.is_file())
}

/// Dónde va la copia de esa versión del archivo (ruta, tamaño y fecha de modificación).
fn target(source: &Path, cache: &Path) -> Option<PathBuf> {
    let meta = std::fs::metadata(source).ok()?;
    let modified = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs());
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    meta.len().hash(&mut hasher);
    modified.hash(&mut hasher);
    Some(cache.join(format!("{:016x}.mp4", hasher.finish())))
}

/// La copia ya convertida, si existe.
pub fn converted(source: &Path, cache: &Path) -> Option<PathBuf> {
    target(source, cache).filter(|p| p.is_file())
}

/// Convierte (o toma la copia que ya había) y devuelve la ruta del MP4.
pub fn convert(source: &Path, cache: &Path) -> Result<PathBuf, VideoError> {
    let converters = converters();
    if converters.is_empty() {
        return Err(VideoError::NoConverter);
    }
    let _turn = CONVERTING.lock().unwrap_or_else(|e| e.into_inner());
    let target = target(source, cache).ok_or_else(|| VideoError::Failed("no se pudo leer el archivo".into()))?;
    if target.is_file() {
        return Ok(target);
    }
    std::fs::create_dir_all(cache).map_err(|e| VideoError::Failed(e.to_string()))?;
    // A medio convertir no tiene que parecer listo: se escribe aparte y se renombra al final.
    let partial = target.with_extension("part.mp4");

    let mut failures = Vec::new();
    for converter in &converters {
        let _ = std::fs::remove_file(&partial);
        match run(converter.command(source, &partial)) {
            Ok(()) if partial.is_file() => {
                std::fs::rename(&partial, &target).map_err(|e| VideoError::Failed(e.to_string()))?;
                return Ok(target);
            }
            Ok(()) => failures.push(format!("{}: no creó el archivo", converter.name())),
            Err(reason) => failures.push(format!("{}: {reason}", converter.name())),
        }
        eprintln!("[video] {}", failures.last().map(String::as_str).unwrap_or_default());
    }
    let _ = std::fs::remove_file(&partial);
    Err(VideoError::Failed(failures.join(" · ")))
}

/// Err con la última línea que el programa escribió como error.
fn run(mut command: Command) -> Result<(), String> {
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command.output().map_err(|e| e.to_string())?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let last = stderr.lines().map(str::trim).rev().find(|l| !l.is_empty()).map(str::to_string);
    Err(last.unwrap_or_else(|| format!("terminó con error ({})", output.status)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_copia_depende_del_archivo() {
        let dir = std::env::temp_dir().join(format!("emetpro-video-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("IMG_0001.MOV");
        std::fs::write(&source, b"uno").unwrap();
        let cache = dir.join("cache");
        let first = target(&source, &cache).unwrap();
        assert_eq!(first.extension().unwrap(), "mp4");
        assert!(first.starts_with(&cache));
        assert_eq!(converted(&source, &cache), None);

        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(&first, b"mp4").unwrap();
        assert_eq!(converted(&source, &cache), Some(first.clone()));

        // Otro contenido (otro tamaño): otra copia.
        std::fs::write(&source, b"otro mas largo").unwrap();
        assert_ne!(target(&source, &cache).unwrap(), first);
        assert_eq!(target(&dir.join("no-existe.mov"), &cache), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Necesita ffmpeg (o avconvert en macOS) y un video: EMETPRO_VIDEO_PRUEBA=/ruta/IMG_0001.MOV cargo test -- --ignored convierte
    #[test]
    #[ignore]
    fn convierte_un_video() {
        let source = PathBuf::from(std::env::var("EMETPRO_VIDEO_PRUEBA").expect("EMETPRO_VIDEO_PRUEBA"));
        let cache = std::env::temp_dir().join(format!("emetpro-video-conv-{}", std::process::id()));
        let copy = convert(&source, &cache).unwrap_or_else(|e| match e {
            VideoError::NoConverter => panic!("sin avconvert ni ffmpeg"),
            VideoError::Failed(reason) => panic!("{reason}"),
        });
        assert!(copy.is_file());
        assert_eq!(converted(&source, &cache), Some(copy.clone()));
        assert!(!copy.with_extension("part.mp4").exists());
        eprintln!("copia: {}", copy.display());
    }
}

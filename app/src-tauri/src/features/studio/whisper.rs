// Sacar la letra de una pista (Media Studio, D-029) con whisper.cpp, compilado dentro de la app (whisper-rs).
// El modelo (~190 MB) no va en el instalador: se descarga solo cuando el usuario lo pide, de una revisión fija y
// con su tamaño y SHA-256 comprobados (como la copia de LibreOffice, D-015). Lo que sale es un borrador: la
// interfaz lo limpia, lo arma en secciones y el usuario lo revisa antes de usarlo (whisper.ts).
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Whisper «small» cuantizado (q5_1), de ggml-org/whisper.cpp en Hugging Face (MIT, como los pesos de OpenAI).
/// Elegido en una prueba con canciones cantadas: el «large-v3-turbo» tardaba seis veces más, se saltaba medio verso
/// y se inventaba texto en una pista instrumental; este marca la música como música y corta una línea por frase.
pub const MODEL_FILE: &str = "ggml-small-q5_1.bin";
const MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-small-q5_1.bin";
pub const MODEL_SIZE: u64 = 190_085_487;
const MODEL_SHA256: &str = "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb";

/// Frecuencia que espera Whisper.
pub const SAMPLE_RATE: u32 = 16_000;
/// Más que esto no es una canción (y no cabría en memoria con holgura): 20 minutos.
pub const MAX_SECONDS: u32 = 20 * 60;

pub enum WhisperError {
    /// Ya se está descargando el modelo o sacando una letra.
    Busy,
    /// El procesador no tiene las instrucciones con que se compiló whisper.cpp.
    Unsupported,
    Canceled,
    Download(String),
    Failed(String),
}

fn failed(e: impl std::fmt::Display) -> WhisperError {
    WhisperError::Failed(e.to_string())
}

static BUSY: AtomicBool = AtomicBool::new(false);
static CANCEL: AtomicBool = AtomicBool::new(false);

/// Una sola tarea a la vez (descargar o transcribir): las dos usan mucho disco o procesador.
struct Turn;

impl Turn {
    fn take() -> Result<Self, WhisperError> {
        BUSY.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).map_err(|_| WhisperError::Busy)?;
        CANCEL.store(false, Ordering::SeqCst);
        Ok(Self)
    }
}

impl Drop for Turn {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::SeqCst);
    }
}

/// Los instaladores compilan whisper.cpp sin `GGML_NATIVE` pero con AVX, AVX2, FMA, F16C y BMI2 (lo habitual desde
/// 2013–2015). En un procesador sin ellas, whisper.cpp cerraría la app de golpe: se comprueba antes y se avisa.
pub fn cpu_supported() -> bool {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        std::arch::is_x86_feature_detected!("avx")
            && std::arch::is_x86_feature_detected!("avx2")
            && std::arch::is_x86_feature_detected!("fma")
            && std::arch::is_x86_feature_detected!("f16c")
            && std::arch::is_x86_feature_detected!("bmi2")
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        true
    }
}

pub fn busy() -> bool {
    BUSY.load(Ordering::SeqCst)
}

/// Pide parar la descarga o la transcripción en curso.
pub fn cancel() {
    CANCEL.store(true, Ordering::SeqCst);
}

fn root(data: &Path) -> PathBuf {
    data.join("whisper")
}

/// El modelo, si está descargado entero (solo se renombra a su nombre final tras comprobarlo).
pub fn model(data: &Path) -> Option<PathBuf> {
    let path = root(data).join(MODEL_FILE);
    path.is_file().then_some(path)
}

pub fn remove(data: &Path) -> Result<(), WhisperError> {
    let _turn = Turn::take()?;
    match std::fs::remove_dir_all(root(data)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(failed(e)),
    }
}

/// Descarga el modelo con su avance (bytes); se deja en su sitio solo si el tamaño y la suma coinciden.
pub async fn download(data: &Path, mut progress: impl FnMut(u64) + Send) -> Result<PathBuf, WhisperError> {
    let _turn = Turn::take()?;
    let root = root(data);
    std::fs::create_dir_all(&root).map_err(failed)?;
    let part = root.join(format!("{MODEL_FILE}.part"));
    let result = fetch(&part, &mut progress).await;
    if let Err(e) = result {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    let path = root.join(MODEL_FILE);
    std::fs::rename(&part, &path).map_err(failed)?;
    Ok(path)
}

async fn fetch(file: &Path, progress: &mut impl FnMut(u64)) -> Result<(), WhisperError> {
    let download = |e: reqwest::Error| WhisperError::Download(e.to_string());
    let client = reqwest::Client::builder()
        .user_agent(concat!("EmetPro/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(download)?;
    let mut response = client.get(MODEL_URL).send().await.and_then(|r| r.error_for_status()).map_err(download)?;
    let mut out = std::fs::File::create(file).map_err(failed)?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0u64;
    let mut reported = Instant::now();
    while let Some(chunk) = response.chunk().await.map_err(download)? {
        if CANCEL.load(Ordering::SeqCst) {
            return Err(WhisperError::Canceled);
        }
        if downloaded + chunk.len() as u64 > MODEL_SIZE {
            return Err(WhisperError::Download("el archivo es más grande de lo esperado".into()));
        }
        out.write_all(&chunk).map_err(failed)?;
        hasher.update(&chunk);
        downloaded += chunk.len() as u64;
        if reported.elapsed() > Duration::from_millis(250) {
            reported = Instant::now();
            progress(downloaded);
        }
    }
    out.flush().map_err(failed)?;
    progress(downloaded);
    check(&format!("{:x}", hasher.finalize()), downloaded).map_err(WhisperError::Download)
}

/// La descarga tiene que ser exactamente el modelo fijado.
fn check(sha256: &str, size: u64) -> Result<(), String> {
    if size != MODEL_SIZE {
        return Err(format!("descarga incompleta ({size} de {MODEL_SIZE} bytes)"));
    }
    if sha256 != MODEL_SHA256 {
        return Err("la suma SHA-256 no coincide".into());
    }
    Ok(())
}

/// Un tramo de lo que se oyó, con su texto tal cual lo da Whisper.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    pub start_ms: u32,
    pub end_ms: u32,
    pub text: String,
    /// Probabilidad de que no haya voz (0 a 1): alta en tramos de música sola.
    pub no_speech: f32,
    /// Sus palabras con el tiempo de cada una (para sincronizar las diapositivas con lo que se canta).
    pub words: Vec<Word>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Word {
    pub text: String,
    pub start_ms: u32,
    pub end_ms: u32,
}

/// Junta los trozos de palabra de Whisper (un trozo que empieza con espacio empieza palabra) y quita los especiales
/// («[_BEG_]», «<|es|>»). Los tiempos van en centésimas de segundo. (Se probó el modo DTW de whisper.cpp: daba las
/// palabras hasta 1,2 s tarde; los de los trozos erraban ±0,5 s, y con la voz separada se ajustan a sus arranques.)
fn join_words(tokens: impl IntoIterator<Item = (String, i64, i64)>) -> Vec<Word> {
    let ms = |t: i64| (t.max(0) * 10).min(u32::MAX as i64) as u32;
    let mut words: Vec<Word> = Vec::new();
    for (text, t0, t1) in tokens {
        if text.starts_with("[_") || text.starts_with("<|") || text.trim().is_empty() {
            continue;
        }
        match words.last_mut() {
            Some(last) if !text.starts_with(' ') => {
                last.text.push_str(&text);
                last.end_ms = ms(t1).max(last.end_ms);
            }
            _ => words.push(Word { text: text.trim().to_string(), start_ms: ms(t0), end_ms: ms(t1) }),
        }
    }
    words
}

/// Idiomas que se pueden pedir; None = que lo adivine.
pub fn language(code: &str) -> Option<&'static str> {
    ["es", "en", "it", "pt", "fr", "de"].into_iter().find(|l| *l == code)
}

/// Transcribe la pista (mono, 16 kHz). Bloquea: se llama desde un hilo aparte. `progress` recibe 0–100.
pub fn transcribe(
    model: &Path,
    samples: &[f32],
    language: Option<&str>,
    prompt: &str,
    progress: impl FnMut(i32) + 'static,
) -> Result<Vec<Segment>, WhisperError> {
    if !cpu_supported() {
        return Err(WhisperError::Unsupported);
    }
    let _turn = Turn::take()?;
    // Sin esto whisper.cpp escribe cientos de líneas en la terminal.
    whisper_rs::install_logging_hooks();
    let context = WhisperContext::new_with_params(model, WhisperContextParameters::default()).map_err(failed)?;
    let mut state = context.create_state().map_err(failed)?;
    // Búsqueda en haz, como en la prueba: más lenta que la voraz, pero se equivoca menos con la voz cantada.
    let mut params = FullParams::new(SamplingStrategy::BeamSearch { beam_size: 5, patience: -1.0 });
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 8);
    params.set_n_threads(threads as i32);
    params.set_language(Some(language.unwrap_or("auto")));
    if !prompt.trim().is_empty() {
        params.set_initial_prompt(prompt);
    }
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_special(false);
    params.set_print_timestamps(false);
    // El tiempo de cada trozo de palabra: con él se sabe cuándo se canta cada palabra de la letra.
    params.set_token_timestamps(true);
    params.set_progress_callback_safe(progress);
    let canceled = Arc::new(AtomicBool::new(false));
    let flag = canceled.clone();
    params.set_abort_callback_safe(move || {
        let stop = CANCEL.load(Ordering::SeqCst);
        if stop {
            flag.store(true, Ordering::SeqCst);
        }
        stop
    });
    let result = state.full(params, samples);
    if canceled.load(Ordering::SeqCst) || CANCEL.load(Ordering::SeqCst) {
        return Err(WhisperError::Canceled);
    }
    result.map_err(failed)?;
    let mut segments = Vec::new();
    for i in 0..state.full_n_segments() {
        let Some(segment) = state.get_segment(i) else { continue };
        let text = segment.to_str_lossy().map_err(failed)?.trim().to_string();
        // Centésimas de segundo → milisegundos.
        let ms = |t: i64| (t.max(0) * 10).min(u32::MAX as i64) as u32;
        let tokens = (0..segment.n_tokens()).filter_map(|t| {
            let token = segment.get_token(t)?;
            let data = token.token_data();
            Some((token.to_str_lossy().ok()?.into_owned(), data.t0, data.t1))
        });
        let words = join_words(tokens);
        segments.push(Segment {
            start_ms: ms(segment.start_timestamp()),
            end_ms: ms(segment.end_timestamp()),
            text,
            no_speech: segment.no_speech_probability(),
            words,
        });
    }
    Ok(segments)
}

/// Muestras f32 en little-endian (como las manda la interfaz) a un vector.
pub fn samples_from_bytes(bytes: &[u8]) -> Option<Vec<f32>> {
    if !bytes.len().is_multiple_of(4) || bytes.is_empty() || bytes.len() / 4 > (MAX_SECONDS * SAMPLE_RATE) as usize {
        return None;
    }
    Some(bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solo_el_modelo_fijado() {
        assert!(check(MODEL_SHA256, MODEL_SIZE).is_ok());
        assert!(check(MODEL_SHA256, MODEL_SIZE - 1).is_err());
        assert!(check(&"0".repeat(64), MODEL_SIZE).is_err());
    }

    #[test]
    fn muestras_de_la_interfaz() {
        let bytes: Vec<u8> = [0.5f32, -1.0].iter().flat_map(|v| v.to_le_bytes()).collect();
        assert_eq!(samples_from_bytes(&bytes), Some(vec![0.5, -1.0]));
        assert_eq!(samples_from_bytes(&[0, 0, 0]), None);
        assert_eq!(samples_from_bytes(&[]), None);
    }

    #[test]
    fn palabras_de_los_trozos() {
        let tokens = [("[_BEG_]", 0, 0), (" No", 50, 60), ("che", 60, 80), (" de", 80, 90), (" paz", 90, 130), (",", 130, 131), ("<|endoftext|>", 0, 0)];
        let words = join_words(tokens.iter().map(|(t, a, b)| (t.to_string(), *a, *b)));
        assert_eq!(words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>(), ["Noche", "de", "paz,"]);
        assert_eq!((words[0].start_ms, words[0].end_ms), (500, 800));
        assert_eq!(words[2].end_ms, 1310);
    }

    #[test]
    fn idiomas_conocidos() {
        assert_eq!(language("es"), Some("es"));
        assert_eq!(language("auto"), None);
        assert_eq!(language("xx; rm"), None);
    }

    /// Descarga el modelo de verdad (190 MB) en una carpeta temporal:
    /// cargo test descarga_de_verdad -- --ignored --nocapture
    #[test]
    #[ignore]
    fn descarga_de_verdad() {
        let dir = std::env::temp_dir().join(format!("emetpro-whisper-{}", std::process::id()));
        let mut last = 0;
        let path = tauri::async_runtime::block_on(download(&dir, |done| last = done)).unwrap_or_else(|_| panic!("falló"));
        assert_eq!(last, MODEL_SIZE);
        assert_eq!(model(&dir), Some(path));
        assert!(!root(&dir).join(format!("{MODEL_FILE}.part")).exists());
        remove(&dir).unwrap_or_else(|_| panic!("no se borró"));
        assert_eq!(model(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Con el modelo y una grabación en el equipo (rutas por variables de entorno):
    /// EMETPRO_WHISPER_MODEL=… EMETPRO_WHISPER_WAV=… cargo test transcribe_de_verdad -- --ignored --nocapture
    #[test]
    #[ignore]
    fn transcribe_de_verdad() {
        let model = PathBuf::from(std::env::var("EMETPRO_WHISPER_MODEL").unwrap());
        let wav = std::fs::read(std::env::var("EMETPRO_WHISPER_WAV").unwrap()).unwrap();
        // WAV PCM 16 bits mono 16 kHz con cabecera de 44 bytes (ffmpeg -ar 16000 -ac 1).
        let samples: Vec<f32> = wav[44..].chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0).collect();
        let started = Instant::now();
        let segments = transcribe(&model, &samples, Some("es"), "Noche de paz", |_| {}).unwrap_or_else(|_| panic!("falló"));
        for s in &segments {
            println!("{:>6} {:>6} {:.2} {}", s.start_ms, s.end_ms, s.no_speech, s.text);
        }
        println!("{} tramos en {:?}", segments.len(), started.elapsed());
        assert!(segments.iter().any(|s| s.text.to_lowercase().contains("noche de paz")));
    }
}

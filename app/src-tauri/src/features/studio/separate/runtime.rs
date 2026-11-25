// El separador no va en el instalador: ONNX Runtime (Microsoft, MIT) y el modelo htdemucs (Meta, MIT) se descargan
// solo cuando el usuario lo pide, de versiones fijas y con su tamaño y SHA-256 comprobados (como el reconocedor de
// voz y la copia de LibreOffice, D-015). ONNX Runtime sale del paquete oficial de PyPI (Microsoft ya no publica los
// archivos sueltos en GitHub): de ese zip solo se guardan la biblioteca y sus avisos de licencia.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use ort::environment::Environment;
use ort::session::Session;
use ort::value::Tensor;
use sha2::{Digest, Sha256};

use super::audio;
use super::demucs::{self, ModelOutput, SeparateError};
use super::dsp::{FRAMES, FREQS, SEGMENT};

struct Download {
    url: &'static str,
    size: u64,
    sha256: &'static str,
}

/// htdemucs exportado a ONNX sin la STFT (con demucs.onnx de sevagh, MIT), en un solo archivo.
const MODEL: Download = Download {
    url: "https://github.com/joseMChavez/emetpro/releases/download/modelo-htdemucs-1/htdemucs-emetpro-1.onnx",
    size: 171_189_055,
    sha256: "5ec4819b40676d48991ff75087a5966b188368e2f16cc6a08cbeb0f79d8875b4",
};
pub const MODEL_FILE: &str = "htdemucs-emetpro-1.onnx";

/// ONNX Runtime 1.31.1 de PyPI para este sistema: el paquete y lo que se saca de él.
struct Runtime {
    package: Download,
    /// La biblioteca dentro del paquete.
    library: &'static str,
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
const RUNTIME: Option<Runtime> = Some(Runtime {
    package: Download {
        url: "https://files.pythonhosted.org/packages/eb/27/c6a823bac55b2977bea2027c3ec9393a2c1002cb9a09eb40354563e02730/onnxruntime-1.31.1-cp312-cp312-manylinux_2_28_x86_64.whl",
        size: 23_760_988,
        sha256: "1f3dea47f2f0e5117c2404cea7d21fd253ae7836a3bf167a501cdfbb2600d536",
    },
    library: "onnxruntime/capi/libonnxruntime.so.1.31.1",
});
#[cfg(all(windows, target_arch = "x86_64"))]
const RUNTIME: Option<Runtime> = Some(Runtime {
    package: Download {
        url: "https://files.pythonhosted.org/packages/45/b2/793b4051d8dc6acb379ead9b72050f0ffa283ca49a6904add2e218b83e2e/onnxruntime-1.31.1-cp312-cp312-win_amd64.whl",
        size: 14_888_466,
        sha256: "ae8a9a351efda685cc59a66226cdefaa37a126c1d66872ca23a2de9425326130",
    },
    library: "onnxruntime/capi/onnxruntime.dll",
});
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const RUNTIME: Option<Runtime> = Some(Runtime {
    package: Download {
        url: "https://files.pythonhosted.org/packages/0e/f7/cecc0efeeea83bff2f7789ae1b80f27fa0f0210d6d9d66d1f0031a6b5e47/onnxruntime-1.31.1-cp312-cp312-macosx_14_0_arm64.whl",
        size: 20_882_125,
        sha256: "96dd2564b10ea7153ad9c207e7cd8e5917cb3ba521e0e0a62ac03486fdc8d56c",
    },
    library: "onnxruntime/capi/libonnxruntime.1.31.1.dylib",
});
/// Mac con Intel y otros: Microsoft ya no publica ONNX Runtime para ellos.
#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(windows, target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64"),
)))]
const RUNTIME: Option<Runtime> = None;

/// Avisos de licencia que se guardan junto a la biblioteca (la licencia MIT pide conservarlos).
const NOTICES: &[&str] = &["onnxruntime/LICENSE", "onnxruntime/ThirdPartyNotices.txt"];

const HTDEMUCS_NOTICE: &str = "htdemucs (Demucs v4), de Meta Platforms, Inc. Licencia MIT.\n\
https://github.com/facebookresearch/demucs\n\
Exportado a ONNX con demucs.onnx (Sevag Hanssian, licencia MIT): https://github.com/sevagh/demucs.onnx\n";

pub enum SeparatorError {
    Busy,
    Canceled,
    /// No hay ONNX Runtime para este sistema.
    Unavailable,
    Missing,
    Download(String),
    Failed(String),
}

fn failed(e: impl std::fmt::Display) -> SeparatorError {
    SeparatorError::Failed(e.to_string())
}

static BUSY: AtomicBool = AtomicBool::new(false);
static CANCEL: AtomicBool = AtomicBool::new(false);

struct Turn;

impl Turn {
    fn take() -> Result<Self, SeparatorError> {
        BUSY.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).map_err(|_| SeparatorError::Busy)?;
        CANCEL.store(false, Ordering::SeqCst);
        Ok(Self)
    }
}

impl Drop for Turn {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::SeqCst);
    }
}

pub fn busy() -> bool {
    BUSY.load(Ordering::SeqCst)
}

pub fn cancel() {
    CANCEL.store(true, Ordering::SeqCst);
}

pub fn available() -> bool {
    RUNTIME.is_some()
}

/// Lo que ocupa la descarga entera, en bytes.
pub fn download_size() -> u64 {
    RUNTIME.as_ref().map_or(0, |r| r.package.size) + MODEL.size
}

fn root(data: &Path) -> PathBuf {
    data.join("separador")
}

fn library_path(data: &Path) -> Option<PathBuf> {
    let runtime = RUNTIME.as_ref()?;
    let name = Path::new(runtime.library).file_name()?;
    Some(root(data).join(name))
}

/// Biblioteca y modelo, si están los dos (cada uno se pone en su sitio solo tras comprobarlo).
pub fn installed(data: &Path) -> Option<(PathBuf, PathBuf)> {
    let library = library_path(data).filter(|p| p.is_file())?;
    let model = root(data).join(MODEL_FILE);
    model.is_file().then_some((library, model))
}

pub fn remove(data: &Path) -> Result<(), SeparatorError> {
    let _turn = Turn::take()?;
    match std::fs::remove_dir_all(root(data)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        // En Windows la biblioteca cargada no se puede borrar hasta cerrar la app.
        Err(e) => Err(failed(e)),
    }
}

/// Descarga ONNX Runtime y el modelo; `progress` recibe los bytes descargados de los dos juntos.
pub async fn download(data: &Path, mut progress: impl FnMut(u64) + Send) -> Result<(), SeparatorError> {
    let runtime = RUNTIME.as_ref().ok_or(SeparatorError::Unavailable)?;
    let _turn = Turn::take()?;
    let root = root(data);
    std::fs::create_dir_all(&root).map_err(failed)?;
    let mut done = 0;
    if library_path(data).is_none_or(|p| !p.is_file()) {
        let part = root.join("onnxruntime.whl.part");
        let result = fetch(&runtime.package, &part, &mut |n| progress(n)).await;
        if let Err(e) = result {
            let _ = std::fs::remove_file(&part);
            return Err(e);
        }
        let extracted = extract(&part, &root, runtime.library);
        let _ = std::fs::remove_file(&part);
        extracted?;
    }
    done += runtime.package.size;
    let model = root.join(MODEL_FILE);
    if !model.is_file() {
        let part = root.join(format!("{MODEL_FILE}.part"));
        let result = fetch(&MODEL, &part, &mut |n| progress(done + n)).await;
        if let Err(e) = result {
            let _ = std::fs::remove_file(&part);
            return Err(e);
        }
        std::fs::rename(&part, &model).map_err(failed)?;
        let _ = std::fs::write(root.join("LICENSE-htdemucs.txt"), HTDEMUCS_NOTICE);
    }
    progress(download_size());
    Ok(())
}

async fn fetch(file: &Download, path: &Path, progress: &mut impl FnMut(u64)) -> Result<(), SeparatorError> {
    let download = |e: reqwest::Error| SeparatorError::Download(e.to_string());
    let client = reqwest::Client::builder()
        .user_agent(concat!("EmetPro/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(30))
        .build()
        .map_err(download)?;
    let mut response = client.get(file.url).send().await.and_then(|r| r.error_for_status()).map_err(download)?;
    let mut out = std::fs::File::create(path).map_err(failed)?;
    let mut hasher = Sha256::new();
    let mut received = 0u64;
    let mut reported = Instant::now();
    while let Some(chunk) = response.chunk().await.map_err(download)? {
        if CANCEL.load(Ordering::SeqCst) {
            return Err(SeparatorError::Canceled);
        }
        if received + chunk.len() as u64 > file.size {
            return Err(SeparatorError::Download("el archivo es más grande de lo esperado".into()));
        }
        out.write_all(&chunk).map_err(failed)?;
        hasher.update(&chunk);
        received += chunk.len() as u64;
        if reported.elapsed() > Duration::from_millis(250) {
            reported = Instant::now();
            progress(received);
        }
    }
    out.flush().map_err(failed)?;
    check(&format!("{:x}", hasher.finalize()), received, file).map_err(SeparatorError::Download)
}

fn check(sha256: &str, size: u64, file: &Download) -> Result<(), String> {
    if size != file.size {
        return Err(format!("descarga incompleta ({size} de {} bytes)", file.size));
    }
    if sha256 != file.sha256 {
        return Err("la suma SHA-256 no coincide".into());
    }
    Ok(())
}

/// Saca del paquete (un zip) la biblioteca y los avisos de licencia; la biblioteca queda en su sitio al final.
fn extract(package: &Path, root: &Path, library: &str) -> Result<(), SeparatorError> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(package).map_err(failed)?).map_err(failed)?;
    for name in NOTICES {
        if let Ok(mut entry) = archive.by_name(name) {
            let target = root.join(format!("onnxruntime-{}", Path::new(name).file_name().unwrap().to_string_lossy()));
            let mut out = std::fs::File::create(target).map_err(failed)?;
            std::io::copy(&mut entry, &mut out).map_err(failed)?;
        }
    }
    let file_name = Path::new(library).file_name().ok_or_else(|| failed("biblioteca sin nombre"))?;
    let part = root.join(format!("{}.part", file_name.to_string_lossy()));
    {
        let mut entry = archive.by_name(library).map_err(failed)?;
        let mut out = std::fs::File::create(&part).map_err(failed)?;
        std::io::copy(&mut entry, &mut out).map_err(failed)?;
    }
    std::fs::rename(&part, root.join(file_name)).map_err(failed)
}

/// ONNX Runtime se carga una sola vez por ejecución de la app.
static ENVIRONMENT: OnceLock<Result<Environment, String>> = OnceLock::new();

fn environment(library: &Path) -> Result<&'static Environment, SeparatorError> {
    ENVIRONMENT
        .get_or_init(|| ort::init_from(library).map_err(|e| e.to_string())?.build().map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| SeparatorError::Failed(format!("no se pudo cargar ONNX Runtime: {e}")))
}

/// Fases de `separate` (las pruebas).
#[cfg(test)]
pub enum Step {
    Reading,
    /// Trozos separados y total.
    Separating(usize, usize),
    Saving,
}

/// Saca el instrumental de `track` y lo guarda en `target` (FLAC). Devuelve su duración en ms. Bloquea. (La app lo
/// hace con `prepare`, que además oye la letra; esto queda para las pruebas.)
#[cfg(test)]
pub fn separate(data: &Path, track: &Path, target: &Path, mut step: impl FnMut(Step)) -> Result<u32, SeparatorError> {
    step(Step::Reading);
    let mix = audio::decode(track).map_err(SeparatorError::Failed)?;
    let stems = separate_audio(data, &mix, &mut |k, n| step(Step::Separating(k, n)))?;
    step(Step::Saving);
    save_instrumental(target, stems.instrumental, &mix)?;
    Ok(duration_ms(&mix))
}

pub fn duration_ms(audio: &audio::Stereo) -> u32 {
    (audio[0].len() as u64 * 1000 / audio::RATE as u64) as u32
}

/// Guarda el instrumental al volumen de la original (ver `audio::match_loudness`).
pub fn save_instrumental(target: &Path, mut instrumental: audio::Stereo, mix: &audio::Stereo) -> Result<(), SeparatorError> {
    audio::match_loudness(&mut instrumental, mix);
    audio::write_flac(target, &instrumental).map_err(SeparatorError::Failed)
}

/// Separa la voz y el resto de una pista ya leída; `progress` recibe los trozos hechos y el total. Bloquea.
pub fn separate_audio(data: &Path, mix: &audio::Stereo, progress: &mut impl FnMut(usize, usize)) -> Result<demucs::Stems, SeparatorError> {
    let (library, model) = installed(data).ok_or(SeparatorError::Missing)?;
    let _turn = Turn::take()?;
    if CANCEL.load(Ordering::SeqCst) {
        return Err(SeparatorError::Canceled);
    }
    let environment = environment(&library)?;
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 8);
    let mut session = Session::builder(environment)
        .map_err(failed)?
        .with_intra_threads(threads)
        .map_err(|e| failed(e.to_string()))?
        .commit_from_file(&model)
        .map_err(failed)?;
    let run = |input: &[f32], mag: &[f32]| -> Result<ModelOutput, String> {
        let wave = Tensor::from_array(([1usize, 2, SEGMENT], input.to_vec())).map_err(|e| e.to_string())?;
        let spec = Tensor::from_array(([1usize, 4, FREQS, FRAMES], mag.to_vec())).map_err(|e| e.to_string())?;
        let outputs = session.run(ort::inputs![wave, spec]).map_err(|e| e.to_string())?;
        let (_, spec) = outputs[0].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
        let (_, wave) = outputs[1].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
        Ok(ModelOutput { spec: spec.to_vec(), wave: wave.to_vec() })
    };
    demucs::separate(mix, run, progress, &CANCEL).map_err(|e| match e {
        SeparateError::Canceled => SeparatorError::Canceled,
        SeparateError::Model(e) => SeparatorError::Failed(e),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solo_los_archivos_fijados() {
        assert!(check(MODEL.sha256, MODEL.size, &MODEL).is_ok());
        assert!(check(MODEL.sha256, MODEL.size - 1, &MODEL).is_err());
        assert!(check(&"0".repeat(64), MODEL.size, &MODEL).is_err());
    }

    /// La descarga de verdad (ONNX Runtime de PyPI y el modelo de las Releases), en una carpeta temporal:
    /// cargo test --release descarga_el_separador -- --ignored --nocapture
    #[test]
    #[ignore]
    fn descarga_el_separador() {
        let data = std::env::temp_dir().join(format!("emetpro-descarga-separador-{}", std::process::id()));
        let mut last = 0;
        let started = Instant::now();
        tauri::async_runtime::block_on(download(&data, |done| last = done)).unwrap_or_else(|_| panic!("descargar"));
        println!("{} MB en {:?}", last / 1_000_000, started.elapsed());
        assert_eq!(last, download_size());
        let (library, model) = installed(&data).expect("instalado");
        assert!(library.is_file() && model.is_file());
        assert!(root(&data).join("onnxruntime-LICENSE").is_file() && root(&data).join("LICENSE-htdemucs.txt").is_file());
        // La biblioteca descargada se carga y abre el modelo.
        let environment = environment(&library).unwrap_or_else(|_| panic!("cargar ONNX Runtime"));
        Session::builder(environment).unwrap().commit_from_file(&model).unwrap();
        remove(&data).unwrap_or_else(|_| panic!("borrar"));
        assert!(installed(&data).is_none());
    }

    /// Como lo hace la app, con archivos ya descargados (el paquete de PyPI y el modelo) en lugar de la red:
    /// EMETPRO_ORT_WHEEL=… EMETPRO_DEMUCS_MODEL=… EMETPRO_TRACK=… EMETPRO_OUT=….flac
    /// cargo test --release separa_una_cancion_como_la_app -- --ignored --nocapture
    #[test]
    #[ignore]
    fn separa_una_cancion_como_la_app() {
        let var = |name: &str| PathBuf::from(std::env::var(name).unwrap());
        let data = std::env::temp_dir().join(format!("emetpro-separador-{}", std::process::id()));
        let root = root(&data);
        std::fs::create_dir_all(&root).unwrap();
        let runtime = RUNTIME.as_ref().unwrap();
        // Lo que hace `download` tras comprobar cada archivo.
        let wheel = var("EMETPRO_ORT_WHEEL");
        let bytes = std::fs::read(&wheel).unwrap();
        check(&format!("{:x}", Sha256::digest(&bytes)), bytes.len() as u64, &runtime.package).unwrap();
        let part = root.join("onnxruntime.whl.part");
        std::fs::copy(&wheel, &part).unwrap();
        extract(&part, &root, runtime.library).unwrap_or_else(|_| panic!("extraer"));
        std::fs::copy(var("EMETPRO_DEMUCS_MODEL"), root.join(MODEL_FILE)).unwrap();
        assert!(installed(&data).is_some());
        assert!(root.join("onnxruntime-LICENSE").is_file());

        let started = Instant::now();
        let mut steps = Vec::new();
        let ms = separate(&data, &var("EMETPRO_TRACK"), &var("EMETPRO_OUT"), |step| {
            steps.push(match step {
                Step::Reading => "leer".to_string(),
                Step::Separating(k, n) => format!("{k}/{n}"),
                Step::Saving => "guardar".to_string(),
            })
        })
        .unwrap_or_else(|_| panic!("separar"));
        println!("{ms} ms de canción separados en {:?}; pasos: {}", started.elapsed(), steps.join(" "));
        assert_eq!(steps.first().map(String::as_str), Some("leer"));
        assert_eq!(steps.last().map(String::as_str), Some("guardar"));
        let back = audio::decode(&var("EMETPRO_OUT")).unwrap();
        assert!((back[0].len() as i64 - (ms as i64 * 44_100 / 1000)).abs() < 5000);
        let _ = std::fs::remove_dir_all(&data);
    }

    /// Todo de verdad con la biblioteca y el modelo del equipo (prueba de concepto):
    /// EMETPRO_ORT_LIB=… EMETPRO_DEMUCS_MODEL=… EMETPRO_DEMUCS_GOLDEN=… cargo test --release separa_un_clip -- --ignored --nocapture
    #[test]
    #[ignore]
    fn separa_un_clip() {
        let read = |name: &str| -> Vec<f32> {
            let bytes = std::fs::read(format!("{}/{name}.f32", std::env::var("EMETPRO_DEMUCS_GOLDEN").unwrap())).unwrap();
            bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect()
        };
        let clip = read("clip");
        let half = clip.len() / 2;
        let mix = [clip[..half].to_vec(), clip[half..].to_vec()];
        let environment = environment(Path::new(&std::env::var("EMETPRO_ORT_LIB").unwrap())).unwrap_or_else(|_| panic!("ort"));
        let mut session = Session::builder(environment).unwrap().with_intra_threads(8).unwrap().commit_from_file(std::env::var("EMETPRO_DEMUCS_MODEL").unwrap()).unwrap();
        let started = Instant::now();
        let stems = demucs::separate(
            &mix,
            |input, mag| {
                let wave = Tensor::from_array(([1usize, 2, SEGMENT], input.to_vec())).unwrap();
                let spec = Tensor::from_array(([1usize, 4, FREQS, FRAMES], mag.to_vec())).unwrap();
                let outputs = session.run(ort::inputs![wave, spec]).unwrap();
                let (_, spec) = outputs[0].try_extract_tensor::<f32>().unwrap();
                let (_, wave) = outputs[1].try_extract_tensor::<f32>().unwrap();
                Ok(ModelOutput { spec: spec.to_vec(), wave: wave.to_vec() })
            },
            |_, _| {},
            &AtomicBool::new(false),
        )
        .unwrap_or_else(|_| panic!("separar"));
        println!("20 s separados en {:?}", started.elapsed());
        let golden = read("clip_instrumental");
        for channel in 0..2 {
            let db = super::super::tests::sdr(&golden[channel * half..(channel + 1) * half], &stems.instrumental[channel]);
            println!("instrumental canal {channel}: {db:.1} dB frente a Python");
            assert!(db > 60.0);
        }
    }
}

// Plan de EmetPro (.emetplan): un plan entero o una sección, para mandarlo a otro equipo (D-028).
// Un archivo binario que se escribe y se lee por partes (un video puede pesar GB, no se carga en memoria):
//
//   «EMETPLAN»            8 bytes
//   versión               u16 little-endian (1)
//   largo del manifiesto  u32 little-endian
//   manifiesto            JSON UTF-8 (PlanManifest)
//   archivos              los bytes de cada archivo, uno tras otro, en el orden de `files`
//
// En los elementos, una ruta «@3» es el archivo 3 de `files`; cualquier otra es solo el nombre de un archivo que
// no viaja (un video que se decidió no incluir): al importar queda como «el archivo no está».
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::emetpro::SongEntry;

pub const EXTENSION: &str = "emetplan";
const MAGIC: &[u8; 8] = b"EMETPLAN";
const VERSION: u16 = 1;
const FORMAT: &str = "emetpro-plan";
const MAX_MANIFEST: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanManifest {
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub version: u16,
    pub exported_at: i64,
    pub app: String,
    pub name: String,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub animate: Option<bool>,
    /// Si es una sección: su título.
    #[serde(default)]
    pub section: Option<String>,
    /// Elementos del plan (PlanItem en JSON), con las rutas cambiadas por «@n». Se leen de uno en uno: uno que esta
    /// versión no entienda se salta.
    pub items: Vec<serde_json::Value>,
    /// Letras usadas por los elementos (por su id); `background` es un índice en `files`.
    #[serde(default)]
    pub songs: Vec<SongEntry>,
    /// Canciones de Media Studio (D-029) usadas por los elementos, como MediaSong en JSON con su pista y sus fondos
    /// como «@n». Se leen de una en una: una que esta versión no entienda se salta. Las versiones anteriores no
    /// conocen este campo ni el elemento «studioSong»: los saltan.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub studio_songs: Vec<serde_json::Value>,
    /// Versiones de la Biblia fijadas en las citas, para avisar si el que importa no las tiene.
    #[serde(default)]
    pub bibles: Vec<BibleRef>,
    #[serde(default)]
    pub files: Vec<FileEntry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BibleRef {
    pub id: String,
    pub abbreviation: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    /// Nombre original: el plan lo sigue mostrando.
    pub name: String,
    pub size: u64,
    pub sha256: String,
}

/// Referencia a un archivo incluido, como va en las rutas de los elementos.
pub fn file_ref(index: usize) -> String {
    format!("@{index}")
}

/// «@3» → 3. Cualquier otra cosa no es un archivo incluido.
pub fn parse_ref(path: &str) -> Option<usize> {
    path.strip_prefix('@')?.parse().ok()
}

fn hash_file(path: &Path) -> io::Result<(u64, String)> {
    let mut hasher = Sha256::new();
    let size = io::copy(&mut File::open(path)?, &mut hasher)?;
    Ok((size, format!("{:x}", hasher.finalize())))
}

/// Escribe el archivo: primero el manifiesto (con el tamaño y la suma de cada archivo) y después los archivos.
pub fn write(path: &Path, mut manifest: PlanManifest, files: &[(String, PathBuf)]) -> io::Result<()> {
    manifest.format = FORMAT.into();
    manifest.version = VERSION;
    manifest.files = files
        .iter()
        .map(|(name, source)| hash_file(source).map(|(size, sha256)| FileEntry { name: name.clone(), size, sha256 }))
        .collect::<io::Result<_>>()?;
    let json = serde_json::to_vec_pretty(&manifest).map_err(io::Error::other)?;
    let mut out = BufWriter::new(File::create(path)?);
    out.write_all(MAGIC)?;
    out.write_all(&VERSION.to_le_bytes())?;
    out.write_all(&(json.len() as u32).to_le_bytes())?;
    out.write_all(&json)?;
    for ((_, source), entry) in files.iter().zip(&manifest.files) {
        // Si el archivo cambió entre el cálculo de la suma y la copia, el archivo saldría roto: se avisa.
        let copied = io::copy(&mut File::open(source)?, &mut out)?;
        if copied != entry.size {
            return Err(io::Error::other(format!("«{}» cambió mientras se exportaba", entry.name)));
        }
    }
    out.flush()
}

/// Abre el archivo y lee el manifiesto; el lector queda justo al principio de los archivos.
pub fn open(path: &Path) -> Result<(PlanManifest, BufReader<File>), String> {
    let mut reader = BufReader::new(File::open(path).map_err(|e| e.to_string())?);
    let mut head = [0u8; 14];
    reader.read_exact(&mut head).map_err(|_| "no es un plan de EmetPro".to_string())?;
    if &head[..8] != MAGIC {
        return Err("no es un plan de EmetPro".into());
    }
    let version = u16::from_le_bytes([head[8], head[9]]);
    if version > VERSION {
        return Err(format!("lo escribió una versión más nueva de EmetPro (formato {version}); actualiza EmetPro"));
    }
    let len = u32::from_le_bytes([head[10], head[11], head[12], head[13]]) as usize;
    if len > MAX_MANIFEST {
        return Err("el archivo está dañado".into());
    }
    let mut json = vec![0u8; len];
    reader.read_exact(&mut json).map_err(|_| "el archivo está incompleto o dañado".to_string())?;
    let manifest: PlanManifest = serde_json::from_slice(&json).map_err(|e| format!("manifiesto ilegible: {e}"))?;
    if manifest.format != FORMAT {
        return Err(format!("formato desconocido: {}", manifest.format));
    }
    Ok((manifest, reader))
}

/// Es un plan de EmetPro (por la cabecera).
pub fn is_plan(path: &Path) -> bool {
    let mut head = [0u8; 8];
    File::open(path).and_then(|mut f| f.read_exact(&mut head)).is_ok() && &head == MAGIC
}

/// Saca los archivos, en orden, a `dir` (cada uno con un nombre propio, sin pisar nada) comprobando su suma.
/// Devuelve sus rutas, en el orden de `files`. Si uno está dañado, borra lo sacado y falla.
pub fn extract(reader: &mut impl Read, files: &[FileEntry], dir: &Path) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut out: Vec<PathBuf> = Vec::with_capacity(files.len());
    let fail = |out: &[PathBuf], reason: String| {
        for p in out {
            let _ = std::fs::remove_file(p);
        }
        Err(reason)
    };
    for (i, entry) in files.iter().enumerate() {
        let target = dir.join(format!("{i}"));
        let result = (|| -> io::Result<String> {
            let mut file = BufWriter::new(File::create(&target)?);
            let mut hasher = Sha256::new();
            let mut limited = reader.by_ref().take(entry.size);
            let mut buf = vec![0u8; 256 * 1024];
            let mut copied = 0u64;
            loop {
                let n = limited.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buf[..n]);
                file.write_all(&buf[..n])?;
                copied += n as u64;
            }
            file.flush()?;
            if copied != entry.size {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "corto"));
            }
            Ok(format!("{:x}", hasher.finalize()))
        })();
        out.push(target);
        match result {
            Ok(sum) if sum == entry.sha256 => {}
            Ok(_) => return fail(&out, format!("«{}» está dañado", entry.name)),
            Err(_) => return fail(&out, "el archivo está incompleto o dañado".into()),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("emetpro-plan-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn manifest() -> PlanManifest {
        PlanManifest {
            format: String::new(),
            version: 0,
            exported_at: 0,
            app: "prueba".into(),
            name: "Culto".into(),
            date: Some("2026-11-22".into()),
            notes: String::new(),
            animate: None,
            section: Some("Predicación".into()),
            items: vec![serde_json::json!({"id":"1","kind":"media","path":"@0","media":"image"})],
            songs: Vec::new(),
            studio_songs: Vec::new(),
            bibles: Vec::new(),
            files: Vec::new(),
        }
    }

    #[test]
    fn ida_y_vuelta_con_archivos() {
        let dir = temp("ida");
        let a = dir.join("anuncio.png");
        let b = dir.join("video.mp4");
        std::fs::write(&a, b"imagen").unwrap();
        std::fs::write(&b, vec![7u8; 600_000]).unwrap();
        let file = dir.join("culto.emetplan");
        write(&file, manifest(), &[("anuncio.png".into(), a), ("video.mp4".into(), b)]).unwrap();
        assert!(is_plan(&file));

        let (m, mut reader) = open(&file).unwrap();
        assert_eq!(m.name, "Culto");
        assert_eq!(m.section.as_deref(), Some("Predicación"));
        assert_eq!(m.files.len(), 2);
        assert_eq!(m.files[1].size, 600_000);
        let out = extract(&mut reader, &m.files, &dir.join("sacados")).unwrap();
        assert_eq!(std::fs::read(&out[0]).unwrap(), b"imagen");
        assert_eq!(std::fs::read(&out[1]).unwrap().len(), 600_000);
        assert_eq!(parse_ref("@1"), Some(1));
        assert_eq!(parse_ref("IMG_0001.MOV"), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rechaza_archivos_dañados() {
        let dir = temp("roto");
        let a = dir.join("anuncio.png");
        std::fs::write(&a, b"imagen").unwrap();
        let file = dir.join("culto.emetplan");
        write(&file, manifest(), &[("anuncio.png".into(), a)]).unwrap();

        // Un byte cambiado en el archivo incluido.
        let mut bytes = std::fs::read(&file).unwrap();
        *bytes.last_mut().unwrap() ^= 1;
        std::fs::write(&file, &bytes).unwrap();
        let (m, mut reader) = open(&file).unwrap();
        assert!(extract(&mut reader, &m.files, &dir.join("x")).unwrap_err().contains("dañado"));
        assert!(!dir.join("x").join("0").exists(), "no deja lo sacado a medias");

        // Cortado.
        std::fs::write(&file, &bytes[..bytes.len() - 3]).unwrap();
        let (m, mut reader) = open(&file).unwrap();
        assert!(extract(&mut reader, &m.files, &dir.join("y")).is_err());

        // Otra cosa, o una versión futura.
        std::fs::write(&file, b"SELAHLTR....").unwrap();
        assert!(open(&file).is_err());
        let mut future = bytes.clone();
        future[8] = 9;
        std::fs::write(&file, &future).unwrap();
        assert!(open(&file).unwrap_err().contains("más nueva"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

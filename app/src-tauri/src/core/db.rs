// Base de datos local (SQLite) compartida por las funcionalidades: cada una añade sus consultas con su propio
// `impl Store` (features/settings/repo.rs, features/songs/repo.rs, features/plans/repo.rs). Aquí solo se abre el archivo y se migra.
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use serde::Serialize;

use crate::core::error::CommandError;

pub const FILE: &str = "emetpro.db";

/// Cada entrada lleva la base de una versión a la siguiente; PRAGMA user_version guarda cuántas se aplicaron.
/// Nunca se edita una entrada publicada: los cambios van en una entrada nueva.
const MIGRATIONS: &[&str] = &["
    CREATE TABLE settings (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    CREATE TABLE songs (
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        singer TEXT NOT NULL DEFAULT '',
        writer TEXT NOT NULL DEFAULT '',
        body TEXT NOT NULL,
        background TEXT,
        updated_at INTEGER NOT NULL
    );
    CREATE TABLE setlist (
        position INTEGER PRIMARY KEY,
        song_id TEXT NOT NULL REFERENCES songs(id) ON DELETE CASCADE
    );
", "
    -- Planes del culto (D-019). Los elementos van juntos como JSON (features/plans/domain.rs): el plan se guarda entero.
    CREATE TABLE plans (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        date TEXT,
        notes TEXT NOT NULL DEFAULT '',
        items TEXT NOT NULL DEFAULT '[]',
        updated_at INTEGER NOT NULL
    );
    -- La lista del culto pasa a ser un plan, con sus letras en el mismo orden.
    INSERT INTO plans (id, name, date, notes, items, updated_at)
    SELECT 'lista', 'Lista', NULL, '',
        json_group_array(json_object('id', song_id, 'kind', 'song', 'songId', song_id)),
        CAST(strftime('%s', 'now') AS INTEGER)
    FROM (SELECT song_id FROM setlist ORDER BY position)
    HAVING COUNT(*) > 0;
    DROP TABLE setlist;
", "
    -- Animaciones (D-024): la propia de cada letra (JSON) y si un plan anima el texto (NULL = como lo general).
    ALTER TABLE songs ADD COLUMN animation TEXT;
    ALTER TABLE plans ADD COLUMN animate INTEGER;
", "
    -- Media Studio (D-029): cada canción entera como JSON (features/studio/domain.rs). El título va aparte para ordenar.
    CREATE TABLE studio_songs (
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        data TEXT NOT NULL,
        updated_at INTEGER NOT NULL
    );
"];

pub struct Store {
    pub(crate) conn: Connection,
    /// None si se trabaja en memoria porque no se pudo abrir el archivo.
    path: Option<PathBuf>,
    /// Por qué se trabaja en memoria (para decírselo al usuario en Ajustes).
    problem: Option<StoreProblem>,
}

/// Por qué no se pudo abrir la base. Solo `Damaged` justifica apartar el archivo: un disco lleno o sin permisos
/// no estropea la base, y basta con resolverlo y volver a abrir la app para tenerlo todo como estaba.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum StoreProblem {
    /// No queda espacio en el disco.
    DiskFull,
    /// La carpeta o el archivo no se pueden escribir.
    NoPermission,
    /// El archivo está dañado y ni siquiera se pudo apartar para empezar de cero.
    Damaged,
    Other { message: String },
}

/// Un fallo al abrir: si el archivo está dañado (y se aparta) o si es otra cosa (y no se toca).
enum OpenError {
    Damaged(String),
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Damaged(why) => write!(f, "archivo dañado: {why}"),
            Self::Sqlite(e) => write!(f, "{e}"),
            Self::Io(e) => write!(f, "{e}"),
        }
    }
}

impl From<rusqlite::Error> for OpenError {
    fn from(e: rusqlite::Error) -> Self {
        use rusqlite::ErrorCode::{DatabaseCorrupt, NotADatabase};
        match e.sqlite_error_code() {
            Some(DatabaseCorrupt | NotADatabase) => Self::Damaged(e.to_string()),
            _ => Self::Sqlite(e),
        }
    }
}

pub(crate) fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

impl Store {
    /// Abre (o crea) la base en esa carpeta. Siempre devuelve un Store utilizable:
    /// si el archivo está dañado se aparta con otro nombre y se empieza de cero;
    /// si es otra cosa (disco lleno, sin permisos) o ni así se puede, se trabaja en memoria y los cambios no se
    /// guardan, pero el archivo queda intacto para el siguiente arranque.
    pub fn open(dir: &Path) -> Self {
        let path = dir.join(FILE);
        let opened = std::fs::create_dir_all(dir).map_err(OpenError::Io).and_then(|_| Self::open_file(&path));
        let error = match opened {
            Ok(store) => return store,
            Err(e) => e,
        };
        eprintln!("[base de datos] no se pudo abrir {}: {error}", path.display());
        let problem = match error {
            OpenError::Damaged(_) => match Self::set_aside(&path) {
                Ok(store) => return store,
                Err(e) => {
                    eprintln!("[base de datos] tampoco se pudo empezar de cero: {e}");
                    StoreProblem::Damaged
                }
            },
            other => classify(dir, &other),
        };
        eprintln!("[base de datos] se trabaja en memoria: los cambios no se guardarán ({problem:?})");
        let conn = Connection::open_in_memory().expect("SQLite en memoria");
        migrate(&conn).expect("migraciones en memoria");
        Self { conn, path: None, problem: Some(problem) }
    }

    /// Aparta la base dañada (en vez de borrarla: así se puede intentar recuperar a mano) y empieza una nueva.
    fn set_aside(path: &Path) -> Result<Self, OpenError> {
        if path.exists() {
            let aside = path.with_extension(format!("db.dañada-{}", now_secs()));
            std::fs::rename(path, &aside).map_err(OpenError::Io)?;
            eprintln!("[base de datos] la anterior quedó en {}", aside.display());
            for ext in ["db-wal", "db-shm"] {
                let _ = std::fs::remove_file(path.with_extension(ext));
            }
        }
        Self::open_file(path)
    }

    fn open_file(path: &Path) -> Result<Self, OpenError> {
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(3))?;
        let check: String = conn.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
        if check != "ok" {
            return Err(OpenError::Damaged(check));
        }
        // WAL: un corte de luz a mitad de una escritura no deja la base a medias.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        migrate(&conn)?;
        Ok(Self { conn, path: Some(path.to_path_buf()), problem: None })
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn problem(&self) -> Option<&StoreProblem> {
        self.problem.as_ref()
    }
}

/// Qué le pasa a la carpeta cuando la base no abre por algo que no es un archivo dañado. SQLite no siempre dice
/// «disco lleno» (al crear el -wal puede decir solo «no se pudo abrir»): se prueba a escribir un archivo pequeño.
fn classify(dir: &Path, error: &OpenError) -> StoreProblem {
    use rusqlite::ErrorCode::{DiskFull, PermissionDenied, ReadOnly};
    use std::io::ErrorKind;
    let from_io = |kind: ErrorKind| match kind {
        ErrorKind::StorageFull | ErrorKind::QuotaExceeded => Some(StoreProblem::DiskFull),
        ErrorKind::PermissionDenied | ErrorKind::ReadOnlyFilesystem => Some(StoreProblem::NoPermission),
        _ => None,
    };
    let direct = match error {
        OpenError::Sqlite(e) => match e.sqlite_error_code() {
            Some(DiskFull) => Some(StoreProblem::DiskFull),
            Some(PermissionDenied | ReadOnly) => Some(StoreProblem::NoPermission),
            _ => None,
        },
        OpenError::Io(e) => from_io(e.kind()),
        OpenError::Damaged(_) => Some(StoreProblem::Damaged),
    };
    direct.or_else(|| probe(dir).err().and_then(|e| from_io(e.kind()))).unwrap_or_else(|| StoreProblem::Other {
        message: error.to_string(),
    })
}

/// Escribe (y borra) un archivo del tamaño de unas páginas de SQLite en la carpeta de la base.
fn probe(dir: &Path) -> std::io::Result<()> {
    use std::io::Write;
    let file = dir.join(".emetpro-prueba-de-escritura");
    let result = std::fs::File::create(&file).and_then(|mut f| {
        f.write_all(&[0u8; 64 * 1024])?;
        f.sync_all()
    });
    let _ = std::fs::remove_file(&file);
    result
}

fn migrate(conn: &Connection) -> rusqlite::Result<()> {
    conn.pragma_update(None, "foreign_keys", true)?;
    let applied: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    // Una base de una versión más nueva de EmetPro se usa tal cual: no se toca su esquema.
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(applied.max(0) as usize) {
        conn.execute_batch(&format!("BEGIN; {sql} PRAGMA user_version = {}; COMMIT;", i + 1))?;
    }
    Ok(())
}

/// La base de la app, gestionada por Tauri. None solo antes de abrirla al arrancar.
#[derive(Default)]
pub struct Db(pub Mutex<Option<Store>>);

pub fn with_store<T>(
    db: &Db,
    action: impl FnOnce(&mut Store) -> rusqlite::Result<T>,
) -> Result<T, CommandError> {
    let mut guard = db.0.lock().unwrap();
    let store = guard.as_mut().ok_or_else(|| CommandError::Storage {
        message: "la base de datos aún no está lista".into(),
    })?;
    action(store).map_err(|e| CommandError::Storage { message: e.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("emetpro-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn base_dañada_se_aparta_y_se_empieza_de_cero() {
        let dir = temp_dir("dañada");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(FILE), b"esto no es una base de datos SQLite, solo basura de prueba").unwrap();
        let store = Store::open(&dir);
        assert!(store.path().is_some());
        store.set_setting("theme", &Value::from("light")).unwrap();
        let aside = std::fs::read_dir(&dir).unwrap().flatten().any(|e| e.file_name().to_string_lossy().contains("dañada-"));
        assert!(aside);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn base_con_una_cancion(name: &str) -> PathBuf {
        let dir = temp_dir(name);
        let store = Store::open(&dir);
        store.set_setting("theme", &Value::from("light")).unwrap();
        drop(store);
        dir
    }

    fn sin_apartar(dir: &Path) -> bool {
        !std::fs::read_dir(dir).unwrap().flatten().any(|e| e.file_name().to_string_lossy().contains("dañada-"))
    }

    #[cfg(unix)]
    #[test]
    fn carpeta_sin_permisos_no_aparta_la_base() {
        use std::os::unix::fs::PermissionsExt;
        let dir = base_con_una_cancion("sin-permisos");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        // Como root los permisos no cuentan: no hay nada que probar.
        if probe(&dir).is_ok() {
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        let store = Store::open(&dir);
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(store.path().is_none());
        assert_eq!(store.problem(), Some(&StoreProblem::NoPermission));
        assert!(sin_apartar(&dir));
        drop(store);
        // Resuelto el permiso, la base vuelve tal como estaba.
        let store = Store::open(&dir);
        assert!(store.problem().is_none());
        assert_eq!(store.settings().unwrap().get("theme"), Some(&Value::from("light")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn archivo_de_solo_lectura_no_se_aparta() {
        use std::os::unix::fs::PermissionsExt;
        let dir = base_con_una_cancion("solo-lectura");
        let file = dir.join(FILE);
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o444)).unwrap();
        let store = Store::open(&dir);
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        if store.path().is_none() {
            assert_ne!(store.problem(), Some(&StoreProblem::Damaged));
        }
        assert!(sin_apartar(&dir));
        drop(store);
        let store = Store::open(&dir);
        assert_eq!(store.settings().unwrap().get("theme"), Some(&Value::from("light")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Con un disco lleno de verdad (EMETPRO_DISCO_LLENO: una carpeta en él con una base ya creada, hecha cuando
    /// aún había espacio): la base no se aparta y se avisa de la falta de espacio.
    #[test]
    #[ignore]
    fn disco_lleno_de_verdad() {
        let dir = PathBuf::from(std::env::var("EMETPRO_DISCO_LLENO").expect("EMETPRO_DISCO_LLENO"));
        let store = Store::open(&dir);
        assert!(store.path().is_none(), "con el disco lleno no debería abrir");
        assert_eq!(store.problem(), Some(&StoreProblem::DiskFull));
        assert!(dir.join(FILE).is_file());
        assert!(sin_apartar(&dir));
    }

    #[test]
    fn disco_lleno_se_reconoce() {
        let dir = temp_dir("lleno");
        let full = OpenError::Io(std::io::Error::from(std::io::ErrorKind::StorageFull));
        assert_eq!(classify(&dir, &full), StoreProblem::DiskFull);
        let sqlite_full = OpenError::Sqlite(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_FULL),
            None,
        ));
        assert_eq!(classify(&dir, &sqlite_full), StoreProblem::DiskFull);
    }
}

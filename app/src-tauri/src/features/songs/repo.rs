// Letras de la biblioteca en la base. Las listas para el culto son planes (features/plans).
use rusqlite::{params, Connection};

use super::domain::Song;
use crate::core::db::{now_secs, Store};

/// Crea o actualiza una letra. También la usan otras funcionalidades dentro de su propia transacción
/// (features/transfer/repo.rs).
pub fn upsert_song(conn: &Connection, song: &Song) -> rusqlite::Result<()> {
    let animation = song.animation.map(|a| serde_json::to_string(&a)).transpose().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    conn.execute(
        "INSERT INTO songs (id, title, singer, writer, body, background, animation, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(id) DO UPDATE SET title = excluded.title, singer = excluded.singer,
             writer = excluded.writer, body = excluded.body, background = excluded.background,
             animation = excluded.animation, updated_at = excluded.updated_at",
        params![song.id, song.title, song.singer, song.writer, song.body, song.background, animation, now_secs()],
    )?;
    Ok(())
}

impl Store {
    pub fn songs(&self) -> rusqlite::Result<Vec<Song>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, singer, writer, body, background, animation FROM songs ORDER BY title COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Song {
                id: row.get(0)?,
                title: row.get(1)?,
                singer: row.get(2)?,
                writer: row.get(3)?,
                body: row.get(4)?,
                background: row.get(5)?,
                // Una animación que no se entiende (de una versión más nueva) se ignora: la letra sigue lo general.
                animation: row.get::<_, Option<String>>(6)?.and_then(|json| serde_json::from_str(&json).ok()),
            })
        })?;
        rows.collect()
    }

    pub fn save_song(&self, song: &Song) -> rusqlite::Result<()> {
        upsert_song(&self.conn, song)
    }

    /// Los planes que la usaban la muestran como borrada (features/plans).
    pub fn delete_song(&mut self, id: &str) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM songs WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Borra la biblioteca de letras.
    pub fn reset_songs(&mut self) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM songs", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::output::domain::{Entrance, TextAnimation, Transition};
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("emetpro-store-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn song(id: &str, title: &str) -> Song {
        Song {
            id: id.into(),
            title: title.into(),
            singer: String::new(),
            writer: String::new(),
            body: "a\n****\nb".into(),
            background: None,
            // Con su propia animación: se guarda y se lee igual (D-024).
            animation: Some(TextAnimation { transition: Transition::Zoom, entrance: Entrance::Words, ..TextAnimation::default() }),
        }
    }

    #[test]
    fn letras() {
        let dir = temp_dir("letras");
        let mut store = Store::open(&dir);
        store.save_song(&song("b", "Bendito")).unwrap();
        store.save_song(&song("a", "Alabanza")).unwrap();
        assert_eq!(store.songs().unwrap()[0].title, "Alabanza");
        assert_eq!(store.songs().unwrap()[0].animation.map(|a| a.transition), Some(Transition::Zoom));

        store.delete_song("b").unwrap();
        assert_eq!(store.songs().unwrap().len(), 1);
        store.reset_songs().unwrap();
        assert!(store.songs().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

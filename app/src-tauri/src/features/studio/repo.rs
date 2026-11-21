// Canciones de Media Studio en la base: cada una entera como JSON, como los elementos de un plan.
use rusqlite::params;

use super::domain::MediaSong;
use crate::core::db::{now_secs, Store};

impl Store {
    /// Por título. Una canción que no se entiende (de una versión más nueva, o dañada) se salta con un aviso en el
    /// registro, en vez de impedir ver las demás.
    pub fn studio_songs(&self) -> rusqlite::Result<Vec<MediaSong>> {
        let mut stmt = self.conn.prepare("SELECT id, data FROM studio_songs ORDER BY title COLLATE NOCASE")?;
        let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
        let mut songs = Vec::new();
        for row in rows {
            let (id, data) = row?;
            match serde_json::from_str::<MediaSong>(&data) {
                Ok(song) => songs.push(song),
                Err(e) => eprintln!("[media studio] se salta la canción {id}: {e}"),
            }
        }
        Ok(songs)
    }

    pub fn save_studio_song(&self, song: &MediaSong) -> rusqlite::Result<()> {
        let data = serde_json::to_string(song).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        self.conn.execute(
            "INSERT INTO studio_songs (id, title, data, updated_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET title = excluded.title, data = excluded.data, updated_at = excluded.updated_at",
            params![song.id, song.title, data, now_secs()],
        )?;
        Ok(())
    }

    pub fn delete_studio_song(&self, id: &str) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM studio_songs WHERE id = ?1", [id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::studio::domain::SongMode;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("emetpro-studio-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn song(id: &str, title: &str) -> MediaSong {
        serde_json::from_value(serde_json::json!({
            "id": id, "title": title, "mode": "live",
            "sections": [{"id": "c", "kind": "chorus", "slides": [{"id": "a", "lines": ["Santo"]}]}],
            "order": [{"id": "e1", "section": "c"}, {"id": "e2", "section": "c"}]
        }))
        .unwrap()
    }

    #[test]
    fn guardar_leer_y_borrar() {
        let dir = temp_dir("repo");
        let store = Store::open(&dir);
        store.save_studio_song(&song("b", "Bendito")).unwrap();
        store.save_studio_song(&song("a", "alabad")).unwrap();
        let mut changed = song("b", "Bendito sea");
        changed.mode = SongMode::Live;
        store.save_studio_song(&changed).unwrap();
        let songs = store.studio_songs().unwrap();
        assert_eq!(songs.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(), ["alabad", "Bendito sea"]);
        assert_eq!(songs[1].order.len(), 2);

        // Una fila que no se entiende no impide ver las demás.
        store.conn.execute("INSERT INTO studio_songs VALUES ('x', 'Rota', '{\"id\": 1}', 0)", []).unwrap();
        assert_eq!(store.studio_songs().unwrap().len(), 2);

        store.delete_studio_song("a").unwrap();
        assert_eq!(store.studio_songs().unwrap().len(), 2 - 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

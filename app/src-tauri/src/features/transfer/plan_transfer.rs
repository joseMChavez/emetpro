// Reglas para exportar e importar un plan (D-028), sin archivos ni base de datos.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::features::media::domain::{media_kind, MediaKind};
use crate::features::plans::domain::{ItemContent, PlanItem};

use super::formats::plan::{file_ref, parse_ref};

/// Los elementos de la sección `section_id` (ella incluida, hasta la siguiente sección), o todos.
pub fn select(items: &[PlanItem], section_id: Option<&str>) -> Vec<PlanItem> {
    let Some(id) = section_id else { return items.to_vec() };
    let Some(start) = items.iter().position(|i| i.id == id) else { return Vec::new() };
    let end = items[start + 1..]
        .iter()
        .position(|i| matches!(i.content, ItemContent::Section { .. }))
        .map_or(items.len(), |p| start + 1 + p);
    items[start..end].to_vec()
}

/// Lo que va en el archivo.
#[derive(Debug, Default)]
pub struct Packed {
    pub items: Vec<PlanItem>,
    /// (nombre, ruta en este equipo), en el orden de las referencias «@n».
    pub files: Vec<(String, PathBuf)>,
    /// Ids de las letras que usan los elementos, sin repetir y en orden.
    pub songs: Vec<String>,
    /// Videos que no van (solo su nombre queda en el plan).
    pub omitted: usize,
}

fn file_name(path: &str) -> String {
    Path::new(path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string())
}

/// Cambia las rutas por referencias a los archivos que viajan. Un archivo que ya no está, que no es un medio o un
/// video que no se incluye viaja solo con su nombre: nunca se mete en el archivo algo que no sea imagen, video, PDF
/// o presentación (un plan importado con una ruta inventada no puede sacar otros archivos del equipo, D-028).
pub fn pack(items: &[PlanItem], include_videos: bool, exists: impl Fn(&Path) -> bool) -> Packed {
    let mut out = Packed::default();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut add = |path: &str, out: &mut Packed| -> String {
        if let Some(&i) = index.get(path) {
            return file_ref(i);
        }
        if !exists(Path::new(path)) || media_kind(Path::new(path)).is_none() {
            return file_name(path);
        }
        out.files.push((file_name(path), PathBuf::from(path)));
        index.insert(path.to_string(), out.files.len() - 1);
        file_ref(out.files.len() - 1)
    };
    for item in items {
        let mut item = item.clone();
        match &mut item.content {
            ItemContent::Media { path, media } => {
                if *media == MediaKind::Video && !include_videos {
                    *path = file_name(path);
                    out.omitted += 1;
                } else {
                    *path = add(path, &mut out);
                }
            }
            ItemContent::Pause { background: Some(path), .. } => *path = add(path, &mut out),
            ItemContent::Song { song_id, .. } if !out.songs.contains(song_id) => out.songs.push(song_id.clone()),
            _ => {}
        }
        out.items.push(item);
    }
    out
}

/// Elementos leídos del archivo, listos para un plan de este equipo: ids nuevos, rutas a los archivos ya guardados
/// (`stored`, por índice) y letras con su id en la biblioteca (`songs`, por el id que traía el archivo). Una letra
/// que no se importó se quita; un elemento que esta versión no entiende, también.
pub fn unpack(
    values: &[serde_json::Value],
    stored: &HashMap<usize, String>,
    songs: &HashMap<String, String>,
    mut new_id: impl FnMut() -> String,
) -> (Vec<PlanItem>, usize) {
    let mut skipped = 0;
    let mut out = Vec::new();
    // Solo un «@n» guardado se vuelve una ruta de este equipo. Cualquier otra cosa queda en su nombre: una ruta
    // escrita en el archivo («/Users/ana/.ssh/id_rsa») no apunta a nada de este equipo.
    let resolve = |path: &mut String| match parse_ref(path).and_then(|i| stored.get(&i)) {
        Some(stored) => *path = stored.clone(),
        None => *path = file_name(path),
    };
    for value in values {
        let Ok(mut item) = serde_json::from_value::<PlanItem>(value.clone()) else {
            skipped += 1;
            continue;
        };
        item.id = new_id();
        match &mut item.content {
            ItemContent::Media { path, .. } => resolve(path),
            ItemContent::Pause { background: Some(path), .. } => resolve(path),
            ItemContent::Song { song_id, .. } => match songs.get(song_id) {
                Some(id) => *song_id = id.clone(),
                None => {
                    skipped += 1;
                    continue;
                }
            },
            _ => {}
        }
        out.push(item);
    }
    (out, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::output::domain::Marks;

    fn item(id: &str, content: ItemContent) -> PlanItem {
        PlanItem { id: id.into(), content }
    }

    fn sample() -> Vec<PlanItem> {
        vec![
            item("s1", ItemContent::Section { title: "Alabanza".into(), animate: None }),
            item("a", ItemContent::Song { song_id: "admirable".into(), marks: Marks::new() }),
            item("s2", ItemContent::Section { title: "Predicación".into(), animate: Some(false) }),
            item("v", ItemContent::Media { path: "/x/IMG_0001.MOV".into(), media: MediaKind::Video }),
            item("i", ItemContent::Media { path: "/x/anuncio.png".into(), media: MediaKind::Image }),
            item("p", ItemContent::Pause { title: "Receso".into(), duration_ms: 1000, background: Some("/x/anuncio.png".into()) }),
            item("t", ItemContent::Text { title: "Sermón".into(), slides: vec![], marks: Marks::new() }),
            item("s3", ItemContent::Section { title: "Final".into(), animate: None }),
            item("b", ItemContent::Song { song_id: "admirable".into(), marks: Marks::new() }),
        ]
    }

    #[test]
    fn una_seccion_va_hasta_la_siguiente() {
        let items = sample();
        let ids: Vec<_> = select(&items, Some("s2")).into_iter().map(|i| i.id).collect();
        assert_eq!(ids, ["s2", "v", "i", "p", "t"]);
        assert_eq!(select(&items, Some("s3")).len(), 2);
        assert_eq!(select(&items, None).len(), items.len());
        assert!(select(&items, Some("no")).is_empty());
    }

    #[test]
    fn empaca_sin_repetir_archivos_y_sin_videos_si_no_se_quieren() {
        let items = sample();
        let packed = pack(&items, false, |_| true);
        assert_eq!(packed.files, [("anuncio.png".to_string(), PathBuf::from("/x/anuncio.png"))]);
        assert_eq!(packed.omitted, 1);
        assert_eq!(packed.songs, ["admirable"]);
        assert!(matches!(&packed.items[3].content, ItemContent::Media { path, .. } if path == "IMG_0001.MOV"));
        assert!(matches!(&packed.items[4].content, ItemContent::Media { path, .. } if path == "@0"));
        assert!(matches!(&packed.items[5].content, ItemContent::Pause { background: Some(p), .. } if p == "@0"));

        let with_videos = pack(&items, true, |_| true);
        assert_eq!(with_videos.files.len(), 2);
        assert_eq!(with_videos.omitted, 0);
        // Un archivo que ya no está viaja solo con su nombre.
        let missing = pack(&items, true, |p| !p.ends_with("anuncio.png"));
        assert_eq!(missing.files.len(), 1);
        // Algo que no es un medio no viaja nunca, aunque exista.
        let secret = vec![item("k", ItemContent::Media { path: "/home/ana/.ssh/id_rsa".into(), media: MediaKind::Image })];
        let packed = pack(&secret, true, |_| true);
        assert!(packed.files.is_empty());
        assert!(matches!(&packed.items[0].content, ItemContent::Media { path, .. } if path == "id_rsa"));
    }

    #[test]
    fn desempaca_con_ids_nuevos_y_rutas_de_este_equipo() {
        let packed = pack(&sample(), false, |_| true);
        let values: Vec<_> = packed.items.iter().map(|i| serde_json::to_value(i).unwrap()).collect();
        let mut values = values;
        values.push(serde_json::json!({"id":"z","kind":"holograma"}));
        let stored = HashMap::from([(0, "/medios/ab/anuncio.png".to_string())]);
        let songs = HashMap::from([("admirable".to_string(), "lib-1".to_string())]);
        let mut n = 0;
        let (items, skipped) = unpack(&values, &stored, &songs, || {
            n += 1;
            format!("n{n}")
        });
        assert_eq!(skipped, 1, "el elemento desconocido se salta");
        assert_eq!(items.len(), 9);
        assert_eq!(items[0].id, "n1");
        assert!(matches!(&items[1].content, ItemContent::Song { song_id, .. } if song_id == "lib-1"));
        assert!(matches!(&items[3].content, ItemContent::Media { path, .. } if path == "IMG_0001.MOV"));
        assert!(matches!(&items[4].content, ItemContent::Media { path, .. } if path == "/medios/ab/anuncio.png"));

        // Una ruta escrita en el archivo (no «@n») o un «@n» que no se guardó quedan solo con su nombre.
        let forged = vec![
            serde_json::json!({"id":"f","kind":"media","path":"/home/ana/.ssh/id_rsa","media":"image"}),
            serde_json::json!({"id":"g","kind":"pause","title":"R","durationMs":1000,"background":"C:\\Users\\ana\\secreto.png"}),
            serde_json::json!({"id":"h","kind":"media","path":"@9","media":"image"}),
        ];
        let (items, _) = unpack(&forged, &stored, &songs, || "x".into());
        assert!(matches!(&items[0].content, ItemContent::Media { path, .. } if path == "id_rsa"));
        assert!(matches!(&items[1].content, ItemContent::Pause { background: Some(p), .. } if !p.contains('/') && p.ends_with("secreto.png")));
        assert!(matches!(&items[2].content, ItemContent::Media { path, .. } if path == "@9"));

        // Sin la letra en la biblioteca, el elemento no entra.
        let (items, skipped) = unpack(&values, &stored, &HashMap::new(), || "x".into());
        assert_eq!(items.len(), 7);
        assert_eq!(skipped, 3);
    }
}

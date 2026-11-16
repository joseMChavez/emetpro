// Un plan del culto: lo que se va a presentar, en orden (D-019). Tipos puros: sin Tauri.
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

use crate::features::media::domain::MediaKind;
use crate::features::output::domain::{Marks, RichSlide};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub id: String,
    pub name: String,
    /// Día del culto, «AAAA-MM-DD»; None = sin fecha.
    pub date: Option<String>,
    pub notes: String,
    pub items: Vec<PlanItem>,
    /// Animar el texto (D-024): None = como lo general.
    #[serde(default)]
    pub animate: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    /// Único dentro del plan; la interfaz lo usa para seguir cada elemento al reordenar.
    pub id: String,
    #[serde(flatten)]
    pub content: ItemContent,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ItemContent {
    /// Un título que ordena el plan («Alabanza», «Predicación»); no se muestra en pantalla.
    Section {
        title: String,
        /// Animar lo que sigue hasta la sección siguiente (D-024): None = como el plan.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        animate: Option<bool>,
    },
    /// Una letra de la biblioteca. Si se borra de la biblioteca, el plan la marca como borrada.
    Song {
        song_id: String,
        /// Palabras resaltadas en esta letra, para este plan (D-023).
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        marks: Marks,
    },
    /// Imagen, video, PDF o presentación, por su ruta en este equipo.
    Media { path: String, media: MediaKind },
    /// Versículos que el predicador quiere tener a un clic: un capítulo entero o un rango.
    Scripture {
        book: u32,
        chapter: u32,
        /// None = el capítulo entero.
        verse_start: Option<u32>,
        verse_end: Option<u32>,
        /// Versión fija (id de la Biblia); None = la elegida en la consola al mostrarlo.
        #[serde(default)]
        bible: Option<String>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        marks: Marks,
    },
    /// Texto con formato escrito en el plan (D-027): un sermón, avisos.
    Text {
        #[serde(default)]
        title: String,
        slides: Vec<RichSlide>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        marks: Marks,
    },
    /// Un tiempo con cuenta atrás en pantalla (receso, conferencia).
    Pause {
        title: String,
        duration_ms: u64,
        #[serde(default)]
        background: Option<String>,
    },
}

impl ItemContent {
    /// El archivo que este elemento necesita mostrar, si lo hay.
    pub fn file(&self) -> Option<&str> {
        match self {
            Self::Media { path, .. } => Some(path),
            Self::Pause { background, .. } => background.as_deref(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elementos_en_json() {
        let json = r#"[
            {"id":"1","kind":"section","title":"Alabanza"},
            {"id":"2","kind":"song","songId":"abc"},
            {"id":"3","kind":"media","path":"/x/a.mp4","media":"video"},
            {"id":"4","kind":"pause","title":"Receso","durationMs":600000},
            {"id":"5","kind":"scripture","book":43,"chapter":3,"verseStart":16,"verseEnd":18}
        ]"#;
        let items: Vec<PlanItem> = serde_json::from_str(json).unwrap();
        assert_eq!(items[1].content, ItemContent::Song { song_id: "abc".into(), marks: Marks::new() });
        assert_eq!(items[2].content.file(), Some("/x/a.mp4"));
        assert_eq!(items[3].content, ItemContent::Pause { title: "Receso".into(), duration_ms: 600_000, background: None });
        assert_eq!(
            items[4].content,
            ItemContent::Scripture { book: 43, chapter: 3, verse_start: Some(16), verse_end: Some(18), bible: None, marks: Marks::new() }
        );
        let back = serde_json::to_string(&items[1]).unwrap();
        assert_eq!(back, r#"{"id":"2","kind":"song","songId":"abc"}"#);

        // Con marcas (D-023): van y vuelven igual.
        let marked = r#"{"id":"6","kind":"song","songId":"abc","marks":{"0":[{"w":2,"t":"gracia","style":"color","color":"pink"}]}}"#;
        let item: PlanItem = serde_json::from_str(marked).unwrap();
        assert_eq!(serde_json::to_string(&item).unwrap(), marked);

        // Texto con formato (D-027): va y vuelve igual.
        let text = r#"{"id":"7","kind":"text","title":"Sermón","slides":[[{"type":"title","runs":[{"text":"La Reforma","b":true}]},{"type":"body","align":"left","runs":[{"text":"Lutero ","color":"cyan"},{"text":"buscaba paz"}]}]],"marks":{"0":[{"w":0,"t":"La","style":"marker","color":"yellow"}]}}"#;
        let item: PlanItem = serde_json::from_str(text).unwrap();
        assert!(matches!(&item.content, ItemContent::Text { slides, .. } if slides[0].len() == 2));
        assert_eq!(serde_json::to_string(&item).unwrap(), text);
    }
}

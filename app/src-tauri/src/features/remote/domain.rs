// Control remoto: lo que puede pedir un teléfono y cómo se le da acceso. Reglas puras, sin red ni Tauri.
use serde::{Deserialize, Serialize};

use crate::features::output::domain::{MarkColor, MarkStyle};

/// Puertos que se prueban en orden: el primero libre es el del control remoto.
pub const PORTS: std::ops::RangeInclusive<u16> = 7300..=7309;

/// Órdenes que llegan del teléfono. La consola las aplica igual que si se pulsara el botón en ella.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Command {
    /// Diapositiva, versículo o página siguiente de lo que está en pantalla.
    Next,
    Previous,
    /// Pantalla en negro, o volver a lo que había.
    Black,
    /// Quitar lo que hay en pantalla.
    Clear,
    /// Mostrar un versículo con la Biblia elegida en la consola.
    Scripture { book: u32, chapter: u32, verse: u32 },
    /// Elegir la versión de la Biblia (la de la consola cambia también).
    SetBible { id: String },
    /// Volver a mostrar algo de la lista «Para mostrar» (lo preparado o lo reciente), por su clave.
    Show { key: String },
    /// Mostrar un elemento del plan abierto en la consola, por su id (D-019).
    PlanItem { id: String },
    /// Ir a la posición n (desde 1) de lo que está en pantalla: página, diapositiva o versículo.
    GoTo { n: u32 },
    /// Poner en pantalla una hoja de la pizarra (board.rs), por su id.
    ShowSheet { id: String },
    /// Resaltar (o desmarcar) de la palabra `from` a la `to` de lo que está en pantalla (D-023).
    Mark { part: MarkPart, from: u32, to: u32, style: MarkStyle, color: MarkColor },
    /// Quitar las palabras resaltadas del versículo o la diapositiva en pantalla.
    ClearMarks,
    /// Media Studio (D-029): reproducir o pausar la pista de la canción en pantalla.
    TrackToggle,
    /// Repetir (o dejar de repetir) la sección en pantalla.
    TrackLoop,
    /// Terminar la canción con un fundido.
    TrackFinish,
    /// Dejar sonando de fondo, más suave, la canción que siguió sonando al poner otra cosa en pantalla.
    TrackBackground,
    /// Saltar a una sección de la canción en pantalla, por su id.
    StudioSection { id: String },
    /// El tono de la canción que suena, en semitonos (de −6 a +6; 0 = el original).
    TrackTone { semitones: i8 },
    /// Con voz o sin voz (si la canción tiene versión sin voz).
    TrackVoice { voiceless: bool },
}

/// El texto principal o, en la Biblia, la segunda versión.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MarkPart {
    Text,
    Second,
}

/// Más largo que esto no es un id ni una clave de la consola.
const MAX_TEXT: usize = 512;

impl Command {
    /// None si no es una orden conocida o trae números imposibles.
    pub fn parse(text: &str) -> Option<Command> {
        let command: Command = serde_json::from_str(text).ok()?;
        command.is_valid().then_some(command)
    }

    fn is_valid(&self) -> bool {
        match self {
            // Hay Biblias con libros deuterocanónicos: el número de libro llega hasta poco más de 80.
            Command::Scripture { book, chapter, verse } => {
                (1..=100).contains(book) && (1..=200).contains(chapter) && (1..=200).contains(verse)
            }
            Command::SetBible { id } => !id.is_empty() && id.len() <= MAX_TEXT,
            Command::Show { key } => !key.is_empty() && key.len() <= MAX_TEXT,
            Command::PlanItem { id } | Command::StudioSection { id } => !id.is_empty() && id.len() <= MAX_TEXT,
            Command::GoTo { n } => (1..=10_000).contains(n),
            Command::TrackTone { semitones } => semitones.abs() <= crate::features::studio::domain::TRANSPOSE_MAX,
            Command::ShowSheet { id } => !id.is_empty() && id.len() <= 64,
            // La ventana de salida mide como mucho 600 palabras.
            Command::Mark { from, to, .. } => *from < 1000 && *to < 1000,
            _ => true,
        }
    }
}

/// Clave nueva: va en el QR y la pide el servidor en cada conexión.
pub fn new_key() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// Una clave guardada sirve si tiene la forma de las que genera `new_key`.
pub fn is_key(text: &str) -> bool {
    text.len() == 32 && text.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Compara sin cortar en el primer carácter distinto, para no dar pistas por el tiempo de respuesta.
pub fn same_key(given: &str, expected: &str) -> bool {
    given.len() == expected.len() && given.bytes().zip(expected.bytes()).fold(0, |acc, (a, b)| acc | (a ^ b)) == 0
}

/// Dirección que se abre en el teléfono (la del QR).
pub fn url(host: &str, port: u16, key: &str) -> String {
    format!("http://{host}:{port}/remote?key={key}")
}

/// Manifiesto de la app del teléfono (PWA). A pantalla completa y en vertical, como una app.
pub fn manifest(start_url: &str) -> String {
    serde_json::json!({
        "name": "EmetPro · Control remoto",
        "short_name": "EmetPro",
        "start_url": start_url,
        "scope": "/",
        "display": "fullscreen",
        "display_override": ["fullscreen", "standalone"],
        "orientation": "portrait",
        "background_color": "#f4f2ed",
        "theme_color": "#1d3a78",
        "icons": [
            { "src": "/remote/icon-192.png", "sizes": "192x192", "type": "image/png" },
            { "src": "/remote/icon-512.png", "sizes": "512x512", "type": "image/png", "purpose": "any" }
        ]
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lee_las_ordenes() {
        assert_eq!(Command::parse(r#"{"type":"next"}"#), Some(Command::Next));
        assert_eq!(Command::parse(r#"{"type":"black"}"#), Some(Command::Black));
        assert_eq!(
            Command::parse(r#"{"type":"scripture","book":43,"chapter":3,"verse":16}"#),
            Some(Command::Scripture { book: 43, chapter: 3, verse: 16 })
        );
    }

    #[test]
    fn versiones_y_lista() {
        assert_eq!(Command::parse(r#"{"type":"setBible","id":"SpanishLBLABible"}"#), Some(Command::SetBible { id: "SpanishLBLABible".into() }));
        assert_eq!(Command::parse(r#"{"type":"show","key":"pdf:/a.pdf"}"#), Some(Command::Show { key: "pdf:/a.pdf".into() }));
        assert_eq!(Command::parse(r#"{"type":"planItem","id":"abc"}"#), Some(Command::PlanItem { id: "abc".into() }));
        assert_eq!(Command::parse(r#"{"type":"planItem","id":""}"#), None);
        assert_eq!(Command::parse(r#"{"type":"goTo","n":12}"#), Some(Command::GoTo { n: 12 }));
        assert_eq!(Command::parse(r#"{"type":"trackToggle"}"#), Some(Command::TrackToggle));
        assert_eq!(Command::parse(r#"{"type":"trackLoop"}"#), Some(Command::TrackLoop));
        assert_eq!(Command::parse(r#"{"type":"trackFinish"}"#), Some(Command::TrackFinish));
        assert_eq!(Command::parse(r#"{"type":"trackBackground"}"#), Some(Command::TrackBackground));
        assert_eq!(Command::parse(r#"{"type":"studioSection","id":"coro"}"#), Some(Command::StudioSection { id: "coro".into() }));
        assert_eq!(Command::parse(r#"{"type":"studioSection","id":""}"#), None);
        assert_eq!(Command::parse(r#"{"type":"trackTone","semitones":-2}"#), Some(Command::TrackTone { semitones: -2 }));
        assert_eq!(Command::parse(r#"{"type":"trackTone","semitones":7}"#), None);
        assert_eq!(Command::parse(r#"{"type":"trackTone","semitones":300}"#), None);
        assert_eq!(Command::parse(r#"{"type":"trackVoice","voiceless":true}"#), Some(Command::TrackVoice { voiceless: true }));
        assert_eq!(Command::parse(r#"{"type":"trackVoice"}"#), None);
        assert_eq!(Command::parse(r#"{"type":"goTo","n":0}"#), None);
        assert_eq!(Command::parse(r#"{"type":"goTo","n":-1}"#), None);
        assert_eq!(Command::parse(r#"{"type":"showSheet","id":"h1"}"#), Some(Command::ShowSheet { id: "h1".into() }));
        assert_eq!(Command::parse(r#"{"type":"showSheet","id":""}"#), None);
        assert_eq!(
            Command::parse(r#"{"type":"mark","part":"second","from":3,"to":1,"style":"underline","color":"pink"}"#),
            Some(Command::Mark { part: MarkPart::Second, from: 3, to: 1, style: MarkStyle::Underline, color: MarkColor::Pink })
        );
        assert_eq!(Command::parse(r#"{"type":"mark","part":"text","from":0,"to":5000,"style":"marker","color":"red"}"#), None);
        assert_eq!(Command::parse(r#"{"type":"mark","part":"text","from":0,"to":1,"style":"glitter","color":"red"}"#), None);
        assert_eq!(Command::parse(r#"{"type":"clearMarks"}"#), Some(Command::ClearMarks));
        assert_eq!(Command::parse(r#"{"type":"setBible","id":""}"#), None);
        assert_eq!(Command::parse(&format!(r#"{{"type":"show","key":"{}"}}"#, "x".repeat(600))), None);
    }

    #[test]
    fn rechaza_lo_desconocido() {
        assert_eq!(Command::parse(r#"{"type":"shutdown"}"#), None);
        assert_eq!(Command::parse("no es json"), None);
        assert_eq!(Command::parse(r#"{"type":"scripture","book":0,"chapter":3,"verse":16}"#), None);
        assert_eq!(Command::parse(r#"{"type":"scripture","book":43,"chapter":3}"#), None);
    }

    #[test]
    fn claves() {
        let key = new_key();
        assert!(is_key(&key));
        assert!(same_key(&key, &key));
        assert!(!same_key(&key, &new_key()));
        assert!(!same_key("", &key));
        assert!(!is_key("../../etc"));
        assert_eq!(url("192.168.1.20", 7300, "ab"), "http://192.168.1.20:7300/remote?key=ab");
    }

    #[test]
    fn manifiesto() {
        let m: serde_json::Value = serde_json::from_str(&manifest("/remote?key=ab")).unwrap();
        assert_eq!(m["start_url"], "/remote?key=ab");
        assert_eq!(m["display"], "fullscreen");
    }
}

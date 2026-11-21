// Lo que se muestra en la ventana de salida y con qué estilo. Tipos puros: sin Tauri.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Palabras resaltadas (D-023), por versículo o diapositiva: «versión|cita» en la Biblia, el número de diapositiva en
/// las letras. Un BTreeMap para que el JSON salga siempre en el mismo orden.
pub type Marks = BTreeMap<String, Vec<WordMark>>;

/// Una palabra resaltada: su número en el texto (cada tramo sin espacios, desde 0) y la palabra misma, para no pintar
/// otra si el texto cambió.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WordMark {
    pub w: u32,
    pub t: String,
    pub style: MarkStyle,
    pub color: MarkColor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MarkStyle {
    Marker,
    Underline,
    Color,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MarkColor {
    Yellow,
    Green,
    Blue,
    Pink,
    Orange,
    Red,
}

// ---- Texto con formato (D-027) ----

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlockType {
    Title,
    Subtitle,
    Body,
    Bullet,
    Number,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlockAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextColor {
    White,
    Yellow,
    Orange,
    Red,
    Pink,
    Violet,
    Blue,
    Cyan,
    Green,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// Un tramo de texto con su formato.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RichRun {
    pub text: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub b: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub i: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub u: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<TextColor>,
}

/// Un párrafo: título, subtítulo, texto o elemento de lista, con su alineación (None = la del estilo).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RichBlock {
    #[serde(rename = "type")]
    pub kind: BlockType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<BlockAlign>,
    #[serde(default)]
    pub runs: Vec<RichRun>,
}

/// Una diapositiva con formato.
pub type RichSlide = Vec<RichBlock>;

#[derive(Clone, Default, Serialize, Deserialize)]
// rename_all solo cambia el nombre de las variantes; rename_all_fields, el de sus campos (duration_ms → durationMs).
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum OutputContent {
    #[default]
    Empty,
    Black,
    TestPattern,
    /// Serie completa de diapositivas: la salida conserva una copia fija aunque el operador edite el texto preparado.
    Text {
        slides: Vec<String>,
        index: usize,
        /// Autoría de una letra (cantante, escritor); se muestra en la primera diapositiva.
        #[serde(default)]
        credit: String,
        /// Imagen o GIF propio de esta letra; tapa el fondo general.
        #[serde(default)]
        background: Option<String>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        marks: Marks,
        /// Solo si la cascada (plan, sección, letra) cambia lo general (D-024).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        animation: Option<TextAnimation>,
        /// Con formato (D-027), una por diapositiva; `slides` lleva su texto plano.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rich: Option<Vec<RichSlide>>,
        /// Canción de Media Studio (D-029) de la que salen estas diapositivas: la consola la sigue con su pista.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        studio: Option<String>,
        /// Fondo propio de cada diapositiva (el de su sección en Media Studio); None = `background`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        backgrounds: Option<Vec<Option<String>>>,
        /// Media Studio: cuándo empieza (ms de la pista) cada palabra de cada diapositiva, para iluminar la que suena.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        words: Option<Vec<Option<Vec<Option<u32>>>>>,
    },
    Image {
        path: String,
    },
    Video {
        path: String,
        paused: bool,
        /// Volumen de 0 a 1. Lo que no lo trae (contenidos anteriores) suena entero.
        #[serde(default = "full_volume")]
        volume: f64,
        #[serde(default)]
        muted: bool,
    },
    Pdf {
        path: String,
        page: u32,
    },
    /// Pasaje bíblico, un versículo por diapositiva. El copyright viaja con el contenido para mostrarse siempre.
    Scripture {
        verses: Vec<ScriptureVerse>,
        index: usize,
        version: String,
        copyright: String,
        /// Segunda versión, debajo de la primera (por ejemplo, en otro idioma). Como mucho dos.
        #[serde(default)]
        second: Option<SecondVersion>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        marks: Marks,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        animation: Option<TextAnimation>,
    },
    /// Hora actual; la dibuja la salida con su propio reloj.
    Clock {
        hour12: bool,
        seconds: bool,
    },
    /// Cuenta atrás. En marcha si ends_at (ms desde 1970) tiene valor; en pausa guarda lo que queda.
    /// Al llegar a cero sigue contando el tiempo excedido con un borde rojo.
    Timer {
        duration_ms: u64,
        ends_at: Option<u64>,
        remaining_ms: i64,
        /// Título sobre la cuenta («Receso», «Conferencia»); vacío = solo la cuenta.
        #[serde(default)]
        label: String,
        /// Imagen o GIF propio de esta cuenta; tapa el fondo general.
        #[serde(default)]
        background: Option<String>,
    },
    /// Una hoja de la pizarra del control remoto (remote/board.rs): su fondo y lo dibujado viven allí.
    Board {
        sheet: String,
    },
}

/// Cómo se anima el texto en pantalla (D-024).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TextAnimation {
    pub transition: Transition,
    pub entrance: Entrance,
    pub ambient: Ambient,
    pub background: BackgroundMotion,
}

impl Default for TextAnimation {
    /// Lo de siempre más un fundido suave entre diapositivas.
    fn default() -> Self {
        Self { transition: Transition::Fade, entrance: Entrance::None, ambient: Ambient::None, background: BackgroundMotion::Still }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Transition {
    #[default]
    None,
    Fade,
    Zoom,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Entrance {
    #[default]
    None,
    Lines,
    Words,
    Typewriter,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Ambient {
    #[default]
    None,
    Breathe,
    Float,
    Glow,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BackgroundMotion {
    #[default]
    Still,
    Slow,
}

fn yes() -> bool {
    true
}

fn full_volume() -> f64 {
    1.0
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecondVersion {
    pub version: String,
    pub copyright: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptureVerse {
    pub reference: String,
    pub text: String,
    /// El mismo versículo en la segunda versión; None si esa versión no lo tiene.
    #[serde(default)]
    pub second: Option<ParallelVerse>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParallelVerse {
    pub reference: String,
    pub text: String,
}

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextAlign {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VerticalPosition {
    Top,
    #[default]
    Middle,
    Bottom,
}

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Background {
    #[default]
    Black,
    Navy,
    Charcoal,
}

/// Tipos de letra incluidos en la app (no dependen de las fuentes instaladas en cada equipo).
#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Font {
    #[default]
    System,
    Montserrat,
    Oswald,
    BebasNeue,
    Lora,
    Merriweather,
}

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Corner {
    TopLeft,
    #[default]
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LogoSize {
    Small,
    #[default]
    Medium,
    Large,
}

/// Logo o emblema de la iglesia, como marca de agua en una esquina de la pantalla.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Logo {
    /// None = sin logo.
    pub image: Option<String>,
    pub corner: Corner,
    pub size: LogoSize,
    /// 0..1; como marca de agua, por defecto semitransparente.
    pub opacity: f64,
}

impl Default for Logo {
    fn default() -> Self {
        Self { image: None, corner: Corner::TopRight, size: LogoSize::Medium, opacity: 0.6 }
    }
}

/// Cómo se coloca el texto en la pantalla. Se aplica a lo que está al aire en cuanto cambia.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputStyle {
    /// Tamaño máximo relativo; el texto se reduce solo si no cabe.
    pub font_scale: f32,
    pub align: TextAlign,
    pub position: VerticalPosition,
    pub background: Background,
    #[serde(default)]
    pub font: Font,
    /// Imagen o GIF detrás del texto, la Biblia y el reloj.
    #[serde(default)]
    pub background_image: Option<String>,
    #[serde(default)]
    pub logo: Logo,
    /// Si se anima el texto (D-024); un plan, una sección o una letra pueden decir otra cosa.
    #[serde(default = "yes")]
    pub animate: bool,
    #[serde(default)]
    pub animation: TextAnimation,
}

impl Default for OutputStyle {
    fn default() -> Self {
        Self {
            font_scale: 1.0,
            align: TextAlign::Center,
            position: VerticalPosition::Middle,
            background: Background::Black,
            font: Font::System,
            background_image: None,
            logo: Logo::default(),
            animate: true,
            animation: TextAnimation::default(),
        }
    }
}

impl OutputContent {
    pub fn media_path(&self) -> Option<&str> {
        match self {
            Self::Image { path } | Self::Video { path, .. } | Self::Pdf { path, .. } => Some(path),
            Self::Text { background, .. } | Self::Timer { background, .. } => background.as_deref(),
            _ => None,
        }
    }
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputState {
    pub open: bool,
    pub monitor_name: Option<String>,
    pub content: OutputContent,
    pub style: OutputStyle,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn estilo_con_y_sin_logo() {
        // Un estilo guardado antes de que existiera el logo sigue valiendo: sin logo, esquina superior derecha.
        let old: OutputStyle = serde_json::from_str(
            r#"{"fontScale":1.1,"align":"center","position":"middle","background":"black","font":"system","backgroundImage":null}"#,
        )
        .unwrap();
        assert!(old.logo.image.is_none());
        assert!(old.animate);
        assert_eq!(old.animation.transition, Transition::Fade);
        assert!(matches!(old.logo.corner, Corner::TopRight));

        let json = r#"{"fontScale":1.5,"align":"left","position":"top","background":"navy","font":"oswald","backgroundImage":null,"logo":{"image":"/x/logo.png","corner":"bottomRight","size":"large","opacity":0.4},"animate":false,"animation":{"transition":"zoom","entrance":"typewriter","ambient":"glow","background":"slow"}}"#;
        let style: OutputStyle = serde_json::from_str(json).unwrap();
        assert_eq!(serde_json::to_value(&style).unwrap(), serde_json::from_str::<serde_json::Value>(json).unwrap());
    }

    #[test]
    pub fn video_sin_volumen_suena_entero() {
        let old: OutputContent = serde_json::from_str(r#"{"kind":"video","path":"/x/a.mp4","paused":true}"#).unwrap();
        assert!(matches!(old, OutputContent::Video { volume, muted: false, .. } if volume == 1.0));
    }

    // Los contenidos llegan de la interfaz en camelCase: un campo mal nombrado solo falla al enviarlo.
    #[test]
    pub fn contenidos_de_la_interfaz() {
        for json in [
            r#"{"kind":"timer","durationMs":60000,"endsAt":null,"remainingMs":60000,"label":"Receso","background":null}"#,
            r#"{"kind":"clock","hour12":true,"seconds":false}"#,
            r#"{"kind":"board","sheet":"h1"}"#,
            r#"{"kind":"text","slides":["a"],"index":0,"credit":"","background":"/x/f.mp4","animation":{"transition":"none","entrance":"lines","ambient":"float","background":"still"}}"#,
            r#"{"kind":"text","slides":["Título\nUno dos"],"index":0,"credit":"","background":null,"rich":[[{"type":"title","align":"center","runs":[{"text":"Título","b":true,"color":"yellow"}]},{"type":"bullet","runs":[{"text":"Uno ","i":true},{"text":"dos","u":true}]}]]}"#,
            r#"{"kind":"video","path":"/x/a.mp4","paused":false,"volume":0.4,"muted":true}"#,
            r#"{"kind":"text","slides":["a"],"index":0,"credit":"c","background":null}"#,
            r#"{"kind":"text","slides":["a b"],"index":0,"credit":"","background":null,"marks":{"0":[{"w":1,"t":"b","style":"marker","color":"yellow"}]}}"#,
            r#"{"kind":"scripture","verses":[{"reference":"Jn 3:16","text":"t","second":null}],"index":0,"version":"V","copyright":"","second":null,"marks":{"V|Jn 3:16":[{"w":0,"t":"t","style":"underline","color":"blue"}]}}"#,
            r#"{"kind":"scripture","verses":[{"reference":"Jn 3:16","text":"t","second":{"reference":"Jn 3:16","text":"t"}}],"index":0,"version":"V","copyright":"","second":{"version":"W","copyright":""}}"#,
        ] {
            let content: OutputContent = serde_json::from_str(json).expect(json);
            assert_eq!(serde_json::to_value(&content).unwrap(), serde_json::from_str::<serde_json::Value>(json).unwrap());
        }
    }
}

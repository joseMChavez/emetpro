// Media Studio (D-029): una canción con secciones escritas una vez y un orden que las repite. Tipos puros: sin Tauri.
//
// Tiempos: milisegundos enteros desde el comienzo de la pista. Solo se guarda dónde empieza cada cosa; cada tramo
// dura `[comienzo, comienzo del siguiente)`, con el comienzo incluido y el final excluido:
//   aparición    → hasta la siguiente aparición con tiempo, o el final de la pista;
//   diapositiva  → hasta la siguiente diapositiva de su aparición, o el final de la aparición;
//   línea        → hasta su `end_ms` si lo tiene (un silencio), o la siguiente línea, o el final de la diapositiva;
//   palabra      → hasta la siguiente palabra, o el final de la línea.
// Así no hay solapamientos posibles entre apariciones ni entre diapositivas: el siguiente comienzo cierra el
// anterior. Lo único que puede pisarse es una línea con `end_ms` más allá del comienzo de la siguiente, y es error.
use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::features::output::domain::TextAnimation;

/// Cómo se presenta: con la grabación o siguiendo a músicos en vivo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SongMode {
    /// «Pista sincronizada»: la diapositiva sale del tiempo de la pista.
    Track,
    /// «Interpretación en directo»: el operador avanza a mano y no se carga ningún audio.
    Live,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SectionKind {
    Intro,
    Verse,
    PreChorus,
    Chorus,
    Bridge,
    Instrumental,
    Interlude,
    Tag,
    Outro,
    Custom,
}

/// La pista de audio: una copia en la carpeta de la app (bloque 3), y su duración medida al cargarla.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTrack {
    pub path: String,
    /// El nombre del archivo original, para mostrarlo.
    pub name: String,
    pub duration_ms: u32,
}

/// Derechos, por separado: tener el archivo no significa poder usarlo ni repartirlo. Solo texto.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Rights {
    /// Música (composición).
    pub composition: String,
    pub lyrics: String,
    /// La grabación de la pista.
    pub recording: String,
    /// Licencia o número de licencia de la iglesia (CCLI, etc.).
    pub license: String,
    pub notes: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongSlide {
    pub id: String,
    #[serde(default)]
    pub lines: Vec<String>,
}

/// Una sección se escribe una vez. Sin diapositivas (una intro instrumental) deja la pantalla solo con el fondo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongSection {
    pub id: String,
    pub kind: SectionKind,
    /// Vacío = el nombre de su tipo («Coro», «Estrofa 2»), que pone la interfaz.
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub slides: Vec<SongSlide>,
}

/// Una línea dentro de la pista. `words[i]` es el comienzo de la palabra i de la línea (cada tramo sin espacios,
/// como las marcas de D-023); vacío = sincronizada solo por línea, y pueden faltar las últimas.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineCue {
    pub start_ms: u32,
    /// Solo si la línea termina antes de que empiece la siguiente (un silencio en medio de la diapositiva).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_ms: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub words: Vec<u32>,
}

/// Cuándo empieza una diapositiva en esta aparición. `lines[i]` es la línea i de la diapositiva.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideCue {
    pub slide: String,
    pub start_ms: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<LineCue>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Timing {
    pub start_ms: u32,
    /// En el orden de la sección. Puede faltar alguna: esa diapositiva no avanza sola en esta aparición.
    #[serde(default)]
    pub slides: Vec<SlideCue>,
}

/// Una aparición de una sección en la canción. El mismo coro aparece varias veces, cada una con sus tiempos.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderEntry {
    pub id: String,
    pub section: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing: Option<Timing>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSong {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub artist: String,
    pub mode: SongMode,
    /// Una canción en directo puede conservar su pista (otro domingo se usa); en directo no se carga.
    #[serde(default)]
    pub audio: Option<AudioTrack>,
    #[serde(default)]
    pub sections: Vec<SongSection>,
    #[serde(default)]
    pub order: Vec<OrderEntry>,
    #[serde(default)]
    pub rights: Rights,
    /// Imagen o video de fondo propio; tapa el fondo general.
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub animation: Option<TextAnimation>,
}

// ---- Validación ----

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    /// No se puede guardar así.
    Error,
    /// Se guarda, pero conviene mirarlo.
    Warning,
}

/// Qué está mal. La interfaz lo traduce (`studio.issue.<code>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum IssueCode {
    MissingTitle,
    MissingId,
    DuplicateId,
    EmptyOrder,
    UnknownSection,
    TrackWithoutAudio,
    InvalidAudio,
    /// Empieza en el final de la pista o más allá.
    TimeOutOfTrack,
    /// Las apariciones con tiempo deben ir hacia adelante en la pista, en el orden de la canción.
    EntriesOutOfOrder,
    UnknownSlide,
    /// La misma diapositiva dos veces en una aparición.
    DuplicateCue,
    /// Fuera del orden de la sección o hacia atrás en el tiempo.
    SlidesOutOfOrder,
    SlideOutsideEntry,
    LinesOutOfOrder,
    LineOutsideSlide,
    LineEndBeforeStart,
    /// Una línea termina después de que empiece la siguiente.
    LinesOverlap,
    WordsOutOfOrder,
    WordOutsideLine,
    // Avisos
    UnusedSection,
    EmptySlide,
    UntimedEntry,
    UntimedSlide,
    /// Hay tiempos de más líneas que las que tiene la diapositiva (se cambió el texto): se ignoran.
    ExtraLineCues,
    /// Ídem con las palabras de una línea.
    ExtraWordCues,
}

impl IssueCode {
    pub fn severity(self) -> Severity {
        use IssueCode::*;
        match self {
            UnusedSection | EmptySlide | UntimedEntry | UntimedSlide | ExtraLineCues | ExtraWordCues => Severity::Warning,
            _ => Severity::Error,
        }
    }
}

/// Dónde está, de lo más general a lo más concreto. Las líneas y las palabras se cuentan desde 0.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slide: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub code: IssueCode,
    pub severity: Severity,
    pub at: Location,
}

/// Una diapositiva de la canción desplegada según su orden. `slide: None` es una sección sin diapositivas
/// (instrumental): la pantalla queda con el fondo.
// Lo usarán el elemento de plan y la exportación (D-029); la interfaz despliega por su cuenta (studio/song.ts).
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExpandedSlide<'a> {
    pub entry: &'a str,
    pub section: &'a SongSection,
    pub slide: Option<&'a SongSlide>,
}

impl MediaSong {
    pub fn section(&self, id: &str) -> Option<&SongSection> {
        self.sections.iter().find(|s| s.id == id)
    }

    /// Las diapositivas en el orden en que se cantan; un coro que se repite aparece cada vez.
    #[allow(dead_code)]
    pub fn expand(&self) -> Vec<ExpandedSlide<'_>> {
        let mut out = Vec::new();
        for entry in &self.order {
            let Some(section) = self.section(&entry.section) else { continue };
            if section.slides.is_empty() {
                out.push(ExpandedSlide { entry: &entry.id, section, slide: None });
            }
            for slide in &section.slides {
                out.push(ExpandedSlide { entry: &entry.id, section, slide: Some(slide) });
            }
        }
        out
    }

    /// Todo lo que hay que revisar, errores y avisos. Sin errores, se puede guardar.
    pub fn validate(&self) -> Vec<Issue> {
        let mut issues = Issues::default();
        self.check_structure(&mut issues);
        if self.mode == SongMode::Track {
            match &self.audio {
                None => issues.add(IssueCode::TrackWithoutAudio, Location::default()),
                Some(audio) if audio.duration_ms == 0 => issues.add(IssueCode::InvalidAudio, Location::default()),
                Some(audio) => self.check_timing(audio.duration_ms, &mut issues),
            }
        }
        // En directo los tiempos no se usan: se conservan tal cual por si vuelve a la pista, y se revisan entonces.
        issues.0
    }

    pub fn has_errors(issues: &[Issue]) -> bool {
        issues.iter().any(|i| i.severity == Severity::Error)
    }

    fn check_structure(&self, issues: &mut Issues) {
        if self.title.trim().is_empty() {
            issues.add(IssueCode::MissingTitle, Location::default());
        }
        let mut ids = HashSet::new();
        let mut check_id = |id: &str, at: Location, issues: &mut Issues| {
            if id.is_empty() {
                issues.add(IssueCode::MissingId, at);
            } else if !ids.insert(id.to_owned()) {
                issues.add(IssueCode::DuplicateId, at);
            }
        };
        for section in &self.sections {
            check_id(&section.id, at_section(&section.id), issues);
            for slide in &section.slides {
                check_id(&slide.id, at_slide(None, &section.id, &slide.id), issues);
                if slide.lines.iter().all(|l| l.trim().is_empty()) {
                    issues.add(IssueCode::EmptySlide, at_slide(None, &section.id, &slide.id));
                }
            }
        }
        for entry in &self.order {
            check_id(&entry.id, at_entry(&entry.id), issues);
            if self.section(&entry.section).is_none() {
                issues.add(IssueCode::UnknownSection, Location { entry: Some(entry.id.clone()), section: Some(entry.section.clone()), ..Location::default() });
            }
        }
        if self.order.is_empty() {
            issues.add(IssueCode::EmptyOrder, Location::default());
        }
        let used: HashSet<&str> = self.order.iter().map(|e| e.section.as_str()).collect();
        for section in self.sections.iter().filter(|s| !used.contains(s.id.as_str())) {
            issues.add(IssueCode::UnusedSection, at_section(&section.id));
        }
    }

    fn check_timing(&self, duration: u32, issues: &mut Issues) {
        // Las apariciones con tiempo, en el orden de la canción; cada una termina donde empieza la siguiente.
        let timed: Vec<(&OrderEntry, &Timing)> = self.order.iter().filter_map(|e| e.timing.as_ref().map(|t| (e, t))).collect();
        for entry in self.order.iter().filter(|e| e.timing.is_none()) {
            issues.add(IssueCode::UntimedEntry, at_entry(&entry.id));
        }
        for (i, (entry, timing)) in timed.iter().enumerate() {
            if timing.start_ms >= duration {
                issues.add(IssueCode::TimeOutOfTrack, at_entry(&entry.id));
                continue;
            }
            let next = timed.get(i + 1).map(|(_, t)| t.start_ms);
            if next.is_some_and(|next| next <= timing.start_ms) {
                issues.add(IssueCode::EntriesOutOfOrder, at_entry(&timed[i + 1].0.id));
            }
            // Si la siguiente está mal colocada, esta se revisa hasta el final de la pista.
            let end = next.filter(|&n| n > timing.start_ms && n <= duration).unwrap_or(duration);
            if let Some(section) = self.section(&entry.section) {
                check_entry(entry, timing, end, section, issues);
            }
        }
    }
}

fn check_entry(entry: &OrderEntry, timing: &Timing, end: u32, section: &SongSection, issues: &mut Issues) {
    let position: HashMap<&str, usize> = section.slides.iter().enumerate().map(|(i, s)| (s.id.as_str(), i)).collect();
    let mut seen = HashSet::new();
    let mut last: Option<(usize, u32)> = None;
    for (i, cue) in timing.slides.iter().enumerate() {
        let at = at_slide(Some(&entry.id), &section.id, &cue.slide);
        let Some(&index) = position.get(cue.slide.as_str()) else {
            issues.add(IssueCode::UnknownSlide, at);
            continue;
        };
        if !seen.insert(index) {
            issues.add(IssueCode::DuplicateCue, at);
            continue;
        }
        if last.is_some_and(|(prev_index, prev_start)| index <= prev_index || cue.start_ms <= prev_start) {
            issues.add(IssueCode::SlidesOutOfOrder, at.clone());
        }
        last = Some((index, cue.start_ms));
        if cue.start_ms < timing.start_ms || cue.start_ms >= end {
            issues.add(IssueCode::SlideOutsideEntry, at);
            continue;
        }
        let slide_end = timing.slides[i + 1..]
            .iter()
            .map(|c| c.start_ms)
            .find(|&s| s > cue.start_ms && s <= end)
            .unwrap_or(end);
        check_slide(&entry.id, &section.slides[index], &section.id, cue, slide_end, issues);
    }
    for slide in section.slides.iter().filter(|s| !timing.slides.iter().any(|c| c.slide == s.id)) {
        issues.add(IssueCode::UntimedSlide, at_slide(Some(&entry.id), &section.id, &slide.id));
    }
}

fn check_slide(entry: &str, slide: &SongSlide, section: &str, cue: &SlideCue, end: u32, issues: &mut Issues) {
    let at_line = |line: usize, word: Option<usize>| Location {
        line: Some(line as u32),
        word: word.map(|w| w as u32),
        ..at_slide(Some(entry), section, &slide.id)
    };
    if cue.lines.len() > slide.lines.len() {
        issues.add(IssueCode::ExtraLineCues, at_line(slide.lines.len(), None));
    }
    let lines = &cue.lines[..cue.lines.len().min(slide.lines.len())];
    for (i, line) in lines.iter().enumerate() {
        if i > 0 && line.start_ms <= lines[i - 1].start_ms {
            issues.add(IssueCode::LinesOutOfOrder, at_line(i, None));
        }
        if line.start_ms < cue.start_ms || line.start_ms >= end {
            issues.add(IssueCode::LineOutsideSlide, at_line(i, None));
            continue;
        }
        let next = lines.get(i + 1).map(|l| l.start_ms).filter(|&n| n > line.start_ms && n <= end);
        let line_end = match line.end_ms {
            Some(e) if e <= line.start_ms => {
                issues.add(IssueCode::LineEndBeforeStart, at_line(i, None));
                next.unwrap_or(end)
            }
            Some(e) if e > end => {
                issues.add(IssueCode::LineOutsideSlide, at_line(i, None));
                next.unwrap_or(end)
            }
            Some(e) if next.is_some_and(|n| e > n) => {
                issues.add(IssueCode::LinesOverlap, at_line(i, None));
                next.unwrap_or(end)
            }
            Some(e) => e,
            None => next.unwrap_or(end),
        };
        let words = slide.lines[i].split_whitespace().count();
        if line.words.len() > words {
            issues.add(IssueCode::ExtraWordCues, at_line(i, Some(words)));
        }
        let timed = &line.words[..line.words.len().min(words)];
        for (w, &start) in timed.iter().enumerate() {
            if w > 0 && start <= timed[w - 1] {
                issues.add(IssueCode::WordsOutOfOrder, at_line(i, Some(w)));
            }
            if start < line.start_ms || start >= line_end {
                issues.add(IssueCode::WordOutsideLine, at_line(i, Some(w)));
            }
        }
    }
}

#[derive(Default)]
struct Issues(Vec<Issue>);

impl Issues {
    fn add(&mut self, code: IssueCode, at: Location) {
        self.0.push(Issue { code, severity: code.severity(), at });
    }
}

fn at_entry(entry: &str) -> Location {
    Location { entry: Some(entry.to_owned()), ..Location::default() }
}

fn at_section(section: &str) -> Location {
    Location { section: Some(section.to_owned()), ..Location::default() }
}

fn at_slide(entry: Option<&str>, section: &str, slide: &str) -> Location {
    Location { entry: entry.map(str::to_owned), section: Some(section.to_owned()), slide: Some(slide.to_owned()), ..Location::default() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slide(id: &str, lines: &[&str]) -> SongSlide {
        SongSlide { id: id.into(), lines: lines.iter().map(|l| l.to_string()).collect() }
    }

    fn section(id: &str, kind: SectionKind, slides: Vec<SongSlide>) -> SongSection {
        SongSection { id: id.into(), kind, label: String::new(), slides }
    }

    fn entry(id: &str, section: &str, timing: Option<Timing>) -> OrderEntry {
        OrderEntry { id: id.into(), section: section.into(), timing }
    }

    fn cue(slide: &str, start_ms: u32, lines: Vec<LineCue>) -> SlideCue {
        SlideCue { slide: slide.into(), start_ms, lines }
    }

    fn line(start_ms: u32, end_ms: Option<u32>, words: &[u32]) -> LineCue {
        LineCue { start_ms, end_ms, words: words.to_vec() }
    }

    fn timing(start_ms: u32, slides: Vec<SlideCue>) -> Option<Timing> {
        Some(Timing { start_ms, slides })
    }

    /// Intro instrumental, estrofa, coro, estrofa, coro: el coro se escribe una vez y suena dos.
    fn sample() -> MediaSong {
        MediaSong {
            id: "s".into(),
            title: "Sublime gracia".into(),
            artist: String::new(),
            mode: SongMode::Track,
            audio: Some(AudioTrack { path: "/pistas/a.mp3".into(), name: "a.mp3".into(), duration_ms: 100_000 }),
            sections: vec![
                section("intro", SectionKind::Intro, vec![]),
                section("v1", SectionKind::Verse, vec![slide("v1a", &["Sublime gracia del Señor"])]),
                section("v2", SectionKind::Verse, vec![slide("v2a", &["En los peligros"])]),
                section("coro", SectionKind::Chorus, vec![slide("c1", &["Gloria a Dios", "por siempre"]), slide("c2", &["Amén"])]),
            ],
            order: vec![
                entry("e0", "intro", timing(0, vec![])),
                entry("e1", "v1", timing(8_000, vec![cue("v1a", 8_000, vec![line(8_000, None, &[8_000, 8_500, 9_000, 9_500])])])),
                entry("e2", "coro", timing(20_000, vec![cue("c1", 20_000, vec![line(20_000, None, &[]), line(24_000, Some(27_000), &[])]), cue("c2", 30_000, vec![])])),
                entry("e3", "v2", timing(40_000, vec![cue("v2a", 40_000, vec![])])),
                entry("e4", "coro", timing(60_000, vec![cue("c1", 60_000, vec![]), cue("c2", 70_000, vec![])])),
            ],
            rights: Rights::default(),
            background: None,
            animation: None,
        }
    }

    fn codes(song: &MediaSong) -> Vec<IssueCode> {
        song.validate().into_iter().map(|i| i.code).collect()
    }

    #[test]
    fn coro_repetido_con_sus_tiempos_es_valido() {
        let song = sample();
        assert_eq!(codes(&song), vec![]);
        let expanded: Vec<_> = song.expand().iter().map(|e| (e.entry, e.slide.map(|s| s.id.as_str()))).collect();
        assert_eq!(
            expanded,
            vec![("e0", None), ("e1", Some("v1a")), ("e2", Some("c1")), ("e2", Some("c2")), ("e3", Some("v2a")), ("e4", Some("c1")), ("e4", Some("c2"))]
        );
    }

    #[test]
    fn en_directo_no_necesita_pista_ni_tiempos() {
        let mut song = sample();
        song.mode = SongMode::Live;
        song.audio = None;
        for e in &mut song.order {
            e.timing = None;
        }
        assert_eq!(codes(&song), vec![]);
        // Conserva una pista para otro domingo sin que sus tiempos (aunque estén mal) cuenten en directo.
        let mut with_audio = sample();
        with_audio.mode = SongMode::Live;
        with_audio.order[1].timing = timing(500_000, vec![]);
        assert_eq!(codes(&with_audio), vec![]);
    }

    #[test]
    fn con_pista_hace_falta_un_audio_valido() {
        let mut song = sample();
        song.audio = None;
        assert_eq!(codes(&song), vec![IssueCode::TrackWithoutAudio]);
        song.audio = Some(AudioTrack { path: "x".into(), name: "x".into(), duration_ms: 0 });
        assert_eq!(codes(&song), vec![IssueCode::InvalidAudio]);
    }

    #[test]
    fn estructura() {
        let mut song = sample();
        song.title = "  ".into();
        song.sections[2].slides[0].id = "v1a".into();
        song.order.push(entry("e5", "puente", None));
        song.sections.push(section("tag", SectionKind::Tag, vec![slide("t1", &[" "])]));
        let issues = song.validate();
        let found: Vec<_> = issues.iter().map(|i| (i.code, i.severity)).collect();
        assert!(found.contains(&(IssueCode::MissingTitle, Severity::Error)));
        assert!(found.contains(&(IssueCode::DuplicateId, Severity::Error)));
        assert!(found.contains(&(IssueCode::UnknownSection, Severity::Error)));
        assert!(found.contains(&(IssueCode::UnusedSection, Severity::Warning)));
        assert!(found.contains(&(IssueCode::EmptySlide, Severity::Warning)));
        assert!(found.contains(&(IssueCode::UntimedEntry, Severity::Warning)));
        // La sección que falta no se despliega.
        assert!(song.expand().iter().all(|e| e.entry != "e5"));

        let mut empty = sample();
        empty.order.clear();
        assert!(codes(&empty).contains(&IssueCode::EmptyOrder));
        assert!(MediaSong::has_errors(&empty.validate()));
    }

    #[test]
    fn apariciones_hacia_adelante_y_dentro_de_la_pista() {
        let mut song = sample();
        song.order[3].timing = timing(15_000, vec![cue("v2a", 15_000, vec![])]);
        assert!(codes(&song).contains(&IssueCode::EntriesOutOfOrder));

        let mut song = sample();
        song.order[4].timing = timing(100_000, vec![]);
        assert_eq!(codes(&song), vec![IssueCode::TimeOutOfTrack]);
    }

    #[test]
    fn diapositivas_dentro_de_su_aparicion() {
        // [inicio, fin): empezar justo donde empieza la siguiente aparición ya es de la otra.
        let mut song = sample();
        song.order[2].timing.as_mut().unwrap().slides[1].start_ms = 40_000;
        assert_eq!(codes(&song), vec![IssueCode::SlideOutsideEntry]);

        let mut song = sample();
        song.order[2].timing.as_mut().unwrap().slides.swap(0, 1);
        assert!(codes(&song).contains(&IssueCode::SlidesOutOfOrder));

        let mut song = sample();
        song.order[4].timing.as_mut().unwrap().slides[1].slide = "v1a".into();
        assert_eq!(codes(&song), vec![IssueCode::UnknownSlide, IssueCode::UntimedSlide]);

        let mut song = sample();
        song.order[4].timing.as_mut().unwrap().slides[1].slide = "c1".into();
        assert_eq!(codes(&song), vec![IssueCode::DuplicateCue, IssueCode::UntimedSlide]);

        let mut song = sample();
        song.order[4].timing.as_mut().unwrap().slides.pop();
        assert_eq!(codes(&song), vec![IssueCode::UntimedSlide]);
    }

    #[test]
    fn lineas_dentro_de_su_diapositiva() {
        fn c1(song: &mut MediaSong) -> &mut SlideCue {
            &mut song.order[2].timing.as_mut().unwrap().slides[0]
        }
        let mut song = sample();
        // La diapositiva c1 va de 20 000 a 30 000: una línea que termina justo en el final cabe.
        c1(&mut song).lines[1].end_ms = Some(30_000);
        assert_eq!(codes(&song), vec![]);
        c1(&mut song).lines[1].end_ms = Some(30_001);
        assert_eq!(codes(&song), vec![IssueCode::LineOutsideSlide]);
        c1(&mut song).lines[1].end_ms = Some(24_000);
        assert_eq!(codes(&song), vec![IssueCode::LineEndBeforeStart]);
        c1(&mut song).lines[1] = line(30_000, None, &[]);
        assert_eq!(codes(&song), vec![IssueCode::LineOutsideSlide]);
        c1(&mut song).lines = vec![line(20_000, Some(25_000), &[]), line(24_000, None, &[])];
        assert_eq!(codes(&song), vec![IssueCode::LinesOverlap]);
        c1(&mut song).lines = vec![line(22_000, None, &[]), line(21_000, None, &[])];
        assert_eq!(codes(&song), vec![IssueCode::LinesOutOfOrder]);
        // El texto quedó con menos líneas que tiempos: los de más se ignoran con un aviso.
        c1(&mut song).lines = vec![line(20_000, None, &[]), line(21_000, None, &[]), line(22_000, None, &[])];
        assert_eq!(song.validate()[0].severity, Severity::Warning);
        assert_eq!(codes(&song), vec![IssueCode::ExtraLineCues]);
    }

    #[test]
    fn palabras_dentro_de_su_linea() {
        fn words(song: &mut MediaSong) -> &mut Vec<u32> {
            &mut song.order[1].timing.as_mut().unwrap().slides[0].lines[0].words
        }
        let mut song = sample();
        // La línea va de 8 000 al final de la aparición (20 000).
        *words(&mut song) = vec![8_000, 8_000];
        assert_eq!(codes(&song), vec![IssueCode::WordsOutOfOrder]);
        *words(&mut song) = vec![7_999];
        assert_eq!(codes(&song), vec![IssueCode::WordOutsideLine]);
        *words(&mut song) = vec![8_000, 20_000];
        assert_eq!(codes(&song), vec![IssueCode::WordOutsideLine]);
        // Cuatro palabras en «Sublime gracia del Señor»: una quinta sobra.
        *words(&mut song) = vec![8_000, 9_000, 10_000, 11_000, 12_000];
        let issues = song.validate();
        assert_eq!(issues.len(), 1);
        assert_eq!((issues[0].code, issues[0].at.word), (IssueCode::ExtraWordCues, Some(4)));
        // Por línea, sin palabras, también vale.
        words(&mut song).clear();
        assert_eq!(codes(&song), vec![]);
    }

    #[test]
    fn json_de_la_interfaz() {
        let json = r#"{
            "id": "s", "title": "Cuán grande es Él", "mode": "live",
            "sections": [{"id": "c", "kind": "preChorus", "slides": [{"id": "a", "lines": ["Mi corazón"]}]}],
            "order": [{"id": "e", "section": "c", "timing": {"startMs": 0, "slides": [{"slide": "a", "startMs": 0, "lines": [{"startMs": 0, "endMs": 900, "words": [0, 400]}]}]}}]
        }"#;
        let song: MediaSong = serde_json::from_str(json).unwrap();
        assert_eq!(song.sections[0].kind, SectionKind::PreChorus);
        assert_eq!(song.order[0].timing.as_ref().unwrap().slides[0].lines[0].end_ms, Some(900));
        assert_eq!(song.rights, Rights::default());
        let back = serde_json::to_value(&song).unwrap();
        assert_eq!(back["order"][0]["timing"]["slides"][0]["lines"][0]["words"][1], 400);
        let issue = serde_json::to_value(Issue { code: IssueCode::UntimedSlide, severity: Severity::Warning, at: at_entry("e") }).unwrap();
        assert_eq!(issue, serde_json::json!({"code": "untimedSlide", "severity": "warning", "at": {"entry": "e"}}));
    }
}

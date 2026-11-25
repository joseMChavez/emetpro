// «Preparar con la pista» (D-029): de una vez, lo que hace falta para que la canción quede lista. Se lee la pista
// una sola vez; si hay separador, se separa la voz (y se guarda la versión sin voz si se pidió) y Whisper oye la
// voz sola, que entiende mucho mejor que con la banda. Con lo que oye, la interfaz arma la letra si no la hay y
// coloca cada diapositiva donde se canta (whisper.ts, sync.ts).
use std::path::Path;

use super::separate::{audio, runtime as separator};
use super::whisper::{self, Segment};

pub enum PrepareError {
    Separator(separator::SeparatorError),
    Whisper(whisper::WhisperError),
    Failed(String),
}

pub enum Step {
    Reading,
    Separating(usize, usize),
    Saving,
    Listening(i32),
}

pub struct Prepared {
    /// Lo que se oyó, con el tiempo de cada palabra (vacío si no se pidió oír).
    pub segments: Vec<Segment>,
    /// Duración de la pista sin voz guardada, si se pidió.
    pub instrumental_ms: Option<u32>,
    /// Whisper oyó la voz separada (no la mezcla).
    pub clean_voice: bool,
    /// Dónde arranca a cantar la voz separada (ms); vacío sin separar.
    pub onsets: Vec<u32>,
}

pub struct Request<'a> {
    pub track: &'a Path,
    /// Guardar aquí la versión sin voz (FLAC).
    pub instrumental: Option<&'a Path>,
    /// Oír la letra con este modelo de Whisper.
    pub whisper_model: Option<&'a Path>,
    pub language: Option<&'a str>,
    pub prompt: &'a str,
}

/// Bloquea: se llama desde un hilo aparte.
pub fn prepare(data: &Path, request: Request, step: impl Fn(Step) + Send + Sync + 'static) -> Result<Prepared, PrepareError> {
    step(Step::Reading);
    let mix = audio::decode(request.track).map_err(PrepareError::Failed)?;
    // Se separa si se pidió la versión sin voz o, para oír mejor, si el separador ya está descargado.
    let separate = request.instrumental.is_some() || (request.whisper_model.is_some() && separator::installed(data).is_some());
    let mut voice = None;
    if separate {
        let stems = separator::separate_audio(data, &mix, &mut |k, n| step(Step::Separating(k, n))).map_err(PrepareError::Separator)?;
        if let Some(target) = request.instrumental {
            step(Step::Saving);
            separator::save_instrumental(target, stems.instrumental, &mix).map_err(PrepareError::Separator)?;
        }
        voice = Some(stems.vocals);
    }
    let instrumental_ms = request.instrumental.map(|_| separator::duration_ms(&mix));
    let Some(model) = request.whisper_model else {
        return Ok(Prepared { segments: Vec::new(), instrumental_ms, clean_voice: false, onsets: Vec::new() });
    };
    step(Step::Listening(0));
    let clean_voice = voice.is_some();
    let onsets = voice.as_ref().map(audio::voice_onsets).unwrap_or_default();
    let samples = audio::mono_at(voice.as_ref().unwrap_or(&mix), whisper::SAMPLE_RATE).map_err(PrepareError::Failed)?;
    drop(voice);
    drop(mix);
    let progress = move |p: i32| step(Step::Listening(p));
    let segments = whisper::transcribe(model, &samples, request.language, request.prompt, progress).map_err(PrepareError::Whisper)?;
    Ok(Prepared { segments, instrumental_ms, clean_voice, onsets })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// De verdad, con el modelo de Whisper y (si `EMETPRO_DATA` tiene el separador) la voz separada; guarda lo oído
    /// en JSON para las pruebas del alineamiento:
    /// EMETPRO_DATA=… EMETPRO_WHISPER_MODEL=… EMETPRO_TRACK=… EMETPRO_OUT=….json
    /// cargo test --release prepara_de_verdad -- --ignored --nocapture
    #[test]
    #[ignore]
    fn prepara_de_verdad() {
        let var = |n: &str| std::path::PathBuf::from(std::env::var(n).unwrap());
        let started = std::time::Instant::now();
        let request = Request {
            track: &var("EMETPRO_TRACK"),
            instrumental: None,
            whisper_model: Some(&var("EMETPRO_WHISPER_MODEL")),
            language: Some("es"),
            prompt: "Noche de paz",
        };
        let prepared = prepare(&var("EMETPRO_DATA"), request, |_| {}).unwrap_or_else(|_| panic!("preparar"));
        let words: usize = prepared.segments.iter().map(|s| s.words.len()).sum();
        println!("voz separada: {} · {} tramos, {words} palabras en {:?}", prepared.clean_voice, prepared.segments.len(), started.elapsed());
        std::fs::write(var("EMETPRO_OUT"), serde_json::to_string_pretty(&serde_json::json!({ "segments": prepared.segments, "onsets": prepared.onsets })).unwrap()).unwrap();
    }
}

// Leer una pista (mp3, m4a/aac, wav, ogg, flac) a estéreo de 44,1 kHz para htdemucs, y guardar el resultado en FLAC
// (sin pérdida, la mitad que un WAV; los webviews de los tres sistemas lo reproducen). Todo en Rust puro.
use std::fs::File;
use std::path::Path;

use rubato::audioadapter_buffers::direct::SequentialSliceOfVecs;
use rubato::{Fft, FixedSync, Resampler};
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

/// Frecuencia con que se entrenó htdemucs.
pub const RATE: u32 = 44_100;

pub type Stereo = [Vec<f32>; 2];

/// La pista en estéreo a 44,1 kHz (un canal se duplica; de más de dos se toman los dos primeros).
pub fn decode(path: &Path) -> Result<Stereo, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe()
        .probe(&hint, stream, FormatOptions::default(), MetadataOptions::default())
        .map_err(|e| format!("formato no reconocido: {e}"))?;
    let track = format.default_track(TrackType::Audio).ok_or("la pista no tiene audio")?;
    let id = track.id;
    let params = track.codec_params.as_ref().and_then(|p| p.audio()).ok_or("la pista no tiene audio")?.clone();
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .map_err(|e| format!("este formato de audio no se puede leer aquí: {e}"))?;
    let mut rate = params.sample_rate.unwrap_or(RATE);
    let mut out: Stereo = [Vec::new(), Vec::new()];
    let mut planes: Vec<Vec<f32>> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.to_string()),
        };
        if packet.track_id != id {
            continue;
        }
        let buffer = match decoder.decode(&packet) {
            Ok(buffer) => buffer,
            // Un paquete dañado se salta, como hacen los reproductores.
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(e.to_string()),
        };
        rate = buffer.spec().rate();
        buffer.copy_to_vecs_planar::<f32>(&mut planes);
        let left = &planes[0];
        let right = planes.get(1).unwrap_or(left);
        out[0].extend_from_slice(left);
        out[1].extend_from_slice(right);
    }
    if out[0].is_empty() {
        return Err("la pista está vacía".into());
    }
    if rate != RATE { resample(out, rate) } else { Ok(out) }
}

fn resample(input: Stereo, from: u32) -> Result<Stereo, String> {
    let frames = input[0].len();
    let mut resampler = Fft::<f32>::new(from as usize, RATE as usize, 1024, 2, FixedSync::Input).map_err(|e| e.to_string())?;
    let capacity = resampler.process_all_needed_output_len(frames);
    let mut output = vec![vec![0f32; capacity]; 2];
    let source = SequentialSliceOfVecs::new(&input[..], 2, frames).map_err(|e| e.to_string())?;
    let mut target = SequentialSliceOfVecs::new_mut(&mut output[..], 2, capacity).map_err(|e| e.to_string())?;
    let (_, written) = resampler.process_all_into_buffer(&source, &mut target, frames, None).map_err(|e| e.to_string())?;
    let [mut left, mut right]: [Vec<f32>; 2] = output.try_into().map_err(|_| "remuestreo")?;
    left.truncate(written);
    right.truncate(written);
    Ok([left, right])
}

/// Guarda en FLAC estéreo, 16 bits, 44,1 kHz.
pub fn write_flac(path: &Path, audio: &Stereo) -> Result<(), String> {
    use flacenc::component::BitRepr;
    use flacenc::error::Verify;
    let interleaved: Vec<i32> = audio[0]
        .iter()
        .zip(&audio[1])
        .flat_map(|(l, r)| [l, r])
        .map(|v| (v.clamp(-1.0, 1.0) * 32767.0).round() as i32)
        .collect();
    let config = flacenc::config::Encoder::default().into_verified().map_err(|(_, e)| format!("{e:?}"))?;
    // flacenc 0.5 numera mal un último bloque incompleto (`flac -t` avisa de que no se puede buscar y symphonia no
    // lo lee): se completa con silencio hasta un bloque entero (menos de una décima de segundo al final).
    let block = config.block_size * 2;
    let mut interleaved = interleaved;
    interleaved.resize(interleaved.len().div_ceil(block) * block, 0);
    let source = flacenc::source::MemSource::from_samples(&interleaved, 2, 16, RATE as usize);
    let stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size).map_err(|e| format!("{e:?}"))?;
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream.write(&mut sink).map_err(|e| format!("{e:?}"))?;
    std::fs::write(path, sink.as_slice()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: u32, seconds: f32) -> Stereo {
        let n = (rate as f32 * seconds) as usize;
        let left = (0..n).map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / rate as f32).sin() * 0.5).collect();
        let right = (0..n).map(|i| (2.0 * std::f32::consts::PI * 660.0 * i as f32 / rate as f32).sin() * 0.25).collect();
        [left, right]
    }

    #[test]
    fn flac_de_ida_y_vuelta() {
        let dir = std::env::temp_dir().join(format!("emetpro-flac-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tono.flac");
        let original = tone(RATE, 2.0);
        write_flac(&path, &original).unwrap();
        let back = decode(&path).unwrap();
        // Lo de más al final es silencio de relleno, menos de un bloque.
        assert!(back[0].len() >= original[0].len() && back[0].len() - original[0].len() < 4096);
        assert!(back[0][original[0].len()..].iter().all(|v| *v == 0.0));
        let worst = original[1].iter().zip(&back[1]).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
        assert!(worst < 1e-3, "diferencia {worst}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn remuestrea_a_44100() {
        let out = resample(tone(48_000, 1.0), 48_000).unwrap();
        let expected = RATE as usize;
        assert!((out[0].len() as i64 - expected as i64).abs() < 50, "{} muestras", out[0].len());
        // Sigue siendo un La de 440 Hz: cruces por cero ≈ 880 por segundo.
        let crossings = out[0].windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        assert!((430..=450).contains(&crossings), "{crossings}");
    }
}


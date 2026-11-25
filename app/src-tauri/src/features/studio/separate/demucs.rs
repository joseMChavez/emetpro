// La canción entera por trozos de 7,8 s que se solapan un 25 %, como `apply_model` de Demucs (split=True,
// overlap=0.25, shifts=0): cada trozo pasa por la red y se suma con un peso en triángulo, máximo en su centro.
use std::sync::atomic::{AtomicBool, Ordering};

use super::dsp::{Stft, FRAMES, FREQS, SEGMENT};

/// Fuentes de htdemucs, en su orden: batería, bajo, otros, voz.
pub const SOURCES: usize = 4;
const VOCALS: usize = 3;
const CHANNELS: usize = 2;

/// Lo que da la red para un trozo: espectrograma de cada fuente [SOURCES][CHANNELS*2][FREQS][FRAMES] y onda de cada
/// fuente [SOURCES][CHANNELS][SEGMENT], aplanados.
pub struct ModelOutput {
    pub spec: Vec<f32>,
    pub wave: Vec<f32>,
}

pub struct Stems {
    /// La voz sola (para que Whisper la oiga sin la banda), [CHANNELS][muestras].
    pub vocals: [Vec<f32>; CHANNELS],
    /// Todo menos la voz (batería, bajo y otros sumados).
    pub instrumental: [Vec<f32>; CHANNELS],
}

pub enum SeparateError {
    Canceled,
    Model(String),
}

/// Un trozo como lo prepara `TensorChunk.padded`: si es el último y más corto, se completa con la música de
/// alrededor (y ceros fuera de la canción), centrado.
fn chunk(mix: &[Vec<f32>; CHANNELS], offset: usize) -> (Vec<f32>, usize, usize) {
    let length = mix[0].len();
    let chunk_length = (length - offset).min(SEGMENT);
    let delta = SEGMENT - chunk_length;
    let start = offset as isize - (delta / 2) as isize;
    let mut input = vec![0f32; CHANNELS * SEGMENT];
    for (c, channel) in mix.iter().enumerate() {
        for i in 0..SEGMENT {
            let j = start + i as isize;
            if j >= 0 && (j as usize) < length {
                input[c * SEGMENT + i] = channel[j as usize];
            }
        }
    }
    (input, chunk_length, delta / 2)
}

/// Espectrograma del trozo para la red: [CHANNELS*2][FREQS][FRAMES] (real e imaginaria de cada canal).
pub fn magnitude(stft: &Stft, input: &[f32]) -> Vec<f32> {
    let mut mag = Vec::with_capacity(CHANNELS * 2 * FREQS * FRAMES);
    for c in 0..CHANNELS {
        let (re, im) = stft.spec(&input[c * SEGMENT..(c + 1) * SEGMENT]);
        mag.extend(re);
        mag.extend(im);
    }
    mag
}

/// Onda de una fuente y un canal a partir de lo que da la red (sin recortar: SEGMENT muestras).
pub fn source_wave(stft: &Stft, out: &ModelOutput, source: usize, channel: usize) -> Vec<f32> {
    let plane = FREQS * FRAMES;
    let base = (source * CHANNELS * 2 + channel * 2) * plane;
    let mut wave = stft.ispec(&out.spec[base..base + plane], &out.spec[base + plane..base + 2 * plane]);
    let t = (source * CHANNELS + channel) * SEGMENT;
    for (w, x) in wave.iter_mut().zip(&out.wave[t..t + SEGMENT]) {
        *w += x;
    }
    wave
}

/// Separa la voz del resto. `model` recibe la onda del trozo [CHANNELS][SEGMENT] y su espectrograma.
pub fn separate(
    mix: &[Vec<f32>; CHANNELS],
    mut model: impl FnMut(&[f32], &[f32]) -> Result<ModelOutput, String>,
    mut progress: impl FnMut(usize, usize),
    cancel: &AtomicBool,
) -> Result<Stems, SeparateError> {
    let length = mix[0].len();
    let stride = SEGMENT * 3 / 4;
    let half = SEGMENT / 2;
    // torch: cat(arange(1, half + 1), arange(SEGMENT - half, 0, -1)) / máximo.
    let weight: Vec<f32> = (0..SEGMENT).map(|i| if i < half { (i + 1) as f32 } else { (SEGMENT - i) as f32 } / half as f32).collect();
    let stft = Stft::default();
    let mut vocals = [vec![0f32; length], vec![0f32; length]];
    let mut rest = [vec![0f32; length], vec![0f32; length]];
    let mut total = vec![0f32; length];
    let offsets: Vec<usize> = (0..length).step_by(stride).collect();
    for (k, &offset) in offsets.iter().enumerate() {
        if cancel.load(Ordering::SeqCst) {
            return Err(SeparateError::Canceled);
        }
        progress(k, offsets.len());
        let (input, chunk_length, trim) = chunk(mix, offset);
        let mag = magnitude(&stft, &input);
        let out = model(&input, &mag).map_err(SeparateError::Model)?;
        for source in 0..SOURCES {
            let targets = if source == VOCALS { &mut vocals } else { &mut rest };
            for (channel, target) in targets.iter_mut().enumerate() {
                let wave = source_wave(&stft, &out, source, channel);
                for i in 0..chunk_length {
                    target[offset + i] += weight[i] * wave[trim + i];
                }
            }
        }
        for i in 0..chunk_length {
            total[offset + i] += weight[i];
        }
    }
    progress(offsets.len(), offsets.len());
    for channel in vocals.iter_mut().chain(rest.iter_mut()) {
        for (x, w) in channel.iter_mut().zip(&total) {
            *x /= w;
        }
    }
    Ok(Stems { vocals, instrumental: rest })
}

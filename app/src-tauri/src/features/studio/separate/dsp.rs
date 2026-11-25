// Espectrograma de Demucs (htdemucs), igual que `standalone_spec`, `standalone_magnitude`, `standalone_mask` y
// `standalone_ispec` de demucs.onnx (MIT): la red exportada a ONNX no lleva la STFT dentro, así que se hace aquí.
// Las pruebas comparan con lo que da Python sobre las mismas entradas (separate/tests).
use std::sync::Arc;

use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};

pub const NFFT: usize = 4096;
pub const HOP: usize = NFFT / 4;
/// Frecuencias que usa la red (sin la de Nyquist).
pub const FREQS: usize = NFFT / 2;
/// Trozo con que se entrenó htdemucs: 7,8 s a 44,1 kHz.
pub const SEGMENT: usize = 343_980;
/// Columnas del espectrograma de un trozo: ceil(SEGMENT / HOP).
pub const FRAMES: usize = SEGMENT.div_ceil(HOP);
/// Relleno previo de Demucs para que las columnas cuadren con la rama de onda: 3/8 de NFFT.
const PAD: usize = HOP / 2 * 3;

/// Reflejo como `F.pad(..., mode="reflect")`: sin repetir el borde. `left` y `right` menores que la señal.
pub fn reflect_pad(x: &[f32], left: usize, right: usize) -> Vec<f32> {
    let n = x.len();
    let mut out = Vec::with_capacity(n + left + right);
    out.extend((1..=left).rev().map(|i| x[i]));
    out.extend_from_slice(x);
    out.extend((0..right).map(|j| x[n - 2 - j]));
    out
}

pub struct Stft {
    forward: Arc<dyn Fft<f32>>,
    inverse: Arc<dyn Fft<f32>>,
    /// Hann periódica, como `torch.hann_window`.
    window: Vec<f32>,
}

impl Default for Stft {
    fn default() -> Self {
        let mut planner = FftPlanner::new();
        let window = (0..NFFT).map(|i| 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / NFFT as f64).cos() as f32).collect();
        Self { forward: planner.plan_fft_forward(NFFT), inverse: planner.plan_fft_inverse(NFFT), window }
    }
}

impl Stft {
    /// Espectrograma de un canal de un trozo (SEGMENT muestras): parte real e imaginaria, cada una [FREQS][FRAMES].
    pub fn spec(&self, x: &[f32]) -> (Vec<f32>, Vec<f32>) {
        debug_assert_eq!(x.len(), SEGMENT);
        // standalone_spec: relleno propio de Demucs y después el centrado de torch.stft (los dos por reflejo).
        let padded = reflect_pad(x, PAD, PAD + FRAMES * HOP - x.len());
        let centered = reflect_pad(&padded, NFFT / 2, NFFT / 2);
        let scale = 1.0 / (NFFT as f32).sqrt();
        let mut re = vec![0f32; FREQS * FRAMES];
        let mut im = vec![0f32; FREQS * FRAMES];
        let mut buffer = vec![Complex32::default(); NFFT];
        for t in 0..FRAMES {
            // Se descartan las dos primeras y las dos últimas columnas de torch.stft.
            let start = (t + 2) * HOP;
            for (i, b) in buffer.iter_mut().enumerate() {
                *b = Complex32::new(centered[start + i] * self.window[i], 0.0);
            }
            self.forward.process(&mut buffer);
            for f in 0..FREQS {
                re[f * FRAMES + t] = buffer[f].re * scale;
                im[f * FRAMES + t] = buffer[f].im * scale;
            }
        }
        (re, im)
    }

    /// Inversa para una fuente y un canal ([FREQS][FRAMES] real e imaginaria): SEGMENT muestras.
    pub fn ispec(&self, re: &[f32], im: &[f32]) -> Vec<f32> {
        // standalone_ispec: se añade la frecuencia de Nyquist (cero) y dos columnas vacías a cada lado.
        let frames = FRAMES + 4;
        let length = HOP * SEGMENT.div_ceil(HOP) + 2 * PAD;
        let total = NFFT + HOP * (frames - 1);
        let mut out = vec![0f32; total];
        let mut envelope = vec![0f32; total];
        let scale = 1.0 / (NFFT as f32).sqrt();
        let mut buffer = vec![Complex32::default(); NFFT];
        for t in 0..frames {
            let column = t.checked_sub(2).filter(|c| *c < FRAMES);
            buffer.fill(Complex32::default());
            if let Some(c) = column {
                for f in 0..FREQS {
                    buffer[f] = Complex32::new(re[f * FRAMES + c], im[f * FRAMES + c]);
                }
                // Espectro hermítico: la mitad alta es el conjugado de la baja (Nyquist = 0).
                for f in 1..FREQS {
                    buffer[NFFT - f] = buffer[f].conj();
                }
                self.inverse.process(&mut buffer);
            }
            let start = t * HOP;
            for i in 0..NFFT {
                let w = self.window[i];
                out[start + i] += buffer[i].re * scale * w;
                envelope[start + i] += w * w;
            }
        }
        // torch.istft centrado: se quita NFFT/2 al principio y se divide por la envolvente de las ventanas.
        let begin = NFFT / 2;
        (0..SEGMENT)
            .map(|i| {
                let j = begin + PAD + i;
                debug_assert!(j < begin + length);
                if envelope[j] > 1e-11 { out[j] / envelope[j] } else { 0.0 }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reflejo_como_torch() {
        assert_eq!(reflect_pad(&[1.0, 2.0, 3.0, 4.0], 2, 3), vec![3.0, 2.0, 1.0, 2.0, 3.0, 4.0, 3.0, 2.0, 1.0]);
    }

    #[test]
    fn ida_y_vuelta_devuelve_la_senal() {
        let stft = Stft::default();
        let x: Vec<f32> = (0..SEGMENT).map(|i| ((i as f32) * 0.013).sin() * 0.5 + ((i as f32) * 0.0007).cos() * 0.2).collect();
        let (re, im) = stft.spec(&x);
        let y = stft.ispec(&re, &im);
        // Demucs descarta la frecuencia de Nyquist y columnas de los bordes: se mide lejos de ellos.
        let middle = NFFT..SEGMENT - NFFT;
        let worst = x[middle.clone()].iter().zip(&y[middle]).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
        assert!(worst < 1e-4, "diferencia máxima {worst}");
    }
}

// Comparación con lo que da Demucs en Python sobre las mismas entradas. Los archivos de referencia (f32 en
// little-endian) se generan con la prueba de concepto; las pruebas se saltan si no están:
// EMETPRO_DEMUCS_GOLDEN=<carpeta> cargo test --release separa_igual -- --ignored --nocapture
use super::demucs::{magnitude, source_wave, ModelOutput};
use super::dsp::{Stft, SEGMENT};

fn read(name: &str) -> Vec<f32> {
    let dir = std::env::var("EMETPRO_DEMUCS_GOLDEN").expect("EMETPRO_DEMUCS_GOLDEN");
    let bytes = std::fs::read(format!("{dir}/{name}.f32")).unwrap();
    bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect()
}

/// Relación señal/diferencia en dB (como el SDR): más alto = más igual.
pub fn sdr(reference: &[f32], estimate: &[f32]) -> f64 {
    let signal: f64 = reference.iter().map(|x| (*x as f64).powi(2)).sum();
    let noise: f64 = reference.iter().zip(estimate).map(|(a, b)| ((a - b) as f64).powi(2)).sum();
    10.0 * (signal / (noise + 1e-12)).log10()
}

#[test]
#[ignore]
fn separa_igual_espectrograma() {
    let stft = Stft::default();
    let mag = magnitude(&stft, &read("chunk"));
    let golden = read("mag");
    let db = sdr(&golden, &mag);
    println!("espectrograma: {db:.1} dB");
    assert!(db > 80.0);
}

#[test]
#[ignore]
fn separa_igual_despues_de_la_red() {
    let stft = Stft::default();
    let out = ModelOutput { spec: read("out_x"), wave: read("out_xt") };
    let golden = read("chunk_out");
    for source in 0..4 {
        for channel in 0..2 {
            let wave = source_wave(&stft, &out, source, channel);
            let at = (source * 2 + channel) * SEGMENT;
            let db = sdr(&golden[at..at + SEGMENT], &wave);
            println!("fuente {source} canal {channel}: {db:.1} dB");
            assert!(db > 70.0);
        }
    }
}

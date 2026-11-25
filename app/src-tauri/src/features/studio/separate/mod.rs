// Quitar la voz de una pista (Media Studio, D-029) con htdemucs (Meta, MIT) en ONNX Runtime. El modelo y el motor no
// van en el instalador: se descargan solo si el usuario lo pide.
pub mod audio;
pub mod demucs;
pub mod dsp;
pub mod runtime;

#[cfg(test)]
mod tests;

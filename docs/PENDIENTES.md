# Pendientes

Ideas acordadas que todavía no se hacen. Cuando una se decide, pasa a [DECISIONS.md](DECISIONS.md) y sale de aquí.

## En discusión

### Media Studio (D-029), ideas tomadas de StemKit

[StemKit](https://github.com/danielravina/stemkit) (MIT) separa una canción en pistas por instrumento y la reproduce como una mesa de mezclas. Ya se tomó el cambio de tono (0.11.1). Falta, en este orden sugerido:

1. **Detectar la tonalidad** de la pista («Sol mayor») y mostrarla junto al tono («Sol → Fa» al bajar un tono). Para los músicos. Pequeño; candidato a 0.11.2.
2. **Separar la canción en pistas** (voz, batería, bajo, piano, otros). Candidato a 0.12, experimental.
   - Quitar la voz de una grabación original para tener la pista instrumental (iglesias que tienen la canción pero no la pista).
   - En vivo: subir o bajar una voz guía; de fondo, dejar solo el piano en vez de bajar todo.
   - Sin Python: modelos ONNX desde Rust, descargados en Ajustes solo si se quieren (como LibreOffice, D-015).
   - Costo: modelos de 300 MB a 1 GB; varios minutos por canción sin tarjeta gráfica (se hace al preparar, no en el culto); el `.emetplan` pesa más con varias pistas.
   - Primer paso: prueba de concepto para ver qué modelo da buena calidad en una computadora normal, cuánto tarda y **qué licencia tienen sus pesos** (algunos no permiten repartirlos en una app).
3. **Sincronización automática por la voz**: con la voz separada, un modelo tipo Whisper propone dónde empieza cada línea, para revisar. Depende de la 2.
4. **Onda de cada pista** en el editor. Depende de la 2.

No se toma de StemKit: la descarga desde YouTube (sus condiciones lo prohíben; YouTube queda solo como enlace de referencia) ni el envío de datos de uso.

### Otros de Media Studio

- **Fondos Lottie**, si alguien los pide (hoy: imagen, video o SVG animado).
- **Acordes** de las canciones, dentro del mismo formato con manifiesto que ya llevan letras, planes y Media Studio (D-028, D-029).

## Descartado

- **Importar LRC** en Media Studio: el autor no lo necesita y sería tedioso para el fin que se busca.

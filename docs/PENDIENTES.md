# Pendientes

Ideas acordadas que todavía no se hacen. Cuando una se decide, pasa a [DECISIONS.md](DECISIONS.md) y sale de aquí.

## En discusión

### Media Studio (D-029), ideas tomadas de StemKit

[StemKit](https://github.com/danielravina/stemkit) (MIT) separa una canción en pistas por instrumento y la reproduce como una mesa de mezclas. Ya se tomaron el cambio de tono (0.11.1) y la detección de la tonalidad (ver D-029). Falta, en este orden sugerido:

1. **Separar la canción en pistas** (voz, batería, bajo, piano, otros). Candidato a 0.12, experimental.
   - Quitar la voz de una grabación original para tener la pista instrumental (iglesias que tienen la canción pero no la pista).
   - En vivo: subir o bajar una voz guía; de fondo, dejar solo el piano en vez de bajar todo.
   - Sin Python: modelos ONNX desde Rust, descargados en Ajustes solo si se quieren (como LibreOffice, D-015).
   - Costo: modelos de 300 MB a 1 GB; varios minutos por canción sin tarjeta gráfica (se hace al preparar, no en el culto); el `.emetplan` pesa más con varias pistas.
   - Primer paso: prueba de concepto para ver qué modelo da buena calidad en una computadora normal, cuánto tarda y **qué licencia tienen sus pesos** (algunos no permiten repartirlos en una app).
2. **Sincronización automática por la voz**: con la voz separada, un modelo tipo Whisper propone dónde empieza cada línea, para revisar. Depende de la 1.
   - **Sacar la letra de la misma canción** cuando no se tiene a mano: el modelo la transcribe y el usuario la limpia en el editor (palabras mal oídas, repeticiones, cortes de línea y de diapositiva) antes de usarla. La transcripción es un borrador: no se guarda ni se proyecta sin revisarla.
   - Con la letra ya escrita, el modelo solo propone los tiempos; con la letra sacada de la canción, propone texto y tiempos a la vez.
   - Respetar los derechos de la letra como hoy (D-029): transcribir no da permiso para proyectarla.
3. **Onda de cada pista** en el editor. Depende de la 1.

No se toma de StemKit: la descarga desde YouTube (sus condiciones lo prohíben; YouTube queda solo como enlace de referencia) ni el envío de datos de uso.

### Otros de Media Studio

- **Fondos Lottie**, si alguien los pide (hoy: imagen, video o SVG animado).
- **Acordes** de las canciones, dentro del mismo formato con manifiesto que ya llevan letras, planes y Media Studio (D-028, D-029).

## Descartado

- **Importar LRC** en Media Studio: el autor no lo necesita y sería tedioso para el fin que se busca.

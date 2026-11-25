# Pendientes

Ideas acordadas que todavía no se hacen. Cuando una se decide, pasa a [DECISIONS.md](DECISIONS.md) y sale de aquí.

## En discusión

### Media Studio (D-029), ideas tomadas de StemKit

[StemKit](https://github.com/danielravina/stemkit) (MIT) separa una canción en pistas por instrumento y la reproduce como una mesa de mezclas. Ya se tomaron el cambio de tono (0.11.1), la detección de la tonalidad, sacar la letra con Whisper y quitar la voz con htdemucs (ver D-029). Falta, en este orden sugerido:

1. **Sincronización automática por la voz**: con los tiempos de cada línea que ya da Whisper (y la voz separada, más limpia), proponer dónde empieza cada diapositiva, para revisar.
   - Sacar la letra de la misma canción ya está hecho (D-029); falta pasarle la voz separada en vez de la mezcla, para que oiga mejor con banda completa.
   - Con la letra ya escrita, el modelo solo propone los tiempos; con la letra sacada de la canción, propone texto y tiempos a la vez.
   - Respetar los derechos de la letra como hoy (D-029): transcribir no da permiso para proyectarla.
2. **Voz guía**: guardar también la voz separada y subirla o bajarla en vivo (hoy solo se elige con voz o sin voz).
3. **Separar también batería, bajo y piano** (htdemucs ya los da): de fondo, dejar solo el piano.
4. **Limpiar pistas que ninguna canción usa** (la original al cambiarla, las versiones sin voz borradas), como ya se hace con los medios de los planes.

No se toma de StemKit: la descarga desde YouTube (sus condiciones lo prohíben; YouTube queda solo como enlace de referencia) ni el envío de datos de uso.

### Otros de Media Studio

- **Fondos Lottie**, si alguien los pide (hoy: imagen, video o SVG animado).
- **Acordes** de las canciones, dentro del mismo formato con manifiesto que ya llevan letras, planes y Media Studio (D-028, D-029).

## Descartado

- **Importar LRC** en Media Studio: el autor no lo necesita y sería tedioso para el fin que se busca.

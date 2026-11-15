// La herramienta de resaltar de la consola (D-023): si está activa y con qué estilo y color se marca.
import type { MarkColor, MarkStyle } from "./marks";

export const markTool = $state({
  on: false,
  style: "marker" as MarkStyle,
  color: "yellow" as MarkColor,
});

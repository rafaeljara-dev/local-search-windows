# Estado congelado · 11 de octubre de 2026

El desarrollo se detuvo después de conectar videos a la UI y de investigar modelos de decisión. La publicación conserva el motor y las funciones existentes; no incorpora una ronda nueva de optimización ni APIs de pago.

## Funciones y límites

| Área | Implementado | Pendiente |
|---|---|---|
| Ventana | Rust/egui, Segoe UI, Acrylic/fallback, controles nativos, reparto 50/50 | Accesibilidad y matriz amplia de DPI/hardware; reproducción incrustada |
| Resultados | Toda la fila pulsable, teclado/doble clic, subrayado, posición y tipo de coincidencia | Confianza calibrada y abstención para consultas sin respuesta |
| Texto | TXT/MD/CSV/logs/código UTF-8; pasajes con línea | PDF/DOCX/Office, paginación, OCR, detección de codificación |
| Imágenes | Importación visual PNG/JPEG/WebP/BMP y vista previa | OCR separado y evaluación detallada de texto pequeño |
| Video | Audio mono, Whisper base en español, tiempos, embeddings y transcripción visible | Fotogramas, videos sin audio, frases cruzando pasajes y alineación manual validada |
| Reproducción | FFplay en otra ventana, seek, pausa y cierre controlado | Reproductor dentro del panel y pruebas humanas de sincronía audiovisual |
| Colección | Selección, importación, F5, reutilización, eliminación y guardado atómico | Watcher, cola persistente, reanudación completa, varias colecciones |
| Memoria | Worker descargable; ASR y embeddings no se cargan juntos | Presupuesto completo <500 MB en consulta/importación/reproducción y equipos modestos |
| Escala | Catálogo JSON local, comparación vectorial exhaustiva | SQLite/FTS5, vectores binarios/mapeados, decisión justificada de ANN |
| Distribución | Fuentes y comandos de compilación | Instalador, firma, actualización y paquete de dependencias |

Límites del prototipo: 500 archivos textuales por recorrido, 2.500 segmentos por colección, lectura textual acotada a los primeros 64 KiB; hasta 32 imágenes y 10 videos por lote; videos con audio de hasta 30 minutos. Estas restricciones no describen un producto que ya cubra toda la computadora.

## Motor y evidencia

Modelo de búsqueda: EmbeddingGemma 2 Q8, salida normalizada de 768 dimensiones, worker CPU de dos hilos y contexto 512. Los pasajes se indexan con título + texto; las consultas usan el prefijo de tarea de búsqueda. El catálogo identifica el pipeline para impedir mezclar vectores incompatibles.

El ranking combina coincidencias literales y semánticas; las coincidencias literales reciben prioridad y los empates se resuelven por similitud antes de la ruta. Los identificadores requieren tokens completos. No es todavía un índice BM25/FTS5. Una puntuación de coseno visible no es un porcentaje de certeza.

El subrayado reconoce palabras completas y variantes de mayúsculas/tildes, conserva Unicode y distingue `ñ` de `n`. No inventa una palabra para explicar una coincidencia semántica: cuando no hay evidencia literal se resalta el pasaje completo. Para video conserva offsets de fragmentos y tokens; una coincidencia de nombre/ruta sin palabra en la transcripción no recibe una falsa referencia de voz.

## Validación realizada

15 pruebas Rust, build Release y comprobación de formato aprobados en Windows. Cubren filas pulsables, filas adyacentes/doble clic, palabras/Unicode/identificadores, priorización literal, segmentación, selección de colección, exclusión de credenciales, persistencia, cancelación y reutilización/eliminación de imágenes y videos. Las pruebas de video emplean fixtures sintéticos y caché; no son una evaluación de precisión del ASR.

Para esta publicación se repitieron las 15 pruebas sobre la copia pública, se comprobó la demo portable de 32 documentos, se compiló nuevamente llama.cpp/mtmd con `build-runtime-clang.cmd` y el worker con `build-worker.cmd`. Una consulta sintética produjo 768 dimensiones y norma ≈1. Se verificaron sintaxis PowerShell, enlaces internos y exclusiones del índice de Git. La preparación `-TextOnly` se verificó reutilizando el modelo existente con su SHA-256; no se repitió toda la instalación desde una máquina limpia.

La UI se revisó mediante capturas de su renderizador a 1120×760 y 920×660. Estas capturas no validan Acrylic ni la barra nativa del escritorio. Hubo una prueba de FFplay sin salida de audio real y de decodificación FFmpeg desde una referencia temporal, ambas con salida 0. No se cotejaron los saltos escuchando el audio.

Una colección privada de desarrollo contenía 3 textos, 8 imágenes y 5 videos, 204 pasajes. Los ocho casos positivos de texto/imagen previamente elegidos siguieron primeros tras añadir videos; dos consultas de video encontraron sus objetivos primero. El identificador inventado no produjo resultados. Una consulta natural negativa siguió devolviendo candidatos irrelevantes. Las muestras pequeñas y conocidas no permiten estimar calidad general; tampoco se comparó contra Windows Search con corpus equivalente.

Los originales, nombres/rutas personales, capturas, índices, transcripciones, miniaturas y logs de esa colección **no se publican**. La demo pública es un corpus sintético distinto de 32 documentos. Las métricas históricas son agregados, no una evaluación que un clon pueda repetir con esos mismos datos privados.

## Recursos observados

| Estado | Medición |
|---|---|
| Ventana + worker de texto, colección pequeña previa a video | Final 405,81 MB, pico 410,45 MB de working set; privada máxima 371,86 MB; muestra ≈20 s |
| Importación visual, sin UI | ≈1.167 MB residentes; incumple el objetivo de 500 MB |
| Whisper aislado, cinco clips / 338,56 s de contenido | 47,895 s de transcripción acumulada; pico 408,05 MiB residentes y 807,72 MiB privados |
| Preparación integrada de los cinco videos | 56,85 s, incluida extracción/transcripción/embeddings; sin medición conjunta certificada de RAM |
| Reimportar los mismos videos | 0,70 s; cinco reutilizados sin inferencia nueva |
| Búsqueda semántica en videos | Primera 812 ms; posteriores 31–37 ms en esa ejecución |

MB significa bytes/1.000.000 y MiB bytes/1.048.576. Working set, memoria privada comprometida y tamaño de pesos son métricas diferentes. Algunas páginas compartidas pueden contarse más de una vez al sumar procesos. No extrapolar las muestras a una sesión prolongada o a un corpus grande.

Un ensayo aislado de reranker multilingüe INT8 de 118,62 MB en disco alcanzó 482,05 MiB residentes (≈505,47 MB), 872,70 MiB privados y mediana de 670,95 ms por lista de 20 pasajes. Conservó seis aciertos que ya existían y no demostró mejora. Se ejecutó fuera de la app y no se adoptó. [Investigación](SYSTEM-ONE.md).

## Decisiones tomadas

Conservar Q8 y CPU como referencia; no adoptar Q4 por tamaño sin validar calidad y latencia. Mantener procesos descargables y parche no causal de llama.cpp. Priorizar primero funcionalidad y evidencia antes de optimizar. No instalar un servicio, desactivar Windows Search ni indexar todo el equipo automáticamente. El monitor de recursos de la barra de tareas desarrollado por separado no forma parte de este repositorio.

Para retomar, seguir [HANDOFF.md](HANDOFF.md). El código es un punto de partida verificable; las metas futuras se deben medir antes de anunciarlas como cumplidas.

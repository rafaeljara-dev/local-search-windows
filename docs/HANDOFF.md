# Guía para continuar el desarrollo

## Dónde se quedó

Último cambio funcional: UI 50/50 con vista previa más corta y detalles mayores; importación de voz de videos conectada al catálogo, búsqueda y reproducción temporal. Última investigación: Jev/Clef y alternativas locales como segunda evaluación de candidatos. Ninguna API ni reranker adicional está integrado.

La arquitectura futura de SQLite/FTS5/ANN aparece como propuesta; **el código actual aún usa JSON y comparación exhaustiva**. PDF/Office/fotogramas y el consumo global <500 MB tampoco están resueltos. Este documento es la referencia de continuidad; no hay que deducir implementación a partir de un diagrama de planificación.

## Mapa de código

| Archivo | Responsabilidad |
|---|---|
| `app/src/main.rs` | Raíz de recursos, entradas CLI, self-test, configuración e inicio de la ventana |
| `app/src/ui.rs` | Estado de interfaz, selector nativo, filas, preview, menú, teclado y reproductor |
| `app/src/backend.rs` | Catálogo/segmentos, actor de solicitudes, worker, ranking, importación, sincronización y publicación atómica |
| `app/src/highlight.rs` | Normalización Unicode y rangos originales para subrayar evidencia literal |
| `app/src/media.rs` | Preparación y presupuesto de píxeles de imágenes |
| `app/src/video.rs` | FFprobe/FFmpeg/Whisper, caché, pasajes temporales, tokens, limpieza y FFplay |
| `app/src/appearance.rs` | Fuente y material del escritorio de Windows |
| `app/src/probe.rs` | Evaluación por CLI con el mismo motor de la interfaz |
| `app/src/capture.rs` | Capturas opt-in del renderizador mediante feature `ui-capture` |
| `spike/embed-worker.cpp` | Carga dinámica de DLL, protocolo JSON por línea, codificación y memoria del worker |

`Request`/`Event` comunican la UI con el motor; los procesos nativos pertenecen a Job Objects que terminan al cerrarse sus handles. El catálogo se publica después de preparar el lote, no después de cada resultado parcial. El caché de video puede reutilizar etapas ya completadas aunque la publicación del catálogo se cancele.

Los pesos no cambian al indexar. `app-data/catalog.json` guarda texto/pasajes/vectores y ubicaciones; el catálogo de demo vive aparte. El archivo JSON y la clonación de documentos/hits no son una solución optimizada para millones de pasajes. El selector de carpeta no equivale a vigilancia automática.

## Primeros pasos del siguiente colaborador

1. Compilar, preparar la demo sintética y ejecutar las 15 pruebas. Confirmar el self-test con los recursos fijados. Probar luego una carpeta pequeña propia, nunca empezar por todo el disco.
2. Anotar un conjunto independiente de consultas reales en español, con archivo y línea/página/segundo correctos antes de ejecutar búsquedas. Incluir negativos, marcas, errores de escritura, duplicados y nombres exactos. Separar calibración de evaluación.
3. Escuchar los saltos de video y medir error temporal y transcripción. Comparar modelos ASR solo si la muestra muestra fallos relevantes. Conservar la distinción entre voz, nombre/ruta, OCR y evidencia visual.
4. Incorporar PDF/DOCX con posición original y OCR selectivo. Un mejor modelo no puede recuperar un archivo cuyo contenido no se extrajo.
5. Migrar metadatos/texto a SQLite/FTS5 con transacciones, migraciones y reconciliación; separar vectores binarios del JSON. Mantener compatibilidad/versionado o ofrecer regeneración explícita. Medir ranking híbrido contra el actual antes de sustituirlo.
6. Añadir tareas persistentes, reanudación, filtros, exclusiones, identidad de archivos y watcher. Resolver overflow/pérdida de eventos; evaluar USN Journal solo después y con permisos apropiados.
7. Optimizar con mediciones de todo el árbol: reposo, consulta fría/caliente, importación visual, ASR, reproducción, descarga y cierre. Probar un equipo con menos CPU/RAM y CPU AVX2. No presentar una cifra del worker como RAM total de la aplicación.
8. Comparar un modelo local adicional sobre 5–20 candidatos si sigue existiendo ambigüedad. Si lo local es insuficiente y la mejora está probada, estudiar una API opcional, con presupuesto y fallback. [Investigación disponible](SYSTEM-ONE.md).

## Restricciones que conviene conservar

- Ubicación y texto fuente verificables; no sustituirlos por explicaciones generadas.
- Identificadores completos y evidencia literal protegidos del ranking semántico.
- Escritura atómica, cancelación que no destruye el índice publicado y verificación de cambios durante la importación.
- Puntuaciones del modelo claramente distintas de porcentajes de confianza calibrados.
- IA y extractores bajo demanda; terminar procesos y limpiar temporales.
- Archivos locales excluidos del repositorio: un índice/transcripción contiene datos originales, aunque no se guarden los archivos completos.
- Revisiones del runtime/modelo fijadas y paridad vectorial al cambiar compilador, cuantización, dimensión o backend.

## Medición y CLI

`--index-folder ruta` prepara textos compatibles; `--index-selected archivo.json` recibe `{ "folder": "ruta", "paths": ["archivo"] }`. `--index-images archivo.json` y `--index-videos archivo.json` reciben listas JSON de rutas. `--probe-queries archivo.json` recibe casos `{ "query": "consulta", "expected_file": "nombre" }` y evalúa modo léxico y semántico. Todos usan el catálogo y motor de producción; sus informes se guardan bajo `app-data/`.

`measure-window.ps1` muestrea ventana + worker de texto; **no cubre por sí solo ASR/FFmpeg/reproductor ni todo el árbol**. `measure-search.ps1` mide procesos de Windows Search; sus cifras no establecen una comparación de calidad. `measure-probe.ps1` es un auxiliar histórico para los modos indicados en su parámetro; no usarlo como certificación de importación de video.

Antes de publicar una mejora, registrar corpus/consultas sintéticos o debidamente autorizados, errores, p50/p95, working set/commit, CPU, disco y versiones. No hacer públicos informes con rutas, fragmentos o datos privados por comodidad. Contribuciones pueden enviarse como fork/PR; este snapshot no promete mantenimiento activo.

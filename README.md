# Buscador local para Windows

Prototipo de buscador de archivos en **Rust + egui**, con **Google EmbeddingGemma 2 local** para búsqueda semántica, importación visual de imágenes y transcripción local de videos. Inspirado en la metodología de [DigUp](https://github.com/ARahim3/DigUp), con una implementación de escritorio para Windows.

**Estado al 11 de octubre de 2026: desarrollo detenido en una versión funcional para evaluación.** El repositorio se publica para que otras personas puedan estudiarlo y continuarlo. Todavía no es un reemplazo completo de Windows Search, ni un instalador listo para distribuir. La meta de menos de 500 MB de RAM del conjunto sigue pendiente de validar en todos los estados.

## Qué funciona

- Búsqueda por palabras e identificadores y búsqueda por significado sobre contenido previamente preparado.
- TXT, Markdown, CSV, logs y varios formatos de código UTF-8.
- Importación manual de PNG, JPEG, WebP y BMP usando sus píxeles.
- Importación de videos con audio: voz en español → pasajes con tiempos → embeddings → resultados con referencia temporal.
- Vista previa de documentos, imágenes y transcripciones; palabras coincidentes subrayadas, texto seleccionable y filas completamente pulsables.
- UI con división inicial 50/50, Segoe UI, controles nativos de ventana y Acrylic cuando Windows lo permite.
- Sincronización manual/F5, reutilización de archivos sin cambios, cancelación que conserva el catálogo anterior y guardado atómico.
- Reproducción de video desde un segundo antes de la referencia, en una ventana separada de FFplay.
- Botón para liberar el modelo de búsqueda; cierre de workers y reproductor mediante Job Objects de Windows.

Los modelos se descargan una vez; la inferencia y la búsqueda de archivos funcionan localmente. No hay integración de API de pago, servicio residente, arranque automático con Windows ni servidor web.

## Qué falta

PDF/DOCX/Office, OCR documental, búsqueda visual dentro de videos, reproducción incrustada en el panel, índices adecuados para todo el equipo, observación automática de cambios, reanudación completa de tareas y abstención semántica calibrada. Las consultas naturales sin respuesta todavía pueden devolver archivos irrelevantes. Los tiempos del ASR son aproximados y falta validarlos escuchando una muestra anotada.

Consulta **[estado detallado](docs/STATUS.md)** y **[guía para continuar](docs/HANDOFF.md)** antes de modificar el motor. Ahí se distinguen funcionalidades implementadas, resultados observados y propuestas futuras.

## Compilar y ejecutar

Windows x64, CPU con AVX2, Rust 1.94.1, Visual Studio Build Tools con C++, LLVM/clang-cl y CMake/Ninja. FFmpeg/FFprobe/FFplay son necesarios para videos. Python no es necesario para ejecutar la aplicación.

Desde una terminal PowerShell en la raíz del clon:

```powershell
& .\scripts\fetch-assets.ps1
& .\spike\build-runtime-clang.cmd
if ($LASTEXITCODE -ne 0) { throw 'Falló el runtime.' }
& .\spike\build-worker.cmd
if ($LASTEXITCODE -ne 0) { throw 'Falló el worker.' }
& .\scripts\prepare-demo.ps1
cargo build --release --locked --manifest-path app/Cargo.toml
if ($LASTEXITCODE -ne 0) { throw 'Falló la aplicación.' }
& .\app\target\release\local-search-preview.exe
```

Para comenzar solo con textos, `fetch-assets.ps1 -TextOnly` omite el proyector visual y Whisper. Compila igualmente el runtime y el worker. Instrucciones de dependencias, hashes, versiones y solución de problemas: **[BUILD.md](docs/BUILD.md)**.

La demostración contiene **32 documentos sintéticos**, sin datos personales. Sus vectores están incluidos; el script genera rutas válidas para cada clon y no reemplaza un catálogo personal existente. Escribe «quiero que me regresen el dinero de mi compra» o «F-1042». Para tus archivos, usa el menú de colección: **Elegir carpeta**, **Añadir imágenes…** o **Añadir videos…**.

## Verificación y mediciones

```powershell
cargo fmt --check --manifest-path app/Cargo.toml
cargo test --release --locked --manifest-path app/Cargo.toml
```

La versión congelada pasó **15 pruebas** y el build Release en Windows. Las pruebas unitarias no requieren descargar modelos. Existe una prueba de integración opcional `--self-test` que sí necesita runtime, pesos y demostración preparada.

| Observación del desarrollo | Resultado y límite |
|---|---|
| Ventana + modelo de texto, colección pequeña | ≈406 MB residentes, pico ≈410 MB en una muestra breve |
| Importación de imágenes | ≈1.167 MB sin interfaz; supera el objetivo de 500 MB |
| ASR aislado de cinco clips | Pico 408,05 MiB residentes / 807,72 MiB privados; excluye UI y otros procesos |
| Preparación integrada de cinco videos | 56,85 s; reutilización sin cambios 0,70 s |
| Consultas de video después de cargar el modelo | 31–37 ms en una colección de 16 archivos/204 pasajes |

Estas observaciones proceden de una laptop Ryzen 5 7530U con 12 procesadores lógicos y unos 31 GiB de RAM. No certifican un límite de consumo, calidad general, desempeño en computadoras modestas ni superioridad sobre Windows Search. Los archivos privados utilizados durante el desarrollo no se publican. Más contexto en [STATUS.md](docs/STATUS.md).

## Organización

| Ruta | Contenido |
|---|---|
| `app/src/` | Interfaz, catálogo, búsqueda, medios, video y pruebas Rust |
| `spike/embed-worker.cpp` | Worker JSON por stdin/stdout sobre llama.cpp |
| `patches/` | Corrección de memoria para embeddings no causales |
| `scripts/` | Preparación de dependencias y demostración portable |
| `examples/` | Corpus sintético y vectores compatibles |
| `docs/` | Estado, compilación, arquitectura, continuidad e investigación |
| `licenses/` | Licencias de componentes y atribuciones |

`app-data/`, modelos, DLL, ejecutables, fuentes descargadas y mediciones generadas están excluidos de Git. El índice local contiene fragmentos originales y transcripciones: trátalo como contenido privado. La ausencia de conexión a una API no cifra automáticamente esos archivos.

## Continuidad y licencia

La implementación propia se publica bajo **MIT**. Se agradecen continuaciones y forks; la publicación de este snapshot no implica mantenimiento activo ni fechas de entrega. Los primeros trabajos recomendados son validar evidencia y formatos, incorporar PDF/Office, preparar evaluaciones independientes y mejorar almacenamiento/recuperación antes de añadir otro modelo.

La investigación de **Jev, Clef y alternativas locales** está en [SYSTEM-ONE.md](docs/SYSTEM-ONE.md). Es una comparación fechada y un diagnóstico, no una integración comercial.

Créditos y restricciones de dependencias/modelos: [THIRD-PARTY.md](docs/THIRD-PARTY.md).

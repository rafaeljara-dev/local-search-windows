# Compilar y reproducir

## Entorno

Probado en Windows 11 x64, Ryzen 5 7530U y CPU AVX2. El código usa APIs de Windows; no se ha portado a Linux/macOS/ARM. La compilación CPU elegida usa AVX2 y no se validó en procesadores anteriores.

Instala Git, Rust 1.94.1 con `rustup`, Visual Studio 2022 Build Tools con **Desktop development with C++**, SDK de Windows, CMake/Ninja y LLVM con `clang-cl.exe`. Los scripts localizan Visual Studio mediante `vswhere.exe` y esperan LLVM en `%ProgramFiles%\LLVM\bin`. Si elegiste otra ubicación, ajusta `TASK_CLANG` en `spike/build-runtime-clang.cmd`.

Para video, instala FFmpeg y confirma desde esta terminal:

```powershell
ffmpeg -version
ffprobe -version
ffplay -version
```

No se incluye FFmpeg en el repositorio ni se instala automáticamente. Para imágenes y textos no necesitas Whisper ni FFmpeg.

## Dependencias fijadas

| Dependencia | Revisión |
|---|---|
| Rust usado para verificar | 1.94.1, edición 2024 |
| egui/eframe | 0.33.3; restantes versiones en `app/Cargo.lock` |
| llama.cpp | `b11461`, commit `4d756bc72bf00a4aacf410ae15a2d315f3db400d` |
| EmbeddingGemma 2 GGUF | `ggml-org/embeddinggemma-2-GGUF`, revisión `bfcd298762cc34d0357ece5ebdd31791a3a374d8` |
| Whisper runtime | `ggml-org/whisper.cpp`, release CPU x64 `b5454` |
| Whisper base multilingüe | `ggerganov/whisper.cpp`, revisión `5359861c739e955e79d9a303bcbc70fb988958b1` |

`scripts/fetch-assets.ps1` descarga pesos públicos y runtime ASR, comprueba SHA-256, clona llama.cpp en la revisión fijada y aplica el parche. No arranca servidores ni aplicaciones. Descarga aproximadamente 1 GB más código/runtime en la modalidad completa; deja espacio adicional para compilar. `-TextOnly` descarga únicamente el modelo de texto y llama.cpp.

| Archivo | Bytes / SHA-256 |
|---|---|
| `embeddinggemma-2-Q8_0.gguf` | 309.855.456 / `2188ac1deca4b77dffefd603c2776a9d76d9d74ec01841392982ebb840b09135` |
| `mmproj-embeddinggemma-2-Q8_0.gguf` | 554.821.024 / `c4a8a52691ecef40618438928bdf9e68379b854e24166f292592353db0aab64f` |
| `ggml-base.bin` | 147.951.465 / `60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe` |
| `whisper-bin-x64.zip` | 8.928.640 / `6ba69e3482d7826214f90a6a9c84ca07782aec1e1d0c6a7c30c994fd5d816ccb` |

## Pasos

Ejecuta el bloque de la raíz [README.md](../README.md). Los scripts `.cmd` cargan el entorno MSVC; no es necesario tener `cl.exe` en el PATH inicial. Se construye llama.cpp con clang-cl/Ninja y el worker con MSVC. Los encabezados públicos de llama.cpp y mtmd que necesita el worker están fijados junto a su fuente.

La estructura final esperada es:

```text
models/embeddinggemma-2-Q8_0.gguf
models/mmproj-embeddinggemma-2-Q8_0.gguf       # imágenes
models/ggml-base.bin                         # videos
tools/cpu-patched-avx2/bin/*.dll
tools/whisper-b5454/Release/whisper-cli.exe   # videos
spike/embed-worker.exe
app-data/demo.json
app/target/release/local-search-preview.exe
```

El ejecutable busca una raíz antecesora con modelo y worker. Conserva la estructura del clon; copiar solamente el EXE a otra carpeta no constituye una distribución instalada.

Preparar de nuevo la demostración actualiza `app-data/demo.json` y sus archivos sintéticos, pero no `app-data/catalog.json`. El primer inicio usa la demo cuando no hay catálogo personal.

## Validación

```powershell
cargo fmt --check --manifest-path app/Cargo.toml
cargo test --release --locked --manifest-path app/Cargo.toml
cargo build --release --locked --manifest-path app/Cargo.toml
& .\app\target\release\local-search-preview.exe --self-test
```

`--self-test` realiza búsqueda exacta/semántica y sincronización en una colección de prueba, guarda un informe local y termina. La aplicación Release es un proceso de Windows sin consola; si se lanza desde PowerShell puede ser necesario esperar el proceso antes de leer el informe.

Para capturas del renderizador, compila con `--features ui-capture`; esa característica permite `--capture ruta.png --capture-query "consulta" --capture-small`. No representa una captura de la composición Acrylic ni de los controles nativos de Windows. El build distribuido normal no habilita esa característica.

## Problemas conocidos al reproducir

- **No se encontraron modelo y motor:** falta un archivo requerido o se movió el EXE fuera del árbol.
- **La aplicación no carga el catálogo:** ejecutar `scripts/prepare-demo.ps1` después de clonar; no se distribuye un catálogo con rutas de otro equipo.
- **Error del runtime o instrucciones CPU:** comprobar revisión, parche, AVX2 y DLL de `tools/cpu-patched-avx2/bin`.
- **Falta el proyector o ASR:** `-TextOnly` prepara solo textos. Repetir sin ese parámetro para medios.
- **Video sin audio:** se rechaza explícitamente; todavía no existe extracción visual de fotogramas.
- **Límite de tokens:** el worker usa contexto 512; cambios de segmentación requieren verificar límites reales y regenerar embeddings compatibles.

El conjunto completo de instalación desde una máquina limpia no se validó durante la publicación. Se verificaron la aplicación y sus pruebas en el equipo de desarrollo, la demo portable, la preparación de texto con pesos existentes y hash comprobado, y compilaciones nuevas del runtime/worker con una consulta de inferencia válida. Detalles en STATUS. Si se modifica una dependencia, registrar nuevamente hashes, paridad de vectores y memoria.

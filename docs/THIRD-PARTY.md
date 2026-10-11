# Dependencias y atribuciones

- **DigUp**, Abdur Rahim: MIT. Inspiración metodológica y origen del parche para embeddings no causales. Revisión estudiada `6faa29e4c315a3a591de1b41bd78751bdf39588d`. [Proyecto](https://github.com/ARahim3/DigUp), [aviso conservado](../licenses/DigUp-MIT.txt).
- **llama.cpp / ggml / mtmd**, ggml-org y colaboradores: MIT. Encabezados públicos fijados de `b11461` presentes en `spike/`; el runtime completo se descarga/compila por separado. [Proyecto](https://github.com/ggml-org/llama.cpp), [aviso](../licenses/llama.cpp-MIT.txt).
- **nlohmann/json 3.12.0**, Niels Lohmann: MIT. Single-header `spike/json.hpp` con su atribución original. [Proyecto](https://github.com/nlohmann/json), [aviso](../licenses/nlohmann-json-MIT.txt).
- **eframe/egui**, **image**, **rfd**, **serde**, **unicode-normalization**, **windows-sys** y demás dependencias Rust: versiones exactas en `app/Cargo.lock`; respetar sus licencias al redistribuir binarios. La licencia MIT de este repositorio no cambia las de terceros.
- **EmbeddingGemma 2**, Google; conversión GGUF de ggml-org. Pesos no incluidos: consultar [modelo original](https://huggingface.co/google/embeddinggemma-2) y [conversión fijada](https://huggingface.co/ggml-org/embeddinggemma-2-GGUF/tree/bfcd298762cc34d0357ece5ebdd31791a3a374d8). Los scripts fijan revisión y SHA-256.
- **whisper.cpp**, ggml-org, y pesos Whisper base: descargas externas. Consultar [runtime](https://github.com/ggml-org/whisper.cpp) y [pesos](https://huggingface.co/ggerganov/whisper.cpp). No están incluidos en el snapshot público.
- **FFmpeg/FFprobe/FFplay**: instalación externa. La licencia de una distribución depende de cómo se compiló; no se redistribuyen sus binarios aquí. Revisar la distribución elegida antes de crear un instalador.

Los modelos System One investigados tienen sus propias licencias y condiciones; no son dependencias de ejecución de este prototipo. El informe enlaza sus fuentes y no incorpora pesos ni código remoto de dichos modelos.

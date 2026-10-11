# Modelos System One para el buscador local

Investigación y diagnóstico del **11 de octubre de 2026**. Equipo de prueba: Ryzen 5 7530U, 12 procesadores lógicos, unos 31,3 GiB de RAM y Radeon integrada. Precios en USD. Las cifras de proveedores, los tamaños descargables y nuestras mediciones se distinguen expresamente.

## Decisión propuesta

**Mantener el buscador local y mejorar primero extracción, indexación y recuperación.** Jev, Clef y otros modelos de decisión pueden evaluar una lista corta de resultados; no construyen por sí solos un índice de toda la computadora. Tampoco solucionan la ausencia de un PDF/DOCX que nunca extrajimos, una palabra mal transcrita o un fotograma que no procesamos.

Hay opciones locales pequeñas. Todavía no tenemos evidencia suficiente para declararlas superiores a nuestro motor en español ni para descartarlas todas. La investigación identifica candidatos y un ensayo local ya completado; no autoriza una migración a API ni demuestra que hayamos agotado las alternativas locales. No se enviaron documentos, imágenes ni videos personales a servicios de inferencia.

Propuesta de orden: **búsqueda híbrida local → evaluación de candidatos locales → API opcional solo si demuestra una mejora útil**. No se añadió ningún modelo nuevo al buscador durante esta investigación.

## Qué aportan Jev y Clef

Un modelo de decisión recibe contenido y opciones definidas por el programa y devuelve puntuaciones/probabilidades sobre ellas. Para nosotros podría responder: «¿Este pasaje satisface la consulta?», «¿Qué archivo de esta lista es más pertinente?» o «¿Ninguno contiene evidencia suficiente?». El resultado debe conservar la ruta, cita y ubicación originales; la puntuación del modelo no demuestra que una mención exista.

| Modelo | Disponibilidad y entrada | Encaje en esta aplicación |
|---|---|---|
| **Jev 1.13** | API de TypeSafe; texto/JSON, sin imagen, audio o video; USD 0,042 por millón de tokens de entrada | Revisar pasajes transcritos o extraídos. Modelo principalmente entrenado en inglés: comprobar español y nombres propios. No hay pesos locales ofrecidos en su documentación. [Modelo y condiciones](https://docs.typesafe.ai/models). |
| **Clef-flash 9B** | Pesos Apache 2.0 y API; visión; USD 0,038/M. La documentación vigente del endpoint indica 24.576 tokens de contexto | Primera candidata de API para texto e imágenes si lo local resulta insuficiente. La bajada de precio del 9 de octubre reemplaza el antiguo USD 0,09/M. [Endpoint](https://developers.cloudflare.com/workers-ai/models/clef-flash/), [actualización](https://blog.cloudflare.com/clef-faster-cheaper-multimodal/). |
| **Clef 27B** | Pesos Apache 2.0, texto y visión; USD 0,24/M en API | Comparador de calidad si Flash falla en casos difíciles. Su mayor tamaño no prueba una ventaja en nuestros archivos. [Modelo oficial](https://huggingface.co/Cloudflare/clef), [precio](https://developers.cloudflare.com/workers-ai/platform/pricing/). |
| **Clef-omni** | Variante multimodal anunciada el 9 de octubre, basada en Qwen3-Omni-30B-A3B; audio/video/imagen/texto; USD 0,15/M | Posible evaluación futura de clips ya seleccionados. Sus 3B parámetros activos no significan que solo necesite memoria para 3B. No reemplazaría todavía el ASR con tiempos que necesitamos para saltar a una palabra. [Anuncio oficial](https://blog.cloudflare.com/clef-faster-cheaper-multimodal/). |
| **Microsoft-Decision-1** | API, anunciado el 9 de octubre; USD 0,042/M de entrada, salida sin cargo | Otro comparador para decisiones sobre candidatos. No lo hemos ejecutado; los benchmarks del proveedor no miden latencia desde esta laptop. [Anuncio y precio](https://commandline.microsoft.com/microsoft-decision-1-model-foundry/). |
| **GLiDE** | API de Fastino, decisiones estructuradas; USD 0,15/M de entrada | Alternativa para la comparación de calidad, con menor prioridad inicial por coste relativo. [Modelo](https://fastino.ai/blog/introducing-glide-the-first-thinking-decision-model), [tarifa](https://docs.fastino.ai/pricing). |

Los Clef oficiales son ejecutables localmente, pero no encajan en un presupuesto estricto de 500 MB para toda la aplicación. Solo los valores de 9.000 millones de parámetros a 4 bits representarían **4,5 GB decimales**, antes de escalas, visión, activaciones y runtime. Es un cálculo de almacenamiento ideal, no una medición de RAM ni una cuantización instalada. Cargar por partes desde disco puede reducir residencia a costa de trabajo de E/S; no acredita búsqueda rápida en una laptop modesta.

## Alternativas locales pequeñas

Los tamaños siguientes describen **archivos/pesos**, salvo la fila que incluye nuestra medición. Un archivo de 119 MB no equivale a un proceso de 119 MB.

| Candidato | Evidencia primaria y tamaño | Evaluación para Windows y español |
|---|---|---|
| **Bekko System One 17M / 68M** | ONNX publicado de aproximadamente 29 / 196 MB. Son exportaciones con embeddings de tokens cuantizados; otras capas siguen FP32. [Ficha del autor](https://huggingface.co/hotchpotch/bekko-system-one-v0-17m). | El 17M es interesante como control muy ligero. Es experimental y orientado al inglés; el propio autor limita las conclusiones de generalización. No asumir buena búsqueda en español. No ejecutado aquí. |
| **Julia-1, 144M** | Modelo multilingüe basado en mmBERT-small, Apache 2.0. Pesos FP32 de unos 550,5 MiB; la exportación ONNX publicada no elimina ese coste. [Modelo](https://huggingface.co/SupersonicLabs/Julia-1), [ONNX](https://huggingface.co/SupersonicLabs/Julia-1-ONNX). | Candidato de decisión local para un ensayo de cuantización. INT8 podría reducir pesos, pero RAM, fidelidad y soporte del runtime se deben medir. No hay una versión <500 MB de aplicación completa demostrada aquí. |
| **GLiNER2.5-multi-Decide, 287M** | Clasificador multilingüe Apache 2.0. El safetensors oficial consultado pesa **1.149.461.028 bytes**. [Ficha](https://huggingface.co/fastino/GLiNER2.5-multi-Decide). | Preferible para el ensayo en español a la versión Decide de 340M enfocada en inglés. Más adecuado para categorías, intención y filtros que para asumir automáticamente el mejor ranking documental. Requiere evaluar cuantización; no ejecutado. |
| **Liquid d1-omni-600M** | GGUF oficial Q8 de **407.207.584 bytes**; proyector multimodal Q8 de **262.791.232 bytes** adicionales. Licencia LFM 1.0. [Ficha y runtime](https://huggingface.co/LiquidAI/d1-omni-600M-GGUF). | Existe una alternativa local reciente y multimodal. Texto + proyector ya suman unos 670 MB en disco: no cabe asumir <500 MB en uso. El runtime necesita soporte de decisiones; no es un reemplazo directo del worker actual de embeddings. No ejecutado. |
| **Clef-text-0.6b de FluidInference** | Destilación de terceros, 0,70B total. Backbone **1.192.135.096 bytes** y cabeza **205.776.960 bytes**. [Pesos PyTorch](https://huggingface.co/FluidInference/clef-text-0.6b), [Core ML](https://huggingface.co/FluidInference/clef-text-0.6b-coreml). | No confundirlo con Clef-flash oficial. La versión Core ML y sus tiempos publicados son para Apple; no prueban rendimiento Windows. Una adaptación cuantizada podría estudiarse, pero la distribución actual no cumple nuestro presupuesto. |
| **Laya** | Encoder de decisiones de 421M parámetros; ofrece también ruta multilingüe. [Ficha oficial](https://huggingface.co/convaiinnovations/laya). | Menor prioridad: el tamaño y la posible carga de más de un modelo hacen necesario medir la configuración exacta. No importar el router completo suponiendo consumo mínimo. No ejecutado. |
| **mMARCO mMiniLMv2 L12, reranker especializado** | ONNX INT8 AVX2 oficial de **118.620.016 bytes**, más tokenizer de 17.082.660 bytes. [Repositorio del modelo](https://huggingface.co/cross-encoder/mmarco-mMiniLMv2-L12-H384-v1). | **Sí probado en esta laptop**. Es multilingüe y se entrenó para puntuar consulta/pasaje; esa tarea es directamente pertinente al buscador. Los resultados siguientes no justifican añadirlo por defecto. |

Las publicaciones cambian rápidamente: por ejemplo, la familia de Clef ya tiene nuevas variantes y tarifas respecto al lanzamiento del 1 de octubre. Se consultaron las fichas oficiales actuales y los metadatos de sus archivos, sin depender de las cifras de un catálogo agregado.

## Prueba local realizada

Se ejecutó un diagnóstico de reranking en un entorno aislado con ONNX Runtime 1.31.0, tokenizers 0.23.3 y NumPy 2.5.4. Modelo y tokenizer se fijaron a la revisión `1427fd652930e4ba29e8149678df786c240d8825` y se verificaron los SHA-256 publicados. Dos hilos CPU, sin GPU ni red de inferencia. La aplicación Rust no depende de este entorno Python.

Protocolo: ocho consultas en español sobre los pasajes textuales de la colección real, incluidos videos; recuperar por similitud de EmbeddingGemma 2, diversificar hasta 20 pasajes (máximo cuatro por archivo) y reordenarlos con el cross-encoder. Se cerró el proceso de embeddings antes de cargar el reranker. Las imágenes no se evaluaron. El comparador es coseno diversificado, **no el ranking híbrido completo de la aplicación**.

| Medición | Resultado observado |
|---|---:|
| Carga de sesión ONNX + tokenizer | 904,66 ms |
| Mediana por lista de 20 pasajes | 670,95 ms |
| Intervalo en las ocho consultas | 539,23–804,38 ms |
| Working set final/pico del proceso de prueba | 482,05 MiB (≈505,47 MB decimales) |
| Memoria privada comprometida final/pico | 872,70 MiB |
| Casos positivos con archivo esperado primero | 6/6 antes y 6/6 después |

Esto **no demuestra mejora de precisión**: los seis positivos ya estaban bien ordenados y la muestra es pequeña, conocida y con objetivos compartidos. El ensayo sí revela un coste adicional relevante. La memoria incluye Python, tokenizer, runtime y datos de prueba; excluye la UI y el worker de embeddings ya cerrado. No demuestra cuánto consumiría una integración nativa Rust ni permite afirmar <500 MB para todo el producto. La suma simultánea con nuestro modelo actual sería problemática.

También apareció una razón concreta para no usar un umbral universal: el mejor candidato irrelevante para «manual para reparar el motor de un submarino» obtuvo **−3,798**, mientras que el pasaje correcto para «condiciones adicionales» obtuvo **−3,909**. Un corte global sencillo perdería un acierto antes de eliminar ese falso candidato. Estos logits tampoco son porcentajes de confianza calibrados.

Los datos, salidas y script de casos privados del ensayo no se incluyen en este repositorio. El protocolo y las versiones anteriores permiten preparar una evaluación propia independiente. No se adoptó el reranker ni se optimizó su implementación: esta fase evalúa viabilidad.

## Mejor metodología para toda la computadora

1. **Inventario ligero primero.** Indexar nombres, extensiones, fechas, tamaños e identidad de archivo. Resolver rutas y códigos exactos sin cargar IA. Mantener filtros por carpeta/tipo y resultados progresivos. La cobertura de nombres puede ser amplia aunque la extracción de contenidos se limite a carpetas elegidas.
2. **Extracción por formato con ubicación.** PDF/Office con página o sección; OCR solo cuando haga falta; ASR con tiempos; imágenes y fotogramas con evidencia visual separada. Guardar explícitamente qué está preparado, pendiente, excluido o falló. Actualmente los contratos PDF/DOCX originales siguen fuera del índice: esto tiene más prioridad que otro modelo de ranking.
3. **Índice persistente híbrido.** SQLite para metadatos, texto y ubicaciones; FTS5/BM25 para términos y frases, más embeddings para conceptos. Combinar candidatos, deduplicar por archivo y preservar varias ubicaciones. [FTS5 oficial](https://sqlite.org/fts5.html). La combinación concreta debe compararse con el motor actual en nuestro corpus.
4. **Segundo modelo solo sobre pocos candidatos.** Evaluar los mejores 5–20 pasajes cuando haya ambigüedad y mostrar antes los resultados exactos. No volver a leer todo el disco ni pasar todos los vectores por un modelo de decisión en cada consulta. Es el patrón documentado de recuperación y reordenamiento. [Sentence Transformers](https://www.sbert.net/examples/sentence_transformer/applications/retrieve_rerank/README.html).
5. **Actualizaciones incrementales.** Guardar versión del extractor/modelo y una huella del archivo, reutilizar trabajo completo y hacer cola con pausa/reanudación. Usar notificaciones de cambios; en NTFS, evaluar USN Journal para volúmenes y permisos compatibles. Si se pierde el historial o desborda el buffer, reconciliar mediante recorrido. [Change Journals](https://learn.microsoft.com/en-us/windows/win32/fileio/change-journals), [ReadDirectoryChangesW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-readdirectorychangesw).
6. **Escalar el almacenamiento antes que el modelo.** El JSON y la comparación lineal actuales sirven para el prototipo de 204 pasajes. Un millón de vectores de 768 valores float32 ocupa **3,072 GB** sin texto ni estructura de búsqueda. Evaluar vectores binarios en disco, cuantización y búsqueda aproximada con un presupuesto explícito. La reducción de dimensión exige soporte del modelo y pruebas de recuperación; no recortar vectores arbitrariamente.
7. **Estados de consumo comprensibles.** Búsqueda léxica sin IA; IA bajo demanda y descarga por inactividad o botón; indexación pausable y con límite de concurrencia; salida completa para liberar todos los procesos. La opción actual de liberar IA no elimina la RAM de la ventana ni del catálogo. No confundir minimizar con apagar.

Propuesta de alcance inicial: carpetas personales elegidas, sin recorrer dependencias, cachés, compilaciones, archivos del sistema o enlaces recursivos. Respetar permisos y archivos disponibles solo en la nube. El escaneo acotado y la reanudación son esenciales: un intento anterior de explorar Descargas completo se canceló tras 26 minutos entre datos de aplicaciones portátiles.

## Qué exigir antes de elegir otro modelo

Preparar un conjunto independiente de al menos 100 consultas, con archivo y página/tiempo correctos anotados antes de probar candidatos. Incluir marcas y nombres propios, acentos, errores de escritura, contratos similares, documentos escaneados, consultas por significado y al menos un 20 % sin respuesta. Separar ajuste y evaluación; ninguna transcripción automática sirve como única verdad de referencia.

Comparar: motor actual; híbrido léxico/vectorial; híbrido + reranker; híbrido + Julia/Bekko/GLiNER multilingüe según viabilidad del runtime. Liquid/Clef destilado quedan para una segunda ronda si el tamaño y soporte lo permiten. Medir Recall@20, MRR@10/nDCG@10, falsos resultados en negativos y exactitud de ubicación. Registrar latencia fría/caliente p50/p95, disco, CPU y working set/commit de **todo el árbol** en reposo, consulta, importación, reproducción y apagado.

Puertas propuestas, todavía no alcanzadas: resultados por nombre p95 <100 ms, recuperación semántica caliente p95 <300 ms en el equipo objetivo, mejora de al menos cinco puntos de nDCG@10 para justificar un segundo modelo sin degradar identificadores, y <500 MB residentes del conjunto durante uso normal si esa sigue siendo la restricción. Indexación multimedia necesita presupuesto separado o cambios demostrados: ya superó ese límite en pruebas anteriores. Comparar contra Windows Search con idénticos archivos indexados y consultas; aún no se ha realizado esa comparación.

Si lo local cumple, conservarlo. Si falla en un grupo identificable de consultas, evaluar Jev y Clef-flash sobre **esa misma lista de candidatos**, con datos de prueba permitidos y un presupuesto pequeño. La API debe ser opcional, con plazo máximo, fallback local, caché y contador de gasto; una caída de red no debe bloquear la búsqueda.

## Estimación mensual de API

Supuesto ilustrativo: 30 días, una llamada por búsqueda y **11.000 tokens facturados** por llamada (consulta/instrucciones + hasta 20 pasajes). No es una medición del tokenizer de cada proveedor. Se deben contar opciones, preguntas y medios según `usage` real. Fórmula: búsquedas/día × 30 × 11.000 / 1.000.000 × tarifa.

| API | USD / millón de tokens de entrada | 50 búsquedas/día | 100/día | 300/día |
|---|---:|---:|---:|---:|
| Jev | 0,042 | 0,69 | **1,39** | 4,16 |
| Clef-flash | 0,038 | 0,63 | **1,25** | 3,76 |
| Clef | 0,240 | 3,96 | 7,92 | 23,76 |
| Clef-omni | 0,150 | 2,48 | 4,95 | 14,85 |
| Microsoft-Decision-1 | 0,042 | 0,69 | 1,39 | 4,16 |
| GLiDE | 0,150 | 2,48 | 4,95 | 14,85 |
| GLiNER-2.5-Decide, API | 0,030 | 0,50 | 0,99 | 2,97 |

Tarifas consultadas: [TypeSafe](https://docs.typesafe.ai/models), [Cloudflare](https://developers.cloudflare.com/workers-ai/platform/pricing/), [Microsoft](https://commandline.microsoft.com/microsoft-decision-1-model-foundry/) y [Fastino](https://docs.fastino.ai/pricing). Para GLiNER se muestra el producto API publicado, sin asumir que sea el checkpoint multilingüe local.

La tabla representa **consumo bruto teórico**, no factura final: excluye impuestos, mínimos/planes, intermediarios y reintentos. Cloudflare incluye 10.000 neuronas diarias; este ejemplo de 100 consultas/día con Clef-flash suma unas 3.801 neuronas/día y podría caber en la cuota gratuita si no se comparte con otros usos. Superarla exige revisar el plan: Workers Paid tiene base de USD 5/mes. La disponibilidad y límites de la cuenta deben comprobarse antes de activarlo. [Cuotas de Workers AI](https://developers.cloudflare.com/workers-ai/platform/pricing/), [plan Workers](https://developers.cloudflare.com/workers/platform/pricing/).

Si solo el 10 % de búsquedas necesita revisión externa, el consumo variable de la tabla se divide entre diez. Si se enviaran 100.000 archivos de 500 tokens en cada consulta, serían 50 millones de tokens: **USD 2,10 por consulta con Jev**, unos USD 6.300/mes a 100 diarias. La economía depende de recuperar localmente los candidatos, no de usar una tarifa baja para volver a examinar todo.

Para voz/video, el coste de transferencia, tokens multimodales y segmentación requiere otro cálculo; los USD 1–1,40 anteriores corresponden a pasajes textuales, no a analizar continuamente todos los videos de la computadora.

## Próxima implementación recomendada

Cerrar primero la prueba humana de videos y conectar PDF/DOCX con evidencia de página. Después migrar el catálogo a persistencia e índices adecuados, implementar recuperación híbrida y medir el conjunto independiente. Con esa base se puede decidir si un modelo adicional aporta lo suficiente. No recomendaría contratar una API ni prometer un consumo universal de 500 MB con los resultados actuales.

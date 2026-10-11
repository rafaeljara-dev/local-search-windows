# Instrucciones del proyecto

Este repositorio es un snapshot público de un prototipo Windows. Lee README.md,
docs/STATUS.md y docs/HANDOFF.md antes de proponer cambios. No confundas propuestas
de arquitectura con funciones ya implementadas.

- Español para documentación y contenido de producto; inglés para identificadores,
  código, commits y ramas. Mantener tildes y ñ.
- No arrancar servidores: la aplicación es nativa y no usa un servidor web.
- No abrir ventanas/navegadores para verificar salvo que se solicite. Para checks,
  usar cargo fmt, cargo test y build; la revisión visual requiere coordinación.
- No commitear ni pushear salvo instrucción. Consultar la rama default real del
  remoto antes de empezar cambios y trabajar en una rama propia.
- No publicar app-data, rutas privadas, documentos, capturas de colección personal,
  embeddings personales, transcripciones, logs ni credenciales.
- Mantener las licencias y atribuciones de DigUp, llama.cpp y nlohmann/json.
- Conservar runtime/modelos fijados hasta medir paridad vectorial y recursos.
- Cada cambio del formato o modelo requiere compatibilidad/migración/versionado
  explícitos. Nunca mezclar silenciosamente vectores de distintos pipelines.
- Cancelación no debe destruir el catálogo publicado; conservar escritura atómica
  y cierre de procesos con Job Objects. No afirmar porcentajes de confianza sin
  calibración ni presentar coincidencias de ruta como evidencia de voz.
- Registrar builds/tests fallidos y pasos no realizados con fidelidad. Las metas
  de 500 MB y superioridad sobre Windows Search todavía no están certificadas.

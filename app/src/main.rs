#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod appearance;
mod backend;
#[cfg(feature = "ui-capture")]
mod capture;
mod highlight;
mod media;
mod probe;
mod ui;
mod video;
use eframe::egui;
use std::path::{Path, PathBuf};
use ui::SearchApp;

fn root_directory() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    executable
        .ancestors()
        .find(|p| {
            p.join("models/embeddinggemma-2-Q8_0.gguf").is_file()
                && p.join("spike/embed-worker.exe").is_file()
        })
        .map(Path::to_path_buf)
        .ok_or("No se encontraron el modelo y el motor junto al proyecto.".into())
}
fn main() {
    let root = match root_directory() {
        Ok(root) => root,
        Err(error) => {
            rfd::MessageDialog::new()
                .set_title("Buscador local")
                .set_description(error)
                .show();
            return;
        }
    };
    if let Some(code) = probe::run(&root) {
        std::process::exit(code);
    }
    if std::env::args().any(|a| a == "--self-test") {
        let result = (|| -> Result<String, String> {
            let mut engine = backend::Engine::load_demo(root.clone())?;
            let exact = engine.search("F-1042", false)?;
            if exact
                .first()
                .is_none_or(|h| !h.document.title.contains("F-1042"))
            {
                return Err("Falló búsqueda de identificador".into());
            }
            if !engine.search("F-9999", true)?.is_empty() {
                return Err("Identificador inexistente produjo resultados".into());
            }
            let semantic = engine.search("quiero que me regresen el dinero de mi compra", true)?;
            if semantic
                .first()
                .is_none_or(|h| !h.document.title.contains("devoluciones"))
            {
                return Err("Falló búsqueda semántica".into());
            }
            engine.unload();
            backend::verify_synchronization(&mut engine)?;
            Ok("Correctos: búsqueda exacta y semántica, identificador ausente, descarga del worker, sincronización inicial, reutilización sin cambios, modificación, eliminación, persistencia y cancelación.".into())
        })();
        let _ = std::fs::write(
            root.join("app-data/self-test.json"),
            serde_json::to_vec_pretty(&result).unwrap(),
        );
        if result.is_err() {
            std::process::exit(1);
        }
        return;
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 760.0])
            .with_min_inner_size([920.0, 660.0])
            .with_decorations(true)
            .with_transparent(true)
            .with_icon(egui::IconData::default()),
        renderer: eframe::Renderer::Glow,
        centered: true,
        ..Default::default()
    };
    #[cfg(feature = "ui-capture")]
    let options = if std::env::args().any(|a| a == "--capture-small") {
        eframe::NativeOptions {
            viewport: options.viewport.with_inner_size([920.0, 660.0]),
            ..options
        }
    } else {
        options
    };
    if let Err(error) = eframe::run_native(
        "Buscador local",
        options,
        Box::new(move |cc| Ok(Box::new(SearchApp::new(cc, root)))),
    ) {
        rfd::MessageDialog::new()
            .set_title("No se pudo abrir el buscador")
            .set_description(error.to_string())
            .show();
    }
}

//! Local evaluation commands use the same engine and persistent catalog as the UI.
use crate::backend::Engine;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Instant,
};

#[derive(Deserialize)]
struct QueryCase {
    query: String,
    #[serde(default)]
    expected_file: Option<String>,
}

#[derive(Deserialize)]
struct FileSelection {
    folder: PathBuf,
    paths: Vec<PathBuf>,
}

pub fn run(root: &Path) -> Option<i32> {
    let args: Vec<_> = std::env::args().collect();
    let mode = if args.iter().any(|a| a == "--index-selected") {
        "--index-selected"
    } else if args.iter().any(|a| a == "--index-folder") {
        "--index-folder"
    } else if args.iter().any(|a| a == "--index-images") {
        "--index-images"
    } else if args.iter().any(|a| a == "--index-videos") {
        "--index-videos"
    } else if args.iter().any(|a| a == "--probe-queries") {
        "--probe-queries"
    } else {
        return None;
    };
    let argument = args
        .iter()
        .position(|a| a == mode)
        .and_then(|i| args.get(i + 1));
    let started = Instant::now();
    let result = (|| -> Result<Value, String> {
        let argument = argument.ok_or("Falta la ruta para la operación")?;
        let mut engine = Engine::load(root.to_path_buf())?;
        if mode == "--index-videos" {
            let paths: Vec<PathBuf> =
                serde_json::from_slice(&fs::read(argument).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let catalog = engine.import_videos(paths, &AtomicBool::new(false), |status| {
                let _ = fs::write(
                    root.join("app-data/real-data-progress.json"),
                    serde_json::to_vec(
                        &json!({"elapsed_seconds":started.elapsed().as_secs_f64(),"status":status}),
                    )
                    .unwrap(),
                );
            })?;
            Ok(
                json!({"files":catalog.documents.len(),"videos":catalog.documents.iter().filter(|d| d.video.is_some()).count(),"notice":catalog.notice,"index_seconds":started.elapsed().as_secs_f64()}),
            )
        } else if mode == "--index-images" {
            let paths: Vec<PathBuf> =
                serde_json::from_slice(&fs::read(argument).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let requested = paths.len();
            let catalog = engine.import_images(paths, &AtomicBool::new(false), |status| {
                let _ = fs::write(
                    root.join("app-data/real-data-progress.json"),
                    serde_json::to_vec(
                        &json!({"elapsed_seconds":started.elapsed().as_secs_f64(),"status":status}),
                    )
                    .unwrap(),
                );
            })?;
            Ok(
                json!({"files":catalog.documents.len(),"images":catalog.documents.iter().filter(|d| d.kind == "Imagen").count(),"requested":requested,"notice":catalog.notice,"index_seconds":started.elapsed().as_secs_f64(),"profile":crate::media::PROFILE}),
            )
        } else if mode == "--index-folder" || mode == "--index-selected" {
            let selected: Option<FileSelection> = if mode == "--index-selected" {
                Some(
                    serde_json::from_slice(&fs::read(argument).map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())?,
                )
            } else {
                None
            };
            let folder = selected
                .as_ref()
                .map(|s| s.folder.clone())
                .unwrap_or_else(|| PathBuf::from(argument));
            if !folder.is_dir() {
                return Err("La carpeta no existe".into());
            }
            let cancel = AtomicBool::new(false);
            let progress = |message| {
                let progress =
                    json!({"elapsed_seconds":started.elapsed().as_secs_f64(),"status":message});
                let _ = fs::write(
                    root.join("app-data/real-data-progress.json"),
                    serde_json::to_vec(&progress).unwrap(),
                );
            };
            let catalog = if let Some(selected) = selected {
                engine.synchronize_selection(folder, Some(selected.paths), &cancel, progress)?
            } else {
                engine.synchronize(folder, &cancel, progress)?
            };
            let report = json!({
                "source": catalog.source,
                "selection": catalog.selection,
                "files": catalog.documents.len(),
                "segments": catalog.documents.iter().map(|d| d.segments.len()).sum::<usize>(),
                "notice": catalog.notice,
                "catalog_bytes": fs::metadata(root.join("app-data/catalog.json")).map(|m| m.len()).unwrap_or(0),
                "index_seconds": started.elapsed().as_secs_f64(),
            });
            engine.unload();
            Ok(report)
        } else {
            let cases: Vec<QueryCase> =
                serde_json::from_slice(&fs::read(argument).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let mut queries = Vec::new();
            for case in cases {
                let expected_indexed = case.expected_file.as_ref().map(|target| {
                    engine
                        .catalog
                        .documents
                        .iter()
                        .any(|d| d.path.ends_with(target))
                });
                for semantic in [false, true] {
                    let begin = Instant::now();
                    let hits = engine.search(&case.query, semantic)?;
                    let duration = begin.elapsed().as_secs_f64() * 1000.0;
                    let expected_rank = case
                        .expected_file
                        .as_ref()
                        .and_then(|target| {
                            hits.iter().position(|h| h.document.path.ends_with(target))
                        })
                        .map(|rank| rank + 1);
                    queries.push(json!({
                        "query": case.query, "semantic": semantic, "milliseconds": duration,
                        "expected_file": case.expected_file, "expected_indexed": expected_indexed, "expected_rank": expected_rank,
                        "results_count": hits.len(),
                        "top_five": hits.iter().take(5).map(|hit| json!({"title":hit.document.title,"path":hit.document.path,"line":hit.line,"score":hit.score,"similarity":hit.similarity,"literal":hit.literal,"time_ms":hit.time_ms})).collect::<Vec<_>>()
                    }));
                }
            }
            engine.unload();
            Ok(
                json!({"source":engine.catalog.source,"files":engine.catalog.documents.len(),"queries":queries}),
            )
        }
    })();
    let (ok, data, error) = match result {
        Ok(data) => (true, data, None),
        Err(error) => (false, Value::Null, Some(error)),
    };
    let report =
        json!({"ok":ok,"data":data,"error":error,"total_seconds":started.elapsed().as_secs_f64()});
    let filename = if mode == "--index-images" {
        "real-data-images.json"
    } else if mode == "--index-videos" {
        "real-data-videos.json"
    } else if mode == "--index-folder" || mode == "--index-selected" {
        "real-data-index.json"
    } else {
        "real-data-queries.json"
    };
    let saved = fs::write(
        root.join("app-data").join(filename),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .is_ok();
    Some(if ok && saved { 0 } else { 1 })
}

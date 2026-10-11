use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    fs,
    io::{BufRead, BufReader, Read, Write},
    os::windows::{io::AsRawHandle, process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::JobObjects::*,
};

const PIPELINE: &str = "embeddinggemma2-q8-b11461-avx2-768-v1";
const MAX_FILES: usize = 500;
const MAX_SEGMENTS: usize = 2500;

#[derive(Clone, Serialize, Deserialize)]
pub struct Segment {
    pub text: String,
    pub line: usize,
    pub vector: Vec<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cue: Option<crate::video::Cue>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Document {
    pub title: String,
    pub path: PathBuf,
    pub kind: String,
    pub segments: Vec<Segment>,
    #[serde(default)]
    pub stamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video: Option<crate::video::VideoInfo>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Catalog {
    pub pipeline: String,
    pub source: String,
    pub demo: bool,
    pub documents: Vec<Document>,
    pub notice: String,
    #[serde(default)]
    pub selection: Option<Vec<PathBuf>>,
}
#[derive(Clone)]
pub struct Hit {
    pub document: Document,
    pub snippet: String,
    pub line: usize,
    pub literal: bool,
    pub score: f32,
    pub similarity: Option<f32>,
    pub time_ms: Option<u64>,
}
impl Hit {
    fn relevance_cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.score.total_cmp(&other.score).then_with(|| {
            self.similarity
                .unwrap_or(0.0)
                .total_cmp(&other.similarity.unwrap_or(0.0))
        })
    }
}
pub enum Request {
    Search(String, bool),
    Index(PathBuf, bool),
    IndexImages(Vec<PathBuf>),
    IndexVideos(Vec<PathBuf>),
    Demo,
    Unload,
    Quit,
}
pub enum Event {
    Catalog(Catalog),
    Progress(String),
    Hits(Vec<Hit>, u128),
    Ready(String),
    Error(String),
    Model(bool),
}

pub(crate) struct Job(HANDLE);
impl Job {
    pub(crate) fn attach(child: &Child) -> Result<Self, String> {
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let job = Self(handle);
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as _,
                std::mem::size_of_val(&info) as u32,
            ) == 0
                || AssignProcessToJobObject(handle, child.as_raw_handle() as _) == 0
            {
                return Err(std::io::Error::last_os_error().to_string());
            }
            Ok(job)
        }
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

struct Worker {
    child: Child,
    input: ChildStdin,
    output: mpsc::Receiver<Result<String, String>>,
    _job: Job,
}
impl Worker {
    fn start(root: &Path, visual: bool) -> Result<Self, String> {
        let log =
            fs::File::create(root.join("app-data/inference.log")).map_err(|e| e.to_string())?;
        let mut command = Command::new(root.join("spike/embed-worker.exe"));
        command
            .arg(root.join("tools/cpu-patched-avx2/bin"))
            .arg(root.join("models/embeddinggemma-2-Q8_0.gguf"))
            .args(["2", "512"]);
        if visual {
            command.arg(root.join("models/mmproj-embeddinggemma-2-Q8_0.gguf"));
        }
        let mut child = command
            .creation_flags(0x08000000)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(log)
            .spawn()
            .map_err(|e| e.to_string())?;
        let job = match Job::attach(&child) {
            Ok(job) => job,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        let input = child
            .stdin
            .take()
            .ok_or("No se abrió la entrada del modelo")?;
        let mut reader = BufReader::new(
            child
                .stdout
                .take()
                .ok_or("No se abrió la salida del modelo")?,
        );
        let (sender, output) = mpsc::channel();
        std::thread::spawn(move || {
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => {
                        let _ = sender.send(Err("El proceso del modelo se cerró.".into()));
                        break;
                    }
                    Ok(_) => {
                        if sender.send(Ok(line)).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(Err(error.to_string()));
                        break;
                    }
                }
            }
        });
        let mut worker = Self {
            child,
            input,
            output,
            _job: job,
        };
        let ready = worker.read()?;
        if ready["event"] != "ready" {
            return Err("El modelo no pudo iniciar".into());
        }
        Ok(worker)
    }
    fn read(&mut self) -> Result<Value, String> {
        let line = self
            .output
            .recv_timeout(Duration::from_secs(90))
            .map_err(|_| {
                let _ = self.child.kill();
                let _ = self.child.wait();
                "El modelo no respondió a tiempo. Puedes volver a intentar la operación.".to_owned()
            })??;
        if line.is_empty() {
            return Err("El proceso del modelo se cerró. Revisa app-data/inference.log.".into());
        }
        let value: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        if let Some(error) = value["error"].as_str() {
            return Err(error.to_owned());
        }
        Ok(value)
    }
    fn embed(&mut self, text: &str) -> Result<Vec<f32>, String> {
        self.request(json!({"text":text}))
    }
    fn image(&mut self, path: &Path) -> Result<Vec<f32>, String> {
        self.request(json!({"path":path}))
    }
    fn request(&mut self, payload: Value) -> Result<Vec<f32>, String> {
        writeln!(self.input, "{}", payload).map_err(|e| e.to_string())?;
        self.input.flush().map_err(|e| e.to_string())?;
        let value = self.read()?;
        let vector: Vec<f32> =
            serde_json::from_value(value["vector"].clone()).map_err(|e| e.to_string())?;
        if vector.len() != 768 || vector.iter().any(|v| !v.is_finite()) {
            return Err("El modelo devolvió un vector inválido".into());
        }
        Ok(vector)
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub struct Engine {
    pub root: PathBuf,
    pub catalog: Catalog,
    worker: Option<Worker>,
}
impl Engine {
    pub fn load_demo(root: PathBuf) -> Result<Self, String> {
        let catalog = read_catalog(&root.join("app-data/demo.json"))?;
        Ok(Self {
            root,
            catalog,
            worker: None,
        })
    }
    pub fn load(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(root.join("app-data")).map_err(|e| e.to_string())?;
        let file = root.join("app-data/catalog.json");
        let catalog = if file.exists() {
            read_catalog(&file)?
        } else {
            read_catalog(&root.join("app-data/demo.json"))?
        };
        Ok(Self {
            root,
            catalog,
            worker: None,
        })
    }
    fn model(&mut self) -> Result<&mut Worker, String> {
        if self.worker.is_none() {
            self.worker = Some(Worker::start(&self.root, false)?);
        }
        Ok(self.worker.as_mut().unwrap())
    }
    pub fn unload(&mut self) {
        self.worker = None;
    }
    pub fn synchronize(
        &mut self,
        folder: PathBuf,
        cancel: &AtomicBool,
        progress: impl Fn(String),
    ) -> Result<Catalog, String> {
        let selection = if self.catalog.source == folder.to_string_lossy() {
            self.catalog.selection.clone()
        } else {
            None
        };
        self.synchronize_selection(folder, selection, cancel, progress)
    }
    pub fn synchronize_selection(
        &mut self,
        folder: PathBuf,
        selection: Option<Vec<PathBuf>>,
        cancel: &AtomicBool,
        progress: impl Fn(String),
    ) -> Result<Catalog, String> {
        let videos: Vec<_> = if self.catalog.source == folder.to_string_lossy() {
            self.catalog
                .documents
                .iter()
                .filter(|d| d.video.is_some())
                .map(|d| d.path.clone())
                .collect()
        } else {
            Vec::new()
        };
        let images: Vec<_> = if self.catalog.source == folder.to_string_lossy() {
            self.catalog
                .documents
                .iter()
                .filter(|d| d.kind == "Imagen")
                .map(|d| d.path.clone())
                .collect()
        } else {
            Vec::new()
        };
        let mut catalog = self.index_selection(folder, selection, cancel, &progress)?;
        if !images.is_empty() {
            let text_notice = catalog.notice.clone();
            self.append_images(&mut catalog, &images, cancel, &progress)?;
            catalog.notice = format!("{text_notice} {}", catalog.notice);
        }
        if !videos.is_empty() {
            let previous_notice = catalog.notice.clone();
            let result = self.append_videos(&mut catalog, &videos, cancel, &progress);
            self.unload();
            result?;
            catalog.notice = format!("{previous_notice} {}", catalog.notice);
        }
        if catalog.documents.is_empty() && self.catalog.source != catalog.source {
            return Err("No hay archivos compatibles en esa carpeta.".into());
        }
        save_catalog(&self.root.join("app-data/catalog.json"), &catalog)?;
        self.catalog = catalog.clone();
        Ok(catalog)
    }
    pub fn import_images(
        &mut self,
        paths: Vec<PathBuf>,
        cancel: &AtomicBool,
        progress: impl Fn(String),
    ) -> Result<Catalog, String> {
        if paths.is_empty() || paths.len() > 32 {
            return Err("Elige entre 1 y 32 imágenes por lote.".into());
        }
        let mut catalog = if self.catalog.demo {
            Catalog {
                pipeline: PIPELINE.into(),
                source: paths[0]
                    .parent()
                    .unwrap_or(Path::new(""))
                    .to_string_lossy()
                    .into_owned(),
                demo: false,
                documents: Vec::new(),
                notice: String::new(),
                selection: None,
            }
        } else {
            self.catalog.clone()
        };
        self.append_images(&mut catalog, &paths, cancel, &progress)?;
        if !catalog.documents.iter().any(|d| paths.contains(&d.path)) {
            return Err("No se pudo procesar ninguna imagen. Revisa el formato, el tamaño y el acceso a los archivos.".into());
        }
        save_catalog(&self.root.join("app-data/catalog.json"), &catalog)?;
        self.catalog = catalog.clone();
        Ok(catalog)
    }
    pub fn import_videos(
        &mut self,
        paths: Vec<PathBuf>,
        cancel: &AtomicBool,
        progress: impl Fn(String),
    ) -> Result<Catalog, String> {
        if paths.is_empty() || paths.len() > 10 {
            return Err("Elige entre 1 y 10 videos por lote, de hasta 30 minutos cada uno.".into());
        }
        if paths
            .iter()
            .any(|p| !crate::video::is_video(p) || !p.is_file())
        {
            return Err("Selecciona videos locales compatibles que sigan disponibles.".into());
        }
        let mut catalog = if self.catalog.demo {
            Catalog {
                pipeline: PIPELINE.into(),
                source: paths[0]
                    .parent()
                    .unwrap_or(Path::new(""))
                    .to_string_lossy()
                    .into_owned(),
                demo: false,
                documents: Vec::new(),
                notice: String::new(),
                selection: Some(Vec::new()),
            }
        } else {
            self.catalog.clone()
        };
        let result = self.append_videos(&mut catalog, &paths, cancel, &progress);
        self.unload();
        result?;
        save_catalog(&self.root.join("app-data/catalog.json"), &catalog)?;
        self.catalog = catalog.clone();
        Ok(catalog)
    }
    fn append_videos(
        &mut self,
        catalog: &mut Catalog,
        paths: &[PathBuf],
        cancel: &AtomicBool,
        progress: &impl Fn(String),
    ) -> Result<(), String> {
        let mut reused = 0;
        let mut added = 0;
        let mut removed = 0;
        let mut seen = HashSet::new();
        for (index, path) in paths.iter().enumerate() {
            if !seen.insert(path) {
                continue;
            }
            if cancel.load(Ordering::Relaxed) {
                return Err("Preparación cancelada. Tu colección anterior se conserva.".into());
            }
            catalog.documents.retain(|d| d.path != *path);
            if !path.is_file() {
                removed += 1;
                continue;
            }
            let stamp = format!("{}:{}", crate::video::PROFILE, file_stamp(path));
            if let Some(old) = self
                .catalog
                .documents
                .iter()
                .find(|d| d.path == *path && d.video.is_some() && d.stamp == stamp)
            {
                catalog.documents.push(old.clone());
                reused += 1;
                continue;
            }
            self.unload(); // Never keep ASR and embedding weights loaded together.
            let prepared = crate::video::prepare(&self.root, path, &stamp, cancel, &|message| {
                progress(format!(
                    "Video {} de {} · {message}",
                    index + 1,
                    paths.len()
                ));
            })?;
            let title = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let used: usize = catalog.documents.iter().map(|d| d.segments.len()).sum();
            if used + prepared.passages.len() > MAX_SEGMENTS {
                return Err(
                    "La colección superaría 2.500 fragmentos. Tu índice anterior se conserva."
                        .into(),
                );
            }
            let total = prepared.passages.len();
            let mut segments = Vec::new();
            for (position, passage) in prepared.passages.into_iter().enumerate() {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Preparación cancelada. Tu colección anterior se conserva.".into());
                }
                progress(format!(
                    "Video {} de {} · Indexando voz {}/{total} · {title}",
                    index + 1,
                    paths.len(),
                    position + 1
                ));
                let vector = self
                    .model()?
                    .embed(&format!("title: {title} | text: {}", passage.text))?;
                segments.push(Segment {
                    text: passage.text,
                    line: position + 1,
                    vector,
                    cue: Some(passage.cue),
                });
            }
            if cancel.load(Ordering::Relaxed) {
                return Err("Preparación cancelada. Tu colección anterior se conserva.".into());
            }
            if stamp != format!("{}:{}", crate::video::PROFILE, file_stamp(path)) {
                return Err(format!(
                    "El video cambió durante la preparación: {title}. Vuelve a sincronizar."
                ));
            }
            catalog.documents.push(Document {
                title,
                path: path.clone(),
                kind: "Video".into(),
                segments,
                stamp,
                video: Some(prepared.info),
            });
            added += 1;
        }
        catalog.notice = format!(
            "Videos: {added} preparados, {reused} sin cambios, {removed} eliminados. Transcripción automática en español; tiempos aproximados."
        );
        Ok(())
    }
    fn append_images(
        &mut self,
        catalog: &mut Catalog,
        paths: &[PathBuf],
        cancel: &AtomicBool,
        progress: &impl Fn(String),
    ) -> Result<(), String> {
        self.unload();
        let mut vision = None;
        let mut added = 0;
        let mut reused = 0;
        let mut failed = 0;
        for (index, path) in paths.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return Err("Preparación cancelada. Tu colección anterior se conserva.".into());
            }
            progress(format!(
                "Imagen {} de {} · {}",
                index + 1,
                paths.len(),
                path.file_name().unwrap_or_default().to_string_lossy()
            ));
            let stamp = format!("{}:{}", crate::media::PROFILE, file_stamp(path));
            catalog.documents.retain(|d| d.path != *path);
            if let Some(old) = self.catalog.documents.iter().find(|d| {
                d.path == *path && d.kind == "Imagen" && d.stamp == stamp && path.is_file()
            }) {
                catalog.documents.push(old.clone());
                reused += 1;
                continue;
            }
            let prepared = match crate::media::prepare(path, &self.root) {
                Ok(image) => image,
                Err(_) => {
                    failed += 1;
                    continue;
                }
            };
            if vision.is_none() {
                vision = Some(Worker::start(&self.root, true)?);
            }
            let vector = vision.as_mut().unwrap().image(&prepared.0)?;
            if cancel.load(Ordering::Relaxed) {
                return Err("Preparación cancelada. Tu colección anterior se conserva.".into());
            }
            if stamp != format!("{}:{}", crate::media::PROFILE, file_stamp(path)) {
                failed += 1;
                continue;
            }
            catalog.documents.push(Document {
                title: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                path: path.clone(),
                kind: "Imagen".into(),
                segments: vec![Segment {
                    text: String::new(),
                    line: 0,
                    vector,
                    cue: None,
                }],
                stamp,
                video: None,
            });
            added += 1;
        }
        drop(vision);
        catalog.notice = format!(
            "Imágenes: {added} preparadas, {reused} sin cambios, {failed} omitidas. El codificador visual se cerró."
        );
        Ok(())
    }
    pub fn search(&mut self, query: &str, semantic: bool) -> Result<Vec<Hit>, String> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        if query.chars().count() > 500 {
            return Err("Acorta la consulta a menos de 500 caracteres.".into());
        }
        let query_lower = query.to_lowercase();
        let evidence_terms = crate::highlight::QueryTerms::new(query);
        let terms: Vec<String> = query_lower
            .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
            .filter(|s| s.len() > 1)
            .map(str::to_owned)
            .collect();
        let identifiers: Vec<&String> = terms
            .iter()
            .filter(|s| {
                s.chars().any(|c| c.is_ascii_digit()) && s.chars().any(|c| c.is_ascii_alphabetic())
            })
            .collect();
        let vector = if semantic && identifiers.is_empty() {
            Some(
                self.model()?
                    .embed(&format!("task: search result | query: {query}"))?,
            )
        } else {
            None
        };
        let mut hits = Vec::new();
        for doc in &self.catalog.documents {
            if !doc.path.is_file() || fs::File::open(&doc.path).is_err() {
                continue;
            }
            let title = doc.title.to_lowercase();
            let path = doc.path.to_string_lossy().to_lowercase();
            let all = doc
                .segments
                .iter()
                .map(|s| s.text.to_lowercase())
                .collect::<Vec<_>>()
                .join(" ");
            let combined = format!("{title} {path} {all}");
            if !identifiers.iter().all(|id| has_identifier(&combined, id)) {
                continue;
            }
            let name_match = title.contains(&query_lower) || path.contains(&query_lower);
            let mut best: Option<Hit> = None;
            for segment in &doc.segments {
                let text = segment.text.to_lowercase();
                let exact = name_match || text.contains(&query_lower);
                let coverage = terms
                    .iter()
                    .filter(|word| text.contains(word.as_str()) || title.contains(word.as_str()))
                    .count() as f32
                    / terms.len().max(1) as f32;
                let similarity = vector
                    .as_ref()
                    .map(|q| {
                        q.iter()
                            .zip(&segment.vector)
                            .map(|(a, b)| a * b)
                            .sum::<f32>()
                    })
                    .unwrap_or(0.0);
                if !semantic && !exact && coverage == 0.0 {
                    continue;
                }
                let score = if !identifiers.is_empty() {
                    2.0 + coverage
                } else if exact {
                    1.5 + coverage * 0.1
                } else if vector.is_some() {
                    similarity + 0.06 * coverage
                } else {
                    coverage
                };
                let hit = Hit {
                    document: doc.clone(),
                    snippet: segment.text.clone(),
                    line: segment.line,
                    literal: exact || !identifiers.is_empty(),
                    score,
                    similarity: vector.as_ref().map(|_| similarity),
                    time_ms: if name_match && evidence_terms.ranges(&segment.text).is_empty() {
                        None
                    } else {
                        segment.cue.as_ref().map(|cue| cue.focus(query))
                    },
                };
                if best.as_ref().is_none_or(|b| hit.relevance_cmp(b).is_gt()) {
                    best = Some(hit);
                }
            }
            if let Some(hit) = best {
                hits.push(hit);
            }
        }
        hits.sort_by(|a, b| {
            b.relevance_cmp(a)
                .then_with(|| a.document.path.cmp(&b.document.path))
        });
        hits.truncate(30);
        Ok(hits)
    }
    fn index(
        &mut self,
        folder: PathBuf,
        cancel: &AtomicBool,
        progress: impl Fn(String),
    ) -> Result<Catalog, String> {
        self.index_selection(folder, None, cancel, progress)
    }
    fn index_selection(
        &mut self,
        folder: PathBuf,
        selection: Option<Vec<PathBuf>>,
        cancel: &AtomicBool,
        progress: impl Fn(String),
    ) -> Result<Catalog, String> {
        let mut paths = Vec::new();
        let mut skipped = 0;
        collect(&folder, &mut paths, &mut skipped, cancel)?;
        if let Some(selected) = &selection {
            paths.retain(|path| selected.contains(path));
        }
        if cancel.load(Ordering::Relaxed) {
            return Err("Preparación cancelada. Tu colección anterior se conserva.".into());
        }
        paths.sort();
        paths.truncate(MAX_FILES);
        let mut catalog = Catalog {
            pipeline: PIPELINE.into(),
            source: folder.to_string_lossy().into_owned(),
            demo: false,
            documents: Vec::new(),
            notice: String::new(),
            selection,
        };
        let previous: std::collections::HashMap<PathBuf, Document> = self
            .catalog
            .documents
            .iter()
            .map(|d| (d.path.clone(), d.clone()))
            .collect();
        let mut segment_count = 0;
        let mut partial = 0;
        let mut reused = 0;
        let mut changed = 0;
        for (i, path) in paths.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                return Err("Preparación cancelada. Tu colección anterior se conserva.".into());
            }
            if segment_count >= MAX_SEGMENTS {
                partial += paths.len() - i;
                break;
            }
            progress(format!(
                "Preparando {} de {} · {}",
                i + 1,
                paths.len(),
                path.file_name().unwrap_or_default().to_string_lossy()
            ));
            let before_stamp = file_stamp(path);
            if let Some(old) = previous
                .get(path)
                .filter(|old| !before_stamp.is_empty() && old.stamp == before_stamp)
            {
                if segment_count + old.segments.len() <= MAX_SEGMENTS {
                    if fs::metadata(path).is_ok_and(|m| m.len() > 64 * 1024) {
                        partial += 1;
                    }
                    segment_count += old.segments.len();
                    catalog.documents.push(old.clone());
                    reused += 1;
                    continue;
                }
            }
            let mut bytes = Vec::new();
            if fs::File::open(path)
                .and_then(|f| f.take(64 * 1024).read_to_end(&mut bytes))
                .is_err()
            {
                skipped += 1;
                continue;
            }
            if bytes.contains(&0) {
                skipped += 1;
                continue;
            }
            let text = match String::from_utf8(bytes) {
                Ok(text) => text,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };
            if text.trim().is_empty() {
                skipped += 1;
                continue;
            }
            if fs::metadata(path)
                .map(|m| m.len() > 64 * 1024)
                .unwrap_or(false)
            {
                partial += 1;
            }
            let title = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let mut segments = Vec::new();
            let mut queue = chunks(&text, 700);
            while let Some((line, body)) = queue.pop() {
                if cancel.load(Ordering::Relaxed) {
                    return Err("Preparación cancelada. Tu colección anterior se conserva.".into());
                }
                if segment_count >= MAX_SEGMENTS {
                    partial += 1;
                    break;
                }
                progress(format!(
                    "Preparando {} de {} · {title} · fragmento {}",
                    i + 1,
                    paths.len(),
                    segments.len() + 1
                ));
                match self
                    .model()?
                    .embed(&format!("title: {title} | text: {body}"))
                {
                    Ok(vector) => {
                        segments.push(Segment {
                            text: body,
                            line,
                            vector,
                            cue: None,
                        });
                        segment_count += 1;
                    }
                    Err(error)
                        if error.contains("Token input too long") && body.chars().count() > 64 =>
                    {
                        let length = body.chars().count() / 2;
                        let smaller = chunks(&body, length)
                            .into_iter()
                            .map(|(offset, text)| (line + offset - 1, text));
                        queue.extend(smaller);
                    }
                    Err(error) => return Err(format!("{}: {error}", path.display())),
                }
            }
            segments.sort_by_key(|s| s.line);
            if file_stamp(path) != before_stamp {
                skipped += 1;
                continue;
            }
            if !segments.is_empty() {
                catalog.documents.push(Document {
                    title,
                    path: path.clone(),
                    kind: file_kind(path),
                    segments,
                    stamp: before_stamp,
                    video: None,
                });
                changed += 1;
            }
        }
        catalog.notice =
            format!("Sincronización: {changed} archivos preparados; {reused} sin cambios.");
        if skipped > 0 || partial > 0 || paths.len() >= MAX_FILES {
            catalog.notice += &format!(
                " {skipped} omitidos; {partial} parciales o pendientes. Límite de la vista previa: 500 archivos, 2.500 fragmentos y 64 KiB por archivo."
            );
        }
        Ok(catalog)
    }
}

fn file_stamp(path: &Path) -> String {
    fs::metadata(path)
        .and_then(|m| {
            m.modified().map(|t| {
                format!(
                    "{}:{}",
                    m.len(),
                    t.duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos()
                )
            })
        })
        .unwrap_or_default()
}
fn has_identifier(text: &str, id: &str) -> bool {
    text.split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .any(|token| token == id)
}
fn read_catalog(path: &Path) -> Result<Catalog, String> {
    let catalog: Catalog = serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if catalog.pipeline != PIPELINE
        || catalog
            .documents
            .iter()
            .flat_map(|d| &d.segments)
            .any(|s| s.vector.len() != 768 || s.vector.iter().any(|v| !v.is_finite()))
    {
        return Err(
            "El catálogo usa otra versión del modelo; necesita prepararse otra vez.".into(),
        );
    }
    Ok(catalog)
}
fn save_catalog(path: &Path, catalog: &Catalog) -> Result<(), String> {
    let temp = path.with_extension("pending.json");
    let mut file = fs::File::create(&temp).map_err(|e| e.to_string())?;
    serde_json::to_writer(&mut file, catalog).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    fs::rename(temp, path).map_err(|e| e.to_string())
}
pub fn chunks(text: &str, limit: usize) -> Vec<(usize, String)> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut count = 0;
    let mut line = 1;
    let mut start_line = 1;
    for (offset, ch) in text.char_indices() {
        if count >= limit {
            parts.push((start_line, text[start..offset].to_owned()));
            start = offset;
            count = 0;
            start_line = line;
        }
        count += 1;
        if ch == '\n' {
            line += 1;
        }
    }
    if start < text.len() {
        parts.push((start_line, text[start..].to_owned()));
    }
    parts.reverse();
    parts
}
fn file_kind(path: &Path) -> String {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "md" | "txt" | "csv" | "log" => "Texto",
        _ => "Código",
    }
    .into()
}
fn is_credential_file(name: &str) -> bool {
    name.ends_with(".json")
        && (name.starts_with("client_secret")
            || name.starts_with("credentials")
            || name.starts_with("service_account")
            || name.starts_with("service-account")
            || matches!(name, "token.json" | "oauth_token.json"))
}

fn collect(
    folder: &Path,
    files: &mut Vec<PathBuf>,
    skipped: &mut usize,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let exclusions: HashSet<&str> = [
        ".git",
        "node_modules",
        "target",
        ".next",
        "dist",
        "build",
        ".venv",
        "venv",
        "models",
        "tools",
        "app-data",
        "__pycache__",
    ]
    .into_iter()
    .collect();
    let mut stack = vec![folder.to_owned()];
    let mut seen = 0;
    while let Some(directory) = stack.pop() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(_) => {
                *skipped += 1;
                continue;
            }
        };
        for entry in entries.flatten() {
            seen += 1;
            if files.len() >= MAX_FILES || seen > 50000 {
                return Ok(());
            }
            let path = entry.path();
            let meta = match entry.metadata() {
                Ok(meta) => meta,
                Err(_) => continue,
            };
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & (0x400 | 0x1000 | 0x40000 | 0x400000) != 0 {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.starts_with('.') || name.contains("lock") || is_credential_file(&name) {
                *skipped += 1;
                continue;
            }
            if meta.is_dir() {
                if !exclusions.contains(name.as_str()) {
                    stack.push(path);
                }
                continue;
            }
            let extension = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if [
                "txt", "md", "rs", "py", "js", "ts", "tsx", "jsx", "json", "toml", "yaml", "yml",
                "css", "html", "csv", "log", "sql", "c", "cpp", "h",
            ]
            .contains(&extension.as_str())
            {
                files.push(path);
            } else {
                *skipped += 1;
            }
        }
    }
    Ok(())
}

pub fn spawn(
    root: PathBuf,
    ctx: eframe::egui::Context,
    cancel: Arc<AtomicBool>,
) -> (mpsc::Sender<Request>, mpsc::Receiver<Event>) {
    let (tx, commands) = mpsc::channel();
    let (events, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let emit = |event| {
            let _ = events.send(event);
            ctx.request_repaint();
        };
        let mut engine = match Engine::load(root) {
            Ok(engine) => engine,
            Err(error) => {
                emit(Event::Error(error));
                return;
            }
        };
        emit(Event::Catalog(engine.catalog.clone()));
        while let Ok(command) = commands.recv() {
            let result = match command {
                Request::Search(query, semantic) => {
                    emit(Event::Progress(
                        if semantic {
                            "Buscando en tus archivos…"
                        } else {
                            "Buscando coincidencias de texto…"
                        }
                        .into(),
                    ));
                    let start = Instant::now();
                    engine
                        .search(&query, semantic)
                        .map(|hits| emit(Event::Hits(hits, start.elapsed().as_millis())))
                }
                Request::Index(folder, all) => (if all {
                    engine
                        .synchronize_selection(folder, None, &cancel, |s| emit(Event::Progress(s)))
                } else {
                    engine.synchronize(folder, &cancel, |s| emit(Event::Progress(s)))
                })
                .and_then(|catalog| {
                    emit(Event::Catalog(catalog));
                    emit(Event::Ready("Carpeta lista para buscar".into()));
                    Ok(())
                }),
                Request::Demo => {
                    read_catalog(&engine.root.join("app-data/demo.json")).map(|catalog| {
                        engine.catalog = catalog.clone();
                        emit(Event::Catalog(catalog));
                        emit(Event::Ready("Ejemplos listos".into()));
                    })
                }
                Request::IndexImages(paths) => engine
                    .import_images(paths, &cancel, |s| emit(Event::Progress(s)))
                    .map(|catalog| {
                        emit(Event::Catalog(catalog));
                        emit(Event::Ready(
                            "Imágenes listas. Codificador visual cerrado.".into(),
                        ));
                    }),
                Request::IndexVideos(paths) => engine
                    .import_videos(paths, &cancel, |s| emit(Event::Progress(s)))
                    .map(|catalog| {
                        emit(Event::Catalog(catalog));
                        emit(Event::Ready(
                            "Videos listos para buscar. Voz e IA descargadas de memoria.".into(),
                        ));
                    }),
                Request::Unload => {
                    engine.unload();
                    emit(Event::Ready("IA descargada de la memoria. Volverá a cargar si haces una búsqueda semántica.".into()));
                    Ok(())
                }
                Request::Quit => break,
            };
            if let Err(error) = result {
                emit(Event::Error(error));
            }
            emit(Event::Model(engine.worker.is_some()));
        }
    });
    (tx, rx)
}

pub fn verify_synchronization(engine: &mut Engine) -> Result<(), String> {
    let folder = engine
        .root
        .join("app-data")
        .join(format!("sync-check-{}", std::process::id()));
    fs::create_dir(&folder).map_err(|e| e.to_string())?;
    let first = folder.join("invoice.txt");
    let second = folder.join("notes.md");
    fs::write(
        &first,
        "Factura TEST-100. Compra de un escritorio de madera.",
    )
    .map_err(|e| e.to_string())?;
    fs::write(
        &second,
        "Reunión el martes. Revisar el presupuesto de la oficina.",
    )
    .map_err(|e| e.to_string())?;
    let cancel = AtomicBool::new(false);
    engine.catalog = engine.index(folder.clone(), &cancel, |_| {})?;
    if engine.catalog.documents.len() != 2 {
        return Err("Sincronización inicial incompleta".into());
    }
    engine.unload();
    engine.catalog = engine.index(folder.clone(), &cancel, |_| {})?;
    if engine.worker.is_some() {
        return Err("Se recalcularon archivos que no cambiaron".into());
    }
    fs::write(
        &first,
        "Factura TEST-200. Compra de dos computadoras y tres monitores.",
    )
    .map_err(|e| e.to_string())?;
    fs::remove_file(&second).map_err(|e| e.to_string())?;
    engine.catalog = engine.index(folder.clone(), &cancel, |_| {})?;
    if engine.catalog.documents.len() != 1
        || engine.search("TEST-200", false)?.len() != 1
        || !engine.search("TEST-100", false)?.is_empty()
    {
        return Err("No se aplicaron modificaciones o eliminaciones".into());
    }
    let snapshot = engine.root.join("app-data/sync-check-catalog.json");
    save_catalog(&snapshot, &engine.catalog)?;
    save_catalog(&snapshot, &engine.catalog)?;
    if read_catalog(&snapshot)?.documents.len() != 1 {
        return Err("El índice no se conservó en disco".into());
    }
    cancel.store(true, Ordering::Relaxed);
    if engine.index(folder, &cancel, |_| {}).is_ok() {
        return Err("Se ignoró la cancelación".into());
    }
    if read_catalog(&snapshot)?.documents.len() != 1 {
        return Err("Cancelar alteró el índice anterior".into());
    }
    engine.unload();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn videos_reuse_persist_cancel_and_remove_without_losing_timestamp_evidence() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../app-data")
            .join(format!("video-sync-check-{}", std::process::id()));
        let folder = root.join("collection");
        fs::create_dir_all(&folder).unwrap();
        fs::create_dir(root.join("app-data")).unwrap();
        let path = folder.join("demo-system.mp4");
        fs::write(&path, b"already indexed fixture").unwrap();
        let document = Document {
            title: "demo-system.mp4".into(),
            path: path.clone(),
            kind: "Video".into(),
            stamp: format!("{}:{}", crate::video::PROFILE, file_stamp(&path)),
            video: Some(crate::video::VideoInfo {
                duration_ms: 12000,
                poster: None,
            }),
            segments: vec![Segment {
                text: "El contrato tiene condiciones adicionales.".into(),
                line: 1,
                vector: vec![0.0; 768],
                cue: Some(crate::video::Cue {
                    start_ms: 1000,
                    end_ms: 11000,
                    tokens: vec![
                        crate::video::TimedToken {
                            text: "El ".into(),
                            start_ms: 1000,
                            end_ms: 2000,
                        },
                        crate::video::TimedToken {
                            text: "contrato tiene condiciones adicionales.".into(),
                            start_ms: 2490,
                            end_ms: 11000,
                        },
                    ],
                }),
            }],
        };
        let mut engine = Engine {
            root: root.clone(),
            worker: None,
            catalog: Catalog {
                pipeline: PIPELINE.into(),
                source: folder.to_string_lossy().into_owned(),
                demo: false,
                documents: vec![document],
                notice: String::new(),
                selection: Some(Vec::new()),
            },
        };
        let cancel = AtomicBool::new(false);
        engine
            .import_videos(vec![path.clone(), path.clone()], &cancel, |_| {})
            .unwrap();
        assert_eq!(engine.catalog.documents.len(), 1);
        assert!(engine.worker.is_none());
        assert_eq!(
            engine.search("contrato", false).unwrap()[0].time_ms,
            Some(2490)
        );
        assert_eq!(
            engine.search("demo-system", false).unwrap()[0].time_ms,
            None
        );
        engine.synchronize(folder.clone(), &cancel, |_| {}).unwrap();
        let catalog_path = root.join("app-data/catalog.json");
        let before = fs::read(&catalog_path).unwrap();
        assert!(
            read_catalog(&catalog_path).unwrap().documents[0].segments[0]
                .cue
                .is_some()
        );
        cancel.store(true, Ordering::Relaxed);
        assert!(
            engine
                .import_videos(vec![path.clone()], &cancel, |_| {})
                .is_err()
        );
        assert_eq!(fs::read(&catalog_path).unwrap(), before);
        cancel.store(false, Ordering::Relaxed);
        fs::remove_file(path).unwrap();
        engine.synchronize(folder.clone(), &cancel, |_| {}).unwrap();
        assert!(engine.catalog.documents.is_empty());
        assert!(read_catalog(&catalog_path).unwrap().documents.is_empty());
        fs::remove_file(catalog_path).unwrap();
        fs::remove_dir(root.join("app-data")).unwrap();
        fs::remove_dir(folder).unwrap();
        fs::remove_dir(root).unwrap();
    }
    #[test]
    fn literal_ties_use_semantics_but_keep_literal_priority() {
        let literal = Hit {
            document: Document {
                title: "Comisiones".into(),
                path: PathBuf::from("z.md"),
                kind: "Texto".into(),
                segments: Vec::new(),
                stamp: String::new(),
                video: None,
            },
            snippet: "Se pagan comisiones cada viernes.".into(),
            line: 1,
            literal: true,
            score: 1.6,
            similarity: Some(0.8),
            time_ms: None,
        };
        let mut incidental = literal.clone();
        incidental.document.path = PathBuf::from("a.md");
        incidental.similarity = Some(0.4);
        assert!(literal.relevance_cmp(&incidental).is_gt());
        let mut semantic_only = literal.clone();
        semantic_only.literal = false;
        semantic_only.score = 0.99;
        semantic_only.similarity = Some(0.99);
        assert!(literal.relevance_cmp(&semantic_only).is_gt());
        incidental.score = literal.score;
        incidental.similarity = literal.similarity;
        assert!(literal.relevance_cmp(&incidental).is_eq());
    }
    #[test]
    fn selected_documents_remain_scoped_until_explicitly_expanded() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../app-data")
            .join(format!("selection-check-{}", std::process::id()));
        let folder = root.join("collection");
        fs::create_dir_all(&folder).unwrap();
        fs::create_dir(root.join("app-data")).unwrap();
        let documents: Vec<_> = ["selected.txt", "outside.txt"]
            .iter()
            .map(|name| {
                let path = folder.join(name);
                let contents = if *name == "selected.txt" {
                    "Documento de prueba".repeat(4000)
                } else {
                    "Documento de prueba".into()
                };
                fs::write(&path, contents).unwrap();
                Document {
                    title: name.to_string(),
                    stamp: file_stamp(&path),
                    video: None,
                    path,
                    kind: "Texto".into(),
                    segments: vec![Segment {
                        text: "Documento de prueba".into(),
                        line: 1,
                        vector: vec![0.0; 768],
                        cue: None,
                    }],
                }
            })
            .collect();
        let selection = Some(vec![documents[0].path.clone()]);
        let outside = documents[1].clone();
        let mut engine = Engine {
            root: root.clone(),
            worker: None,
            catalog: Catalog {
                pipeline: PIPELINE.into(),
                source: folder.to_string_lossy().into_owned(),
                demo: false,
                documents,
                notice: String::new(),
                selection: selection.clone(),
            },
        };
        let cancel = AtomicBool::new(false);
        let catalog = engine.synchronize(folder.clone(), &cancel, |_| {}).unwrap();
        assert_eq!(catalog.documents.len(), 1);
        assert_eq!(catalog.selection, selection);
        assert!(catalog.notice.contains("1 parciales o pendientes"));
        let saved = root.join("app-data/catalog.json");
        assert_eq!(read_catalog(&saved).unwrap().selection, selection);
        // Supply the already prepared second fixture to verify expansion without a model.
        engine.catalog.documents.push(outside);
        let catalog = engine
            .synchronize_selection(folder.clone(), None, &cancel, |_| {})
            .unwrap();
        assert_eq!(catalog.documents.len(), 2);
        assert!(catalog.selection.is_none());
        assert!(engine.worker.is_none());
        for document in catalog.documents {
            fs::remove_file(document.path).unwrap();
        }
        fs::remove_file(saved).unwrap();
        fs::remove_dir(folder).unwrap();
        fs::remove_dir(root.join("app-data")).unwrap();
        fs::remove_dir(root).unwrap();
    }
    #[test]
    fn image_sync_reuses_vectors_removes_deleted_files_and_preserves_on_cancel() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../app-data")
            .join(format!("image-sync-check-{}", std::process::id()));
        let source = root.join("collection");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir(root.join("app-data")).unwrap();
        let paths = [source.join("first.png"), source.join("second.jpg")];
        // Already indexed fixtures: reuse must not require decoding or model files.
        let documents = paths
            .iter()
            .map(|path| {
                fs::write(path, b"indexed fixture").unwrap();
                Document {
                    title: path.file_name().unwrap().to_string_lossy().into_owned(),
                    path: path.clone(),
                    kind: "Imagen".into(),
                    stamp: format!("{}:{}", crate::media::PROFILE, file_stamp(path)),
                    video: None,
                    segments: vec![Segment {
                        text: String::new(),
                        line: 0,
                        vector: vec![0.0; 768],
                        cue: None,
                    }],
                }
            })
            .collect();
        let mut engine = Engine {
            root: root.clone(),
            worker: None,
            catalog: Catalog {
                pipeline: PIPELINE.into(),
                source: source.to_string_lossy().into_owned(),
                demo: false,
                documents,
                notice: String::new(),
                // A removed text selection must not discard the surviving images.
                selection: Some(vec![source.join("deleted.txt")]),
            },
        };
        let cancel = AtomicBool::new(false);
        assert_eq!(
            engine
                .import_images(paths.to_vec(), &cancel, |_| {})
                .unwrap()
                .documents
                .len(),
            2
        );
        assert_eq!(
            engine
                .synchronize(source.clone(), &cancel, |_| {})
                .unwrap()
                .documents
                .len(),
            2
        );
        assert!(engine.worker.is_none());
        fs::remove_file(&paths[0]).unwrap();
        let catalog = engine.synchronize(source.clone(), &cancel, |_| {}).unwrap();
        assert_eq!(catalog.documents.len(), 1);
        assert_eq!(catalog.documents[0].path, paths[1]);
        let saved = root.join("app-data/catalog.json");
        let previous = fs::read(&saved).unwrap();
        let invalid = source.join("invalid.png");
        fs::write(&invalid, b"not an image").unwrap();
        assert!(
            engine
                .import_images(vec![invalid.clone()], &cancel, |_| {})
                .is_err()
        );
        assert_eq!(fs::read(&saved).unwrap(), previous);
        fs::remove_file(invalid).unwrap();
        cancel.store(true, Ordering::Relaxed);
        assert!(
            engine
                .import_images(vec![paths[1].clone()], &cancel, |_| {})
                .is_err()
        );
        assert_eq!(fs::read(&saved).unwrap(), previous);
        assert_eq!(read_catalog(&saved).unwrap().documents.len(), 1);
        fs::remove_file(&paths[1]).unwrap();
        fs::remove_file(saved).unwrap();
        fs::remove_dir(source).unwrap();
        fs::remove_dir(root.join("app-data")).unwrap();
        fs::remove_dir(root).unwrap();
    }
    #[test]
    fn collection_excludes_credential_files_but_keeps_documentation() {
        let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../app-data")
            .join(format!("credential-check-{}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        let secret = folder.join("client_secret_test.json");
        let article = folder.join("credentials-guide.md");
        fs::write(&secret, "{}").unwrap();
        fs::write(&article, "Documentación sin credenciales").unwrap();
        let mut files = Vec::new();
        collect(&folder, &mut files, &mut 0, &AtomicBool::new(false)).unwrap();
        assert_eq!(files, vec![article.clone()]);
        fs::remove_file(secret).unwrap();
        fs::remove_file(article).unwrap();
        fs::remove_dir(folder).unwrap();
    }
    #[test]
    fn unicode_chunks_preserve_text() {
        let text = "año\nniño 😊\ncontraseña";
        let mut c = chunks(text, 5);
        c.reverse();
        assert_eq!(c.iter().map(|(_, s)| s.as_str()).collect::<String>(), text);
        assert_eq!(c[0].0, 1);
        assert_eq!(c[1].0, 2);
    }
    #[test]
    fn identifier_requires_a_complete_token() {
        assert!(has_identifier("factura f-1042.pdf", "f-1042"));
        assert!(!has_identifier("factura f-10420", "f-1042"));
        assert!(!has_identifier("factura f-1043", "f-9999"));
    }
}

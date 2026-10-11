use crate::{
    appearance::{self, ACCENT, GREEN, Icon, MUTED, TEXT},
    backend::{self, Catalog, Document, Event, Hit, Request},
    highlight::QueryTerms,
};
use eframe::egui::{self, Color32, FontId, RichText, Stroke};
use std::collections::HashMap;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

struct SearchRow {
    title: String,
    path: PathBuf,
    kind: String,
    snippet: String,
    line: Option<usize>,
    literal: bool,
    similarity: Option<f32>,
    time_ms: Option<u64>,
    poster: Option<PathBuf>,
}
impl SearchRow {
    fn from_document(document: &Document) -> Self {
        Self {
            title: document.title.clone(),
            path: document.path.clone(),
            kind: document.kind.clone(),
            snippet: document
                .segments
                .first()
                .map(|s| s.text.chars().take(160).collect())
                .unwrap_or_default(),
            line: None,
            literal: false,
            similarity: None,
            time_ms: None,
            poster: document.video.as_ref().and_then(|v| v.poster.clone()),
        }
    }
    fn from_hit(hit: Hit) -> Self {
        Self {
            title: hit.document.title,
            path: hit.document.path,
            kind: hit.document.kind,
            snippet: hit.snippet,
            line: Some(hit.line),
            literal: hit.literal,
            similarity: hit.similarity,
            time_ms: hit.time_ms,
            poster: hit.document.video.as_ref().and_then(|v| v.poster.clone()),
        }
    }
}

// One interactive widget owns the complete row. Painted text cannot steal clicks
// for text selection; the document preview remains selectable.
fn result_row(
    ui: &mut egui::Ui,
    row: &SearchRow,
    active: bool,
    rank: Option<usize>,
    terms: &QueryTerms,
    thumbnails: &mut HashMap<PathBuf, Option<egui::TextureHandle>>,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 96.0), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, active, &row.title)
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let fade = ui
        .ctx()
        .animate_bool_with_time(response.id.with("selected"), active, 0.15);
    let hover = ui
        .ctx()
        .animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.1);
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    painter.rect_filled(
        rect,
        9.0,
        Color32::from_rgba_unmultiplied(89, 151, 144, (fade * 65.0 + hover * 14.0) as u8),
    );
    if active {
        painter.rect_filled(
            egui::Rect::from_min_size(
                rect.left_center() + egui::vec2(0.0, -13.0),
                egui::vec2(3.0, 26.0),
            ),
            2.0,
            GREEN,
        );
    }
    let thumb = egui::Rect::from_min_size(
        rect.left_top() + egui::vec2(14.0, 21.0),
        egui::vec2(36.0, 48.0),
    );
    if row.kind == "Imagen" || row.poster.is_some() {
        let image_path = row.poster.as_ref().unwrap_or(&row.path);
        let texture = thumbnails
            .entry(image_path.clone())
            .or_insert_with(|| crate::media::texture(ui.ctx(), image_path, 96).ok());
        if let Some(texture) = texture {
            let size = texture.size_vec2();
            let fit = size * (48.0 / size.x).min(55.0 / size.y);
            painter.image(
                texture.id(),
                egui::Rect::from_center_size(thumb.center(), fit),
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    } else {
        appearance::document_thumbnail(&painter, thumb, row.kind == "Código");
    }
    let snippet = if row.kind == "Imagen" {
        "Imagen · búsqueda visual local".into()
    } else {
        row.snippet.replace(['\n', '\r'], " ")
    };
    let ext = row
        .path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_uppercase();
    let relevance = if row.kind == "Video" && row.literal && row.time_ms.is_none() {
        "Nombre o ruta".into()
    } else if row.literal {
        "Coincidencia literal".into()
    } else if let Some(similarity) = row.similarity {
        format!("Similitud {similarity:.2}").replace('.', ",")
    } else {
        "Palabras coincidentes".into()
    };
    let meta = if let Some(rank) = rank {
        if let Some(time) = row.time_ms {
            format!(
                "#{rank}  ·  VIDEO ≈{}  ·  {relevance}",
                crate::video::timestamp(time)
            )
        } else {
            format!("#{rank}  ·  {ext}  ·  {relevance}")
        }
    } else {
        format!(
            "{ext}  ·  {}",
            row.path
                .parent()
                .and_then(|p| p.file_name())
                .unwrap_or_default()
                .to_string_lossy()
        )
    };
    let origin = rect.left_top() + egui::vec2(65.0, 12.0);
    for (text, size, color, y) in [
        (row.title.as_str(), 18.0, TEXT, 0.0),
        (snippet.as_str(), 14.0, MUTED, 28.0),
        (
            meta.as_str(),
            12.0,
            if active { GREEN } else { MUTED },
            53.0,
        ),
    ] {
        let mut job = if y == 53.0 {
            egui::text::LayoutJob::simple_singleline(text.into(), FontId::proportional(size), color)
        } else {
            terms.layout(text, FontId::proportional(size), color)
        };
        job.wrap.max_width = (rect.right() - origin.x - 10.0).max(0.0);
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
        painter.galley(origin + egui::vec2(0.0, y), galley, color);
    }
    let tooltip = if rank.is_some() {
        format!(
            "{}\n\n{relevance}\nOrden por relevancia: primero coincidencias literales; después similitud y coincidencias de palabras.\nLa similitud es una puntuación del modelo, no un porcentaje de certeza.",
            row.path.display()
        )
    } else {
        row.path.to_string_lossy().into_owned()
    };
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(tooltip)
}

struct PreviewBlock {
    text: String,
    line: usize,
    matched: bool,
    cue: Option<crate::video::Cue>,
}
struct Preview {
    path: PathBuf,
    title: String,
    extension: String,
    code: bool,
    blocks: Vec<PreviewBlock>,
    match_line: Option<usize>,
    literal: bool,
    is_image: bool,
    texture: Option<egui::TextureHandle>,
    video: Option<crate::video::VideoInfo>,
    time_ms: Option<u64>,
}

enum Selection {
    Folder(PathBuf),
    Images(Vec<PathBuf>),
    Videos(Vec<PathBuf>),
}

fn video_preview(
    ui: &mut egui::Ui,
    preview: &Preview,
    terms: &QueryTerms,
    query: &str,
    height: f32,
    scroll: &mut bool,
) -> Option<u64> {
    let mut play = None;
    egui::Frame::new().fill(Color32::from_white_alpha(7)).corner_radius(10).inner_margin(12).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.set_min_height(height - 24.0);
        egui::ScrollArea::vertical().id_salt(("video_transcript", &preview.path, query))
            .max_height(height - 24.0).auto_shrink([false, false]).show(ui, |ui| {
                if let Some(texture) = preview.texture.as_ref().filter(|_| query.is_empty()) {
                    let size = texture.size_vec2();
                    let fit = size * (ui.available_width() / size.x).min(105.0 / size.y);
                    ui.horizontal(|ui| {
                        ui.add(egui::Image::new(texture).fit_to_exact_size(fit).corner_radius(6));
                        ui.vertical(|ui| {
                            ui.label(RichText::new("VIDEO").size(12.0).color(MUTED));
                            ui.label(RichText::new(crate::video::timestamp(preview.video.as_ref().unwrap().duration_ms)).size(21.0));
                            ui.label(RichText::new("Miniatura inicial").size(12.0).color(MUTED));
                        });
                    });
                    ui.add_space(10.0);
                }
                ui.label(RichText::new("TRANSCRIPCIÓN AUTOMÁTICA").size(11.0).extra_letter_spacing(1.0).color(MUTED));
                ui.add_space(8.0);
                for block in &preview.blocks {
                    let Some(cue) = &block.cue else { continue; };
                    let at = if block.matched { preview.time_ms.unwrap_or(cue.start_ms) } else { cue.focus(query) };
                    let response = egui::Frame::new().fill(if block.matched { Color32::from_rgba_unmultiplied(89, 151, 144, 44) } else { Color32::TRANSPARENT })
                        .corner_radius(6).inner_margin(10).show(ui, |ui| {
                            if ui.add(egui::Button::new(RichText::new(format!("▶  ≈{}", crate::video::timestamp(at))).size(16.0).color(GREEN)).frame(false))
                                .on_hover_text("Reproducir con un segundo de contexto. El tiempo es aproximado; la transcripción puede contener errores.").clicked() { play = Some(at); }
                            ui.add(egui::Label::new(terms.layout(&block.text, FontId::proportional(18.0), TEXT)).wrap().selectable(true));
                        });
                    if block.matched && *scroll { response.response.scroll_to_me(Some(egui::Align::Center)); *scroll = false; }
                    ui.add_space(5.0);
                }
            });
    });
    play
}

#[cfg(test)]
mod tests {
    use super::*;

    // Feed real egui pointer events to the same widget used by the app. This
    // catches child labels intercepting clicks, unlike a rectangle-only test.
    struct RowHarness {
        ctx: egui::Context,
        time: f64,
    }
    impl RowHarness {
        fn frame(&mut self, events: Vec<egui::Event>) -> (egui::Rect, bool, bool, bool) {
            self.time += 0.06;
            let mut result = (egui::Rect::NOTHING, false, false, false);
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(380.0, 300.0),
                )),
                time: Some(self.time),
                events,
                ..Default::default()
            };
            let _ = self.ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let row = SearchRow {
                        title: "Comisiones y contratos".into(),
                        path: PathBuf::from("C:/prueba/contratos.md"),
                        kind: "Texto".into(),
                        snippet: "Las comisiones son semanales.".into(),
                        line: Some(8),
                        literal: false,
                        similarity: Some(0.72),
                        time_ms: None,
                        poster: None,
                    };
                    let response = result_row(
                        ui,
                        &row,
                        false,
                        Some(1),
                        &QueryTerms::new("comisiones"),
                        &mut HashMap::new(),
                    );
                    let next = result_row(
                        ui,
                        &row,
                        false,
                        Some(2),
                        &QueryTerms::new("comisiones"),
                        &mut HashMap::new(),
                    );
                    result = (
                        response.rect,
                        response.clicked(),
                        response.double_clicked(),
                        next.clicked(),
                    );
                });
            });
            result
        }
        fn click(&mut self, pos: egui::Pos2) -> (egui::Rect, bool, bool, bool) {
            self.frame(vec![egui::Event::PointerMoved(pos)]);
            self.frame(vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }]);
            self.frame(vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }])
        }
    }

    #[test]
    fn whole_result_row_responds_to_pointer_clicks() {
        for (label, offset) in [
            ("thumbnail", egui::vec2(30.0, 40.0)),
            ("title", egui::vec2(100.0, 22.0)),
            ("snippet", egui::vec2(100.0, 48.0)),
            ("metadata", egui::vec2(100.0, 73.0)),
            ("empty space", egui::vec2(345.0, 90.0)),
        ] {
            let mut harness = RowHarness {
                ctx: egui::Context::default(),
                time: 0.0,
            };
            let rect = harness.frame(vec![]).0;
            let result = harness.click(rect.min + offset);
            assert!(result.1, "Row failed to receive click over {label}");
            assert!(!result.3, "Click over {label} leaked into the next row");
        }
    }

    #[test]
    fn double_click_and_adjacent_row_keep_their_targets() {
        let mut harness = RowHarness {
            ctx: egui::Context::default(),
            time: 0.0,
        };
        let rect = harness.frame(vec![]).0;
        let pos = rect.min + egui::vec2(110.0, 22.0);
        assert!(harness.click(pos).1);
        assert!(harness.click(pos).2);
        let second = harness.click(rect.min + egui::vec2(110.0, 130.0));
        assert!(!second.1);
        assert!(second.3);
    }
}

pub struct SearchApp {
    root: PathBuf,
    player: Option<crate::video::Player>,
    query: String,
    searched: String,
    semantic: bool,
    catalog: Option<Catalog>,
    rows: Vec<SearchRow>,
    selected: Option<usize>,
    preview: Option<Preview>,
    scroll_to_match: bool,
    scroll_to_row: bool,
    busy: bool,
    indexing: bool,
    model_loaded: bool,
    status: String,
    error: Option<String>,
    elapsed: u128,
    acrylic: bool,
    first_frame: bool,
    commands: mpsc::Sender<Request>,
    events: mpsc::Receiver<Event>,
    cancel: Arc<AtomicBool>,
    folder_tx: mpsc::Sender<Option<Selection>>,
    folder_rx: mpsc::Receiver<Option<Selection>>,
    folder_dialog: bool,
    context: egui::Context,
    thumbnails: HashMap<PathBuf, Option<egui::TextureHandle>>,
    launch_query: Option<String>,
    #[cfg(feature = "ui-capture")]
    capture: crate::capture::Capture,
}

impl SearchApp {
    pub fn new(cc: &eframe::CreationContext<'_>, root: PathBuf) -> Self {
        appearance::configure(&cc.egui_ctx);
        let acrylic = appearance::native_backdrop(cc);
        let cancel = Arc::new(AtomicBool::new(false));
        let (commands, events) = backend::spawn(root.clone(), cc.egui_ctx.clone(), cancel.clone());
        let (folder_tx, folder_rx) = mpsc::channel();
        Self {
            root,
            player: None,
            query: String::new(),
            searched: String::new(),
            semantic: true,
            catalog: None,
            rows: Vec::new(),
            selected: None,
            preview: None,
            scroll_to_match: false,
            scroll_to_row: false,
            busy: false,
            indexing: false,
            model_loaded: false,
            status: "Abriendo colección…".into(),
            error: None,
            elapsed: 0,
            acrylic,
            first_frame: true,
            commands,
            events,
            cancel,
            folder_tx,
            folder_rx,
            folder_dialog: false,
            context: cc.egui_ctx.clone(),
            thumbnails: HashMap::new(),
            launch_query: {
                let args: Vec<_> = std::env::args().collect();
                args.iter()
                    .position(|a| a == "--search")
                    .and_then(|i| args.get(i + 1))
                    .cloned()
            },
            #[cfg(feature = "ui-capture")]
            capture: crate::capture::Capture::from_args(),
        }
    }

    fn send(&mut self, request: Request) {
        self.error = None;
        self.busy = true;
        if self.commands.send(request).is_err() {
            self.error = Some("El motor está cerrado. Vuelve a abrir la aplicación.".into());
            self.busy = false;
            self.indexing = false;
        }
    }

    fn search(&mut self) {
        if self.busy || self.catalog.is_none() {
            return;
        }
        if self.query.trim().is_empty() {
            self.browse();
            return;
        }
        self.searched = self.query.trim().to_owned();
        self.selected = None;
        self.rows.clear();
        self.preview = None;
        self.status = "Buscando en tu colección…".into();
        self.send(Request::Search(self.searched.clone(), self.semantic));
    }

    fn browse(&mut self) {
        self.searched.clear();
        self.rows = self
            .catalog
            .as_ref()
            .map(|c| c.documents.iter().map(SearchRow::from_document).collect())
            .unwrap_or_default();
        self.selected = None;
        self.preview = None;
        if !self.rows.is_empty() {
            self.select(0);
        }
    }

    fn select(&mut self, index: usize) {
        let Some(row) = self.rows.get(index) else {
            return;
        };
        let Some(document) = self
            .catalog
            .as_ref()
            .and_then(|c| c.documents.iter().find(|d| d.path == row.path))
        else {
            return;
        };
        self.preview = Some(Preview {
            path: document.path.clone(),
            title: document.title.clone(),
            code: document.kind == "Código",
            extension: document
                .path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_uppercase(),
            blocks: document
                .segments
                .iter()
                .map(|s| PreviewBlock {
                    text: s.text.clone(),
                    line: s.line,
                    matched: row.line == Some(s.line) && s.text == row.snippet,
                    cue: s.cue.clone(),
                })
                .collect(),
            match_line: row.line,
            literal: row.literal,
            is_image: document.kind == "Imagen",
            video: document.video.clone(),
            time_ms: row.time_ms,
            texture: if let Some(video) = &document.video {
                video
                    .poster
                    .as_ref()
                    .and_then(|path| crate::media::texture(&self.context, path, 640).ok())
            } else if document.kind == "Imagen" {
                crate::media::texture(&self.context, &document.path, 1280).ok()
            } else {
                None
            },
        });
        self.selected = Some(index);
        self.scroll_to_match = self.preview.as_ref().is_some_and(|p| {
            p.blocks
                .iter()
                .position(|b| b.matched)
                .is_some_and(|index| index > 0)
        });
    }

    fn choose_folder(&mut self, ctx: &egui::Context) {
        if self.folder_dialog || self.busy {
            return;
        }
        self.folder_dialog = true;
        let tx = self.folder_tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let folder = rfd::FileDialog::new()
                .set_title("Elige la carpeta que quieres sincronizar")
                .pick_folder()
                .map(Selection::Folder);
            let _ = tx.send(folder);
            ctx.request_repaint();
        });
    }
    fn choose_images(&mut self, ctx: &egui::Context) {
        if self.folder_dialog || self.busy {
            return;
        }
        self.folder_dialog = true;
        let tx = self.folder_tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let images = rfd::FileDialog::new()
                .set_title("Añadir imágenes al buscador (hasta 32 por lote)")
                .add_filter("Imágenes", &["png", "jpg", "jpeg", "webp", "bmp"])
                .pick_files()
                .map(Selection::Images);
            let _ = tx.send(images);
            ctx.request_repaint();
        });
    }
    fn choose_videos(&mut self, ctx: &egui::Context) {
        if self.folder_dialog || self.busy {
            return;
        }
        self.folder_dialog = true;
        let tx = self.folder_tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let videos = rfd::FileDialog::new()
                .set_title("Añadir videos · hasta 10 por lote, de hasta 30 minutos")
                .add_filter("Videos", &["mp4", "mov", "mkv", "webm", "avi", "m4v"])
                .pick_files()
                .map(Selection::Videos);
            let _ = tx.send(videos);
            ctx.request_repaint();
        });
    }

    fn synchronize(&mut self, folder: PathBuf, all: bool) {
        self.cancel.store(false, Ordering::Relaxed);
        self.indexing = true;
        self.status = "Revisando los archivos de la carpeta…".into();
        self.send(Request::Index(folder, all));
    }

    fn active_folder(&self) -> Option<PathBuf> {
        self.catalog
            .as_ref()
            .filter(|c| !c.demo)
            .map(|c| PathBuf::from(&c.source))
    }

    fn poll(&mut self) {
        if let Some(result) = self.player.as_mut().and_then(|p| p.finished()) {
            self.player = None;
            if let Err(error) = result {
                self.error = Some(error);
            }
        }
        while let Ok(folder) = self.folder_rx.try_recv() {
            self.folder_dialog = false;
            if let Some(selection) = folder {
                match selection {
                    Selection::Folder(folder) => self.synchronize(folder, true),
                    Selection::Images(paths) => {
                        self.cancel.store(false, Ordering::Relaxed);
                        self.indexing = true;
                        self.status = "Preparando el codificador visual local…".into();
                        self.send(Request::IndexImages(paths));
                    }
                    Selection::Videos(paths) => {
                        self.cancel.store(false, Ordering::Relaxed);
                        self.indexing = true;
                        self.status = "Preparando transcripción local de los videos…".into();
                        self.send(Request::IndexVideos(paths));
                    }
                }
            }
        }
        while let Ok(event) = self.events.try_recv() {
            match event {
                Event::Catalog(catalog) => {
                    self.status = format!("{} archivos disponibles", catalog.documents.len());
                    self.catalog = Some(catalog);
                    self.thumbnails.clear();
                    self.query.clear();
                    self.browse();
                }
                Event::Progress(status) => self.status = status,
                Event::Hits(hits, elapsed) => {
                    self.rows = hits.into_iter().map(SearchRow::from_hit).collect();
                    self.elapsed = elapsed;
                    self.busy = false;
                    self.status = "Búsqueda terminada".into();
                    if !self.rows.is_empty() {
                        self.select(0);
                    }
                }
                Event::Ready(status) => {
                    self.status = status;
                    self.busy = false;
                    self.indexing = false;
                }
                Event::Error(error) => {
                    self.error = Some(error);
                    self.busy = false;
                    self.indexing = false;
                    self.status = "No se completó la operación".into();
                }
                Event::Model(loaded) => self.model_loaded = loaded,
            }
        }
    }

    fn header(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("search_header")
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 5)))
            .show(ctx, |ui| {
                ui.add_space(6.0);
                let mut submit = false;
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(31.0, 54.0), egui::Sense::hover());
                    appearance::icon(ui.painter(), rect.center(), Icon::Search, ACCENT, 26.0);
                    let input = ui.add_sized(
                        [ui.available_width() - 94.0, 54.0],
                        egui::TextEdit::singleline(&mut self.query)
                            .id(egui::Id::new("search_input"))
                            .hint_text("Encuentra lo que recuerdas…")
                            .font(FontId::proportional(27.0))
                            .frame(false)
                            .margin(egui::vec2(7.0, 9.0))
                            .char_limit(500),
                    );
                    if self.first_frame {
                        input.request_focus();
                        self.first_frame = false;
                    }
                    if input.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        submit = true;
                    }
                    if input.changed() && self.query.is_empty() && !self.busy {
                        self.browse();
                    }
                    if !self.query.is_empty() {
                        if ui
                            .add_enabled_ui(!self.busy, |ui| {
                                appearance::icon_button(ui, Icon::Close, "Borrar búsqueda")
                            })
                            .inner
                            .clicked()
                        {
                            self.query.clear();
                            self.browse();
                            input.request_focus();
                        }
                    } else {
                        ui.add_space(34.0);
                    }
                    let response = ui
                        .add_enabled_ui(!self.busy && self.catalog.is_some(), |ui| {
                            appearance::icon_button(ui, Icon::Arrow, "Buscar · Enter")
                        })
                        .inner;
                    if response.clicked() {
                        submit = true;
                    }
                });
                if submit {
                    self.search();
                }
                ui.add_space(5.0);
                ui.separator();
                ui.horizontal(|ui| {
                    let folder_name = self
                        .catalog
                        .as_ref()
                        .map(|c| {
                            if c.demo {
                                "Ejemplos".to_owned()
                            } else {
                                PathBuf::from(&c.source)
                                    .file_name()
                                    .map(|s| s.to_string_lossy().into_owned())
                                    .unwrap_or("Mi carpeta".into())
                            }
                        })
                        .unwrap_or("Colección".into());
                    let short_name: String = folder_name.chars().take(34).collect();
                    ui.menu_button(format!("{short_name}  ▾"), |ui| {
                        ui.set_min_width(265.0);
                        ui.label(RichText::new("COLECCIÓN").small().color(MUTED));
                        if let Some(catalog) = &self.catalog {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&catalog.source).small().color(MUTED),
                                )
                                .wrap(),
                            );
                            ui.label(
                                RichText::new(format!("{} archivos", catalog.documents.len()))
                                    .small(),
                            );
                            if let Some(selection) = &catalog.selection {
                                ui.label(
                                    RichText::new(format!(
                                        "Prueba acotada: {} documentos + imágenes y videos añadidos",
                                        selection.len()
                                    ))
                                    .small()
                                    .color(MUTED),
                                );
                                ui.label(
                                    RichText::new("Elegir carpeta permite ampliar la colección.")
                                        .small()
                                        .color(MUTED),
                                );
                            }
                        }
                        ui.separator();
                        if ui
                            .add_enabled(
                                !self.busy && !self.folder_dialog,
                                egui::Button::new("Elegir carpeta…"),
                            )
                            .clicked()
                        {
                            self.choose_folder(ctx);
                            ui.close();
                        }
                        if ui
                            .add_enabled(
                                !self.busy && !self.folder_dialog,
                                egui::Button::new("Añadir imágenes…"),
                            )
                            .clicked()
                        {
                            self.choose_images(ctx);
                            ui.close();
                        }
                        if ui
                            .add_enabled(
                                !self.busy && !self.folder_dialog,
                                egui::Button::new("Añadir videos…"),
                            )
                            .clicked()
                        {
                            self.choose_videos(ctx);
                            ui.close();
                        }
                        if ui
                            .add_enabled(!self.busy, egui::Button::new("Probar los ejemplos"))
                            .clicked()
                        {
                            self.send(Request::Demo);
                            ui.close();
                        }
                        if let Some(catalog) = &self.catalog {
                            if !catalog.notice.is_empty() {
                                ui.separator();
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(&catalog.notice).small().color(MUTED),
                                    )
                                    .wrap(),
                                );
                            }
                        }
                    });
                    let folder = self.active_folder();
                    let sync = ui
                        .add_enabled_ui(!self.busy && folder.is_some(), |ui| {
                            appearance::icon_button(ui, Icon::Sync, "Sincronizar cambios · F5")
                        })
                        .inner;
                    if sync.clicked() {
                        self.synchronize(folder.unwrap(), false);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let more = appearance::icon_button(ui, Icon::More, "Opciones");
                        egui::Popup::menu(&more).show(|ui| {
                            ui.set_min_width(260.0);
                            ui.label(RichText::new("IA LOCAL").small().color(MUTED));
                            ui.label(if self.model_loaded {
                                "Modelo cargado"
                            } else {
                                "Modelo bajo demanda"
                            });
                            if ui
                                .add_enabled(
                                    !self.busy && self.model_loaded,
                                    egui::Button::new("Liberar memoria de la IA"),
                                )
                                .clicked()
                            {
                                self.send(Request::Unload);
                                ui.close();
                            }
                            ui.separator();
                            ui.label(RichText::new("FORMATOS DISPONIBLES").small().color(MUTED));
                            ui.label("Texto, Markdown, CSV, código, imágenes y voz de videos");
                            ui.label(
                                RichText::new("PDF, Office y búsqueda visual en videos: pendientes.")
                                    .small()
                                    .color(MUTED),
                            );
                            ui.separator();
                            ui.label(
                                RichText::new(if self.acrylic {
                                    "Fondo: Acrylic de Windows"
                                } else {
                                    "Fondo: opaco compatible"
                                })
                                .small()
                                .color(MUTED),
                            );
                            ui.label(
                                RichText::new("Ctrl + L  Buscar     Ctrl + Enter  Abrir")
                                    .small()
                                    .color(MUTED),
                            );
                        });
                        let label = if self.semantic {
                            "Por significado  ▾"
                        } else {
                            "Solo texto  ▾"
                        };
                        ui.menu_button(label, |ui| {
                            ui.add_enabled_ui(!self.busy, |ui| {
                                let before = self.semantic;
                                ui.selectable_value(
                                    &mut self.semantic,
                                    true,
                                    "Por significado · IA local",
                                );
                                ui.selectable_value(
                                    &mut self.semantic,
                                    false,
                                    "Solo texto · sin IA",
                                );
                                if before != self.semantic {
                                    if !self.query.trim().is_empty() {
                                        self.search();
                                    }
                                    ui.close();
                                }
                            });
                        });
                    });
                });
            });
    }

    fn footer(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("footer")
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(24, 9)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (dot, _) =
                        ui.allocate_exact_size(egui::vec2(8.0, 16.0), egui::Sense::hover());
                    ui.painter().circle_filled(
                        dot.center(),
                        3.0,
                        if self.busy { ACCENT } else { GREEN },
                    );
                    if self.busy {
                        ui.add(egui::Spinner::new().size(14.0).color(ACCENT));
                    }
                    let status = if self.busy {
                        self.status.clone()
                    } else if !self.searched.is_empty() {
                        format!("{} resultados · {} ms", self.rows.len(), self.elapsed)
                    } else {
                        format!("{} archivos · Todo en tu equipo", self.rows.len())
                    };
                    ui.add(
                        egui::Label::new(RichText::new(status).size(13.0).color(MUTED)).truncate(),
                    )
                    .on_hover_text(&self.status);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (rect, response) =
                            ui.allocate_exact_size(egui::vec2(12.0, 16.0), egui::Sense::drag());
                        ui.painter().line_segment(
                            [
                                rect.right_bottom() - egui::vec2(7.0, 0.0),
                                rect.right_bottom() - egui::vec2(0.0, 7.0),
                            ],
                            Stroke::new(1.2, MUTED),
                        );
                        if response.hovered() {
                            ctx.set_cursor_icon(egui::CursorIcon::ResizeNwSe);
                        }
                        if response.drag_started() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(
                                egui::ResizeDirection::SouthEast,
                            ));
                        }
                        ui.label(RichText::new("↑ ↓  Explorar").size(12.0).color(MUTED));
                        if self.indexing && ui.small_button("Cancelar").clicked() {
                            self.cancel.store(true, Ordering::Relaxed);
                            self.status = "Cancelando al terminar el fragmento…".into();
                        }
                    });
                });
            });
    }

    fn results(&mut self, ctx: &egui::Context) {
        let width = ctx.content_rect().width() * 0.5;
        let terms = QueryTerms::new(&self.searched);
        egui::SidePanel::left("results")
            .exact_width(width)
            .resizable(false)
            .frame(egui::Frame::new().inner_margin(egui::Margin::same(14)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(if self.searched.is_empty() {
                            "TU COLECCIÓN"
                        } else {
                            "POR RELEVANCIA"
                        })
                        .size(11.0)
                        .extra_letter_spacing(1.3)
                        .color(MUTED),
                    );
                    ui.label(
                        RichText::new(format!("{}", self.rows.len()))
                            .size(11.0)
                            .color(MUTED),
                    );
                });
                ui.add_space(5.0);
                if self.searched.is_empty() && self.catalog.as_ref().is_some_and(|c| c.demo) {
                    egui::Frame::new()
                        .inner_margin(egui::Margin::symmetric(8, 5))
                        .show(ui, |ui| {
                            ui.label(RichText::new("Prueba una idea").size(15.0).color(MUTED));
                            if ui.button("hotel con desayuno en Chicago  ↗").clicked() && !self.busy
                            {
                                self.query = "hotel con desayuno en Chicago".into();
                                self.search();
                            }
                        });
                    ui.add_space(8.0);
                }
                if self.rows.is_empty() {
                    ui.add_space(26.0);
                    ui.label(
                        RichText::new(if self.busy {
                            "Buscando conexiones…"
                        } else {
                            "Sin coincidencias"
                        })
                        .size(22.0),
                    );
                    ui.add_space(7.0);
                    ui.label(
                        RichText::new(if self.busy {
                            "La primera búsqueda carga la IA local."
                        } else {
                            "Prueba otra descripción o sincroniza tu carpeta."
                        })
                        .size(16.0)
                        .color(MUTED),
                    );
                }
                let mut clicked = None;
                let mut open = false;
                egui::ScrollArea::vertical()
                    .id_salt("result_list")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (index, row) in self.rows.iter().enumerate() {
                            let response = result_row(
                                ui,
                                row,
                                self.selected == Some(index),
                                (!self.searched.is_empty()).then_some(index + 1),
                                &terms,
                                &mut self.thumbnails,
                            );
                            if self.selected == Some(index) && self.scroll_to_row {
                                response.scroll_to_me(Some(egui::Align::Center));
                                self.scroll_to_row = false;
                            }
                            if response.clicked() {
                                clicked = Some(index);
                            }
                            if response.double_clicked() {
                                clicked = Some(index);
                                open = true;
                            }
                        }
                    });
                if let Some(index) = clicked {
                    self.select(index);
                }
                if open {
                    self.open_selected();
                }
            });
    }

    fn open_selected(&mut self) {
        if let Some(preview) = &self.preview {
            if preview.video.is_some() {
                let time = preview.time_ms.unwrap_or(0);
                self.play_video(time);
                return;
            }
        }
        if let Some(preview) = &self.preview {
            if preview.is_image && crate::media::is_image(&preview.path) {
                use std::os::windows::ffi::OsStrExt;
                use windows_sys::Win32::UI::{
                    Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL,
                };
                let path: Vec<u16> = preview
                    .path
                    .as_os_str()
                    .encode_wide()
                    .chain(Some(0))
                    .collect();
                let result = unsafe {
                    ShellExecuteW(
                        std::ptr::null_mut(),
                        std::ptr::null(),
                        path.as_ptr(),
                        std::ptr::null(),
                        std::ptr::null(),
                        SW_SHOWNORMAL,
                    )
                };
                if result as isize <= 32 {
                    self.error =
                        Some("No se pudo abrir la imagen con la aplicación predeterminada.".into());
                }
                return;
            }
            if let Err(error) = std::process::Command::new("notepad.exe")
                .arg(&preview.path)
                .spawn()
            {
                self.error = Some(error.to_string());
            }
        }
    }
    fn play_video(&mut self, at_ms: u64) {
        let Some(preview) = &self.preview else {
            return;
        };
        self.player = None;
        match crate::video::play(&self.root, &preview.path, at_ms) {
            Ok(player) => {
                self.player = Some(player);
                self.status = format!(
                    "Reproduciendo desde ≈{} · Espacio: pausa · Esc: cerrar",
                    crate::video::timestamp(at_ms)
                );
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn preview_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().frame(egui::Frame::new().fill(Color32::from_black_alpha(18)).inner_margin(egui::Margin::symmetric(32, 22))).show(ctx, |ui| {
            if let Some(error) = &self.error.clone() {
                egui::Frame::new().fill(Color32::from_rgb(71, 43, 42)).corner_radius(8).inner_margin(12).show(ui, |ui| {
                    ui.horizontal(|ui| { ui.add(egui::Label::new(RichText::new(error).size(15.0).color(Color32::from_rgb(255, 205, 190))).wrap()); if ui.small_button("×").clicked() { self.error = None; } });
                });
                ui.add_space(10.0);
            }
            let Some(preview) = &self.preview else {
                ui.add_space(ui.available_height() * 0.28);
                ui.vertical_centered(|ui| {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(52.0, 58.0), egui::Sense::hover());
                    appearance::icon(ui.painter(), rect.center(), Icon::Search, GREEN, 40.0);
                    ui.add_space(15.0);
                    ui.label(RichText::new(if self.busy { "Un momento…" } else { "Tu próximo hallazgo" }).size(28.0));
                    ui.add_space(5.0);
                    ui.label(RichText::new("Selecciona un archivo para verlo aquí.").size(17.0).color(MUTED));
                });
                return;
            };
            let mut open = false;
            let mut play_at = None;
            let mut stop_player = false;
            let mut reveal = false;
            let mut copy = false;
            ui.horizontal(|ui| {
                ui.label(RichText::new("VISTA PREVIA").size(11.0).extra_letter_spacing(1.4).color(MUTED))
                    .on_hover_text("Contenido guardado en el índice local. Sincroniza para reflejar cambios del archivo.\n\nSubrayado: palabras de la consulta presentes en el texto (sin distinguir mayúsculas ni tildes). Fondo: fragmento relacionado sin esas palabras. No es una explicación interna del modelo.");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    copy = appearance::icon_button(ui, Icon::Copy, "Copiar ruta").clicked();
                    reveal = appearance::icon_button(ui, Icon::Folder, "Ver en carpeta").clicked();
                });
            });
            ui.add_space(8.0);
            let paper_height = (ui.available_height() - 190.0).max(180.0);
            let code = preview.code;
            let terms = QueryTerms::new(&self.searched);
            let paper = if code { Color32::from_rgb(25, 34, 37) } else { Color32::from_rgb(237, 238, 226) };
            let ink = if code { Color32::from_rgb(211, 225, 221) } else { Color32::from_rgb(43, 60, 55) };
            let soft_ink = if code { Color32::from_rgb(125, 157, 150) } else { Color32::from_rgb(109, 129, 117) };
            if preview.video.is_some() {
                play_at = video_preview(ui, preview, &terms, &self.searched, paper_height, &mut self.scroll_to_match);
            } else if preview.is_image {
                ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), paper_height), egui::Layout::top_down(egui::Align::Center), |ui| {
                    if let Some(texture) = &preview.texture {
                        let dimensions = texture.size_vec2();
                        let fit = dimensions * (ui.available_width() / dimensions.x).min(paper_height / dimensions.y);
                        ui.add_space(((paper_height - fit.y) / 2.0).max(0.0));
                        ui.add(egui::Image::new(texture).fit_to_exact_size(fit).corner_radius(8));
                    } else { ui.label("No se pudo leer la imagen original."); }
                });
            } else {
            egui::Frame::new().fill(paper).corner_radius(10).inner_margin(egui::Margin::symmetric(30, 20))
                .stroke(Stroke::new(1.0, Color32::from_white_alpha(22)))
                .shadow(egui::Shadow { offset: [0, 5], blur: 18, spread: 0, color: Color32::from_black_alpha(42) })
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.set_min_height(paper_height - 40.0);
                    ui.set_max_height(paper_height - 40.0);
                    ui.visuals_mut().override_text_color = Some(ink);
                    ui.visuals_mut().selection.bg_fill = Color32::from_rgb(186, 207, 183);
                    ui.visuals_mut().selection.stroke = Stroke::new(1.0, ink);
                    egui::ScrollArea::vertical().id_salt(("document", &preview.path, &self.searched))
                        .auto_shrink([false, false]).max_height(paper_height - 40.0).show(ui, |ui| {
                            ui.label(RichText::new(format!("{}  /  DOCUMENTO LOCAL", preview.extension)).size(10.0).extra_letter_spacing(1.1).color(soft_ink));
                            ui.add_space(12.0);
                            ui.add(egui::Label::new(terms.layout(&preview.title, FontId::proportional(27.0), ink)).wrap());
                            ui.add_space(16.0);
                            let start = ui.cursor().min;
                            ui.painter().line_segment([start, start + egui::vec2(38.0, 0.0)], Stroke::new(2.0, soft_ink));
                            ui.add_space(15.0);
                            for block in &preview.blocks {
                                let response = egui::Frame::new().fill(if terms.fragment_fallback(&block.text, block.matched) {
                                    if code { Color32::from_rgb(39, 60, 59) } else { Color32::from_rgb(218, 228, 202) }
                                } else { Color32::TRANSPARENT }).corner_radius(5).inner_margin(egui::Margin::symmetric(10, 9)).show(ui, |ui| {
                                    if code {
                                        ui.label(RichText::new(format!("LÍNEA {}", block.line)).size(10.0).color(soft_ink));
                                        ui.add(egui::Label::new(terms.layout(&block.text, FontId::monospace(16.0), ink)).wrap().selectable(true));
                                    } else {
                                        for line in block.text.split('\n') {
                                            if line.trim().is_empty() { ui.add_space(9.0); continue; }
                                            let heading = if preview.extension == "MD" { line.starts_with('#') } else { false };
                                            let text = if heading { line.trim_start_matches('#').trim_start() } else { line };
                                            ui.add(egui::Label::new(terms.layout(text, FontId::proportional(if heading { 23.0 } else { 19.0 }), ink)).wrap().selectable(true));
                                        }
                                    }
                                });
                                if block.matched && self.scroll_to_match { response.response.scroll_to_me(Some(egui::Align::Center)); self.scroll_to_match = false; }
                                ui.add_space(4.0);
                            }
                        });
                });
            }
            ui.add_space(18.0);
            ui.add(egui::Label::new(RichText::new(&preview.title).size(24.0).color(TEXT)).wrap());
            ui.add(egui::Label::new(RichText::new(preview.path.to_string_lossy()).size(16.0).color(MUTED)).truncate()).on_hover_text(preview.path.to_string_lossy());
            ui.add_space(6.0);
            let detail = if let Some(video) = &preview.video {
                format!("Voz en español · {} · tiempos aproximados", crate::video::timestamp(video.duration_ms))
            } else if preview.is_image { format!("{} · Contenido visual", preview.extension) }
            else if let Some(line) = preview.match_line { format!("{} · línea {line}", if preview.literal { "Coincidencia de texto" } else { "Relacionado por significado" }) }
            else { format!("{} · Contenido indexado", preview.extension) };
            ui.add(egui::Label::new(RichText::new(detail).size(17.0).color(GREEN)).truncate());
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if self.player.is_some() { stop_player = ui.button("Detener video").clicked(); }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = if preview.video.is_some() { format!("▶  Reproducir ≈{}", crate::video::timestamp(preview.time_ms.unwrap_or(0))) } else { "Abrir  ↗".into() };
                    open = ui.add(egui::Button::new(RichText::new(label).size(18.0).color(TEXT)).fill(Color32::from_white_alpha(14)))
                        .on_hover_text(if preview.video.is_some() { "Abrir reproductor desde este momento · Ctrl + Enter. Espacio: pausa; Esc: cerrar." } else if preview.is_image { "Abrir imagen · Ctrl + Enter" } else { "Abrir como texto en Bloc de notas · Ctrl + Enter" }).clicked();
                });
            });
            let path = preview.path.clone();
            if copy { ctx.copy_text(path.to_string_lossy().into_owned()); self.status = "Ruta copiada".into(); }
            if reveal {
                use std::os::windows::process::CommandExt;
                if let Err(error) = std::process::Command::new("explorer.exe").raw_arg(format!("/select,\"{}\"", path.display())).spawn() { self.error = Some(error.to_string()); }
            }
            if stop_player { self.player = None; }
            if let Some(time) = play_at { self.play_video(time); }
            else if open { self.open_selected(); }
        });
    }

    fn keyboard(&mut self, ctx: &egui::Context) {
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::L)) {
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("search_input")));
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Enter)) {
            self.open_selected();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F5)) && !self.busy {
            if let Some(folder) = self.active_folder() {
                self.synchronize(folder, false);
            }
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
            && !self.busy
            && !self.query.is_empty()
        {
            self.query.clear();
            self.browse();
        }
        if !egui::Popup::is_any_open(ctx) && !self.rows.is_empty() {
            let down =
                ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
            let up = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp));
            if down || up {
                let current = self.selected.unwrap_or(0);
                let next = if down {
                    (current + 1).min(self.rows.len() - 1)
                } else {
                    current.saturating_sub(1)
                };
                self.select(next);
                self.scroll_to_row = true;
            }
        }
    }
}

impl eframe::App for SearchApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        if self.catalog.is_some() && !self.busy {
            if let Some(query) = self.launch_query.take() {
                self.query = query;
                self.search();
            }
        }
        #[cfg(feature = "ui-capture")]
        if let Some(query) = self
            .capture
            .begin_query(self.catalog.is_some() && !self.busy)
        {
            self.query = query;
            self.search();
        }
        self.keyboard(ctx);
        // One translucent tint over the OS backdrop; text and the document stay opaque.
        let rect = ctx.content_rect();
        let painter = ctx.layer_painter(egui::LayerId::background());
        painter.rect_filled(
            rect,
            0.0,
            if self.acrylic {
                Color32::from_rgba_unmultiplied(16, 28, 27, 100)
            } else {
                Color32::from_rgb(24, 35, 34)
            },
        );
        self.header(ctx);
        self.footer(ctx);
        self.results(ctx);
        self.preview_panel(ctx);
        #[cfg(feature = "ui-capture")]
        self.capture
            .frame(ctx, self.catalog.is_some() && !self.busy, self.acrylic);
        if self.busy || self.player.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.player = None;
        self.cancel.store(true, Ordering::Relaxed);
        let _ = self.commands.send(Request::Quit);
    }
}

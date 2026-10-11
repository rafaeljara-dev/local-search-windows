//! Opt-in renderer capture for layout review without desktop automation.
//! Run with --features ui-capture and --capture <png> [--capture-query <query>].
use eframe::egui;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

pub struct Capture {
    path: Option<PathBuf>,
    query: Option<String>,
    ready_at: Option<Instant>,
    requested: bool,
    started: Instant,
}
impl Capture {
    pub fn from_args() -> Self {
        let args: Vec<_> = std::env::args().collect();
        let value = |name: &str| {
            args.iter()
                .position(|a| a == name)
                .and_then(|i| args.get(i + 1))
                .cloned()
        };
        Self {
            path: value("--capture").map(PathBuf::from),
            query: value("--capture-query"),
            ready_at: None,
            requested: false,
            started: Instant::now(),
        }
    }
    pub fn begin_query(&mut self, ready: bool) -> Option<String> {
        if self.path.is_some() && ready {
            self.query.take()
        } else {
            None
        }
    }
    pub fn frame(&mut self, ctx: &egui::Context, ready: bool, acrylic: bool) {
        let Some(path) = self.path.as_ref() else {
            return;
        };
        if self.started.elapsed() > Duration::from_secs(120) {
            std::process::exit(2);
        }
        if ready && self.query.is_none() && !self.requested {
            let since = self.ready_at.get_or_insert_with(Instant::now);
            if since.elapsed() > Duration::from_millis(500) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                self.requested = true;
            }
        }
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| {
                if let egui::Event::Screenshot { image, .. } = e {
                    Some(image.clone())
                } else {
                    None
                }
            })
        });
        if let Some(shot) = shot {
            let pixels: Vec<u8> = shot
                .pixels
                .iter()
                .flat_map(|c| c.to_srgba_unmultiplied())
                .collect();
            if image::save_buffer(
                path,
                &pixels,
                shot.width() as u32,
                shot.height() as u32,
                image::ColorType::Rgba8,
            )
            .is_err()
            {
                std::process::exit(3);
            }
            let _ = std::fs::write(
                path.with_extension("json"),
                format!(
                    "{{\"native_acrylic_accepted\":{acrylic},\"width\":{},\"height\":{}}}",
                    shot.width(),
                    shot.height()
                ),
            );
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(Duration::from_millis(50));
    }
}

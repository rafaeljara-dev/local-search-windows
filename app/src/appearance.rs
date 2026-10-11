use eframe::egui::{self, Color32, FontFamily, FontId, Pos2, Rect, Response, Stroke};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

pub const TEXT: Color32 = Color32::from_rgb(235, 239, 239);
pub const MUTED: Color32 = Color32::from_rgb(160, 174, 174);
pub const ACCENT: Color32 = Color32::from_rgb(240, 184, 106);
pub const GREEN: Color32 = Color32::from_rgb(158, 211, 189);
pub const LINE: Color32 = Color32::from_rgba_premultiplied(22, 22, 22, 22);

pub fn configure(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for (name, path, family) in [
        (
            "segoe",
            "C:/Windows/Fonts/segoeui.ttf",
            FontFamily::Proportional,
        ),
        (
            "consolas",
            "C:/Windows/Fonts/consola.ttf",
            FontFamily::Monospace,
        ),
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert(name.into(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .get_mut(&family)
                .unwrap()
                .insert(0, name.into());
        }
    }
    if let Ok(bytes) = std::fs::read("C:/Windows/Fonts/seguisym.ttf") {
        fonts
            .font_data
            .insert("symbols".into(), egui::FontData::from_owned(bytes).into());
        fonts
            .families
            .get_mut(&FontFamily::Proportional)
            .unwrap()
            .insert(1, "symbols".into());
    }
    ctx.set_fonts(fonts);
    ctx.set_visuals(egui::Visuals::dark());
    ctx.style_mut(|s| {
        for (kind, size) in [
            (egui::TextStyle::Body, 17.0),
            (egui::TextStyle::Button, 15.0),
            (egui::TextStyle::Small, 13.0),
            (egui::TextStyle::Heading, 28.0),
        ] {
            s.text_styles.insert(kind, FontId::proportional(size));
        }
        s.spacing.item_spacing = egui::vec2(10.0, 8.0);
        s.spacing.button_padding = egui::vec2(12.0, 7.0);
        s.visuals.panel_fill = Color32::TRANSPARENT;
        s.visuals.window_fill = Color32::from_rgb(32, 40, 41);
        s.visuals.window_stroke = Stroke::new(1.0, LINE);
        s.visuals.window_corner_radius = 12.into();
        s.visuals.override_text_color = Some(TEXT);
        s.visuals.selection.bg_fill = Color32::from_rgb(45, 76, 75);
        s.visuals.selection.stroke = Stroke::new(1.0, TEXT);
        s.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
        s.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        s.visuals.widgets.inactive.bg_fill = Color32::from_white_alpha(9);
        s.visuals.widgets.inactive.bg_stroke = Stroke::NONE;
        s.visuals.widgets.hovered.weak_bg_fill = Color32::from_white_alpha(16);
        s.visuals.widgets.hovered.bg_fill = Color32::from_white_alpha(20);
        s.visuals.widgets.hovered.bg_stroke = Stroke::NONE;
        s.visuals.widgets.active.bg_fill = Color32::from_white_alpha(28);
        s.visuals.widgets.active.weak_bg_fill = Color32::from_white_alpha(28);
        s.visuals.widgets.active.bg_stroke = Stroke::NONE;
        s.visuals.widgets.inactive.corner_radius = 7.into();
        s.visuals.widgets.hovered.corner_radius = 7.into();
        s.visuals.widgets.active.corner_radius = 7.into();
        s.animation_time = 0.16;
    });
}

/// Use the documented Windows 11 Desktop Acrylic backdrop. On older Windows,
/// the UI draws an opaque fallback so text remains readable.
pub fn native_backdrop(cc: &eframe::CreationContext<'_>) -> bool {
    use windows_sys::Win32::{Graphics::Dwm::*, UI::Controls::MARGINS};
    let Ok(handle) = cc.window_handle() else {
        return false;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return false;
    };
    let hwnd = handle.hwnd.get() as _;
    unsafe {
        let dark = 1i32;
        let rounded = DWMWCP_ROUND;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE as _,
            &dark as *const _ as _,
            4,
        );
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as _,
            &rounded as *const _ as _,
            4,
        );
        let material = DWMSBT_TRANSIENTWINDOW;
        let backdrop = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE as _,
            &material as *const _ as _,
            4,
        );
        let margins = MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        };
        let extended = DwmExtendFrameIntoClientArea(hwnd, &margins);
        backdrop >= 0 && extended >= 0
    }
}

#[derive(Clone, Copy)]
pub enum Icon {
    Search,
    Close,
    Sync,
    Folder,
    Arrow,
    More,
    Copy,
}

pub fn icon(p: &egui::Painter, center: Pos2, kind: Icon, color: Color32, size: f32) {
    let s = size / 20.0;
    let point = |x: f32, y: f32| center + egui::vec2(x * s, y * s);
    let stroke = Stroke::new(1.6, color);
    let line = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([point(a.0, a.1), point(b.0, b.1)], stroke);
    };
    match kind {
        Icon::Search => {
            p.circle_stroke(point(-2.0, -2.0), 5.7 * s, stroke);
            line((2.5, 2.5), (7.0, 7.0));
        }
        Icon::Close => {
            line((-5.0, -5.0), (5.0, 5.0));
            line((5.0, -5.0), (-5.0, 5.0));
        }
        Icon::Arrow => {
            line((-6.0, 0.0), (6.0, 0.0));
            line((1.0, -5.0), (6.0, 0.0));
            line((1.0, 5.0), (6.0, 0.0));
        }
        Icon::More => {
            for x in [-6.0, 0.0, 6.0] {
                p.circle_filled(point(x, 0.0), 1.6 * s, color);
            }
        }
        Icon::Folder => {
            p.add(egui::Shape::closed_line(
                vec![
                    point(-8.0, -5.0),
                    point(-2.0, -5.0),
                    point(0.0, -3.0),
                    point(8.0, -3.0),
                    point(8.0, 6.0),
                    point(-8.0, 6.0),
                ],
                stroke,
            ));
        }
        Icon::Copy => {
            p.rect_stroke(
                Rect::from_min_max(point(-3.0, -3.0), point(6.0, 7.0)),
                1.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            line((-6.0, 3.0), (-6.0, -6.0));
            line((-6.0, -6.0), (3.0, -6.0));
        }
        Icon::Sync => {
            let points = (0..24)
                .map(|i| {
                    let a = 0.6 + i as f32 * 5.1 / 23.0;
                    point(a.cos() * 6.5, a.sin() * 6.5)
                })
                .collect();
            p.add(egui::Shape::line(points, stroke));
            line((6.0, -6.0), (6.0, -1.0));
            line((6.0, -1.0), (1.0, -1.0));
        }
    }
}

pub fn icon_button(ui: &mut egui::Ui, kind: Icon, label: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(34.0, 30.0), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    let fade = ui
        .ctx()
        .animate_bool_with_time(response.id, response.hovered(), 0.12);
    ui.painter()
        .rect_filled(rect, 6.0, Color32::from_white_alpha((fade * 20.0) as u8));
    icon(
        ui.painter(),
        rect.center(),
        kind,
        if response.hovered() { TEXT } else { MUTED },
        19.0,
    );
    response.on_hover_text(label)
}

pub fn document_thumbnail(p: &egui::Painter, rect: Rect, code: bool) {
    let fill = if code {
        Color32::from_rgb(53, 66, 77)
    } else {
        Color32::from_rgb(226, 226, 211)
    };
    p.rect_filled(rect, 4.0, fill);
    let ink = if code {
        Color32::from_rgb(146, 185, 207)
    } else {
        Color32::from_rgb(146, 158, 151)
    };
    for (i, length) in [0.66, 0.82, 0.71, 0.84, 0.5].iter().enumerate() {
        let y = rect.top() + 11.0 + i as f32 * 5.0;
        p.line_segment(
            [
                egui::pos2(rect.left() + 7.0, y),
                egui::pos2(rect.left() + 7.0 + (rect.width() - 14.0) * length, y),
            ],
            Stroke::new(1.3, ink),
        );
    }
    p.rect_filled(
        Rect::from_min_size(
            egui::pos2(rect.left() - 3.0, rect.bottom() - 16.0),
            egui::vec2(29.0, 14.0),
        ),
        3.0,
        if code {
            Color32::from_rgb(64, 112, 137)
        } else {
            Color32::from_rgb(64, 107, 91)
        },
    );
    p.text(
        egui::pos2(rect.left() + 11.5, rect.bottom() - 9.0),
        egui::Align2::CENTER_CENTER,
        if code { "</>" } else { "TXT" },
        FontId::proportional(9.0),
        Color32::WHITE,
    );
}

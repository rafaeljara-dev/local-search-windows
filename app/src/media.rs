use eframe::egui;
use std::path::{Path, PathBuf};

pub const PROFILE: &str = "vision-q8-rgb-catmull-280-v1";
pub fn is_image(path: &Path) -> bool {
    path.extension().and_then(|s| s.to_str()).is_some_and(|s| {
        matches!(
            s.to_ascii_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "webp" | "bmp"
        )
    })
}
fn decode(path: &Path) -> Result<image::DynamicImage, String> {
    if !is_image(path) {
        return Err("Formato de imagen no compatible".into());
    }
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 32 * 1024 * 1024 {
        return Err("La imagen supera el límite de 32 MB de esta versión".into());
    }
    let mut reader = image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16000);
    limits.max_image_height = Some(16000);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().map_err(|e| e.to_string())
}

fn target_size(width: u32, height: u32) -> (u32, u32) {
    let factor = (280.0 * 48.0 * 48.0 / (f64::from(width) * f64::from(height))).sqrt();
    let w = ((f64::from(width) * factor / 48.0).floor() as u32 * 48).clamp(48, 280 * 48);
    let h = ((f64::from(height) * factor / 48.0).floor() as u32 * 48).clamp(48, 280 * 48);
    (w, h)
}

pub struct PreparedImage(pub PathBuf);
impl Drop for PreparedImage {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
pub fn prepare(path: &Path, root: &Path) -> Result<PreparedImage, String> {
    let original = decode(path)?.into_rgb8();
    let (w, h) = target_size(original.width(), original.height());
    let resized = image::imageops::resize(&original, w, h, image::imageops::FilterType::CatmullRom);
    let path = root
        .join("app-data")
        .join(format!("vision-input-{}.png", std::process::id()));
    resized
        .save_with_format(&path, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(PreparedImage(path))
}

pub fn texture(
    ctx: &egui::Context,
    path: &Path,
    maximum: u32,
) -> Result<egui::TextureHandle, String> {
    let pixels = decode(path)?.thumbnail(maximum, maximum).into_rgba8();
    let size = [pixels.width() as usize, pixels.height() as usize];
    Ok(ctx.load_texture(
        format!("{}-{maximum}", path.display()),
        egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_raw()),
        egui::TextureOptions::LINEAR,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_budget_preserves_valid_patch_grid() {
        for (w, h) in [
            (1300, 1300),
            (1920, 1080),
            (600, 1800),
            (1, 16000),
            (16000, 1),
        ] {
            let (w, h) = target_size(w, h);
            assert!(w >= 48 && h >= 48);
            assert_eq!(w % 48, 0);
            assert_eq!(h % 48, 0);
            assert!(w / 48 * (h / 48) <= 280);
        }
    }
}

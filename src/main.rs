//! RapidMD — native cross-platform Markdown viewer & editor.
//!
//! Thin entry point: configures the eframe (egui) native window and runs the app.

#![forbid(unsafe_code)]
#![deny(clippy::all)]

use eframe::egui;
use rustdown_viewer::app::App;

fn load_app_icon() -> Option<egui::IconData> {
    let bytes = include_bytes!("../assets/Icon/RMD.png");
    if let Ok(img) = image::load_from_memory(bytes) {
        let resized = img.resize(256, 256, image::imageops::FilterType::Lanczos3);
        let rgba = resized.to_rgba8();
        let (width, height) = rgba.dimensions();
        Some(egui::IconData {
            rgba: rgba.into_raw(),
            width,
            height,
        })
    } else {
        None
    }
}

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("RapidMD")
        .with_inner_size([1280.0, 840.0])
        .with_min_inner_size([720.0, 480.0]);

    if let Some(icon) = load_app_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "RapidMD",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

//! RustDownViewer — native cross-platform Markdown viewer & editor.
//!
//! Thin entry point: configures the eframe (egui) native window and runs the app.

#![forbid(unsafe_code)]
#![deny(clippy::all)]

use eframe::egui;
use rustdown_viewer::app::App;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("RustDownViewer")
            .with_inner_size([1280.0, 840.0])
            .with_min_inner_size([720.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "RustDownViewer",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}

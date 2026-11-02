//! Omola desktop GUI.
//!
//! Usage: `omola-gui [MODEL.mo]`. Checking and simulation are delegated to an installed
//! OpenModelica compiler (`OMOLA_OMC` or `omc` on PATH), exactly like the `omola` CLI.

mod app;
mod backend;
mod model;
mod ui;

use std::path::PathBuf;

fn main() -> eframe::Result {
    let initial_file = std::env::args_os().nth(1).map(PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Omola")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Omola",
        options,
        Box::new(move |cc| Ok(Box::new(app::OmolaApp::new(cc, initial_file)))),
    )
}

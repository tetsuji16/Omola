//! Omola desktop GUI.
//!
//! Usage: `omola-gui [MODEL.mo]`. Checking and simulation are delegated to an installed
//! OpenModelica compiler (`OMOLA_OMC` or `omc` on PATH), exactly like the `omola` CLI.

mod app;
mod backend;
mod cache;
mod decimate;
mod diagram;
mod diagram_view;
mod graphics;
mod highlight;
mod html;
mod instance;
mod library;
mod model;
mod perf;
mod plot;
mod series;
mod text_editor;
mod theme;
mod ui;
mod var_tree;

use std::path::PathBuf;

fn main() -> eframe::Result {
    let initial_file = std::env::args_os().nth(1).map(PathBuf::from);
    // OMOLA_WINDOW_SIZE=WIDTHxHEIGHT sets the initial size (used for screenshot comparisons).
    let size = std::env::var("OMOLA_WINDOW_SIZE")
        .ok()
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some([w.trim().parse::<f32>().ok()?, h.trim().parse::<f32>().ok()?])
        })
        .unwrap_or([1280.0, 800.0]);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Omola")
            .with_decorations(!ui::custom_title_bar())
            .with_inner_size(size)
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Omola",
        options,
        Box::new(move |cc| Ok(Box::new(app::OmolaApp::new(cc, initial_file)))),
    )
}

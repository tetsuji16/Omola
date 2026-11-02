//! Omola desktop GUI.
//!
//! Usage: `omola-gui [MODEL.mo]`. Checking and simulation are delegated to an installed
//! OpenModelica compiler (`OMOLA_OMC` or `omc` on PATH), exactly like the `omola` CLI.

mod app;
mod backend;
mod model;
mod theme;
mod ui;

use std::path::PathBuf;

fn main() -> eframe::Result {
    let initial_file = std::env::args_os().nth(1).map(PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Omola")
            .with_app_id("Omola")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 500.0])
            .with_icon(app_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "Omola",
        options,
        Box::new(move |cc| Ok(Box::new(app::OmolaApp::new(cc, initial_file)))),
    )
}

/// 64 px taskbar/window icon: white ring on the accent-coloured rounded square.
fn app_icon() -> egui::IconData {
    let size = 64u32;
    let half = size as f32 / 2.0;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    for y in 0..size {
        for x in 0..size {
            let px = x as f32 + 0.5 - half;
            let py = y as f32 + 0.5 - half;

            // Signed distance to a rounded square (28 px half-size, 8 px radius).
            let qx = px.abs() - (28.0 - 8.0);
            let qy = py.abs() - (28.0 - 8.0);
            let outside = qx.max(0.0).hypot(qy.max(0.0));
            let dist = outside + qx.max(qy).min(0.0) - 8.0;
            if dist > 0.5 {
                continue;
            }

            let r = px.hypot(py);
            let (red, green, blue) = if (14.0..21.0).contains(&r) {
                (255, 255, 255)
            } else {
                (0, 95, 184)
            };
            let alpha = if dist > -0.5 {
                ((0.5 - dist) * 255.0).clamp(0.0, 255.0) as u8
            } else {
                255
            };

            let i = ((y * size + x) * 4) as usize;
            rgba[i] = red;
            rgba[i + 1] = green;
            rgba[i + 2] = blue;
            rgba[i + 3] = alpha;
        }
    }

    egui::IconData {
        rgba,
        width: size,
        height: size,
    }
}

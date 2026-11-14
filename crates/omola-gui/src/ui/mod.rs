//! UI building blocks. Each file adds `impl OmolaApp` methods for one area of the window:
//!
//! - `chrome.rs`: title bar with quick access toolbar, ribbon tabs and ribbon bodies
//! - `browsers.rs`: package/component browser and variable browser
//! - `views.rs`: edit window (layers) and plot window
//! - `doc_view.rs`: documentation layer (HTML rendering)
//! - `bottom.rs`: command/log window and the bottom bar
//! - `dialogs.rs`: Simulation Setup and path dialogs
//! - `widgets.rs`: ribbon buttons and other small widgets

mod bottom;
pub(crate) mod browsers;
mod chrome;
mod dialogs;
mod doc_view;
mod views;
pub mod widgets;

use crate::app::{OmolaApp, RibbonTab};
use crate::theme;
use egui::{Frame, Margin, Panel};

/// Whether Omola draws its own title bar (no OS decorations). Set
/// `OMOLA_NATIVE_TITLEBAR=1` to keep the operating system's title bar instead.
pub fn custom_title_bar() -> bool {
    std::env::var_os("OMOLA_NATIVE_TITLEBAR").is_none()
}

impl OmolaApp {
    pub fn layout(&mut self, ui: &mut egui::Ui) {
        if custom_title_bar() {
            resize_edges(ui);
        }
        Panel::top("title_bar")
            .exact_size(30.0)
            .show_separator_line(false)
            .frame(Frame::new().fill(theme::TITLE_BAR))
            .show(ui, |ui| self.title_bar(ui));
        Panel::top("ribbon_tabs")
            .exact_size(26.0)
            .show_separator_line(false)
            .frame(Frame::new().fill(theme::TITLE_BAR))
            .show(ui, |ui| self.ribbon_tabs(ui));
        Panel::top("ribbon")
            .exact_size(if self.ribbon_collapsed { 0.0 } else { 102.0 })
            .show_separator_line(false)
            .frame(Frame::new().fill(theme::BG))
            .show(ui, |ui| self.ribbon(ui));
        Panel::bottom("bottom_bar")
            .exact_size(25.0)
            .show_separator_line(false)
            .frame(
                Frame::new()
                    .fill(theme::BG)
                    .inner_margin(Margin::symmetric(4, 1)),
            )
            .show(ui, |ui| self.bottom_bar(ui));
        if self.docks.bottom {
            Panel::bottom("bottom_window")
                .exact_size(self.bottom_height)
                .show_separator_line(false)
                .frame(Frame::new().fill(theme::BG).inner_margin(Margin {
                    left: 0,
                    right: 2,
                    top: 3,
                    bottom: 3,
                }))
                .show(ui, |ui| {
                    // Splitter on the top edge.
                    let edge = egui::Rect::from_min_size(
                        ui.max_rect().left_top() - egui::vec2(0.0, 5.0),
                        egui::vec2(ui.max_rect().width(), 6.0),
                    );
                    let handle = ui.interact(
                        edge,
                        ui.scope_id().with("bottom_splitter"),
                        egui::Sense::drag(),
                    );
                    if handle.hovered() || handle.dragged() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                    }
                    if handle.dragged() {
                        self.bottom_height =
                            (self.bottom_height - handle.drag_delta().y).clamp(60.0, 600.0);
                    }
                    self.bottom_window(ui)
                });
        }
        let simulation = self.tab == RibbonTab::Simulation;
        let left_visible = if simulation {
            self.docks.variable_browser
        } else {
            self.docks.package_browser || self.docks.component_browser
        };
        if left_visible {
            Panel::left(if simulation { "left_sim" } else { "left_model" })
                .resizable(true)
                .default_size(if simulation { 248.0 } else { 300.0 })
                .size_range(150.0..=700.0)
                .frame(Frame::new().fill(theme::BG).inner_margin(Margin {
                    left: 0,
                    right: 2,
                    top: 0,
                    bottom: 0,
                }))
                .show(ui, |ui| {
                    if simulation {
                        self.variable_browser(ui);
                    } else {
                        self.modeling_browsers(ui);
                    }
                });
        }
        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(theme::BG)
                    .inner_margin(Margin::symmetric(2, 2)),
            )
            .show(ui, |ui| {
                if simulation {
                    self.plot_window(ui);
                } else {
                    self.edit_window(ui);
                }
            });
    }
}

/// Let the user resize an undecorated window from its edges.
fn resize_edges(ui: &egui::Ui) {
    use egui::{CursorIcon, ResizeDirection as D};
    let rect = ui.ctx().content_rect();
    let Some(pos) = ui.ctx().pointer_hover_pos() else {
        return;
    };
    let m = 4.0;
    let (l, r, t, b) = (
        pos.x - rect.left() < m,
        rect.right() - pos.x < m,
        pos.y - rect.top() < m,
        rect.bottom() - pos.y < m,
    );
    let dir = match (l, r, t, b) {
        (true, _, true, _) => Some((D::NorthWest, CursorIcon::ResizeNorthWest)),
        (_, true, true, _) => Some((D::NorthEast, CursorIcon::ResizeNorthEast)),
        (true, _, _, true) => Some((D::SouthWest, CursorIcon::ResizeSouthWest)),
        (_, true, _, true) => Some((D::SouthEast, CursorIcon::ResizeSouthEast)),
        (true, ..) => Some((D::West, CursorIcon::ResizeWest)),
        (_, true, ..) => Some((D::East, CursorIcon::ResizeEast)),
        (_, _, true, _) => Some((D::North, CursorIcon::ResizeNorth)),
        (_, _, _, true) => Some((D::South, CursorIcon::ResizeSouth)),
        _ => None,
    };
    if let Some((dir, cursor)) = dir {
        ui.ctx().set_cursor_icon(cursor);
        if ui.input(|i| i.pointer.primary_pressed()) {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
        }
    }
}

/// Compact number formatting shared by ribbon fields and the variable browser.
pub fn browsers_format(v: f64) -> String {
    browsers::format_value(v)
}

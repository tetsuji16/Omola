//! Central views: model tabs + edit window (per layer) and the plot window.

use super::widgets::{flat_tab, paint_icon};
use crate::app::{Layer, Loadable, OmolaApp};
use crate::theme;
use egui::{Align2, Color32, FontId, RichText, Sense, Stroke, Vec2};
use egui_material_icons::icons::*;

/// Curve colours in plot order.
pub const CURVE_COLORS: [Color32; 6] = [
    Color32::from_rgb(0x1F, 0x5F, 0xD6),
    Color32::from_rgb(0xE0, 0x1B, 0x1B),
    Color32::from_rgb(0x00, 0x8C, 0x00),
    Color32::from_rgb(0xC0, 0x00, 0xC0),
    Color32::from_rgb(0x00, 0x00, 0x00),
    Color32::from_rgb(0xD0, 0x7A, 0x00),
];

impl OmolaApp {
    /// Tabs of open models: `Name x` ... `+`.
    pub fn model_tabs_strip(&mut self, ui: &mut egui::Ui) {
        let mut activate = None;
        let mut close = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for class in self.model_tabs.clone() {
                let selected = self.active.as_deref() == Some(class.as_str());
                let label = crate::library::short_name(&class);
                let font = FontId::proportional(12.5);
                let tw = ui.ctx().fonts_mut(|f| {
                    f.layout_no_wrap(label.into(), font.clone(), theme::TEXT)
                        .size()
                        .x
                });
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(tw + 56.0, 22.0), Sense::click());
                let painter = ui.painter();
                painter.rect(
                    rect,
                    0.0,
                    if selected { Color32::WHITE } else { theme::BG },
                    Stroke::new(
                        1.0,
                        if selected {
                            theme::BORDER
                        } else {
                            Color32::TRANSPARENT
                        },
                    ),
                    egui::StrokeKind::Inside,
                );
                painter.text(
                    egui::pos2(rect.left() + 26.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    label,
                    font,
                    theme::TEXT,
                );
                let x_rect = egui::Rect::from_center_size(
                    egui::pos2(rect.right() - 14.0, rect.center().y),
                    Vec2::splat(14.0),
                );
                paint_icon(
                    ui,
                    x_rect.center(),
                    ICON_CLOSE,
                    13.0,
                    if selected {
                        theme::TEXT
                    } else {
                        theme::TEXT_WEAK
                    },
                );
                if resp.clicked() {
                    if resp
                        .interact_pointer_pos()
                        .is_some_and(|p| x_rect.expand(2.0).contains(p))
                    {
                        close = Some(class.clone());
                    } else {
                        activate = Some(class.clone());
                    }
                }
                resp.on_hover_text(&class);
            }
            if flat_tab(ui, "+", false)
                .on_hover_text("New model")
                .clicked()
            {
                self.new_model();
            }
        });
        if let Some(c) = close {
            self.close_tab(&c);
        }
        if let Some(c) = activate {
            self.open_class(&c);
        }
    }

    pub fn edit_window(&mut self, ui: &mut egui::Ui) {
        self.model_tabs_strip(ui);
        let Some(class) = self.active.clone() else {
            empty_canvas(ui, "No model open. Select a class in the Package Browser.");
            return;
        };
        egui::Frame::new()
            .fill(Color32::WHITE)
            .stroke(Stroke::new(1.0, theme::BORDER))
            .show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                match self.layer() {
                    Layer::Icon => self.icon_layer(ui, &class),
                    Layer::Diagram => diagram_canvas(ui, self.layer()),
                    Layer::Documentation => self.documentation_layer(ui, &class),
                    Layer::Text => self.text_layer(ui, &class),
                }
            });
    }

    fn text_layer(&mut self, ui: &mut egui::Ui, class: &str) {
        if let Some(i) = self.doc_of(class) {
            let goto = self.goto_line.take();
            let doc = &mut self.documents[i];
            let editor_id = ui.make_persistent_id(("user_text", i));
            let dark = ui.visuals().dark_mode;
            let font = egui::TextStyle::Monospace.resolve(ui.style());
            let numbers = crate::highlight::line_numbers(&doc.text);
            let job = crate::highlight::layout_job(&doc.text, font.clone(), dark);
            let mut layouter = |ui: &egui::Ui, _text: &dyn egui::TextBuffer, wrap_width: f32| {
                let mut job = job.clone();
                job.wrap.max_width = wrap_width;
                ui.fonts_mut(|f| f.layout_job(job))
            };
            let edited = egui::ScrollArea::both()
                .id_salt(("user_text", i))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(numbers)
                                    .monospace()
                                    .size(font.size)
                                    .color(theme::TEXT_WEAK),
                            )
                            .selectable(false)
                            .wrap_mode(egui::TextWrapMode::Extend),
                        );
                        ui.add(
                            egui::TextEdit::multiline(&mut doc.text)
                                .id(editor_id)
                                .code_editor()
                                .frame(egui::Frame::NONE)
                                .desired_width(f32::INFINITY)
                                .desired_rows(40)
                                .layouter(&mut layouter),
                        )
                    })
                    .inner
                })
                .inner;
            // A click on a Message location moves the caret to that line.
            if let Some((line, column)) = goto {
                if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), editor_id) {
                    let index = crate::highlight::char_index(&doc.text, line, column);
                    state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::one(
                            egui::text::CCursor::new(index),
                        )));
                    state.store(ui.ctx(), editor_id);
                    edited.request_focus();
                }
            }
            if edited.changed() {
                self.user_text_changed(i);
            }
            return;
        }
        match self.texts.get(class) {
            None | Some(Loadable::Loading) => {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Loading Modelica text from OMC...");
                });
                if !self.texts.contains_key(class) {
                    self.request_text(class);
                }
            }
            Some(Loadable::Failed(e)) => {
                ui.add_space(8.0);
                ui.label(RichText::new(e).color(theme::TEXT_WEAK));
            }
            Some(Loadable::Ready(text)) => {
                let mut text = text.as_str();
                let dark = ui.visuals().dark_mode;
                let font = egui::TextStyle::Monospace.resolve(ui.style());
                let job = crate::highlight::layout_job(text, font, dark);
                let mut layouter =
                    |ui: &egui::Ui, _text: &dyn egui::TextBuffer, wrap_width: f32| {
                        let mut job = job.clone();
                        job.wrap.max_width = wrap_width;
                        ui.fonts_mut(|f| f.layout_job(job))
                    };
                egui::ScrollArea::both()
                    .id_salt(("lib_text", class))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut text)
                                .code_editor()
                                .frame(egui::Frame::NONE)
                                .desired_width(f32::INFINITY)
                                .layouter(&mut layouter),
                        );
                    });
            }
        }
    }

    /// Graphics tab > Icon: the read-only `annotation(Icon(graphics={...}))` of `class`.
    fn icon_layer(&mut self, ui: &mut egui::Ui, class: &str) {
        self.request_icon(class);
        let (rect, sheet) = canvas_sheet(ui);
        let painter = ui.painter_at(rect);
        let note = |text: &str| {
            painter.text(
                egui::pos2(sheet.center().x, sheet.bottom() + 12.0),
                Align2::CENTER_CENTER,
                text,
                FontId::proportional(11.0),
                theme::TEXT_WEAK,
            );
        };
        match self.icons.get(class) {
            Some(Loadable::Ready(icon)) if !icon.is_empty() => crate::graphics::paint_icon(
                &painter,
                sheet,
                icon,
                crate::library::short_name(class),
            ),
            Some(Loadable::Ready(_)) => note("This class has no Icon graphics."),
            Some(Loadable::Loading) => note("Loading the Icon annotation ..."),
            Some(Loadable::Failed(message)) => note(message),
            None => note("Loading the Icon annotation ..."),
        }
    }

    pub fn plot_window(&mut self, ui: &mut egui::Ui) {
        let curves: Vec<Curve> = self
            .plotted
            .iter()
            .enumerate()
            .filter_map(|(i, signal)| {
                let run = self.run(signal.run_id)?;
                let series = run.outcome.result.series(&signal.variable)?;
                let points = run
                    .outcome
                    .result
                    .time
                    .iter()
                    .zip(series)
                    .map(|(t, v)| [*t, *v])
                    .collect();
                let name = if self.runs.len() > 1 {
                    format!("{}.{}", run.label, signal.variable)
                } else {
                    signal.variable.clone()
                };
                Some(Curve {
                    name,
                    color: CURVE_COLORS[i % CURVE_COLORS.len()],
                    points,
                })
            })
            .collect();
        let (panel, response) =
            ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let painter = ui.painter_at(panel);
        painter.rect_filled(panel, 0.0, Color32::WHITE);
        let frame = egui::Rect::from_min_max(
            egui::pos2(panel.left() + 53.0, panel.top() + 31.0),
            egui::pos2(panel.right() - 24.0, panel.bottom() - 29.0),
        );
        if frame.width() < 40.0 || frame.height() < 40.0 {
            return;
        }
        // Legend above the frame.
        let mut lx = frame.left() - 2.0;
        for c in &curves {
            let y = panel.top() + 16.5;
            painter.hline(lx..=lx + 26.0, y, Stroke::new(1.0, c.color));
            let g = painter.layout_no_wrap(c.name.clone(), FontId::proportional(12.0), theme::TEXT);
            let w = g.size().x;
            painter.galley(egui::pos2(lx + 31.0, y - g.size().y / 2.0), g, theme::TEXT);
            lx += 31.0 + w + 18.0;
        }
        let data = data_bounds(&curves);
        let fitted = data.map(|(x0, x1, y0, y1)| {
            let (y0, y1, step) = nice_range(y0, y1, frame.height() / 50.0);
            ((x0, x1, y0, y1), step)
        });
        let view = self.plot_view.or(fitted.map(|f| f.0));
        let Some((x0, x1, y0, y1)) = view else {
            painter.rect_stroke(
                frame,
                0.0,
                Stroke::new(1.0, Color32::from_rgb(0x3A, 0x3A, 0x3A)),
                egui::StrokeKind::Inside,
            );
            return;
        };
        let (x0, x1) = if x1 > x0 {
            (x0, x1)
        } else {
            (x0 - 1.0, x0 + 1.0)
        };
        let (y0, y1) = if y1 > y0 {
            (y0, y1)
        } else {
            (y0 - 1.0, y0 + 1.0)
        };
        let to_screen = |x: f64, y: f64| {
            egui::pos2(
                frame.left() + ((x - x0) / (x1 - x0)) as f32 * frame.width(),
                frame.bottom() - ((y - y0) / (y1 - y0)) as f32 * frame.height(),
            )
        };
        let x_major = nice_step((x1 - x0) / (frame.width() / 130.0).max(1.0) as f64);
        let y_major = match (self.plot_view, fitted) {
            (None, Some((_, step))) => step,
            _ => nice_step((y1 - y0) / (frame.height() / 50.0).max(1.0) as f64),
        };
        let grid = Stroke::new(1.0, Color32::from_rgb(0xEA, 0xEA, 0xEA));
        let axis = Color32::from_rgb(0x3A, 0x3A, 0x3A);
        for x in ticks(x0, x1, minor_step(x_major)) {
            let p = to_screen(x, y0);
            painter.vline(p.x.round() + 0.5, frame.y_range(), grid);
        }
        for y in ticks(y0, y1, minor_step(y_major)) {
            let p = to_screen(x0, y);
            let stroke = if y.abs() < y_major * 1e-6 {
                Stroke::new(1.0, Color32::from_rgb(0xCA, 0xCA, 0xCA))
            } else {
                grid
            };
            painter.hline(frame.x_range(), p.y.round() + 0.5, stroke);
            painter.hline(
                (frame.left() - 4.0)..=frame.left(),
                p.y.round() + 0.5,
                Stroke::new(1.0, axis),
            );
        }
        let label_font = FontId::proportional(11.5);
        for x in ticks(x0, x1, x_major) {
            let p = to_screen(x, y0);
            painter.vline(
                p.x.round() + 0.5,
                frame.bottom()..=(frame.bottom() + 4.0),
                Stroke::new(1.0, axis),
            );
            painter.text(
                egui::pos2(p.x, frame.bottom() + 12.0),
                Align2::CENTER_CENTER,
                format_tick(x, x_major),
                label_font.clone(),
                theme::TEXT,
            );
        }
        for y in ticks(y0, y1, y_major) {
            let p = to_screen(x0, y);
            painter.hline(
                (frame.left() - 6.0)..=frame.left(),
                p.y.round() + 0.5,
                Stroke::new(1.0, axis),
            );
            painter.text(
                egui::pos2(frame.left() - 8.0, p.y),
                Align2::RIGHT_CENTER,
                format_tick(y, y_major),
                label_font.clone(),
                theme::TEXT,
            );
        }
        let clip = painter.with_clip_rect(frame);
        for c in &curves {
            let pts: Vec<egui::Pos2> = c.points.iter().map(|[x, y]| to_screen(*x, *y)).collect();
            clip.add(egui::Shape::line(pts, Stroke::new(1.0, c.color)));
        }
        painter.rect_stroke(frame, 0.0, Stroke::new(1.0, axis), egui::StrokeKind::Inside);

        // Interaction: drag a rectangle to zoom, double-click to reset.
        if response.double_clicked() {
            self.plot_view = None;
        }
        let drag_id = ui.scope_id().with("plot_drag_start");
        if response.drag_started() {
            if let Some(p) = response.interact_pointer_pos() {
                ui.ctx().data_mut(|d| d.insert_temp(drag_id, p));
            }
        }
        if let (Some(start), Some(now)) = (
            ui.ctx().data(|d| d.get_temp::<egui::Pos2>(drag_id)),
            response.interact_pointer_pos(),
        ) {
            let r = egui::Rect::from_two_pos(start, now).intersect(frame);
            if response.dragged() {
                painter.rect(
                    r,
                    0.0,
                    Color32::from_rgba_unmultiplied(0x1F, 0x6F, 0xC8, 30),
                    Stroke::new(1.0, theme::ACCENT),
                    egui::StrokeKind::Inside,
                );
            }
            if response.drag_stopped() {
                ui.ctx().data_mut(|d| d.remove::<egui::Pos2>(drag_id));
                if r.width() > 4.0 && r.height() > 4.0 {
                    let from = |p: egui::Pos2| {
                        (
                            x0 + ((p.x - frame.left()) / frame.width()) as f64 * (x1 - x0),
                            y0 + ((frame.bottom() - p.y) / frame.height()) as f64 * (y1 - y0),
                        )
                    };
                    let (ax, ay) = from(r.left_bottom());
                    let (bx, by) = from(r.right_top());
                    self.plot_view = Some((ax, bx, ay, by));
                }
            }
        }
        if let Some(p) = response.hover_pos().filter(|p| frame.contains(*p)) {
            let t = x0 + ((p.x - frame.left()) / frame.width()) as f64 * (x1 - x0);
            let v = y0 + ((frame.bottom() - p.y) / frame.height()) as f64 * (y1 - y0);
            response.on_hover_text_at_pointer(format!(
                "t = {}\ny = {}",
                crate::ui::browsers_format(t),
                crate::ui::browsers_format(v)
            ));
        }
    }
}

struct Curve {
    name: String,
    color: Color32,
    points: Vec<[f64; 2]>,
}

fn data_bounds(curves: &[Curve]) -> Option<(f64, f64, f64, f64)> {
    let mut b: Option<(f64, f64, f64, f64)> = None;
    for [x, y] in curves.iter().flat_map(|c| c.points.iter()) {
        if !x.is_finite() || !y.is_finite() {
            continue;
        }
        b = Some(match b {
            None => (*x, *x, *y, *y),
            Some((a, bb, c, d)) => (a.min(*x), bb.max(*x), c.min(*y), d.max(*y)),
        });
    }
    b
}

/// A "nice" step (1, 2, 2.5, 4, 5 x 10^k) not smaller than `raw`.
pub fn nice_step(raw: f64) -> f64 {
    if raw.is_nan() || raw <= 0.0 || !raw.is_finite() {
        return 1.0;
    }
    let exp = raw.log10().floor();
    let base = 10f64.powf(exp);
    for m in [1.0, 2.0, 2.5, 4.0, 5.0, 10.0] {
        if m * base >= raw * (1.0 - 1e-9) {
            return m * base;
        }
    }
    10.0 * base
}

/// Minor grid step for a major step: 1/5 of 1, 2.5 and 5 mantissas, 1/2 of 2 and 4.
pub fn minor_step(major: f64) -> f64 {
    let mantissa = major / 10f64.powf(major.log10().floor());
    if (mantissa - 2.0).abs() < 1e-6 || (mantissa - 4.0).abs() < 1e-6 {
        major / 2.0
    } else {
        major / 5.0
    }
}

/// Expand `[lo, hi]` to multiples of a nice step chosen for about `intervals` intervals.
pub fn nice_range(lo: f64, hi: f64, intervals: f32) -> (f64, f64, f64) {
    let (lo, hi) = if hi > lo {
        (lo, hi)
    } else {
        (lo - 1.0, lo + 1.0)
    };
    let step = nice_step((hi - lo) / intervals.max(1.0) as f64);
    let a = (lo / step - 1e-9).floor() * step;
    let b = (hi / step + 1e-9).ceil() * step;
    (a, b, step)
}

fn ticks(lo: f64, hi: f64, step: f64) -> Vec<f64> {
    if step.is_nan() || step <= 0.0 {
        return Vec::new();
    }
    let first = (lo / step - 1e-9).ceil() as i64;
    let last = (hi / step + 1e-9).floor() as i64;
    if last - first > 2000 {
        return Vec::new();
    }
    (first..=last).map(|k| k as f64 * step).collect()
}

/// Tick label with just enough decimals for the grid step (`0.0`, `2.5`, `-0.04`).
pub fn format_tick(value: f64, step: f64) -> String {
    let step = step.abs();
    let mut decimals = 0usize;
    while decimals < 8 {
        let scaled = step * 10f64.powi(decimals as i32);
        if (scaled - scaled.round()).abs() < 1e-6 * scaled.max(1.0) {
            break;
        }
        decimals += 1;
    }
    // Integer steps still show one decimal on small axes (0.0, 2.0, 4.0).
    if decimals == 0 && step < 10.0 {
        decimals = 1;
    }
    let v = if value.abs() < step * 1e-9 {
        0.0
    } else {
        value
    };
    format!("{v:.decimals$}")
}

fn empty_canvas(ui: &mut egui::Ui, message: &str) {
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, Color32::WHITE);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        message,
        FontId::proportional(13.0),
        theme::TEXT_WEAK,
    );
}

/// White canvas with the Modelica coordinate system ({-100,-100},{100,100}) and its grid.
fn diagram_canvas(ui: &mut egui::Ui, layer: Layer) {
    let (rect, sheet) = canvas_sheet(ui);
    let note = match layer {
        Layer::Icon => "Icon layer: graphical rendering is tracked in issue G07",
        _ => "Diagram layer: graphical rendering is tracked in issue G12",
    };
    ui.painter_at(rect).text(
        egui::pos2(sheet.center().x, sheet.bottom() + 12.0),
        Align2::CENTER_CENTER,
        note,
        FontId::proportional(11.0),
        theme::TEXT_WEAK,
    );
}

/// Paint the white canvas and its grid, and return the full area plus the square sheet
/// the Modelica coordinates map onto.
fn canvas_sheet(ui: &mut egui::Ui) -> (egui::Rect, egui::Rect) {
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::WHITE);
    let sheet = crate::graphics::sheet(rect);
    let grid = Stroke::new(1.0, Color32::from_rgb(0xEA, 0xEA, 0xEA));
    let steps = 20;
    for i in 1..steps {
        let t = i as f32 / steps as f32;
        let x = sheet.left() + t * sheet.width();
        let y = sheet.top() + t * sheet.height();
        let s = if i == steps / 2 {
            Stroke::new(1.0, Color32::from_rgb(0xD8, 0xD8, 0xD8))
        } else {
            grid
        };
        painter.vline(x, sheet.y_range(), s);
        painter.hline(sheet.x_range(), y, s);
    }
    painter.rect_stroke(
        sheet,
        0.0,
        Stroke::new(1.0, Color32::from_rgb(0xC8, 0xC8, 0xC8)),
        egui::StrokeKind::Inside,
    );
    (rect, sheet)
}

#[cfg(test)]
mod tests {
    use super::format_tick;

    #[test]
    fn tick_labels_follow_the_step() {
        assert_eq!(format_tick(0.12, 0.04), "0.12");
        assert_eq!(format_tick(-0.04, 0.04), "-0.04");
        assert_eq!(format_tick(0.0, 0.04), "0.00");
        assert_eq!(format_tick(2.5, 2.5), "2.5");
        assert_eq!(format_tick(5.0, 2.5), "5.0");
        assert_eq!(format_tick(0.0, 2.5), "0.0");
        assert_eq!(format_tick(1e-17, 0.4), "0.0");
        assert_eq!(format_tick(4.0, 2.0), "4.0");
    }

    #[test]
    fn nice_ranges_match_reference_axes() {
        use super::{minor_step, nice_range, nice_step};
        // Pendulum: phi in [-0.1, 0.1] over 330 px -> [-0.12, 0.12] with 0.04 labels.
        let (a, b, step) = nice_range(-0.1, 0.1, 330.0 / 50.0);
        assert!(
            (a + 0.12).abs() < 1e-12 && (b - 0.12).abs() < 1e-12 && (step - 0.04).abs() < 1e-12
        );
        assert!((minor_step(0.04) - 0.02).abs() < 1e-12);
        // Time 0..10 over 556 px -> labels every 2.5, grid every 0.5.
        let x = nice_step(10.0 / (556.0f64 / 130.0).floor());
        assert!((x - 2.5).abs() < 1e-12, "{x}");
        assert!((minor_step(2.5) - 0.5).abs() < 1e-12);
    }
}

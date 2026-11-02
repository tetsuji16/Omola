//! Central views: the edit window (per modeling layer) and the plot window.

use crate::app::{GraphicsLayer, OmolaApp, RibbonTab};
use egui::{Color32, RichText, Stroke};
use egui_plot::{Legend, Line, Plot, PlotPoints};

impl OmolaApp {
    pub fn edit_window(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.strong(self.document.title());
            if let Some(class) = &self.selected_class {
                ui.label(RichText::new(format!("— {class}")).weak());
            }
        });
        ui.separator();
        match self.tab {
            RibbonTab::Text => {
                let response = egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.document.text)
                                .code_editor()
                                .desired_width(f32::INFINITY)
                                .desired_rows(30),
                        )
                    })
                    .inner;
                if response.changed() {
                    self.document.dirty = true;
                    self.refresh_outline();
                }
            }
            RibbonTab::Graphics => {
                let title = match self.graphics_layer {
                    GraphicsLayer::Diagram => "Diagram layer",
                    GraphicsLayer::Icon => "Icon layer",
                };
                placeholder_canvas(
                    ui,
                    title,
                    "Graphical editing is not implemented yet (issue G07).",
                );
            }
            RibbonTab::Documentation => {
                let entry = self
                    .classes
                    .iter()
                    .find(|e| Some(&e.qualified_name) == self.selected_class.as_ref());
                ui.heading(self.selected_class.clone().unwrap_or_default());
                if let Some(entry) = entry {
                    ui.label(format!("Kind: {}", entry.kind));
                    ui.label(
                        entry
                            .description
                            .clone()
                            .unwrap_or_else(|| "(no description string)".into()),
                    );
                }
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Rendering of annotation(Documentation(info=…)) HTML: issue G08")
                        .weak(),
                );
            }
            RibbonTab::Tools => {
                ui.label("Tools: backend information. Use 'Locate OMC' in the ribbon.");
                ui.label(format!(
                    "Run directory: {}",
                    crate::backend::default_work_root().display()
                ));
            }
            RibbonTab::Simulation => {}
        }
    }

    pub fn plot_window(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.strong("Plot");
            ui.label(
                RichText::new("drag = pan, scroll = zoom, double-click = reset")
                    .small()
                    .weak(),
            );
        });
        let lines: Vec<(String, Vec<[f64; 2]>)> = self
            .plotted
            .iter()
            .filter_map(|signal| {
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
                Some((format!("{}: {}", run.label, signal.variable), points))
            })
            .collect();
        Plot::new("main_plot")
            .legend(Legend::default())
            .x_axis_label("time [s]")
            .show(ui, |plot_ui| {
                for (name, points) in lines {
                    plot_ui.line(Line::new(name, PlotPoints::from(points)));
                }
            });
    }
}

fn placeholder_canvas(ui: &mut egui::Ui, title: &str, note: &str) {
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_gray(250));
    let grid = Stroke::new(1.0, Color32::from_gray(225));
    let step = 20.0;
    let mut x = rect.left();
    while x < rect.right() {
        painter.vline(x, rect.y_range(), grid);
        x += step;
    }
    let mut y = rect.top();
    while y < rect.bottom() {
        painter.hline(rect.x_range(), y, grid);
        y += step;
    }
    // The -100..100 Modelica coordinate system frame, centred.
    let side = rect.width().min(rect.height()) * 0.6;
    let frame = egui::Rect::from_center_size(rect.center(), egui::vec2(side, side));
    painter.rect_stroke(
        frame,
        0.0,
        Stroke::new(1.0, Color32::from_gray(160)),
        egui::StrokeKind::Inside,
    );
    painter.text(
        rect.center() - egui::vec2(0.0, 12.0),
        egui::Align2::CENTER_CENTER,
        title,
        egui::FontId::proportional(18.0),
        Color32::from_gray(90),
    );
    painter.text(
        rect.center() + egui::vec2(0.0, 12.0),
        egui::Align2::CENTER_CENTER,
        note,
        egui::FontId::proportional(13.0),
        Color32::from_gray(120),
    );
}

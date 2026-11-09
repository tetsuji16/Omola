//! Central views: model tabs + edit window (per layer) and the plot window.

use super::widgets::{flat_tab, paint_icon};
use crate::app::{Layer, Loadable, OmolaApp, PlotSignal};
use crate::text_editor::{draw_text_editor, lines_for_file};
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

/// Label of a model tab: the short class name, plus `*` while the document has unsaved edits.
pub fn tab_label(qualified_name: &str, dirty: bool) -> String {
    let name = crate::library::short_name(qualified_name);
    if dirty {
        format!("{name}*")
    } else {
        name.to_owned()
    }
}

impl OmolaApp {
    /// Tabs of open models: `Name x` ... `+`.
    pub fn model_tabs_strip(&mut self, ui: &mut egui::Ui) {
        let mut activate = None;
        let mut close = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for class in self.model_tabs.clone() {
                let selected = self.active.as_deref() == Some(class.as_str());
                // A `*` marks unsaved edits, the same marker `Document::title()` uses.
                let dirty = self.doc_of(&class).is_some_and(|i| self.documents[i].dirty);
                let label = tab_label(&class, dirty);
                let font = FontId::proportional(12.5);
                let tw = ui.ctx().fonts_mut(|f| {
                    f.layout_no_wrap(label.clone(), font.clone(), theme::TEXT)
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
                    Layer::Diagram => self.diagram_layer(ui, &class),
                    Layer::Documentation => self.documentation_layer(ui, &class),
                    Layer::Text => self.text_layer(ui, &class),
                }
            });
    }

    fn text_layer(&mut self, ui: &mut egui::Ui, class: &str) {
        if let Some(i) = self.doc_of(class) {
            // Check reports to Syntax, translation errors to Translation and simulation
            // errors to Simulation; `error_locations` already covers all three (G25).
            let error_lines = lines_for_file(
                &self.error_locations,
                self.documents[i].path.as_deref(),
                Some(class),
            );
            let doc = &mut self.documents[i];

            let changed = draw_text_editor(
                ui,
                &mut doc.editor,
                &mut doc.text,
                doc.path.as_ref(),
                &error_lines,
            );
            if changed {
                doc.dirty = true;
                self.user_text_changed(i);
            }
            return;
        }
        // A library class's text never changes, so its coloured layout is built once and
        // keyed by the class name (G24).
        if !self.text_layouts.contains_key(class) {
            if let Some(Loadable::Ready(text)) = self.texts.get(class) {
                self.text_layouts.insert(
                    class.to_owned(),
                    crate::text_editor::highlight_job(text, &egui::FontId::monospace(13.0)),
                );
            }
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
                // The text of a library class does not change: colour it once (G24).
                let job = self.text_layouts.get(class).cloned();
                let mut text = text.as_str();
                let mut layouter = move |ui: &egui::Ui, _buf: &dyn egui::TextBuffer, _w: f32| {
                    let mut job = job.clone().unwrap_or_default();
                    job.wrap.max_width = f32::INFINITY;
                    ui.ctx().fonts_mut(|f| f.layout_job(job))
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

    /// Graphics tab > Diagram: select, drag and rotate components (G12, G29).
    fn diagram_layer(&mut self, ui: &mut egui::Ui, class: &str) {
        self.request_diagram(class);
        let editable = self.is_user_class(class);
        // Toolbar
        ui.horizontal(|ui| {
            if ui.button("Fit").clicked() {
                self.diagram_zoom = 1.0;
                self.diagram_pan = egui::Vec2::ZERO;
            }
            if ui.button("+").clicked() {
                self.diagram_zoom = (self.diagram_zoom * 1.2).clamp(0.1, 20.0);
            }
            if ui.button("−").clicked() {
                self.diagram_zoom = (self.diagram_zoom / 1.2).clamp(0.1, 20.0);
            }
            ui.label(format!("Zoom: {:.0}%", self.diagram_zoom * 100.0));
            // The status of the layer: a library class can be looked at but not edited.
            if editable {
                ui.label(
                    egui::RichText::new("Drag to move · Ctrl+R rotates").color(theme::TEXT_WEAK),
                );
            } else {
                ui.label(egui::RichText::new("read-only (library class)").color(theme::TEXT_WEAK));
            }
        });
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::WHITE);
        // wheel zoom around cursor
        let scroll = ui.input(|i| i.zoom_delta());
        if scroll != 1.0 && scroll > 0.0 {
            self.diagram_zoom = (self.diagram_zoom * scroll).clamp(0.1, 20.0);
        }
        let zoom = self.diagram_zoom;
        let pan = self.diagram_pan;
        let diagram = match self.model_diagrams.get(class) {
            Some(Loadable::Ready(d)) => d.clone(),
            Some(Loadable::Failed(e)) => {
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    e.clone(),
                    FontId::proportional(13.0),
                    theme::TEXT_WEAK,
                );
                return;
            }
            _ => {
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    "Loading Diagram ...",
                    FontId::proportional(13.0),
                    theme::TEXT_WEAK,
                );
                return;
            }
        };
        let extent = diagram.coordinate_extent;
        // ---------------------------------------------------------------- editing (G29)
        let pointer = response.interact_pointer_pos();
        let to_diagram =
            |pos: egui::Pos2| crate::diagram_view::to_diagram(pos, &extent, rect, zoom, pan);
        // Middle/right drag pans; a left drag on a component moves it.
        if response.dragged() {
            let secondary = ui.input(|i| i.pointer.secondary_down() || i.pointer.middle_down());
            if secondary {
                self.diagram_pan += response.drag_delta();
            }
        }
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(pos) = pointer {
                match crate::diagram::hit_component(&diagram, to_diagram(pos)) {
                    Some(index) => {
                        let comp = &diagram.components[index];
                        self.selected_component = Some(comp.name.clone());
                        self.component_expanded.insert(comp.name.clone());
                        // A library class may be selected, but not moved.
                        self.diagram_drag = editable.then(|| crate::app::DiagramDrag {
                            component: comp.name.clone(),
                            start: to_diagram(pos),
                            offset: [0.0, 0.0],
                        });
                    }
                    None => {
                        self.selected_component = None;
                        self.diagram_drag = None;
                    }
                }
            }
        }
        if let (Some(drag), Some(pos)) = (self.diagram_drag.clone(), pointer) {
            if response.dragged_by(egui::PointerButton::Primary) {
                let p = to_diagram(pos);
                let raw = [p[0] - drag.start[0], p[1] - drag.start[1]];
                // Shift drags without snapping.
                let offset = if ui.input(|i| i.modifiers.shift) {
                    raw
                } else {
                    [
                        crate::diagram_view::snap(raw[0], crate::diagram_view::GRID),
                        crate::diagram_view::snap(raw[1], crate::diagram_view::GRID),
                    ]
                };
                if let Some(active) = &mut self.diagram_drag {
                    active.offset = offset;
                }
            }
        }
        if response.drag_stopped_by(egui::PointerButton::Primary) {
            if let Some(drag) = self.diagram_drag.take() {
                // A plain click is not a move, so it must not send an edit.
                if let Some(comp) = diagram.components.iter().find(|c| c.name == drag.component) {
                    if let Some(placement) = crate::app::dragged(&comp.placement, drag.offset) {
                        let (name, type_name) = (comp.name.clone(), comp.type_name.clone());
                        self.move_component(class, &name, &type_name, placement);
                    }
                }
            }
        }
        if response.clicked() {
            self.selected_component = pointer
                .map(to_diagram)
                .and_then(|p| crate::diagram::hit_component(&diagram, p))
                .map(|i| diagram.components[i].name.clone());
        }
        // A class dragged out of the Package Browser is added where it was dropped (G30).
        // `interact_pointer_pos` is empty here because the drag belongs to the browser row,
        // so the pointer has to come from the context.
        if let Some(payload) = response.dnd_release_payload::<crate::app::ClassPayload>() {
            if let Some(pos) = ui.ctx().pointer_interact_pos() {
                self.add_component_at(class, &payload, to_diagram(pos));
            }
        }
        // Arrow keys nudge the selection by one grid step; Ctrl+R rotates it by 90 degrees.
        if editable && response.hovered() && ui.ctx().memory(|m| m.focused()).is_none() {
            if let Some(name) = self.selected_component.clone() {
                if let Some(comp) = diagram.components.iter().find(|c| c.name == name) {
                    let mut step = [0.0, 0.0];
                    let mut rotate = false;
                    let mut delete = false;
                    ui.input_mut(|i| {
                        if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft) {
                            step[0] -= crate::diagram_view::GRID;
                        }
                        if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight) {
                            step[0] += crate::diagram_view::GRID;
                        }
                        if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                            step[1] += crate::diagram_view::GRID;
                        }
                        if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                            step[1] -= crate::diagram_view::GRID;
                        }
                        rotate = i.consume_key(egui::Modifiers::COMMAND, egui::Key::R);
                        delete = i.consume_key(egui::Modifiers::NONE, egui::Key::Delete);
                    });
                    if delete {
                        self.delete_component(class, &name);
                    } else if step != [0.0, 0.0] || rotate {
                        let mut placement = crate::app::dragged(&comp.placement, step)
                            .unwrap_or_else(|| comp.placement.clone());
                        if rotate {
                            // Modelica rotations are counter-clockwise.
                            placement.rotation_deg = (placement.rotation_deg + 90.0) % 360.0;
                        }
                        let (name, type_name) = (comp.name.clone(), comp.type_name.clone());
                        self.move_component(class, &name, &type_name, placement);
                    }
                }
            }
        }
        // ---------------------------------------------------------------- painting
        // The move is shown before OMC has rewritten the text: the drag offset while the
        // mouse is down, the sent placement until `Event::ModelEdited` comes back.
        let mut shown = diagram;
        if let Some((name, placement)) = &self.pending_placement {
            if let Some(comp) = shown.components.iter_mut().find(|c| &c.name == name) {
                comp.placement = placement.clone();
            }
        }
        if let Some(drag) = &self.diagram_drag {
            if let Some(comp) = shown
                .components
                .iter_mut()
                .find(|c| c.name == drag.component)
            {
                comp.placement.origin = [
                    comp.placement.origin[0] + drag.offset[0],
                    comp.placement.origin[1] + drag.offset[1],
                ];
            }
        }
        crate::diagram_view::paint_diagram(&painter, rect, &shown, zoom, pan);
        if let Some(name) = &self.selected_component {
            if let Some(comp) = shown.components.iter().find(|c| &c.name == name) {
                let bounds =
                    crate::diagram_view::component_screen_rect(&shown, rect, zoom, pan, comp);
                crate::diagram_view::paint_selection(&painter, bounds);
            }
        }
        // Right-click menu (G31). The canvas stays the only interactive widget, so the
        // target is remembered when the right click lands on a component; a right click
        // on empty space clears it and opens nothing.
        if response.secondary_clicked() {
            self.context_component = pointer
                .map(to_diagram)
                .and_then(|p| crate::diagram::hit_component(&shown, p))
                .map(|i| shown.components[i].name.clone());
            if let Some(name) = self.context_component.clone() {
                self.selected_component = Some(name);
            }
        }
        if let Some(name) = self.context_component.clone() {
            if let Some(comp) = shown.components.iter().find(|c| c.name == name) {
                let placement = comp.placement.clone();
                let type_name = comp.type_name.clone();
                response.context_menu(|ui| {
                    if !editable {
                        ui.add_enabled(false, egui::Button::new("Rename..."));
                        ui.add_enabled(false, egui::Button::new("Delete"));
                        ui.separator();
                        ui.add_enabled(false, egui::Button::new("Rotate 90"));
                        return;
                    }
                    if ui.button("Rename...").clicked() {
                        self.open_rename_dialog(class, &name);
                        ui.close();
                    }
                    if ui.button("Delete").clicked() {
                        self.delete_component(class, &name);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Rotate 90").clicked() {
                        let mut rotated = placement.clone();
                        rotated.rotation_deg = (rotated.rotation_deg + 90.0) % 360.0;
                        self.move_component(class, &name, &type_name, rotated);
                        ui.close();
                    }
                });
            }
        }
        // Tooltip of the component under the pointer.
        if let Some(pos) = response.hover_pos() {
            if let Some(index) = crate::diagram::hit_component(&shown, to_diagram(pos)) {
                let comp = &shown.components[index];
                response.on_hover_text(format!("{} : {}", comp.name, comp.type_name));
            }
        }
    }

    pub fn plot_window(&mut self, ui: &mut egui::Ui) {
        let (panel, _) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let painter = ui.painter_at(panel);
        painter.rect_filled(panel, 0.0, Color32::WHITE);
        // The curves borrow the results instead of copying them (G21), so every change
        // to the app state is collected here and applied after the drawing loop.
        let diagrams: Vec<Vec<Curve>> = self
            .diagrams
            .iter()
            .map(|diagram| self.curves(&diagram.signals))
            .collect();
        if diagrams.is_empty() {
            return;
        }
        if diagrams.iter().all(Vec::is_empty) {
            painter.text(
                panel.center(),
                Align2::CENTER_CENTER,
                "No curves. Tick a variable in the Variable Browser.",
                FontId::proportional(12.5),
                theme::TEXT_WEAK,
            );
        }
        // Every diagram shares one time axis.
        let x_range = self.plot_x.or_else(|| {
            diagrams
                .iter()
                .filter_map(|curves| data_bounds(curves))
                .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1), a.2.min(b.2), a.3.max(b.3)))
                .map(|(x0, x1, _, _)| (x0, x1))
        });
        let count = diagrams.len();
        let slot_height = panel.height() / count as f32;
        let cursor_enabled = self.cursor_enabled;
        let mut set_active: Option<usize> = None;
        let mut set_x: Option<Option<(f64, f64)>> = None;
        let mut set_y: Vec<(usize, Option<(f64, f64)>)> = Vec::new();
        for (index, curves) in diagrams.iter().enumerate() {
            let slot = egui::Rect::from_min_size(
                egui::pos2(panel.left(), panel.top() + index as f32 * slot_height),
                egui::vec2(panel.width(), slot_height),
            );
            let frame = egui::Rect::from_min_max(
                egui::pos2(slot.left() + 53.0, slot.top() + 31.0),
                egui::pos2(slot.right() - 24.0, slot.bottom() - 29.0),
            );
            if frame.width() < 40.0 || frame.height() < 40.0 {
                continue;
            }
            // Legend above the frame.
            let mut lx = frame.left() - 2.0;
            for curve in curves {
                let y = slot.top() + 16.5;
                painter.hline(lx..=lx + 26.0, y, Stroke::new(1.0, curve.color));
                let galley = painter.layout_no_wrap(
                    curve.name.clone(),
                    FontId::proportional(12.0),
                    theme::TEXT,
                );
                let width = galley.size().x;
                painter.galley(
                    egui::pos2(lx + 31.0, y - galley.size().y / 2.0),
                    galley,
                    theme::TEXT,
                );
                lx += 31.0 + width + 18.0;
            }
            let active = index == self.active_diagram;
            let border = if active {
                theme::ACCENT
            } else {
                Color32::from_rgb(0x3A, 0x3A, 0x3A)
            };
            let Some((x0, x1)) = x_range else {
                painter.rect_stroke(
                    frame,
                    0.0,
                    Stroke::new(1.0, border),
                    egui::StrokeKind::Inside,
                );
                continue;
            };
            // y: the diagram's own zoom, else fitted to its curves.
            let fitted = data_bounds(curves).map(|(_, _, y0, y1)| {
                let (y0, y1, step) = nice_range(y0, y1, frame.height() / 50.0);
                ((y0, y1), step)
            });
            let y_step = fitted.map(|(_, step)| step);
            let y_range = self.diagrams[index]
                .y_view
                .or(fitted.map(|(range, _)| range));
            let Some(y_range) = y_range else {
                painter.rect_stroke(
                    frame,
                    0.0,
                    Stroke::new(1.0, border),
                    egui::StrokeKind::Inside,
                );
                continue;
            };
            let map = Mapping {
                frame,
                x: sane(x0, x1),
                y: sane(y_range.0, y_range.1),
            };
            draw_diagram(&painter, &map, curves, y_step);

            // Interaction: click selects, drag a rectangle zooms, double-click resets.
            let response = ui.interact(
                frame,
                ui.scope_id().with(("plot_diagram", index)),
                Sense::click_and_drag(),
            );
            if response.clicked() {
                set_active = Some(index);
            }
            if response.double_clicked() {
                set_x = Some(None);
                set_y.push((index, None));
            }
            let drag_id = ui.scope_id().with(("plot_drag", index));
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
                        let (ax, ay) = map.to_data(r.left_bottom());
                        let (bx, by) = map.to_data(r.right_top());
                        // x is shared by every diagram; y belongs to this one.
                        set_x = Some(Some((ax, bx)));
                        set_y.push((index, Some((ay, by))));
                        set_active = Some(index);
                    }
                }
            }
            if let Some(p) = response.hover_pos().filter(|p| frame.contains(*p)) {
                let (t, v) = map.to_data(p);
                if cursor_enabled {
                    draw_cursor(&painter, &map, curves, t);
                }
                response.on_hover_text_at_pointer(format!(
                    "t = {}\ny = {}",
                    crate::ui::browsers_format(t),
                    crate::ui::browsers_format(v)
                ));
            }
        }
        // The borrow of the curves ends with the loop; now the state can change.
        if let Some(index) = set_active {
            self.active_diagram = index.min(self.diagrams.len().saturating_sub(1));
        }
        if let Some(x) = set_x {
            self.plot_x = x;
        }
        for (index, y) in set_y {
            if let Some(diagram) = self.diagrams.get_mut(index) {
                diagram.y_view = y;
            }
        }
    }

    /// Curves of `signals`; the name carries the run label when several results exist.
    ///
    /// The time axis and the values are borrowed from the result instead of being
    /// copied on every frame (G21).
    fn curves<'a>(&'a self, signals: &'a [PlotSignal]) -> Vec<Curve<'a>> {
        signals
            .iter()
            .enumerate()
            .filter_map(|(index, signal)| {
                let run = self.run(signal.run_id)?;
                let values = run.outcome.result.series(&signal.variable)?;
                let name = if self.runs.len() > 1 {
                    format!("{}.{}", run.label, signal.variable)
                } else {
                    signal.variable.clone()
                };
                Some(Curve {
                    name,
                    color: CURVE_COLORS[index % CURVE_COLORS.len()],
                    time: &run.outcome.result.time,
                    values,
                })
            })
            .collect()
    }
}

/// Where a diagram puts its data: the frame on screen and the ranges inside it.
struct Mapping {
    frame: egui::Rect,
    x: (f64, f64),
    y: (f64, f64),
}

impl Mapping {
    fn to_screen(&self, x: f64, y: f64) -> egui::Pos2 {
        egui::pos2(
            self.frame.left()
                + ((x - self.x.0) / (self.x.1 - self.x.0)) as f32 * self.frame.width(),
            self.frame.bottom()
                - ((y - self.y.0) / (self.y.1 - self.y.0)) as f32 * self.frame.height(),
        )
    }

    fn to_data(&self, p: egui::Pos2) -> (f64, f64) {
        (
            self.x.0
                + ((p.x - self.frame.left()) / self.frame.width()) as f64 * (self.x.1 - self.x.0),
            self.y.0
                + ((self.frame.bottom() - p.y) / self.frame.height()) as f64
                    * (self.y.1 - self.y.0),
        )
    }
}

/// Guard against a zero-width range so the mapping never divides by zero.
fn sane(a: f64, b: f64) -> (f64, f64) {
    if b > a {
        (a, b)
    } else {
        (a - 1.0, a + 1.0)
    }
}

/// Grid, tick labels and curves of one diagram.
fn draw_diagram(painter: &egui::Painter, map: &Mapping, curves: &[Curve], y_step: Option<f64>) {
    let frame = map.frame;
    let (x0, x1) = map.x;
    let (y0, y1) = map.y;
    let x_major = nice_step((x1 - x0) / (frame.width() / 130.0).max(1.0) as f64);
    let y_major =
        y_step.unwrap_or_else(|| nice_step((y1 - y0) / (frame.height() / 50.0).max(1.0) as f64));
    let grid = Stroke::new(1.0, Color32::from_rgb(0xEA, 0xEA, 0xEA));
    let axis = Color32::from_rgb(0x3A, 0x3A, 0x3A);
    for x in ticks(x0, x1, minor_step(x_major)) {
        let p = map.to_screen(x, y0);
        painter.vline(p.x.round() + 0.5, frame.y_range(), grid);
    }
    for y in ticks(y0, y1, minor_step(y_major)) {
        let p = map.to_screen(x0, y);
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
        let p = map.to_screen(x, y0);
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
        let p = map.to_screen(x0, y);
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
    // One column per pixel is all the screen can show (G21).
    let columns = frame.width().ceil().max(1.0) as usize;
    for curve in curves {
        let indices = crate::decimate::min_max_indices(curve.time, curve.values, x0, x1, columns);
        let points: Vec<egui::Pos2> = indices
            .iter()
            .map(|&i| map.to_screen(curve.time[i], curve.values[i]))
            .collect();
        clip.add(egui::Shape::line(points, Stroke::new(1.0, curve.color)));
    }
    painter.rect_stroke(frame, 0.0, Stroke::new(1.0, axis), egui::StrokeKind::Inside);
}

/// Measurement cursor: a vertical line at `t` and `name = value` for every curve.
fn draw_cursor(painter: &egui::Painter, map: &Mapping, curves: &[Curve], t: f64) {
    let x = map.to_screen(t, map.y.0).x.round() + 0.5;
    painter.vline(
        x,
        map.frame.y_range(),
        Stroke::new(1.0, Color32::from_rgb(0x80, 0x80, 0x80)),
    );
    let font = FontId::proportional(11.5);
    let mut y = map.frame.top() + 4.0;
    for curve in curves {
        // The cursor value comes from the full data, not from the decimated line.
        let Some(value) = crate::series::value_at(curve.time, curve.values, t) else {
            continue;
        };
        painter.text(
            egui::pos2(map.frame.right() - 4.0, y),
            Align2::RIGHT_TOP,
            format!("{} = {}", curve.name, crate::ui::browsers_format(value)),
            font.clone(),
            curve.color,
        );
        y += 14.0;
    }
}

struct Curve<'a> {
    name: String,
    color: Color32,
    time: &'a [f64],
    values: &'a [f64],
}

fn data_bounds(curves: &[Curve<'_>]) -> Option<(f64, f64, f64, f64)> {
    let mut bounds: Option<(f64, f64, f64, f64)> = None;
    for curve in curves {
        for (x, y) in curve.time.iter().zip(curve.values) {
            if !x.is_finite() || !y.is_finite() {
                continue;
            }
            bounds = Some(match bounds {
                None => (*x, *x, *y, *y),
                Some((a, b, c, d)) => (a.min(*x), b.max(*x), c.min(*y), d.max(*y)),
            });
        }
    }
    bounds
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
    use super::{format_tick, tab_label};

    #[test]
    fn tab_label_marks_unsaved_documents() {
        assert_eq!(tab_label("BouncingBall", false), "BouncingBall");
        assert_eq!(tab_label("BouncingBall", true), "BouncingBall*");
        // Nested classes show the short name only.
        assert_eq!(tab_label("Modelica.Blocks.Gain", true), "Gain*");
    }

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

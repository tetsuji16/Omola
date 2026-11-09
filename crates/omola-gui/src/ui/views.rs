//! Central views: model tabs + edit window (per layer) and the plot window.

use super::widgets::{flat_tab, paint_icon};
use crate::app::{Layer, Loadable, OmolaApp, PlotSignal};
use crate::text_editor::{draw_text_editor, lines_for_file};
use crate::theme;
use egui::{Align2, Color32, FontId, Rect, RichText, Sense, Stroke, Vec2};
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
                let font = FontId::monospace(13.0);
                let layout = self
                    .library_text_layouts
                    .entry(class.to_owned())
                    .or_insert_with(|| crate::text_editor::highlight_job(text, &font))
                    .clone();
                let mut text = text.as_str();
                egui::ScrollArea::both()
                    .id_salt(("lib_text", class))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let mut layouter =
                            |ui: &egui::Ui, _buffer: &dyn egui::TextBuffer, _wrap| {
                                ui.ctx().fonts_mut(|fonts| fonts.layout_job(layout.clone()))
                            };
                        ui.add(
                            egui::TextEdit::multiline(&mut text)
                                .code_editor()
                                .font(font)
                                .layouter(&mut layouter)
                                .frame(egui::Frame::NONE)
                                .desired_width(f32::INFINITY),
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

    /// Graphics tab > Diagram: selectable and movable model components (G29).
    fn diagram_layer(&mut self, ui: &mut egui::Ui, class: &str) {
        self.request_diagram(class);
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
        });
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, Color32::WHITE);
        // wheel zoom around cursor
        let scroll = ui.input(|i| i.zoom_delta());
        if scroll != 1.0 && scroll > 0.0 {
            self.diagram_zoom = (self.diagram_zoom * scroll).clamp(0.1, 20.0);
        }
        // Pan with middle/right drag. Primary drag is reserved for component movement.
        if response.dragged() {
            let secondary = ui.input(|i| i.pointer.secondary_down() || i.pointer.middle_down());
            if secondary {
                self.diagram_pan += response.drag_delta();
            }
        }
        let diagram = match self.model_diagrams.get(class) {
            Some(Loadable::Ready(d)) => d.clone(),
            Some(Loadable::Loading) => {
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    "Loading Diagram ...",
                    FontId::proportional(13.0),
                    theme::TEXT_WEAK,
                );
                return;
            }
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
            None => {
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
        let user_class = self.is_user_class(class);
        let mut draw_diagram = diagram.clone();
        if let Some((component, placement)) = &self.pending_placement {
            if let Some(comp) = draw_diagram
                .components
                .iter_mut()
                .find(|c| &c.name == component)
            {
                comp.placement = placement.clone();
            }
        }
        if let Some(drag) = &self.diagram_drag {
            if let Some(comp) = draw_diagram
                .components
                .iter_mut()
                .find(|c| c.name == drag.component)
            {
                comp.placement.origin[0] += drag.offset[0];
                comp.placement.origin[1] += drag.offset[1];
            }
        }
        let primary_drag_started = (response.drag_started() || response.dragged())
            && ui.input(|i| {
                i.pointer.button_down(egui::PointerButton::Primary)
                    || i.pointer.button_pressed(egui::PointerButton::Primary)
                    || i.pointer.button_released(egui::PointerButton::Primary)
            });
        if primary_drag_started {
            let press_origin = ui.input(|i| i.pointer.press_origin());
            if let Some(pos) = press_origin.or_else(|| response.interact_pointer_pos()) {
                let point = crate::diagram_view::to_diagram(
                    pos,
                    &diagram.coordinate_extent,
                    rect,
                    self.diagram_zoom,
                    self.diagram_pan,
                );
                let pin_hit = crate::diagram_view::hit_pin(
                    &diagram,
                    point,
                    6.0 / diagram_scale(&diagram, rect, self.diagram_zoom),
                );
                if let Some((ci, pi)) = pin_hit.filter(|_| user_class) {
                    let comp = &diagram.components[ci];
                    let pin = &comp.connectors[pi];
                    let at = crate::diagram_view::pin_position(comp, pin);
                    self.connection_drag = Some(crate::app::ConnectionDrag {
                        from: format!("{}.{}", comp.name, pin.name),
                        start: at,
                        current: point,
                    });
                } else if let Some(i) = crate::diagram_view::hit_component(&diagram, point) {
                    let name = diagram.components[i].name.clone();
                    self.selected_component = Some(name.clone());
                    self.component_expanded.insert(name.clone());
                    if user_class {
                        self.diagram_drag = Some(crate::app::DiagramDrag {
                            component: name,
                            start: point,
                            offset: [0.0, 0.0],
                        });
                    }
                } else {
                    self.selected_component = None;
                }
            }
        }
        if response.dragged_by(egui::PointerButton::Primary) {
            if let (Some(drag), Some(pos)) =
                (&mut self.connection_drag, response.interact_pointer_pos())
            {
                drag.current = crate::diagram_view::to_diagram(
                    pos,
                    &diagram.coordinate_extent,
                    rect,
                    self.diagram_zoom,
                    self.diagram_pan,
                );
            }
            if let (Some(drag), Some(pos)) =
                (&mut self.diagram_drag, response.interact_pointer_pos())
            {
                let current = crate::diagram_view::to_diagram(
                    pos,
                    &diagram.coordinate_extent,
                    rect,
                    self.diagram_zoom,
                    self.diagram_pan,
                );
                drag.offset = [current[0] - drag.start[0], current[1] - drag.start[1]];
            }
        }
        if response.drag_stopped_by(egui::PointerButton::Primary) {
            if let Some(drag) = self.connection_drag.take() {
                if user_class && self.pending_edit.is_none() {
                    if let Some(pos) = response
                        .hover_pos()
                        .or_else(|| response.interact_pointer_pos())
                    {
                        let p = crate::diagram_view::to_diagram(
                            pos,
                            &diagram.coordinate_extent,
                            rect,
                            self.diagram_zoom,
                            self.diagram_pan,
                        );
                        if let Some((ci, pi)) = crate::diagram_view::hit_pin(
                            &diagram,
                            p,
                            6.0 / diagram_scale(&diagram, rect, self.diagram_zoom),
                        ) {
                            let comp = &diagram.components[ci];
                            let pin = &comp.connectors[pi];
                            let to = format!("{}.{}", comp.name, pin.name);
                            let end = crate::diagram_view::pin_position(comp, pin);
                            let route = crate::diagram_view::orthogonal_route(drag.start, end);
                            if let Some(op) =
                                crate::diagram::new_connection_op(&diagram, drag.from, to, route)
                            {
                                self.selected_connection = None;
                                self.apply_edit(class, vec![op]);
                            }
                        }
                    }
                }
            }
            if let Some(drag) = self.diagram_drag.take() {
                let final_offset = response
                    .hover_pos()
                    .map(|pos| {
                        let current = crate::diagram_view::to_diagram(
                            pos,
                            &diagram.coordinate_extent,
                            rect,
                            self.diagram_zoom,
                            self.diagram_pan,
                        );
                        [current[0] - drag.start[0], current[1] - drag.start[1]]
                    })
                    .unwrap_or(drag.offset);
                if final_offset != [0.0, 0.0] {
                    if let Some(comp) = diagram.components.iter().find(|c| c.name == drag.component)
                    {
                        let shift = ui.input(|i| i.modifiers.shift);
                        let delta = if shift {
                            final_offset
                        } else {
                            [
                                crate::diagram_view::snap(final_offset[0], 2.0),
                                crate::diagram_view::snap(final_offset[1], 2.0),
                            ]
                        };
                        if delta != [0.0, 0.0] && self.pending_edit.is_none() {
                            let (placement, _) =
                                crate::diagram_view::placement_edit(comp, delta, 0.0);
                            self.pending_placement = Some((comp.name.clone(), placement.clone()));
                            self.apply_edit(
                                class,
                                crate::diagram::move_component_ops(
                                    &diagram, &comp.name, &placement,
                                ),
                            );
                        }
                    }
                }
            }
        }
        // Single-step keyboard transforms on a hovered canvas.
        if response.hovered() {
            let selected = self.selected_component.clone();
            let movement = ui.input(|i| {
                if i.key_pressed(egui::Key::ArrowUp) {
                    Some([0.0, 2.0])
                } else if i.key_pressed(egui::Key::ArrowDown) {
                    Some([0.0, -2.0])
                } else if i.key_pressed(egui::Key::ArrowLeft) {
                    Some([-2.0, 0.0])
                } else if i.key_pressed(egui::Key::ArrowRight) {
                    Some([2.0, 0.0])
                } else {
                    None
                }
            });
            let rotate = ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::R));
            let delete = ui.input(|i| i.key_pressed(egui::Key::Delete));
            if delete
                && user_class
                && self.pending_edit.is_none()
                && self.selected_connection.is_some()
            {
                if let Some(index) = self.selected_connection.take() {
                    if let Some(c) = diagram.connections.get(index) {
                        self.apply_edit(
                            class,
                            vec![crate::edit::EditOp::DeleteConnection {
                                from: c.from.clone(),
                                to: c.to.clone(),
                            }],
                        );
                    }
                }
            } else if delete && user_class && self.selected_component.is_some() {
                self.delete_selected_component();
            }
            if let Some(name) = selected {
                if user_class && self.pending_edit.is_none() {
                    if let Some(comp) = diagram.components.iter().find(|c| c.name == name) {
                        if movement.is_some() || rotate {
                            let (p, _) = crate::diagram_view::placement_edit(
                                comp,
                                movement.unwrap_or([0.0, 0.0]),
                                if rotate { 90.0 } else { 0.0 },
                            );
                            let ops = crate::diagram::move_component_ops(&diagram, &name, &p);
                            self.pending_placement = Some((name, p));
                            self.apply_edit(class, ops);
                        }
                    }
                }
            }
        }
        crate::diagram_view::paint_diagram(
            &painter,
            rect,
            &draw_diagram,
            self.diagram_zoom,
            self.diagram_pan,
        );
        let scale = diagram_scale(&diagram, rect, self.diagram_zoom);
        let hovered_diagram = response.hover_pos().map(|pos| {
            crate::diagram_view::to_diagram(
                pos,
                &diagram.coordinate_extent,
                rect,
                self.diagram_zoom,
                self.diagram_pan,
            )
        });
        if let Some(p) = hovered_diagram {
            if let Some((ci, pi)) = crate::diagram_view::hit_pin(&diagram, p, 6.0 / scale) {
                let c = &diagram.components[ci];
                let at = crate::diagram_view::pin_position(c, &c.connectors[pi]);
                painter.circle_filled(
                    crate::diagram_view::to_screen(
                        at,
                        &diagram.coordinate_extent,
                        rect,
                        self.diagram_zoom,
                        self.diagram_pan,
                    ),
                    5.0,
                    theme::ACCENT,
                );
            }
        }
        for comp in &diagram.components {
            for pin in &comp.connectors {
                let at = crate::diagram_view::pin_position(comp, pin);
                let screen = crate::diagram_view::to_screen(
                    at,
                    &diagram.coordinate_extent,
                    rect,
                    self.diagram_zoom,
                    self.diagram_pan,
                );
                painter.circle_filled(screen, 2.5, Color32::from_rgb(40, 90, 150));
            }
        }
        if let Some(index) = self
            .selected_connection
            .filter(|i| *i < diagram.connections.len())
        {
            let conn = &diagram.connections[index];
            let pts: Vec<_> = conn
                .points
                .iter()
                .map(|p| {
                    crate::diagram_view::to_screen(
                        *p,
                        &diagram.coordinate_extent,
                        rect,
                        self.diagram_zoom,
                        self.diagram_pan,
                    )
                })
                .collect();
            if pts.len() >= 2 {
                painter.add(egui::Shape::line(pts, Stroke::new(3.0, theme::ACCENT)));
            }
        }
        if let Some(drag) = &self.connection_drag {
            let route = crate::diagram_view::orthogonal_route(drag.start, drag.current);
            let pts: Vec<_> = route
                .iter()
                .map(|p| {
                    crate::diagram_view::to_screen(
                        *p,
                        &diagram.coordinate_extent,
                        rect,
                        self.diagram_zoom,
                        self.diagram_pan,
                    )
                })
                .collect();
            if pts.len() >= 2 {
                painter.add(egui::Shape::line(pts, Stroke::new(2.0, theme::ACCENT)));
            }
        }
        // Selection outline and four grab handles.
        if let Some(name) = self.selected_component.as_deref() {
            if let Some(comp) = draw_diagram.components.iter().find(|c| c.name == name) {
                let r = crate::diagram_view::component_screen_rect(
                    &draw_diagram,
                    rect,
                    self.diagram_zoom,
                    self.diagram_pan,
                    comp,
                );
                let color = theme::ACCENT;
                let points = [
                    r.left_top(),
                    r.right_top(),
                    r.right_bottom(),
                    r.left_bottom(),
                ];
                for i in 0..4 {
                    painter.add(egui::Shape::Vec(egui::Shape::dashed_line(
                        &[points[i], points[(i + 1) % 4]],
                        Stroke::new(1.0, color),
                        4.0,
                        3.0,
                    )));
                }
                for p in points {
                    painter.rect_filled(Rect::from_center_size(p, Vec2::splat(6.0)), 0.0, color);
                }
            }
        }
        // Hover / click on components. Component Browser selection stays synchronized.
        if let Some(pos) = response.hover_pos() {
            let point = crate::diagram_view::to_diagram(
                pos,
                &diagram.coordinate_extent,
                rect,
                self.diagram_zoom,
                self.diagram_pan,
            );
            if let Some(i) = crate::diagram_view::hit_component(&diagram, point) {
                let comp = &diagram.components[i];
                response.clone().on_hover_text(if user_class {
                    format!("{} : {}", comp.name, comp.type_name)
                } else {
                    format!("{} : {} (read-only)", comp.name, comp.type_name)
                });
                if response.clicked_by(egui::PointerButton::Primary) {
                    if let Some(ci) =
                        crate::diagram_view::hit_connection(&diagram, point, 4.0 / scale)
                    {
                        self.selected_connection = Some(ci);
                        self.selected_component = None;
                    } else {
                        self.selected_connection = None;
                        self.selected_component = Some(comp.name.clone());
                        self.component_expanded.insert(comp.name.clone());
                    }
                }
            } else if response.clicked_by(egui::PointerButton::Primary) {
                self.selected_connection =
                    crate::diagram_view::hit_connection(&diagram, point, 4.0 / scale);
                self.selected_component = None;
            }
            if !user_class
                && response.drag_started_by(egui::PointerButton::Primary)
                && crate::diagram_view::hit_component(&diagram, point).is_some()
            {
                self.info(crate::backend::LogTab::Syntax, "read-only".into());
            }
        }
        if response.secondary_clicked() {
            if let Some(pos) = response.hover_pos() {
                let p = crate::diagram_view::to_diagram(
                    pos,
                    &diagram.coordinate_extent,
                    rect,
                    self.diagram_zoom,
                    self.diagram_pan,
                );
                if let Some(i) = crate::diagram_view::hit_component(&diagram, p) {
                    self.selected_component = Some(diagram.components[i].name.clone());
                }
            }
        }
        let mut action = None;
        response.context_menu(|ui| {
            let selected = self.selected_component.clone();
            let enabled = user_class
                && selected
                    .as_ref()
                    .is_some_and(|n| diagram.components.iter().any(|c| &c.name == n));
            if ui
                .add_enabled(enabled, egui::Button::new("Rename..."))
                .clicked()
            {
                action = Some("rename");
                ui.close();
            }
            if ui
                .add_enabled(enabled, egui::Button::new("Delete"))
                .clicked()
            {
                action = Some("delete");
                ui.close();
            }
            ui.separator();
            if ui
                .add_enabled(enabled, egui::Button::new("Rotate 90"))
                .clicked()
            {
                action = Some("rotate");
                ui.close();
            }
        });
        if let Some(name) = self.selected_component.clone() {
            match action {
                Some("rename") => {
                    self.rename_dialog = Some(crate::app::RenameDialog {
                        class: class.to_owned(),
                        old: name.clone(),
                        new_text: name,
                    })
                }
                Some("delete") => self.delete_selected_component(),
                Some("rotate") => {
                    if let Some(comp) = diagram.components.iter().find(|c| c.name == name) {
                        let (p, op) = crate::diagram_view::placement_edit(comp, [0.0, 0.0], 90.0);
                        self.pending_placement = Some((name, p));
                        self.apply_edit(class, vec![op]);
                    }
                }
                _ => {}
            }
        }
    }

    pub fn plot_window(&mut self, ui: &mut egui::Ui) {
        let (panel, _) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let painter = ui.painter_at(panel);
        painter.rect_filled(panel, 0.0, Color32::WHITE);
        // Collect the curves first so the borrow of `runs` ends before drawing.
        let diagrams: Vec<Vec<Curve<'_>>> = self
            .diagrams
            .iter()
            .map(|diagram| Self::curves(&self.runs, &diagram.signals))
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
        let x_range = self
            .plot_x
            .or_else(|| data_bounds(diagrams.iter().flatten()).map(|(x0, x1, _, _)| (x0, x1)));
        let count = diagrams.len();
        let slot_height = panel.height() / count as f32;
        let cursor_enabled = self.cursor_enabled;
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
            let fitted = data_bounds(curves.iter()).map(|(_, _, y0, y1)| {
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
                self.active_diagram = index;
            }
            if response.double_clicked() {
                self.plot_x = None;
                self.diagrams[index].y_view = None;
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
                        self.plot_x = Some((ax, bx));
                        self.diagrams[index].y_view = Some((ay, by));
                        self.active_diagram = index;
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
    }

    /// Curves of `signals`; the name carries the run label when several results exist.
    fn curves<'a>(runs: &'a [crate::app::ResultRun], signals: &[PlotSignal]) -> Vec<Curve<'a>> {
        signals
            .iter()
            .enumerate()
            .filter_map(|(index, signal)| {
                let run = runs.iter().find(|run| run.id == signal.run_id)?;
                let values = run.outcome.result.series(&signal.variable)?;
                let name = if runs.len() > 1 {
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
fn diagram_scale(diagram: &crate::diagram::Diagram, rect: egui::Rect, zoom: f32) -> f64 {
    let e = diagram.coordinate_extent;
    let width = (e.x2 - e.x1).abs().max(1e-9);
    let height = (e.y2 - e.y1).abs().max(1e-9);
    ((rect.width() / width as f32).min(rect.height() / height as f32) * zoom).max(1e-6) as f64
}

fn sane(a: f64, b: f64) -> (f64, f64) {
    if b > a {
        (a, b)
    } else {
        (a - 1.0, a + 1.0)
    }
}

/// Grid, tick labels and curves of one diagram.
fn draw_diagram(painter: &egui::Painter, map: &Mapping, curves: &[Curve<'_>], y_step: Option<f64>) {
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
    for curve in curves {
        let indices = crate::decimate::min_max_indices(
            curve.time,
            curve.values,
            map.x.0,
            map.x.1,
            frame.width().ceil() as usize,
        );
        let points: Vec<egui::Pos2> = indices
            .into_iter()
            .map(|i| map.to_screen(curve.time[i], curve.values[i]))
            .collect();
        clip.add(egui::Shape::line(points, Stroke::new(1.0, curve.color)));
    }
    painter.rect_stroke(frame, 0.0, Stroke::new(1.0, axis), egui::StrokeKind::Inside);
}

/// Measurement cursor: a vertical line at `t` and `name = value` for every curve.
fn draw_cursor(painter: &egui::Painter, map: &Mapping, curves: &[Curve<'_>], t: f64) {
    let x = map.to_screen(t, map.y.0).x.round() + 0.5;
    painter.vline(
        x,
        map.frame.y_range(),
        Stroke::new(1.0, Color32::from_rgb(0x80, 0x80, 0x80)),
    );
    let font = FontId::proportional(11.5);
    let mut y = map.frame.top() + 4.0;
    for curve in curves {
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

fn data_bounds<'a>(curves: impl Iterator<Item = &'a Curve<'a>>) -> Option<(f64, f64, f64, f64)> {
    let mut bounds: Option<(f64, f64, f64, f64)> = None;
    for curve in curves {
        for (x, y) in curve.time.iter().zip(curve.values.iter()) {
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

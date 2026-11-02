//! Docked browsers: package + component browser (modeling) and variable browser (simulation).

use super::widgets::{dock_header, icon_button, paint_icon};
use crate::app::{LibraryState, OmolaApp, PlotSignal};
use crate::theme;
use egui::{Align2, Color32, FontId, RichText, Sense, Stroke, Vec2};
use egui_material_icons::icons::*;
use egui_material_icons::MaterialIcon;

const ROW: f32 = 20.0;
const INDENT: f32 = 16.0;

/// Icon and colour for a class restriction.
fn class_icon(restriction: &str, name: &str) -> (MaterialIcon, Color32) {
    if name == "ModelicaReference" {
        return (ICON_INFO, Color32::from_rgb(0x2E, 0x9E, 0x45));
    }
    match restriction {
        "package" => (ICON_INVENTORY_2, Color32::from_rgb(0x8A, 0x6D, 0x3B)),
        "model" => (ICON_WIDGETS, Color32::from_rgb(0x1F, 0x6F, 0xC8)),
        "block" => (ICON_CROP_SQUARE, Color32::from_rgb(0x1F, 0x6F, 0xC8)),
        "connector" | "expandable connector" => (ICON_STOP, Color32::from_rgb(0x1F, 0x4F, 0x9F)),
        "record" => (ICON_LIST_ALT, Color32::from_rgb(0x6A, 0x4C, 0x9C)),
        "function" | "operator function" => (ICON_FUNCTIONS, Color32::from_rgb(0x4A, 0x4A, 0x4A)),
        "type" => (ICON_TEXT_FIELDS, Color32::from_rgb(0x4A, 0x4A, 0x4A)),
        _ => (ICON_DESCRIPTION, Color32::from_rgb(0x70, 0x70, 0x70)),
    }
}

#[derive(Default)]
struct RowAction {
    toggle: bool,
    open: bool,
    open_new_tab: bool,
}

/// One tree row: expander, icon box, label. Returns what the user did.
#[allow(clippy::too_many_arguments)]
fn tree_row(
    ui: &mut egui::Ui,
    depth: usize,
    expandable: bool,
    expanded: bool,
    selected: bool,
    icon: (MaterialIcon, Color32),
    label: &str,
    tooltip: &str,
) -> RowAction {
    let width = ui.available_width().max(120.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, ROW), Sense::click());
    let painter = ui.painter_at(rect);
    if selected {
        painter.rect_filled(rect, 0.0, theme::SELECT_BG);
    } else if response.hovered() {
        painter.rect_filled(rect, 0.0, theme::HOVER_BG);
    }
    let fg = if selected {
        Color32::WHITE
    } else {
        theme::TEXT
    };
    let x0 = rect.left() + 4.0 + depth as f32 * INDENT;
    let arrow_rect = egui::Rect::from_min_size(egui::pos2(x0, rect.top()), Vec2::new(14.0, ROW));
    if expandable {
        paint_icon(
            ui,
            arrow_rect.center(),
            if expanded {
                ICON_EXPAND_MORE
            } else {
                ICON_CHEVRON_RIGHT
            },
            14.0,
            if selected {
                Color32::WHITE
            } else {
                theme::TEXT_WEAK
            },
        );
    }
    let icon_box =
        egui::Rect::from_center_size(egui::pos2(x0 + 26.0, rect.center().y), Vec2::splat(16.0));
    painter.rect(
        icon_box,
        1.0,
        Color32::WHITE,
        Stroke::new(1.0, Color32::from_rgb(0xB8, 0xB8, 0xB8)),
        egui::StrokeKind::Inside,
    );
    paint_icon(ui, icon_box.center(), icon.0, 13.0, icon.1);
    painter.text(
        egui::pos2(x0 + 40.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.5),
        fg,
    );
    let mut action = RowAction::default();
    let arrow_hit = response
        .interact_pointer_pos()
        .is_some_and(|p| arrow_rect.expand(2.0).contains(p));
    if response.clicked() {
        if arrow_hit && expandable {
            action.toggle = true;
        } else {
            action.open = true;
        }
    }
    if response.double_clicked() {
        action.toggle = expandable;
        action.open_new_tab = !expandable;
    }
    if !tooltip.is_empty() {
        response.on_hover_text(tooltip);
    }
    action
}

impl OmolaApp {
    pub fn modeling_browsers(&mut self, ui: &mut egui::Ui) {
        let total = ui.available_height();
        let both = self.docks.package_browser && self.docks.component_browser;
        if self.docks.package_browser {
            let h = if both {
                (total * 0.6).max(160.0)
            } else {
                total
            };
            ui.allocate_ui(Vec2::new(ui.available_width(), h), |ui| {
                self.package_browser(ui)
            });
        }
        if self.docks.component_browser {
            ui.add_space(4.0);
            self.component_browser(ui);
        }
    }

    fn browser_frame(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
        egui::Frame::new()
            .fill(Color32::WHITE)
            .stroke(Stroke::new(1.0, theme::BORDER))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                add(ui)
            });
    }

    fn package_browser(&mut self, ui: &mut egui::Ui) {
        if dock_header(ui, "Package Browser") {
            self.docks.package_browser = false;
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 1.0;
            ui.add(
                egui::TextEdit::singleline(&mut self.package_filter)
                    .hint_text("Model name...")
                    .desired_width(ui.available_width() - 110.0),
            );
            if icon_button(
                ui,
                ICON_ARROW_BACK,
                theme::ACCENT,
                14.0,
                self.can_go_back(),
                false,
            )
            .clicked()
            {
                self.go_back();
            }
            if icon_button(
                ui,
                ICON_ARROW_FORWARD,
                theme::ACCENT,
                14.0,
                self.can_go_forward(),
                false,
            )
            .clicked()
            {
                self.go_forward();
            }
            icon_button(ui, ICON_FILTER_ALT, theme::TEXT, 14.0, false, false);
            icon_button(ui, ICON_SORT_BY_ALPHA, theme::TEXT, 14.0, false, false);
            if icon_button(ui, ICON_UNFOLD_LESS, theme::TEXT, 14.0, true, false)
                .on_hover_text("Collapse all")
                .clicked()
            {
                self.expanded.clear();
            }
        });
        let footer = 26.0;
        let height = (ui.available_height() - footer).max(60.0);
        Self::browser_frame(ui, |ui| {
            egui::ScrollArea::both()
                .id_salt("package_tree")
                .max_height(height)
                .min_scrolled_height(height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    if self.package_filter.trim().is_empty() {
                        self.package_tree(ui);
                    } else {
                        self.package_search(ui);
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 1.0;
            let w = (ui.available_width() / 5.0 - 1.0).max(20.0);
            for (icon, enabled, tip) in [
                (ICON_ACCOUNT_TREE, false, "Package hierarchy"),
                (ICON_GRID_VIEW, false, "Component icons"),
                (ICON_ARROW_BACK, self.can_go_back(), "Back"),
                (ICON_ARROW_FORWARD, self.can_go_forward(), "Forward"),
                (ICON_VERTICAL_ALIGN_TOP, true, "Go to top"),
            ] {
                let r = ui.add_enabled(
                    enabled,
                    egui::Button::new(
                        RichText::new(icon.codepoint)
                            .family(egui::FontFamily::Name(
                                egui_material_icons::FONT_FAMILY.into(),
                            ))
                            .size(14.0)
                            .color(theme::ACCENT),
                    )
                    .min_size(Vec2::new(w, 20.0)),
                );
                let r = r.on_hover_text(tip);
                if r.clicked() {
                    match tip {
                        "Back" => self.go_back(),
                        "Forward" => self.go_forward(),
                        _ => self.expanded.clear(),
                    }
                }
            }
        });
    }

    fn package_tree(&mut self, ui: &mut egui::Ui) {
        let mut actions: Vec<(String, RowAction)> = Vec::new();
        // User classes first-level entries (the open file).
        let user_roots: Vec<(String, String, String)> = self
            .user_classes
            .iter()
            .filter(|c| c.depth == 0)
            .map(|c| {
                (
                    c.qualified_name.clone(),
                    c.kind.clone(),
                    c.description.clone().unwrap_or_default(),
                )
            })
            .collect();
        for lib in &self.libraries {
            match lib {
                LibraryState::Loaded(l) => {
                    let root = l.root().map(|c| {
                        (
                            c.qualified_name.clone(),
                            c.restriction.clone(),
                            c.comment.clone(),
                        )
                    });
                    if let Some(root) = root {
                        self.tree_node(ui, 0, root, &mut actions);
                    }
                }
                LibraryState::Loading(spec) => {
                    tree_row(
                        ui,
                        0,
                        false,
                        false,
                        false,
                        (ICON_HOURGLASS_EMPTY, theme::TEXT_WEAK),
                        &format!("{} (loading...)", spec.name),
                        "",
                    );
                }
                LibraryState::Failed(spec, message) => {
                    let _ = tree_row(
                        ui,
                        0,
                        false,
                        false,
                        false,
                        (ICON_ERROR, Color32::from_rgb(0xC6, 0x28, 0x28)),
                        &format!("{} (not installed)", spec.name),
                        &format!(
                            "{message}\n\nInstall it with Tools > Install Libraries, or run scripts/setup-* (docs/install.md)."
                        ),
                    );
                }
            }
        }
        for root in user_roots {
            self.tree_node(ui, 0, root, &mut actions);
        }
        self.apply_tree_actions(actions);
    }

    fn tree_node(
        &self,
        ui: &mut egui::Ui,
        depth: usize,
        (name, restriction, comment): (String, String, String),
        actions: &mut Vec<(String, RowAction)>,
    ) {
        let children = self.children_of(&name);
        let expandable = !children.is_empty();
        let expanded = self.expanded.contains(&name);
        let label = crate::library::short_name(&name).to_owned();
        let tooltip = if comment.is_empty() {
            name.clone()
        } else {
            format!("{name}\n{comment}")
        };
        let action = tree_row(
            ui,
            depth,
            expandable,
            expanded,
            self.active.as_deref() == Some(name.as_str()),
            class_icon(&restriction, &name),
            &label,
            &tooltip,
        );
        if action.toggle || action.open || action.open_new_tab {
            actions.push((name.clone(), action));
        }
        if expanded {
            for child in children {
                self.tree_node(ui, depth + 1, child, actions);
            }
        }
    }

    fn apply_tree_actions(&mut self, actions: Vec<(String, RowAction)>) {
        for (name, action) in actions {
            if action.toggle && !self.expanded.remove(&name) {
                self.expanded.insert(name.clone());
            }
            if action.open_new_tab {
                self.open_in_new_tab(&name);
            } else if action.open {
                self.open_class(&name);
            }
        }
    }

    fn package_search(&mut self, ui: &mut egui::Ui) {
        let query = self.package_filter.trim().to_lowercase();
        let mut hits: Vec<(String, String, String)> = Vec::new();
        for c in &self.user_classes {
            if c.qualified_name.to_lowercase().contains(&query) {
                hits.push((
                    c.qualified_name.clone(),
                    c.kind.clone(),
                    c.description.clone().unwrap_or_default(),
                ));
            }
        }
        for lib in &self.libraries {
            if let LibraryState::Loaded(l) = lib {
                for c in &l.classes {
                    if hits.len() >= 300 {
                        break;
                    }
                    if crate::library::short_name(&c.qualified_name)
                        .to_lowercase()
                        .contains(&query)
                    {
                        hits.push((
                            c.qualified_name.clone(),
                            c.restriction.clone(),
                            c.comment.clone(),
                        ));
                    }
                }
            }
        }
        let mut actions = Vec::new();
        for (name, restriction, comment) in hits {
            let a = tree_row(
                ui,
                0,
                false,
                false,
                self.active.as_deref() == Some(name.as_str()),
                class_icon(&restriction, &name),
                &name,
                &comment,
            );
            if a.open || a.open_new_tab {
                actions.push((name, a));
            }
        }
        self.apply_tree_actions(actions);
    }

    fn component_browser(&mut self, ui: &mut egui::Ui) {
        if dock_header(ui, "Component Browser") {
            self.docks.component_browser = false;
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 1.0;
            let mut filter = String::new();
            ui.add_enabled(
                false,
                egui::TextEdit::singleline(&mut filter)
                    .hint_text("Component name")
                    .desired_width(ui.available_width() - 70.0),
            );
            icon_button(ui, ICON_ARROW_BACK, theme::ACCENT, 14.0, false, false);
            icon_button(ui, ICON_ARROW_FORWARD, theme::ACCENT, 14.0, false, false);
            icon_button(ui, ICON_SORT_BY_ALPHA, theme::TEXT, 14.0, false, false);
        });
        Self::browser_frame(ui, |ui| {
            ui.set_min_height(ui.available_height().max(40.0));
            ui.spacing_mut().item_spacing.y = 0.0;
            if let Some(active) = self.active.clone() {
                let restriction = self.restriction_of(&active).unwrap_or_default();
                tree_row(
                    ui,
                    0,
                    false,
                    false,
                    false,
                    class_icon(&restriction, &active),
                    crate::library::short_name(&active),
                    "Component hierarchy: issue G06",
                );
            }
        });
    }

    // ------------------------------------------------------------------ variable browser

    pub fn variable_browser(&mut self, ui: &mut egui::Ui) {
        if dock_header(ui, "Variable Browser") {
            self.docks.variable_browser = false;
        }
        let widths = [88.0f32, 66.0, 50.0, 140.0];
        const VROW: f32 = 19.0;
        let footer = 24.0;
        let height = (ui.available_height() - footer).max(60.0);
        let mut toggles: Vec<PlotSignal> = Vec::new();
        let mut edits: Vec<(String, String)> = Vec::new();
        Self::browser_frame(ui, |ui| {
            egui::ScrollArea::both()
                .id_salt("variable_tree")
                .max_height(height)
                .min_scrolled_height(height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(0.0, 0.0);
                    // Column headers.
                    ui.horizontal(|ui| {
                        for (title, w) in ["Variable", "Value", "Unit", "Description"]
                            .iter()
                            .zip(widths)
                        {
                            let (r, _) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::hover());
                            ui.painter().text(
                                egui::pos2(r.left() + 4.0, r.center().y),
                                Align2::LEFT_CENTER,
                                *title,
                                FontId::proportional(12.0),
                                theme::TEXT,
                            );
                            ui.painter().vline(
                                r.right() - 0.5,
                                r.y_range(),
                                Stroke::new(1.0, Color32::from_rgb(0xE2, 0xE2, 0xE2)),
                            );
                        }
                    });
                    if self.runs.is_empty() {
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new("  No results. Press Simulate.").color(theme::TEXT_WEAK),
                        );
                        return;
                    }
                    let filter = self.variable_filter.to_lowercase();
                    let latest = self.runs.last().map(|r| r.id);
                    for run in self.runs.iter().rev() {
                        let open_id = egui::Id::NULL.with(("run_open", run.id));
                        let mut open = ui.ctx().data_mut(|d| *d.get_temp_mut_or(open_id, true));
                        let (rect, resp) = ui.allocate_exact_size(
                            Vec2::new(widths.iter().sum(), VROW),
                            Sense::click(),
                        );
                        if resp.clicked() {
                            open = !open;
                            ui.ctx().data_mut(|d| d.insert_temp(open_id, open));
                        }
                        paint_icon(
                            ui,
                            egui::pos2(rect.left() + 9.0, rect.center().y),
                            if open {
                                ICON_EXPAND_MORE
                            } else {
                                ICON_CHEVRON_RIGHT
                            },
                            14.0,
                            theme::TEXT,
                        );
                        paint_icon(
                            ui,
                            egui::pos2(rect.left() + 24.0, rect.center().y),
                            ICON_KEEP,
                            13.0,
                            theme::TEXT_WEAK,
                        );
                        ui.painter().text(
                            egui::pos2(rect.left() + 34.0, rect.center().y),
                            Align2::LEFT_CENTER,
                            &run.label,
                            FontId::proportional(12.0),
                            theme::TEXT,
                        );
                        resp.on_hover_text(format!(
                            "{}\n{}\nSimulated in {:.2} s",
                            run.outcome.result_file.display(),
                            run.outcome.class,
                            run.outcome.elapsed_seconds
                        ));
                        if !open {
                            continue;
                        }
                        let result = &run.outcome.result;
                        let order = variable_order(self, run);
                        for i in order {
                            let var = &result.variables[i];
                            if var.name == "time"
                                || (!filter.is_empty()
                                    && !var.name.to_lowercase().contains(&filter))
                            {
                                continue;
                            }
                            let signal = PlotSignal {
                                run_id: run.id,
                                variable: var.name.clone(),
                            };
                            let mut on = self.plotted.contains(&signal);
                            let is_state = result.index_of(&format!("der({})", var.name)).is_some();
                            let editable = Some(run.id) == latest && (var.is_parameter || is_state);
                            ui.horizontal(|ui| {
                                ui.set_height(VROW);
                                ui.add_space(22.0);
                                let (name_rect, name_resp) = ui.allocate_exact_size(
                                    Vec2::new(widths[0] - 22.0, VROW),
                                    Sense::click(),
                                );
                                let box_rect = egui::Rect::from_min_size(
                                    egui::pos2(name_rect.left() + 1.0, name_rect.center().y - 6.5),
                                    Vec2::splat(13.0),
                                );
                                super::widgets::check_box(ui, box_rect, on);
                                ui.painter().text(
                                    egui::pos2(name_rect.left() + 20.0, name_rect.center().y),
                                    Align2::LEFT_CENTER,
                                    &var.name,
                                    FontId::proportional(12.0),
                                    theme::TEXT,
                                );
                                if name_resp.clicked() {
                                    on = !on;
                                    toggles.push(signal.clone());
                                }
                                let _ = on;
                                let (unit, description) = split_unit(&var.description);
                                let (value_rect, _) = ui.allocate_exact_size(
                                    Vec2::new(widths[1], VROW),
                                    Sense::hover(),
                                );
                                if editable {
                                    let value =
                                        result.values[i].first().copied().unwrap_or(f64::NAN);
                                    let mut text = self
                                        .pending_overrides
                                        .get(&var.name)
                                        .cloned()
                                        .unwrap_or_else(|| format_value(value));
                                    let field = egui::Rect::from_min_size(
                                        value_rect.left_top() + Vec2::new(0.0, 1.0),
                                        Vec2::new(widths[1], VROW - 2.0),
                                    );
                                    ui.painter().rect(
                                        field,
                                        0.0,
                                        Color32::WHITE,
                                        Stroke::new(1.0, theme::FIELD_BORDER),
                                        egui::StrokeKind::Inside,
                                    );
                                    let _ = &field;
                                    let mut child = ui.new_child(
                                        egui::UiBuilder::new()
                                            .max_rect(field.shrink2(Vec2::new(3.0, 0.0))),
                                    );
                                    let changed = child
                                        .add(
                                            egui::TextEdit::singleline(&mut text)
                                                .frame(egui::Frame::NONE)
                                                .font(FontId::proportional(12.0))
                                                .align(egui::Align2::RIGHT_CENTER)
                                                .desired_width(field.width() - 6.0),
                                        )
                                        .changed();
                                    if changed {
                                        edits.push((var.name.clone(), text));
                                    }
                                }
                                let (unit_rect, _) = ui.allocate_exact_size(
                                    Vec2::new(widths[2], VROW),
                                    Sense::hover(),
                                );
                                ui.painter().text(
                                    egui::pos2(unit_rect.left() + 4.0, unit_rect.center().y),
                                    Align2::LEFT_CENTER,
                                    unit,
                                    FontId::proportional(12.0),
                                    theme::TEXT,
                                );
                                let (d_rect, _) = ui.allocate_exact_size(
                                    Vec2::new(widths[3], VROW),
                                    Sense::hover(),
                                );
                                ui.painter().text(
                                    egui::pos2(d_rect.left() + 4.0, d_rect.center().y),
                                    Align2::LEFT_CENTER,
                                    description,
                                    FontId::proportional(12.0),
                                    theme::TEXT,
                                );
                            });
                        }
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let (field, _) = ui
                .allocate_exact_size(Vec2::new(ui.available_width() - 78.0, 20.0), Sense::hover());
            super::widgets::field_frame(ui, field);
            let mut child =
                ui.new_child(egui::UiBuilder::new().max_rect(field.shrink2(Vec2::new(2.0, 0.0))));
            child.add(
                egui::TextEdit::singleline(&mut self.variable_filter)
                    .frame(egui::Frame::NONE)
                    .hint_text("filter variables")
                    .desired_width(field.width() - 4.0),
            );
            let (button, resp) = ui.allocate_exact_size(Vec2::new(76.0, 20.0), Sense::click());
            super::widgets::combo_frame(ui, button);
            ui.painter().text(
                button.center(),
                Align2::CENTER_CENTER,
                "More >>",
                FontId::proportional(12.0),
                theme::TEXT,
            );
            if resp
                .on_hover_text("Advanced variable browser: issue G09")
                .clicked()
            {
                self.not_implemented("Advanced variable browser");
            }
        });
        for signal in toggles {
            if let Some(pos) = self.plotted.iter().position(|s| *s == signal) {
                self.plotted.remove(pos);
            } else {
                self.plotted.push(signal);
            }
        }
        for (name, text) in edits {
            self.pending_overrides.insert(name, text);
        }
    }
}

/// Display order: declaration order in the user's source when available, else file order.
fn variable_order(app: &OmolaApp, run: &crate::app::ResultRun) -> Vec<usize> {
    let vars = &run.outcome.result.variables;
    let mut order: Vec<usize> = (0..vars.len()).collect();
    let text = app
        .is_user_class(&run.outcome.class)
        .then(|| {
            app.doc_of(&run.outcome.class)
                .map(|i| app.documents[i].text.as_str())
        })
        .flatten();
    if let Some(text) = text {
        let key = |i: &usize| -> (usize, usize) {
            let name = &vars[*i].name;
            let (base, is_der) = match name.strip_prefix("der(").and_then(|n| n.strip_suffix(')')) {
                Some(inner) => (inner, 1),
                None => (name.as_str(), 0),
            };
            let leaf = base.split('.').next().unwrap_or(base);
            (find_identifier(text, leaf).unwrap_or(usize::MAX), is_der)
        };
        order.sort_by_key(key);
    }
    order
}

fn find_identifier(text: &str, ident: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(pos) = text[from..].find(ident) {
        let at = from + pos;
        let before = at.checked_sub(1).map(|i| bytes[i]);
        let after = bytes.get(at + ident.len()).copied();
        let is_ident = |b: Option<u8>| b.is_some_and(|b| b == b'_' || b.is_ascii_alphanumeric());
        if !is_ident(before) && !is_ident(after) {
            return Some(at);
        }
        from = at + ident.len();
    }
    None
}

/// OpenModelica appends ` [unit]` to some descriptions; show it in the Unit column.
pub fn split_unit(description: &str) -> (&str, &str) {
    let d = description.trim_end();
    if let Some(open) = d.rfind(" [") {
        if d.ends_with(']') {
            let unit = &d[open + 2..d.len() - 1];
            if !unit.is_empty() && !unit.contains(' ') {
                return (unit.split('|').next().unwrap_or(unit), d[..open].trim_end());
            }
        }
    }
    ("", d)
}

/// Compact number formatting for the value column (`1`, `9.8`, `0.1`, `1e-06`).
pub fn format_value(v: f64) -> String {
    if !v.is_finite() {
        return v.to_string();
    }
    if v == 0.0 {
        return "0".into();
    }
    let abs = v.abs();
    if (1e-4..1e7).contains(&abs) {
        let s = format!("{:.6}", v);
        let s = s.trim_end_matches('0').trim_end_matches('.');
        s.to_owned()
    } else {
        format!("{v:e}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_and_values() {
        assert_eq!(split_unit("Angle [rad]"), ("rad", "Angle"));
        assert_eq!(split_unit("Speed [rad/s|rpm]"), ("rad/s", "Speed"));
        assert_eq!(split_unit("plain [not a unit]"), ("", "plain [not a unit]"));
        assert_eq!(format_value(9.8), "9.8");
        assert_eq!(format_value(1.0), "1");
        assert_eq!(format_value(0.1), "0.1");
        assert_eq!(format_value(0.0), "0");
        assert_eq!(format_value(-0.25), "-0.25");
    }

    #[test]
    fn identifiers_match_whole_words() {
        let text = "parameter Real m=1; parameter Real mm=2; Real phi;";
        assert_eq!(find_identifier(text, "m"), Some(15));
        assert_eq!(find_identifier(text, "phi"), Some(46));
        assert_eq!(find_identifier(text, "x"), None);
    }
}

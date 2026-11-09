//! Docked browsers: package + component browser (modeling) and variable browser (simulation).

use super::widgets::{dock_header, icon_button, paint_icon};
use crate::app::{LibraryState, OmolaApp, PlotSignal};
use crate::instance::ComponentNode;
use crate::theme;
use egui::{Align2, Color32, FontId, RichText, Sense, Stroke, Vec2};
use egui_material_icons::icons::*;
use egui_material_icons::MaterialIcon;

const ROW: f32 = 20.0;
const INDENT: f32 = 16.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClassPayload {
    pub qualified: String,
    pub restriction: String,
}

pub(crate) fn draggable_restriction(restriction: &str) -> bool {
    !matches!(
        restriction,
        "package" | "function" | "operator function" | "type" | "record"
    )
}

pub(crate) fn unique_component_name(type_short: &str, existing: &[String]) -> String {
    if type_short.starts_with('\'') {
        return String::new();
    }
    let mut chars = type_short.chars();
    let base = chars
        .next()
        .map(|c| c.to_lowercase().collect::<String>() + chars.as_str())
        .unwrap_or_default();
    if !existing.contains(&base) {
        return base;
    }
    for i in 1.. {
        let candidate = format!("{base}{i}");
        if !existing.contains(&candidate) {
            return candidate;
        }
    }
    unreachable!()
}

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
    tooltip: impl FnOnce() -> String,
    payload: Option<ClassPayload>,
) -> RowAction {
    let width = ui.available_width().max(120.0);
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, ROW), Sense::click_and_drag());
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
    response.on_hover_ui(|ui| {
        let tooltip = tooltip();
        if !tooltip.is_empty() {
            ui.label(tooltip);
        }
    });
    if let Some(payload) = payload {
        response.dnd_set_drag_payload(payload.clone());
        if response.dragged() {
            if let Some(pos) = ui.ctx().pointer_interact_pos() {
                let painter = ui.ctx().layer_painter(egui::LayerId::new(
                    egui::Order::Tooltip,
                    egui::Id::unique(("package-class-drag", payload.qualified.clone())),
                ));
                painter.text(
                    pos + Vec2::new(12.0, 12.0),
                    Align2::LEFT_TOP,
                    payload.qualified,
                    FontId::proportional(13.0),
                    Color32::from_rgba_unmultiplied(45, 65, 90, 150),
                );
            }
        }
    }
    action
}

/// Icon for a component row: parameters look different from plain signals.
fn component_icon(node: &ComponentNode) -> (MaterialIcon, Color32) {
    if node.is_parameter {
        (ICON_TUNE, Color32::from_rgb(0x8A, 0x6D, 0x3B))
    } else if matches!(
        node.type_name.as_str(),
        "Real" | "Integer" | "Boolean" | "String"
    ) {
        (ICON_TEXT_FIELDS, Color32::from_rgb(0x4A, 0x4A, 0x4A))
    } else {
        (ICON_CROP_SQUARE, Color32::from_rgb(0x1F, 0x6F, 0xC8))
    }
}

/// Draw one component and, when expanded, its children.
///
/// `path` is the dotted prefix used as the expansion key; `toggles` collects the paths
/// whose expander was clicked so the caller can update the state after the frame.
fn component_rows(
    ui: &mut egui::Ui,
    node: &ComponentNode,
    depth: usize,
    path: &str,
    expanded: &std::collections::BTreeSet<String>,
    toggles: &mut Vec<String>,
) {
    let key = if path.is_empty() {
        node.name.clone()
    } else {
        format!("{path}.{}", node.name)
    };
    let label = if node.is_parameter {
        format!("{} (p) : {}", node.name, node.type_name)
    } else {
        format!("{} : {}", node.name, node.type_name)
    };
    let has_children = !node.children.is_empty();
    let open = expanded.contains(&key);
    let action = tree_row(
        ui,
        depth,
        has_children,
        open,
        false,
        component_icon(node),
        &label,
        None,
    );
    if action.toggle {
        toggles.push(key.clone());
    }
    if open {
        for child in &node.children {
            component_rows(ui, child, depth + 1, &key, expanded, toggles);
        }
    }
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
        let user_roots: Vec<String> = self
            .user_classes
            .iter()
            .filter(|c| c.depth == 0)
            .map(|c| c.qualified_name.clone())
            .collect();
        for lib in &self.libraries {
            match lib {
                LibraryState::Loaded(l) => {
                    if let Some(root) = l.root() {
                        self.tree_node(ui, 0, &root.qualified_name, &mut actions);
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
                        String::new,
                        None,
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
                        || {
                            format!(
                            "{message}\n\nInstall it with Tools > Install Libraries, or run scripts/setup-* (docs/install.md)."
                        )
                        },
                        None,
                    );
                }
            }
        }
        for root in user_roots {
            self.tree_node(ui, 0, &root, &mut actions);
        }
        self.apply_tree_actions(actions);
    }

    fn tree_node(
        &self,
        ui: &mut egui::Ui,
        depth: usize,
        name: &str,
        actions: &mut Vec<(String, RowAction)>,
    ) {
        let expandable = self.has_children_of(name);
        let expanded = self.expanded.contains(name);
        let label = crate::library::short_name(name);
        let icon = self
            .class_details(name)
            .map(|(r, _)| class_icon(r, name))
            .unwrap_or_else(|| class_icon("", name));
        let action = tree_row(
            ui,
            depth,
            expandable,
            expanded,
            self.active.as_deref() == Some(name),
            icon,
            label,
            || {
                let comment = self.class_details(name).map(|(_, c)| c).unwrap_or("");
                if comment.is_empty() {
                    name.to_owned()
                } else {
                    format!("{name}\n{comment}")
                }
            },
            self.class_details(name).and_then(|(restriction, _)| {
                draggable_restriction(restriction).then(|| ClassPayload {
                    qualified: name.to_owned(),
                    restriction: restriction.to_owned(),
                })
            }),
        );
        if action.toggle || action.open || action.open_new_tab {
            actions.push((name.to_owned(), action));
        }
        if expanded {
            for child in self.child_names(name) {
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
        self.update_package_search_cache();
        let hits = self
            .package_search_cache
            .as_ref()
            .map(|(_, hits)| hits)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let truncated = hits.len() > 500;
        let hit_count = hits.len().min(500);
        let mut actions = Vec::new();
        egui::ScrollArea::vertical().show_rows(
            ui,
            ROW,
            hit_count + usize::from(truncated),
            |ui, rows| {
                for index in rows {
                    if index >= hit_count {
                        ui.label(
                            RichText::new("... more results, refine your search")
                                .color(theme::TEXT_WEAK),
                        );
                        continue;
                    }
                    let name = hits[index].as_str();
                    let (restriction, comment) = self.class_details(name).unwrap_or(("", ""));
                    let a = tree_row(
                        ui,
                        0,
                        false,
                        false,
                        self.active.as_deref() == Some(name),
                        class_icon(restriction, name),
                        name,
                        || comment.to_owned(),
                        draggable_restriction(restriction).then(|| ClassPayload {
                            qualified: name.to_owned(),
                            restriction: restriction.to_owned(),
                        }),
                    );
                    if a.open || a.open_new_tab {
                        actions.push((name.to_owned(), a));
                    }
                }
            },
        );
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
        let active = self.active.clone();
        let loading = self.components_pending.is_some();
        let mut toggles: Vec<String> = Vec::new();
        Self::browser_frame(ui, |ui| {
            ui.set_min_height(ui.available_height().max(40.0));
            ui.spacing_mut().item_spacing.y = 0.0;
            let Some(active) = active.as_deref() else {
                ui.label(
                    RichText::new("Open a model to see its components.")
                        .small()
                        .weak(),
                );
                return;
            };
            let Some((_, nodes)) = self
                .components
                .as_ref()
                .filter(|(class, _)| class == active)
            else {
                ui.label(
                    RichText::new(if loading {
                        "Loading components ..."
                    } else {
                        "Press Check to load components"
                    })
                    .small()
                    .weak(),
                );
                return;
            };
            if nodes.is_empty() {
                ui.label(
                    RichText::new("This class declares no components.")
                        .small()
                        .weak(),
                );
                return;
            }
            for node in nodes {
                component_rows(ui, node, 0, "", &self.component_expanded, &mut toggles);
            }
        });
        for path in toggles {
            if !self.component_expanded.remove(&path) {
                self.component_expanded.insert(path);
            }
        }
    }

    // ------------------------------------------------------------------ variable browser

    pub fn variable_browser(&mut self, ui: &mut egui::Ui) {
        if dock_header(ui, "Variable Browser") {
            self.docks.variable_browser = false;
        }
        let widths = [116.0f32, 62.0, 46.0, 132.0];
        const VROW: f32 = 19.0;
        // The footer row, plus the two wrapped lines of the `More >>` row when it is open,
        // must be reserved or the advanced row is drawn past the dock and clipped away.
        let footer = if self.variable_advanced { 70.0 } else { 24.0 };
        let height = (ui.available_height() - footer - 22.0).max(60.0);
        let mut toggles: Vec<PlotSignal> = Vec::new();
        let mut edits: Vec<(String, String)> = Vec::new();
        let mut expand: Vec<(String, bool)> = Vec::new();
        let mut reset: Vec<String> = Vec::new();
        self.variable_time_row(ui);
        let run_ids: Vec<u64> = self.runs.iter().map(|r| r.id).collect();
        for id in run_ids {
            self.var_tree_for(id);
        }
        let latest = self.runs.last().map(|r| r.id);
        let time = self.variable_time;
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
                        // Only the newest result can be re-run, so only its parameters are
                        // editable.
                        let ctx = VarCtx {
                            run_id: run.id,
                            latest: Some(run.id) == latest,
                            time,
                            plotted: self.active_signals(),
                            overrides: &self.pending_overrides,
                            expanded: &self.variable_expanded,
                        };
                        let mut actions = VarActions::default();
                        let Some(cache) = self.var_tree_cache.get(&run.id) else {
                            continue;
                        };
                        if cache.key.filter.is_empty() {
                            variable_rows(
                                ui,
                                &cache.tree,
                                0,
                                "",
                                &ctx,
                                result,
                                widths,
                                &mut actions,
                            );
                        } else {
                            // A search shows matching variables as a flat list.
                            for i in cache.flat.iter().take(500) {
                                let name = &result.variables[*i].name;
                                variable_row(
                                    ui,
                                    0,
                                    name,
                                    Some(*i),
                                    None,
                                    name,
                                    &ctx,
                                    result,
                                    widths,
                                    &mut actions,
                                );
                            }
                            if cache.flat.len() > 500 {
                                ui.label(
                                    RichText::new(format!("... {} more", cache.flat.len() - 500))
                                        .color(theme::TEXT_WEAK),
                                );
                            }
                        }
                        toggles.append(&mut actions.toggles);
                        edits.append(&mut actions.edits);
                        expand.append(&mut actions.expand);
                        reset.append(&mut actions.reset);
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
                if self.variable_advanced {
                    "More <<"
                } else {
                    "More >>"
                },
                FontId::proportional(12.0),
                theme::TEXT,
            );
            if resp.on_hover_text("Filter and overrides").clicked() {
                self.variable_advanced = !self.variable_advanced;
            }
        });
        if self.variable_advanced {
            self.variable_advanced_row(ui);
        }
        // The checkboxes belong to the active diagram.
        for signal in toggles {
            let plotted = self.active_signals_mut();
            if let Some(pos) = plotted.iter().position(|s| *s == signal) {
                plotted.remove(pos);
            } else {
                plotted.push(signal);
            }
        }
        for (name, text) in edits {
            self.pending_overrides.insert(name, text);
        }
        for (path, open) in expand {
            self.variable_expanded.insert(path, open);
        }
        for name in reset {
            self.pending_overrides.remove(&name);
        }
    }

    /// `Time:` field of the Variable Browser: the instant shown in the Value column.
    fn variable_time_row(&mut self, ui: &mut egui::Ui) {
        let range = self
            .runs
            .last()
            .and_then(|r| crate::series::time_range(&r.outcome.result.time));
        let Some((min, max)) = range else {
            return;
        };
        let mut t = self.variable_time.unwrap_or(max);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.add_space(4.0);
            ui.label(RichText::new("Time:").size(12.0).color(theme::TEXT));
            let step = ((max - min) / 200.0).max(1e-9);
            let resp = ui.add(
                egui::DragValue::new(&mut t)
                    .range(min..=max)
                    .speed(step)
                    .max_decimals(9),
            );
            if resp.changed() {
                self.variable_time = Some(t);
            }
            if ui.button("End").clicked() {
                self.variable_time = None;
            }
        });
    }

    /// The `More >>` row: the filter toggles and "Reset overrides".
    fn variable_advanced_row(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.add_space(2.0);
            ui.checkbox(&mut self.variable_show_parameters, "Parameters")
                .on_hover_text("Show parameters in the variable tree");
            ui.checkbox(&mut self.variable_show_time_varying, "Time-varying")
                .on_hover_text("Show variables that change over time");
            let edited = !self.pending_overrides.is_empty();
            if ui
                .add_enabled(edited, egui::Button::new("Reset overrides"))
                .on_hover_text("Drop every value edited in the Variable Browser")
                .clicked()
            {
                self.pending_overrides.clear();
            }
        });
    }
}

/// What a row of the Variable Browser asked for, applied after the borrow of `runs` ends.
#[derive(Default)]
struct VarActions {
    toggles: Vec<PlotSignal>,
    edits: Vec<(String, String)>,
    expand: Vec<(String, bool)>,
    reset: Vec<String>,
}

/// Read-only view of the app state the rows need.
struct VarCtx<'a> {
    run_id: u64,
    latest: bool,
    time: Option<f64>,
    plotted: &'a [PlotSignal],
    overrides: &'a std::collections::BTreeMap<String, String>,
    expanded: &'a std::collections::BTreeMap<String, bool>,
}

/// Draw the children of `node`, descending into the expanded ones.
#[allow(clippy::too_many_arguments)]
fn variable_rows(
    ui: &mut egui::Ui,
    node: &crate::var_tree::VarNode,
    depth: usize,
    path: &str,
    ctx: &VarCtx,
    result: &omola_core::mat::TrajectoryResult,
    widths: [f32; 4],
    actions: &mut VarActions,
) {
    for child in &node.children {
        let child_path = if path.is_empty() {
            child.label.clone()
        } else {
            format!("{path}.{}", child.label)
        };
        let key = format!("{}:{}", ctx.run_id, child_path);
        let has_children = !child.children.is_empty();
        // Closed by default, except a top-level node with at most 20 children.
        let open = has_children
            && ctx
                .expanded
                .get(&key)
                .copied()
                .unwrap_or(crate::var_tree::starts_open(depth, child.children.len()));
        variable_row(
            ui,
            depth,
            &child.label,
            child.leaf,
            has_children.then_some(open),
            &key,
            ctx,
            result,
            widths,
            actions,
        );
        if open {
            variable_rows(
                ui,
                child,
                depth + 1,
                &child_path,
                ctx,
                result,
                widths,
                actions,
            );
        }
    }
}

/// One row: expander, checkbox, name, value, unit, description.
#[allow(clippy::too_many_arguments)]
fn variable_row(
    ui: &mut egui::Ui,
    depth: usize,
    label: &str,
    leaf: Option<usize>,
    expander: Option<bool>,
    key: &str,
    ctx: &VarCtx,
    result: &omola_core::mat::TrajectoryResult,
    widths: [f32; 4],
    actions: &mut VarActions,
) {
    const VROW: f32 = 19.0;
    let indent = 6.0 + depth as f32 * 12.0;
    let name_w = (widths[0] - indent).max(40.0);
    let mut signal = None;
    ui.horizontal(|ui| {
        ui.set_height(VROW);
        ui.add_space(indent);
        let (name_rect, name_resp) =
            ui.allocate_exact_size(Vec2::new(name_w, VROW), Sense::click());
        let var = leaf.map(|i| &result.variables[i]);
        let name = var.map(|v| v.name.clone()).unwrap_or_default();
        let overridden = !name.is_empty() && ctx.overrides.contains_key(&name);
        // Only parameters can be overridden with OMC's `-override`, so only they get a
        // text field; a state shows its value at the selected time like any other variable.
        let editable = ctx.latest && var.is_some_and(|v| v.is_parameter);
        if let Some(open) = expander {
            let tw = egui::Rect::from_center_size(
                egui::pos2(name_rect.left() + 6.0, name_rect.center().y),
                Vec2::splat(12.0),
            );
            paint_icon(
                ui,
                tw.center(),
                if open {
                    ICON_EXPAND_MORE
                } else {
                    ICON_CHEVRON_RIGHT
                },
                13.0,
                theme::TEXT,
            );
        }
        if leaf.is_some() {
            let box_rect = egui::Rect::from_min_size(
                egui::pos2(name_rect.left() + 14.0, name_rect.center().y - 6.5),
                Vec2::splat(13.0),
            );
            let on = ctx
                .plotted
                .iter()
                .any(|s| s.run_id == ctx.run_id && s.variable == name);
            super::widgets::check_box(ui, box_rect, on);
        }
        let text_x = if leaf.is_some() { 31.0 } else { 14.0 };
        // An edited value is shown in bold, like Dymola marks a modified parameter.
        let font = if overridden {
            FontId::new(12.0, theme::bold())
        } else {
            FontId::proportional(12.0)
        };
        ui.painter().text(
            egui::pos2(name_rect.left() + text_x, name_rect.center().y),
            Align2::LEFT_CENTER,
            label,
            font,
            theme::TEXT,
        );
        if name_resp.clicked() {
            let in_twisty = name_resp
                .interact_pointer_pos()
                .is_some_and(|p| p.x < name_rect.left() + 14.0);
            if expander.is_some() && (in_twisty || leaf.is_none()) {
                actions
                    .expand
                    .push((key.to_owned(), !expander.unwrap_or(false)));
            } else if leaf.is_some() {
                signal = Some(PlotSignal {
                    run_id: ctx.run_id,
                    variable: name.clone(),
                });
            }
        }
        let (unit, description) = var.map(|v| split_unit(&v.description)).unwrap_or(("", ""));
        let (value_rect, _) = ui.allocate_exact_size(Vec2::new(widths[1], VROW), Sense::hover());
        if let Some(i) = leaf {
            if editable {
                let value = result.values[i].first().copied().unwrap_or(f64::NAN);
                let mut text = ctx
                    .overrides
                    .get(&name)
                    .cloned()
                    .unwrap_or_else(|| format_value(value));
                let field_w = if overridden {
                    widths[1] - 15.0
                } else {
                    widths[1]
                };
                let field = egui::Rect::from_min_size(
                    value_rect.left_top() + Vec2::new(0.0, 1.0),
                    Vec2::new(field_w, VROW - 2.0),
                );
                ui.painter().rect(
                    field,
                    0.0,
                    Color32::WHITE,
                    Stroke::new(1.0, theme::FIELD_BORDER),
                    egui::StrokeKind::Inside,
                );
                let mut child = ui
                    .new_child(egui::UiBuilder::new().max_rect(field.shrink2(Vec2::new(3.0, 0.0))));
                // A value that is not a number cannot reach OMC; show it in red.
                let numeric = text.trim().parse::<f64>().is_ok();
                let changed = child
                    .add(
                        egui::TextEdit::singleline(&mut text)
                            .frame(egui::Frame::NONE)
                            .font(FontId::proportional(12.0))
                            .text_color(if numeric {
                                theme::TEXT
                            } else {
                                Color32::from_rgb(0xC0, 0x1B, 0x1B)
                            })
                            .align(egui::Align2::RIGHT_CENTER)
                            .desired_width(field.width() - 6.0),
                    )
                    .changed();
                if changed {
                    actions.edits.push((name.clone(), text));
                }
                if overridden {
                    let x_rect = egui::Rect::from_center_size(
                        egui::pos2(value_rect.right() - 7.0, value_rect.center().y),
                        Vec2::splat(13.0),
                    );
                    paint_icon(ui, x_rect.center(), ICON_CLOSE, 12.0, theme::TEXT_WEAK);
                    let resp = ui.interact(
                        x_rect,
                        ui.scope_id().with(("reset_override", &name)),
                        Sense::click(),
                    );
                    if resp.clicked() {
                        actions.reset.push(name.clone());
                    }
                    resp.on_hover_text("Discard this override");
                }
            } else {
                let value = match ctx.time {
                    Some(t) => crate::series::value_at(&result.time, &result.values[i], t),
                    None => result.values[i].last().copied(),
                };
                ui.painter().text(
                    egui::pos2(value_rect.right() - 4.0, value_rect.center().y),
                    Align2::RIGHT_CENTER,
                    value.map(format_value).unwrap_or_default(),
                    FontId::proportional(12.0),
                    theme::TEXT,
                );
            }
        }
        let (unit_rect, _) = ui.allocate_exact_size(Vec2::new(widths[2], VROW), Sense::hover());
        ui.painter().text(
            egui::pos2(unit_rect.left() + 4.0, unit_rect.center().y),
            Align2::LEFT_CENTER,
            unit,
            FontId::proportional(12.0),
            theme::TEXT,
        );
        let (d_rect, _) = ui.allocate_exact_size(Vec2::new(widths[3], VROW), Sense::hover());
        ui.painter().text(
            egui::pos2(d_rect.left() + 4.0, d_rect.center().y),
            Align2::LEFT_CENTER,
            description,
            FontId::proportional(12.0),
            theme::TEXT,
        );
    });
    if let Some(signal) = signal {
        actions.toggles.push(signal);
    }
}

/// Display order: declaration order in the user's source when available, else file order.
pub(crate) fn variable_order(app: &OmolaApp, run: &crate::app::ResultRun) -> Vec<usize> {
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
    fn component_name_is_unique_and_lowercases_only_the_first_character() {
        assert_eq!(unique_component_name("Constant", &[]), "constant");
        assert_eq!(
            unique_component_name("Constant", &["constant".into()]),
            "constant1"
        );
        assert_eq!(
            unique_component_name("Constant", &["constant".into(), "constant1".into()]),
            "constant2"
        );
        assert_eq!(unique_component_name("PID", &[]), "pID");
        assert_eq!(unique_component_name("'quoted'", &[]), "");
    }

    #[test]
    fn non_component_restrictions_do_not_create_drag_payloads() {
        for r in ["package", "function", "operator function", "type", "record"] {
            assert!(!draggable_restriction(r));
        }
        for r in ["model", "block", "connector"] {
            assert!(draggable_restriction(r));
        }
    }

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

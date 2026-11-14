//! Ribbon and toolbar widgets: large/small icon buttons, labelled groups, icon buttons.

use crate::theme;
use egui::{Align2, Color32, FontFamily, FontId, Rect, Response, Sense, Stroke, Ui, Vec2};
use egui_material_icons::MaterialIcon;

/// Height of the ribbon body (buttons area, without the group caption).
pub const RIBBON_BODY: f32 = 80.0;
/// Vertical centre of group captions, measured from the top of the ribbon.
pub const CAPTION_Y: f32 = 92.0;

pub fn icon_font(size: f32) -> FontId {
    FontId::new(
        size,
        FontFamily::Name(egui_material_icons::FONT_FAMILY.into()),
    )
}

pub fn paint_icon(ui: &Ui, center: egui::Pos2, icon: MaterialIcon, size: f32, color: Color32) {
    ui.painter().text(
        center,
        Align2::CENTER_CENTER,
        icon.codepoint,
        icon_font(size),
        color,
    );
}

/// Left inset from a model tab's edge to its label.
pub const MODEL_TAB_LABEL_LEFT: f32 = 26.0;
/// Distance from a model tab's right edge to the close icon's centre.
pub const MODEL_TAB_CLOSE_CENTER_INSET: f32 = 14.0;
/// Width of the close icon drawn in a model tab.
pub const MODEL_TAB_CLOSE_ICON: f32 = 13.0;
/// Fixed width of a model tab other than the label text.
pub const MODEL_TAB_CHROME_WIDTH: f32 = 56.0;

/// Width of a model tab whose label is `text_width` wide. The extra space holds
/// the left inset, the close icon and both side margins, so the label and the
/// close icon never overlap.
pub fn model_tab_width_for_text(text_width: f32) -> f32 {
    text_width + MODEL_TAB_CHROME_WIDTH
}

/// Width of a model tab for `label`, measured with the same font used to paint it.
pub fn model_tab_width(ui: &Ui, label: &str, font: FontId) -> f32 {
    let text_width = ui.ctx().fonts_mut(|fonts| {
        fonts
            .layout_no_wrap(label.to_owned(), font, theme::TEXT)
            .size()
            .x
    });
    model_tab_width_for_text(text_width)
}

/// Rectangle in which a model tab's label is painted.
pub fn model_tab_label_rect(rect: Rect) -> Rect {
    let right = rect.right() - MODEL_TAB_CLOSE_CENTER_INSET - MODEL_TAB_CLOSE_ICON / 2.0 - 4.0;
    Rect::from_min_max(
        egui::pos2(rect.left() + MODEL_TAB_LABEL_LEFT, rect.top()),
        egui::pos2(right.max(rect.left() + MODEL_TAB_LABEL_LEFT), rect.bottom()),
    )
}

/// Hit-test rectangle of a model tab's close icon.
pub fn model_tab_close_rect(rect: Rect) -> Rect {
    Rect::from_center_size(
        egui::pos2(rect.right() - MODEL_TAB_CLOSE_CENTER_INSET, rect.center().y),
        Vec2::splat(MODEL_TAB_CLOSE_ICON + 1.0),
    )
}

fn button_frame(ui: &Ui, rect: Rect, response: &Response, selected: bool, enabled: bool) {
    let painter = ui.painter();
    if selected {
        painter.rect(
            rect,
            1.0,
            theme::PRESSED_BG,
            Stroke::new(1.0, Color32::from_rgb(0x7A, 0xB0, 0xE0)),
            egui::StrokeKind::Inside,
        );
    } else if enabled && response.hovered() {
        painter.rect(
            rect,
            1.0,
            theme::HOVER_BG,
            Stroke::new(1.0, Color32::from_rgb(0x9C, 0xC3, 0xE8)),
            egui::StrokeKind::Inside,
        );
    }
}

/// Allocate and draw a ribbon button, reporting the disabled state through
/// `Response::enabled()`.
///
/// The buttons below drop the click sense when they are disabled, but
/// `Response::enabled()` is taken from the `Ui` that allocates the widget. A
/// disabled button would therefore still claim to be enabled, and
/// `on_disabled_hover_text()` (which only opens when `!response.enabled()`) would
/// silently swallow its explanation. Drawing inside a disabled child `Ui` sets the
/// flag without changing the geometry or the colours, which the buttons paint
/// themselves.
fn respond(ui: &mut Ui, enabled: bool, add: impl FnOnce(&mut Ui) -> Response) -> Response {
    if enabled {
        add(ui)
    } else {
        ui.scope_builder(egui::UiBuilder::new().disabled(), add)
            .inner
    }
}

/// A large ribbon button: icon on top, one or two label lines below.
pub fn big_button(
    ui: &mut Ui,
    icon: MaterialIcon,
    color: Color32,
    label: &str,
    enabled: bool,
    selected: bool,
) -> Response {
    respond(ui, enabled, |ui| {
        let font = FontId::proportional(12.0);
        let width = label
            .lines()
            .map(|l| {
                ui.ctx().fonts_mut(|f| {
                    f.layout_no_wrap(l.into(), font.clone(), theme::TEXT)
                        .size()
                        .x
                })
            })
            .fold(30.0f32, f32::max)
            + 10.0;
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(width, RIBBON_BODY - 8.0), sense(enabled));
        button_frame(ui, rect, &response, selected, enabled);
        let fg = if enabled { color } else { theme::DISABLED };
        paint_icon(
            ui,
            egui::pos2(rect.center().x, rect.top() + 22.0),
            icon,
            28.0,
            fg,
        );
        let text = if enabled {
            theme::TEXT
        } else {
            theme::DISABLED
        };
        for (i, line) in label.lines().enumerate() {
            ui.painter().text(
                egui::pos2(rect.center().x, rect.top() + 42.0 + i as f32 * 13.0),
                Align2::CENTER_TOP,
                line,
                font.clone(),
                text,
            );
        }
        response
    })
}

/// A small ribbon button: 16 px icon and a label on one line.
pub fn small_button(
    ui: &mut Ui,
    icon: MaterialIcon,
    color: Color32,
    label: &str,
    enabled: bool,
    selected: bool,
) -> Response {
    respond(ui, enabled, |ui| {
        let font = FontId::proportional(12.0);
        let text_w = if label.is_empty() {
            0.0
        } else {
            ui.ctx().fonts_mut(|f| {
                f.layout_no_wrap(label.into(), font.clone(), theme::TEXT)
                    .size()
                    .x
            }) + 6.0
        };
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(22.0 + text_w, 22.0), sense(enabled));
        button_frame(ui, rect, &response, selected, enabled);
        let fg = if enabled { color } else { theme::DISABLED };
        paint_icon(
            ui,
            egui::pos2(rect.left() + 9.0, rect.center().y),
            icon,
            16.0,
            fg,
        );
        if !label.is_empty() {
            ui.painter().text(
                egui::pos2(rect.left() + 21.0, rect.center().y),
                Align2::LEFT_CENTER,
                label,
                font,
                if enabled {
                    theme::TEXT
                } else {
                    theme::DISABLED
                },
            );
        }
        response
    })
}

/// A square icon-only button (toolbars, browser headers).
pub fn icon_button(
    ui: &mut Ui,
    icon: MaterialIcon,
    color: Color32,
    size: f32,
    enabled: bool,
    selected: bool,
) -> Response {
    respond(ui, enabled, |ui| {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(size + 4.0), sense(enabled));
        button_frame(ui, rect, &response, selected, enabled);
        paint_icon(
            ui,
            rect.center(),
            icon,
            size,
            if enabled { color } else { theme::DISABLED },
        );
        response
    })
}

fn sense(enabled: bool) -> Sense {
    if enabled {
        Sense::click()
    } else {
        Sense::hover()
    }
}

/// A ribbon group: a fixed-height body with the caption underneath, then a separator.
pub fn group(ui: &mut Ui, caption: &str, add: impl FnOnce(&mut Ui)) {
    let top = ui.max_rect().top();
    let response = ui
        .allocate_ui_with_layout(
            Vec2::new(ui.available_width().min(2000.0), RIBBON_BODY),
            egui::Layout::left_to_right(egui::Align::Min),
            |ui| {
                ui.set_height(RIBBON_BODY);
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.add_space(4.0);
                add(ui);
                ui.add_space(4.0);
            },
        )
        .response;
    caption_and_separator(
        ui,
        caption,
        response.rect.left(),
        response.rect.right(),
        top,
    );
    ui.add_space(3.0);
}

fn caption_and_separator(ui: &Ui, caption: &str, left: f32, right: f32, top: f32) {
    ui.painter().text(
        egui::pos2((left + right) / 2.0, top + CAPTION_Y),
        Align2::CENTER_CENTER,
        caption,
        FontId::proportional(12.0),
        theme::CAPTION,
    );
    ui.painter().vline(
        right + 1.0,
        (top + 6.0)..=(top + 98.0),
        Stroke::new(1.0, Color32::from_rgb(0xD5, 0xD5, 0xD5)),
    );
}

/// A ribbon group of fixed width whose content is placed at absolute offsets from the
/// group's top-left corner (used where the layout must match measured coordinates).
pub fn fixed_group(ui: &mut Ui, caption: &str, width: f32, add: impl FnOnce(&mut Ui, egui::Pos2)) {
    let top = ui.max_rect().top();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, RIBBON_BODY), Sense::hover());
    let origin = egui::pos2(rect.left(), top);
    add(ui, origin);
    caption_and_separator(ui, caption, rect.left(), rect.right(), top);
    ui.add_space(1.0);
}

/// Put a widget at `origin + (x, y)` with size `(w, h)`.
pub fn place<R>(
    ui: &mut Ui,
    origin: egui::Pos2,
    (x, y, w, h): (f32, f32, f32, f32),
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    let rect = Rect::from_min_size(origin + Vec2::new(x, y), Vec2::new(w, h));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    add(&mut child)
}

/// A column of small buttons inside a group.
pub fn column(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        ui.add_space(6.0);
        add(ui)
    });
}

/// Title bar of a docked browser ("Variable Browser" with float/close icons), 18 px high.
pub fn dock_header(ui: &mut Ui, title: &str) -> bool {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
    let painter = ui.painter();
    painter.rect(
        rect,
        0.0,
        theme::BG,
        Stroke::new(1.0, Color32::from_rgb(0xD0, 0xD0, 0xD0)),
        egui::StrokeKind::Inside,
    );
    painter.text(
        egui::pos2(rect.left() + 5.0, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(12.0),
        theme::TEXT,
    );
    let close_rect = Rect::from_center_size(
        egui::pos2(rect.right() - 10.0, rect.center().y),
        Vec2::splat(14.0),
    );
    let float_rect = Rect::from_center_size(
        egui::pos2(rect.right() - 27.0, rect.center().y),
        Vec2::splat(14.0),
    );
    let close = ui.interact(
        close_rect,
        ui.scope_id().with((title, "close")),
        Sense::click(),
    );
    if close.hovered() {
        ui.painter().rect_filled(close_rect, 1.0, theme::HOVER_BG);
    }
    paint_icon(
        ui,
        close_rect.center(),
        egui_material_icons::icons::ICON_CLOSE,
        12.0,
        theme::TEXT,
    );
    paint_icon(
        ui,
        float_rect.center(),
        egui_material_icons::icons::ICON_FILTER_NONE,
        11.0,
        theme::TEXT,
    );
    close.on_hover_text("Close").clicked()
}

/// A 13 px check box with a grey border (variable browser).
pub fn check_box(ui: &mut Ui, rect: Rect, checked: bool) {
    let painter = ui.painter();
    painter.rect(
        rect,
        0.0,
        Color32::WHITE,
        Stroke::new(1.0, Color32::from_rgb(0x33, 0x33, 0x33)),
        egui::StrokeKind::Inside,
    );
    if checked {
        let c = rect.center();
        let pts = vec![
            egui::pos2(c.x - 3.5, c.y),
            egui::pos2(c.x - 1.0, c.y + 2.8),
            egui::pos2(c.x + 3.8, c.y - 2.8),
        ];
        painter.add(egui::Shape::line(
            pts,
            Stroke::new(1.3, Color32::from_rgb(0x33, 0x33, 0x33)),
        ));
    }
}

/// White input-field background with a grey border.
pub fn field_frame(ui: &Ui, rect: Rect) {
    ui.painter().rect(
        rect,
        0.0,
        Color32::WHITE,
        Stroke::new(1.0, Color32::from_rgb(0xAB, 0xAD, 0xB3)),
        egui::StrokeKind::Inside,
    );
}

/// Light-grey combo-box background with a grey border.
pub fn combo_frame(ui: &Ui, rect: Rect) {
    ui.painter().rect(
        rect,
        0.0,
        Color32::from_rgb(0xE5, 0xE5, 0xE5),
        Stroke::new(1.0, Color32::from_rgb(0xAC, 0xAC, 0xAC)),
        egui::StrokeKind::Inside,
    );
}

/// A flat tab used for the model tabs, log tabs and the bottom browser bar.
pub fn flat_tab(ui: &mut Ui, label: &str, selected: bool) -> Response {
    let font = FontId::proportional(12.0);
    let w = ui.ctx().fonts_mut(|f| {
        f.layout_no_wrap(label.into(), font.clone(), theme::TEXT)
            .size()
            .x
    }) + 14.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(w, 20.0), Sense::click());
    let painter = ui.painter();
    if selected {
        painter.rect(
            rect,
            0.0,
            theme::HOVER_BG,
            Stroke::new(1.0, Color32::from_rgb(0x9C, 0xC3, 0xE8)),
            egui::StrokeKind::Inside,
        );
    } else if response.hovered() {
        painter.rect_filled(rect, 0.0, Color32::from_rgb(0xE6, 0xE6, 0xE6));
    }
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        font,
        theme::TEXT,
    );
    response
}

#[cfg(test)]
mod tests {
    use super::{
        big_button, icon_button, model_tab_close_rect, model_tab_label_rect,
        model_tab_width_for_text, small_button,
    };
    use egui::{pos2, Color32, Rect, Vec2};

    #[test]
    fn tab_width_includes_the_close_icon_and_margins() {
        assert_eq!(model_tab_width_for_text(40.0), 96.0);
        assert_eq!(model_tab_width_for_text(120.0), 176.0);
    }

    #[test]
    fn label_and_close_icon_do_not_overlap() {
        let rect = Rect::from_min_size(pos2(10.0, 0.0), Vec2::new(160.0, 22.0));
        let label = model_tab_label_rect(rect);
        let close = model_tab_close_rect(rect);
        assert!(label.left() >= rect.left());
        assert!(label.right() < close.left());
        assert!(close.right() <= rect.right());
    }

    /// G52: `on_disabled_hover_text()` only opens when `!response.enabled()`, so a
    /// disabled ribbon button must not report itself as enabled.
    #[test]
    fn disabled_buttons_report_themselves_as_disabled() {
        use egui_material_icons::icons::ICON_ADD;

        let ctx = egui::Context::default();
        let mut flags = Vec::new();
        let mut rects = Vec::new();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            for enabled in [true, false] {
                let small = small_button(ui, ICON_ADD, Color32::BLACK, "Same", enabled, false);
                flags.push(small.enabled());
                rects.push(small.rect);
                flags.push(
                    big_button(ui, ICON_ADD, Color32::BLACK, "Same", enabled, false).enabled(),
                );
                flags.push(
                    icon_button(ui, ICON_ADD, Color32::BLACK, 16.0, enabled, false).enabled(),
                );
            }
        });
        out.textures_delta.clear();

        assert_eq!(flags, [true, true, true, false, false, false]);
        // The disabled wrapper must not resize the button: the ribbon places groups
        // at fixed coordinates.
        assert_eq!(rects[0].size(), rects[1].size());
    }
}

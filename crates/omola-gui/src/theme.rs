//! Light desktop theme, fonts and icon font.
//!
//! Colours are Omola's own (a neutral light-gray workbench with blue selection). Fonts are
//! loaded at run time from the operating system when available (nothing proprietary is
//! bundled); egui's built-in fonts are the fallback. Icons use Material Symbols (Apache-2.0).

use egui::{Color32, FontData, FontDefinitions, FontFamily, Stroke, Visuals};
use std::sync::Arc;

pub const BG: Color32 = Color32::from_rgb(0xF0, 0xF0, 0xF0);
pub const WHITE: Color32 = Color32::WHITE;
pub const BORDER: Color32 = Color32::from_rgb(0xD0, 0xD0, 0xD0);
pub const TEXT: Color32 = Color32::from_rgb(0x1E, 0x1E, 0x1E);
pub const TEXT_WEAK: Color32 = Color32::from_rgb(0x70, 0x70, 0x70);
pub const DISABLED: Color32 = Color32::from_rgb(0xA8, 0xA8, 0xA8);
pub const ACCENT: Color32 = Color32::from_rgb(0x1F, 0x6F, 0xC8);
pub const SELECT_BG: Color32 = Color32::from_rgb(0x0F, 0x5C, 0xA8);
pub const HOVER_BG: Color32 = Color32::from_rgb(0xDD, 0xEB, 0xF8);
pub const PRESSED_BG: Color32 = Color32::from_rgb(0xC5, 0xDD, 0xF3);
pub const TITLE_BAR: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
pub const CAPTION: Color32 = Color32::from_rgb(0x5A, 0x5A, 0x5A);
pub const DOC_GREEN: Color32 = Color32::from_rgb(0x2E, 0x8B, 0x22);
pub const LINK: Color32 = Color32::from_rgb(0x00, 0x33, 0xCC);
pub const CONTEXT_TAB: Color32 = Color32::from_rgb(0x71, 0xBE, 0x59);
pub const FILE_BLUE: Color32 = Color32::from_rgb(0x19, 0x79, 0xCA);
pub const FIELD_BORDER: Color32 = Color32::from_rgb(0xBD, 0xBD, 0xBD);

/// Font family names used by the documentation renderer.
pub const BOLD: &str = "omola-bold";
pub const ITALIC: &str = "omola-italic";
pub const BOLD_ITALIC: &str = "omola-bold-italic";

pub fn bold() -> FontFamily {
    FontFamily::Name(BOLD.into())
}
pub fn italic() -> FontFamily {
    FontFamily::Name(ITALIC.into())
}
pub fn bold_italic() -> FontFamily {
    FontFamily::Name(BOLD_ITALIC.into())
}

pub fn apply(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    egui_material_icons::initialize(ctx);
    ctx.set_theme(egui::Theme::Light);
    ctx.all_styles_mut(|style| {
        let mut v = Visuals::light();
        v.panel_fill = BG;
        v.window_fill = WHITE;
        v.extreme_bg_color = WHITE;
        v.faint_bg_color = Color32::from_rgb(0xF5, 0xF8, 0xFC);
        v.override_text_color = Some(TEXT);
        v.hyperlink_color = LINK;
        v.selection.bg_fill = SELECT_BG;
        v.selection.stroke = Stroke::new(1.0, WHITE);
        v.window_stroke = Stroke::new(1.0, BORDER);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
        v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        v.widgets.inactive.bg_fill = WHITE;
        v.widgets.inactive.bg_stroke = Stroke::NONE;
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
        v.widgets.hovered.weak_bg_fill = HOVER_BG;
        v.widgets.hovered.bg_fill = HOVER_BG;
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x9C, 0xC3, 0xE8));
        v.widgets.active.weak_bg_fill = PRESSED_BG;
        v.widgets.active.bg_fill = PRESSED_BG;
        v.widgets.open.weak_bg_fill = HOVER_BG;
        for w in [
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
            &mut v.widgets.noninteractive,
        ] {
            w.corner_radius = egui::CornerRadius::same(1);
        }
        style.visuals = v;
        style.spacing.item_spacing = egui::vec2(4.0, 2.0);
        style.spacing.button_padding = egui::vec2(4.0, 2.0);
        style.spacing.interact_size.y = 20.0;
        style.spacing.indent = 14.0;
        use egui::{FontId, TextStyle};
        style.text_styles = [
            (TextStyle::Small, FontId::proportional(10.5)),
            (TextStyle::Body, FontId::proportional(12.0)),
            (TextStyle::Button, FontId::proportional(12.0)),
            (TextStyle::Heading, FontId::proportional(16.0)),
            (TextStyle::Monospace, FontId::monospace(12.0)),
        ]
        .into();
    });
}

/// First existing file among `candidates`, loaded as font data.
fn system_font(candidates: &[&str]) -> Option<FontData> {
    candidates
        .iter()
        .find_map(|path| std::fs::read(path).ok())
        .map(FontData::from_owned)
}

fn windows_font(name: &str) -> String {
    let root = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    format!("{root}\\Fonts\\{name}")
}

fn fonts() -> FontDefinitions {
    let mut defs = FontDefinitions::default();
    let win = |n: &str| windows_font(n);
    let regular = system_font(&[
        &win("segoeui.ttf"),
        "/System/Library/Fonts/SFNS.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ]);
    let bold_font = system_font(&[
        &win("segoeuib.ttf"),
        "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
    ]);
    let italic_font = system_font(&[
        &win("segoeuii.ttf"),
        "/usr/share/fonts/truetype/liberation/LiberationSans-Italic.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Oblique.ttf",
        "/usr/share/fonts/TTF/DejaVuSans-Oblique.ttf",
    ]);
    let bold_italic_font = system_font(&[
        &win("segoeuiz.ttf"),
        "/usr/share/fonts/truetype/liberation/LiberationSans-BoldItalic.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-BoldOblique.ttf",
        "/usr/share/fonts/TTF/DejaVuSans-BoldOblique.ttf",
    ]);
    let mono = system_font(&[
        &win("cour.ttf"),
        "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
        "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
    ]);
    // Symbols such as U+25BE are missing from some UI fonts; DejaVu covers them.
    let symbols = system_font(&[
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        &win("seguisym.ttf"),
    ]);
    let cjk = system_font(&[
        &win("YuGothM.ttc"),
        &win("meiryo.ttc"),
        &win("msgothic.ttc"),
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/fonts-japanese-gothic.ttf",
    ]);

    let mut add = |name: &str, data: Option<FontData>| -> bool {
        match data {
            Some(d) => {
                defs.font_data.insert(name.to_owned(), Arc::new(d));
                true
            }
            None => false,
        }
    };
    let has_regular = add("sys-regular", regular);
    let has_bold = add("sys-bold", bold_font);
    let has_italic = add("sys-italic", italic_font);
    let has_bold_italic = add("sys-bold-italic", bold_italic_font);
    let has_mono = add("sys-mono", mono);
    let has_cjk = add("sys-cjk", cjk);
    let has_symbols = add("sys-symbols", symbols);

    let base: Vec<String> = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let with = |first: Option<&str>| -> Vec<String> {
        let mut list: Vec<String> = first.map(|f| vec![f.to_owned()]).unwrap_or_default();
        list.extend(base.iter().cloned());
        if has_symbols {
            list.push("sys-symbols".into());
        }
        if has_cjk {
            list.push("sys-cjk".into());
        }
        list
    };
    let regular_name = has_regular.then_some("sys-regular");
    defs.families
        .insert(FontFamily::Proportional, with(regular_name));
    defs.families.insert(
        bold(),
        with(has_bold.then_some("sys-bold").or(regular_name)),
    );
    defs.families.insert(
        italic(),
        with(has_italic.then_some("sys-italic").or(regular_name)),
    );
    defs.families.insert(
        bold_italic(),
        with(
            has_bold_italic
                .then_some("sys-bold-italic")
                .or(has_bold.then_some("sys-bold"))
                .or(regular_name),
        ),
    );
    if has_mono {
        if let Some(list) = defs.families.get_mut(&FontFamily::Monospace) {
            list.insert(0, "sys-mono".into());
        }
    }
    if has_cjk {
        if let Some(list) = defs.families.get_mut(&FontFamily::Monospace) {
            list.push("sys-cjk".into());
        }
    }
    defs
}

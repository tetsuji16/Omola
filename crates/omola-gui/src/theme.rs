//! Windows 11 (WinUI/Fluent) look for the egui workbench.
//!
//! egui's stock appearance is noticeably un-Windows: bundled Ubuntu fonts, very
//! dark panels, pill-shaped widgets and cramped toolbars. This module installs
//! the system UI fonts (Segoe UI, Consolas), a Fluent light/dark palette based on
//! the current Windows accent colour, and Windows-sized metrics so the native
//! window reads as a regular Windows desktop application.

use egui::style::Selection;
use egui::{
    Color32, CornerRadius, FontData, FontFamily, FontId, FontInsert, FontPriority,
    InsertFontFamily, Margin, Shadow, Stroke, TextStyle, Theme, ThemePreference, Visuals,
};

/// Used when Windows does not report an accent colour.
const FALLBACK_ACCENT: Color32 = Color32::from_rgb(0x00, 0x5F, 0xB8);

/// Install fonts and both theme palettes. Call once before the first frame.
pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);

    let accent = system_accent().unwrap_or(FALLBACK_ACCENT);
    ctx.style_mut_of(Theme::Light, |style| apply(style, false, accent));
    ctx.style_mut_of(Theme::Dark, |style| apply(style, true, accent));
    // Follow Windows' light/dark app setting; both palettes above are customised.
    ctx.set_theme(ThemePreference::System);
}

/// Accent colour for filled controls (primary buttons, tab indicators).
pub fn accent(ui: &egui::Ui) -> Color32 {
    ui.visuals().hyperlink_color
}

/// Readable foreground for a filled accent control.
pub fn on_accent(ui: &egui::Ui) -> Color32 {
    let c = accent(ui);
    let luminance = 0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32;
    if luminance > 150.0 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

fn install_fonts(ctx: &egui::Context) {
    let fonts = std::env::var_os("WINDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows"))
        .join("Fonts");

    // Native UI face. Falls back to egui's bundled fonts when it is unavailable.
    add_font(
        ctx,
        "Segoe UI",
        fonts.join("segoeui.ttf"),
        &[(FontFamily::Proportional, FontPriority::Highest)],
    );
    // Windows code editor face.
    add_font(
        ctx,
        "Consolas",
        fonts.join("consola.ttf"),
        &[(FontFamily::Monospace, FontPriority::Highest)],
    );
    // Japanese fallback for file names, diagnostics and comments.
    add_font(
        ctx,
        "Yu Gothic UI",
        fonts.join("YuGothR.ttc"),
        &[
            (FontFamily::Proportional, FontPriority::Lowest),
            (FontFamily::Monospace, FontPriority::Lowest),
        ],
    );
}

fn add_font(
    ctx: &egui::Context,
    name: &str,
    path: std::path::PathBuf,
    families: &[(FontFamily, FontPriority)],
) {
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let families = families
        .iter()
        .map(|(family, priority)| InsertFontFamily {
            family: family.clone(),
            priority: *priority,
        })
        .collect();
    ctx.add_font(FontInsert::new(name, FontData::from_owned(bytes), families));
}

fn apply(style: &mut egui::Style, dark: bool, accent: Color32) {
    style.visuals = if dark {
        dark_visuals(accent)
    } else {
        light_visuals(accent)
    };

    // Windows control metrics (96 dpi): 14 px body text, ~26 px controls.
    style.spacing.item_spacing = egui::vec2(8.0, 4.0);
    style.spacing.button_padding = egui::vec2(12.0, 4.0);
    style.spacing.menu_margin = Margin::same(4);
    style.spacing.window_margin = Margin::same(10);
    style.spacing.interact_size = egui::vec2(72.0, 26.0);
    style.spacing.combo_width = 180.0;
    style.spacing.text_edit_width = 280.0;
    style.spacing.slider_width = 140.0;
    style.spacing.icon_width = 14.0;
    style.spacing.icon_width_inner = 8.0;
    style.spacing.indent = 18.0;
    style.animation_time = 0.08;

    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(20.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();
}

fn light_visuals(accent: Color32) -> Visuals {
    let text = Color32::from_rgb(0x1A, 0x1A, 0x1A);
    let weak = Color32::from_rgb(0x5D, 0x5D, 0x5D);
    let border = Color32::from_rgb(0xDF, 0xDF, 0xDF);
    let surface = Color32::from_rgb(0xFA, 0xFA, 0xFA);
    let control = Color32::from_rgb(0xFC, 0xFC, 0xFC);
    let hover = Color32::from_rgb(0xF5, 0xF5, 0xF5);
    let pressed = Color32::from_rgb(0xEB, 0xEB, 0xEB);

    let link = darken(accent, 0.25);
    let mut v = Visuals::light();
    v.panel_fill = Color32::from_rgb(0xF3, 0xF3, 0xF3);
    v.window_fill = surface;
    v.window_stroke = Stroke::new(1.0, border);
    v.window_corner_radius = CornerRadius::same(8);
    v.menu_corner_radius = CornerRadius::same(8);
    v.window_highlight_topmost = false;
    v.window_shadow = Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(36),
    };
    v.popup_shadow = Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(48),
    };
    v.extreme_bg_color = Color32::WHITE;
    v.text_edit_bg_color = Some(Color32::WHITE);
    v.code_bg_color = Color32::from_rgb(0xF6, 0xF6, 0xF6);
    v.faint_bg_color = Color32::from_rgb(0xEB, 0xEB, 0xEB);
    v.hyperlink_color = link;
    v.warn_fg_color = Color32::from_rgb(0x9D, 0x5D, 0x00);
    v.error_fg_color = Color32::from_rgb(0xC4, 0x2B, 0x1C);
    v.weak_text_color = Some(weak);
    v.selection = Selection {
        bg_fill: mix(accent, Color32::WHITE, 0.78),
        stroke: Stroke::new(1.0, link),
    };
    v.text_cursor.stroke = Stroke::new(1.0, text);
    v.widgets = widgets(surface, control, hover, pressed, border, text);
    v.striped = false;
    v.button_frame = true;
    v
}

fn dark_visuals(accent: Color32) -> Visuals {
    let text = Color32::from_rgb(0xF2, 0xF2, 0xF2);
    let weak = Color32::from_rgb(0xAF, 0xAF, 0xAF);
    let border = Color32::from_rgb(0x3D, 0x3D, 0x3D);
    let surface = Color32::from_rgb(0x28, 0x28, 0x28);
    let control = Color32::from_rgb(0x2E, 0x2E, 0x2E);
    let hover = Color32::from_rgb(0x35, 0x35, 0x35);
    let pressed = Color32::from_rgb(0x3C, 0x3C, 0x3C);

    let link = mix(accent, Color32::WHITE, 0.35);
    let mut v = Visuals::dark();
    v.panel_fill = Color32::from_rgb(0x20, 0x20, 0x20);
    v.window_fill = surface;
    v.window_stroke = Stroke::new(1.0, border);
    v.window_corner_radius = CornerRadius::same(8);
    v.menu_corner_radius = CornerRadius::same(8);
    v.window_highlight_topmost = false;
    v.window_shadow = Shadow {
        offset: [0, 4],
        blur: 16,
        spread: 0,
        color: Color32::from_black_alpha(120),
    };
    v.popup_shadow = Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(140),
    };
    v.extreme_bg_color = Color32::from_rgb(0x1B, 0x1B, 0x1B);
    v.text_edit_bg_color = Some(Color32::from_rgb(0x1B, 0x1B, 0x1B));
    v.code_bg_color = Color32::from_rgb(0x24, 0x24, 0x24);
    v.faint_bg_color = Color32::from_rgb(0x33, 0x33, 0x33);
    v.hyperlink_color = link;
    v.warn_fg_color = Color32::from_rgb(0xF7, 0xB7, 0x63);
    v.error_fg_color = Color32::from_rgb(0xFF, 0x99, 0xA4);
    v.weak_text_color = Some(weak);
    v.selection = Selection {
        bg_fill: mix(accent, Color32::from_rgb(0x20, 0x20, 0x20), 0.60),
        stroke: Stroke::new(1.0, link),
    };
    v.text_cursor.stroke = Stroke::new(1.0, text);
    v.widgets = widgets(surface, control, hover, pressed, border, text);
    v.striped = false;
    v.button_frame = true;
    v
}

fn widgets(
    surface: Color32,
    control: Color32,
    hover: Color32,
    pressed: Color32,
    border: Color32,
    text: Color32,
) -> egui::style::Widgets {
    let frame = |fill: Color32, stroke: Color32| egui::style::WidgetVisuals {
        bg_fill: fill,
        weak_bg_fill: fill,
        bg_stroke: Stroke::new(1.0, stroke),
        corner_radius: CornerRadius::same(4),
        fg_stroke: Stroke::new(1.0, text),
        expansion: 0.0,
    };
    egui::style::Widgets {
        noninteractive: egui::style::WidgetVisuals {
            bg_fill: surface,
            weak_bg_fill: Color32::TRANSPARENT,
            bg_stroke: Stroke::new(1.0, border),
            corner_radius: CornerRadius::ZERO,
            fg_stroke: Stroke::new(1.0, text),
            expansion: 0.0,
        },
        inactive: frame(control, border),
        hovered: frame(hover, border),
        active: frame(pressed, border),
        open: frame(pressed, border),
    }
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let ch = |x: u8, y: u8| (x as f32 * (1.0 - t) + y as f32 * t).round() as u8;
    Color32::from_rgb(ch(a.r(), b.r()), ch(a.g(), b.g()), ch(a.b(), b.b()))
}

fn darken(c: Color32, amount: f32) -> Color32 {
    mix(c, Color32::BLACK, amount)
}

/// Windows' current accent colour, e.g. from Settings > Personalisation > Colours.
#[cfg(windows)]
fn system_accent() -> Option<Color32> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let output = std::process::Command::new("reg")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\DWM",
            "/v",
            "ColorizationColor",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let value = text
        .split_whitespace()
        .find(|token| token.starts_with("0x"))?;
    // Stored as 0xAARRGGBB.
    let value = u32::from_str_radix(value.trim_start_matches("0x"), 16).ok()?;
    let color = Color32::from_rgb(
        ((value >> 16) & 0xFF) as u8,
        ((value >> 8) & 0xFF) as u8,
        (value & 0xFF) as u8,
    );
    // Greys mean "accent colour on title bars" is off; use the Fluent default.
    let max = color.r().max(color.g()).max(color.b());
    let min = color.r().min(color.g()).min(color.b());
    (max - min > 12).then_some(color)
}

#[cfg(not(windows))]
fn system_accent() -> Option<Color32> {
    None
}

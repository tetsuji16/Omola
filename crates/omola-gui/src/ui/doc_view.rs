//! Documentation layer: renders `Documentation(info=...)` HTML of the active class.

use super::widgets::paint_icon;
use crate::app::{DocView, Loadable, OmolaApp};
use crate::html::{Block, ImageRef, Inline, Style};
use crate::library::{resolve_link, LinkTarget};
use crate::theme;
use egui::{Color32, FontFamily, RichText, Sense, Stroke, Vec2};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const BODY: f32 = 12.5;

/// Rendering context shared by all blocks of one page.
struct Ctx<'a> {
    class: &'a str,
    roots: &'a BTreeMap<String, PathBuf>,
    find: String,
    clicked: Option<String>,
}

impl OmolaApp {
    pub fn documentation_layer(&mut self, ui: &mut egui::Ui, class: &str) {
        if self.find_open {
            ui.horizontal(|ui| {
                ui.add_space(6.0);
                ui.label("Find:");
                let r =
                    ui.add(egui::TextEdit::singleline(&mut self.find_query).desired_width(220.0));
                if !r.has_focus() && self.find_query.is_empty() {
                    r.request_focus();
                }
                if ui.small_button("x").clicked() {
                    self.find_open = false;
                    self.find_query.clear();
                }
            });
            ui.separator();
        }
        let page = match self.docs.get(class) {
            None => {
                self.request_doc(class);
                return loading(ui);
            }
            Some(Loadable::Loading) => return loading(ui),
            Some(Loadable::Failed(e)) => {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!("  Documentation is not available: {e}"))
                        .color(theme::TEXT_WEAK),
                );
                return;
            }
            Some(Loadable::Ready(page)) => page,
        };
        let comment = if page.doc.comment.is_empty() {
            class.to_owned()
        } else {
            page.doc.comment.clone()
        };
        let mut ctx = Ctx {
            class,
            roots: &page.doc.roots,
            find: if self.find_open {
                self.find_query.to_lowercase()
            } else {
                String::new()
            },
            clicked: None,
        };
        let children = if page.doc.restriction == "package" {
            self.children_of(class)
        } else {
            Vec::new()
        };
        let view = self.doc_view;
        egui::ScrollArea::vertical()
            .id_salt(("doc", class))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin {
                        left: 8,
                        right: 12,
                        top: 8,
                        bottom: 12,
                    })
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = 4.0;
                        match view {
                            DocView::Formatted => {
                                ui.label(RichText::new(comment).family(theme::bold()).size(BODY));
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new("Information")
                                        .family(theme::bold())
                                        .size(13.5)
                                        .color(theme::DOC_GREEN),
                                );
                                ui.add_space(2.0);
                                if page.info.is_empty() {
                                    ui.label(
                                        RichText::new("(no information)").color(theme::TEXT_WEAK),
                                    );
                                }
                                blocks(ui, &page.info, &mut ctx);
                                if !children.is_empty() {
                                    ui.add_space(10.0);
                                    ui.label(
                                        RichText::new("Package Content")
                                            .family(theme::bold())
                                            .size(13.5)
                                            .color(theme::DOC_GREEN),
                                    );
                                    ui.add_space(2.0);
                                    package_content(ui, &children, &mut ctx);
                                }
                            }
                            DocView::InfoSource => source_text(ui, &page.doc.info_html),
                            DocView::RevisionSource => source_text(ui, &page.doc.revisions_html),
                        }
                    });
            });
        if let Some(href) = ctx.clicked.take() {
            self.follow_link(class, &href);
        }
    }

    fn follow_link(&mut self, class: &str, href: &str) {
        let roots = match self.docs.get(class) {
            Some(Loadable::Ready(p)) => p.doc.roots.clone(),
            _ => BTreeMap::new(),
        };
        match resolve_link(href, class, &roots) {
            LinkTarget::Class(target) => self.open_class(&target),
            LinkTarget::External(url) => self.open_url(&url),
            LinkTarget::File(path) => self.open_url(&format!("file://{}", path.display())),
            LinkTarget::Unsupported(link) => self.error(
                crate::backend::LogTab::Syntax,
                format!("Unsupported documentation link: {link}"),
            ),
        }
    }

    fn open_url(&mut self, url: &str) {
        // Opened by the operating system; nothing is fetched by Omola itself.
        self.pending_url = Some(url.to_owned());
    }
}

fn loading(ui: &mut egui::Ui) {
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.spinner();
        ui.label("Loading documentation from OMC...");
    });
}

fn source_text(ui: &mut egui::Ui, text: &str) {
    let mut t = text;
    ui.add(
        egui::TextEdit::multiline(&mut t)
            .code_editor()
            .frame(egui::Frame::NONE)
            .desired_width(f32::INFINITY),
    );
}

fn blocks(ui: &mut egui::Ui, list: &[Block], ctx: &mut Ctx<'_>) {
    for block in list {
        block_ui(ui, block, ctx);
    }
}

fn block_ui(ui: &mut egui::Ui, block: &Block, ctx: &mut Ctx<'_>) {
    match block {
        Block::Paragraph(inlines) => {
            paragraph(ui, inlines, BODY, false, ctx);
            ui.add_space(5.0);
        }
        Block::Heading(level, inlines) => {
            let size = match level {
                1 => 20.0,
                2 => 17.0,
                3 => 15.0,
                4 => 13.5,
                _ => BODY,
            };
            ui.add_space(4.0);
            paragraph(ui, inlines, size, true, ctx);
            ui.add_space(4.0);
        }
        Block::List { ordered, items } => {
            for (i, item) in items.iter().enumerate() {
                ui.horizontal_top(|ui| {
                    ui.add_space(18.0);
                    let marker = if *ordered {
                        format!("{}.", i + 1)
                    } else {
                        "\u{2022}".into()
                    };
                    ui.add_sized(
                        Vec2::new(14.0, 16.0),
                        egui::Label::new(RichText::new(marker).size(BODY)),
                    );
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        for b in item {
                            match b {
                                Block::Paragraph(inl) => paragraph(ui, inl, BODY, false, ctx),
                                other => block_ui(ui, other, ctx),
                            }
                        }
                    });
                });
            }
            ui.add_space(5.0);
        }
        Block::Table(rows) => {
            egui::Frame::new()
                .stroke(Stroke::new(1.0, Color32::from_rgb(0xC0, 0xC0, 0xC0)))
                .inner_margin(2.0)
                .show(ui, |ui| {
                    egui::Grid::new(ui.next_auto_id())
                        .striped(false)
                        .spacing(Vec2::new(10.0, 4.0))
                        .show(ui, |ui| {
                            for row in rows {
                                for cell in row {
                                    ui.vertical(|ui| {
                                        for b in &cell.blocks {
                                            match b {
                                                Block::Paragraph(inl) => {
                                                    paragraph(ui, inl, BODY, cell.header, ctx)
                                                }
                                                other => block_ui(ui, other, ctx),
                                            }
                                        }
                                    });
                                }
                                ui.end_row();
                            }
                        });
                });
            ui.add_space(5.0);
        }
        Block::Preformatted(text) => {
            egui::Frame::new()
                .fill(Color32::from_rgb(0xF6, 0xF6, 0xF6))
                .inner_margin(6.0)
                .show(ui, |ui| {
                    ui.label(RichText::new(text).monospace().size(12.0));
                });
            ui.add_space(5.0);
        }
        Block::Rule => {
            ui.separator();
        }
        Block::Quote(inner) => {
            ui.horizontal_top(|ui| {
                ui.add_space(24.0);
                ui.vertical(|ui| blocks(ui, inner, ctx));
            });
        }
    }
}

fn family(style: &Style, force_bold: bool) -> FontFamily {
    let bold = style.bold || force_bold;
    if style.code {
        FontFamily::Monospace
    } else if bold && style.italic {
        theme::bold_italic()
    } else if bold {
        theme::bold()
    } else if style.italic {
        theme::italic()
    } else {
        FontFamily::Proportional
    }
}

fn paragraph(ui: &mut egui::Ui, inlines: &[Inline], size: f32, bold: bool, ctx: &mut Ctx<'_>) {
    // Images that stand alone in a paragraph are laid out as blocks.
    if inlines.iter().all(|i| matches!(i, Inline::Image(_))) {
        for inline in inlines {
            if let Inline::Image(img) = inline {
                image(ui, img, ctx);
            }
        }
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
        for inline in inlines {
            match inline {
                Inline::Text { text, style, link } => {
                    let size = if style.sup || style.sub {
                        size * 0.75
                    } else {
                        size
                    };
                    for (piece, hit) in split_find(text, &ctx.find) {
                        let mut rich = RichText::new(piece).family(family(style, bold)).size(size);
                        if style.underline || link.is_some() {
                            rich = rich.underline();
                        }
                        if link.is_some() {
                            rich = rich.color(theme::LINK);
                        }
                        if hit {
                            rich = rich.background_color(Color32::from_rgb(0xFF, 0xE0, 0x66));
                        }
                        match link {
                            Some(href) => {
                                let r = ui.add(egui::Label::new(rich).sense(Sense::click()));
                                let r = r
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .on_hover_text(href);
                                if r.clicked() {
                                    ctx.clicked = Some(href.clone());
                                }
                            }
                            None => {
                                ui.add(egui::Label::new(rich));
                            }
                        }
                    }
                }
                Inline::LineBreak => ui.end_row(),
                Inline::Image(img) => image(ui, img, ctx),
            }
        }
    });
}

/// Split text into (piece, matches_find_query) runs, case-insensitively.
pub fn split_find<'t>(text: &'t str, query: &str) -> Vec<(&'t str, bool)> {
    if query.is_empty() {
        return vec![(text, false)];
    }
    let lower = text.to_lowercase();
    if lower.len() != text.len() {
        // Lower-casing changed byte lengths; fall back to a whole-run match.
        return vec![(text, lower.contains(query))];
    }
    let mut out = Vec::new();
    let mut last = 0;
    let mut from = 0;
    while let Some(pos) = lower[from..].find(query) {
        let start = from + pos;
        let end = start + query.len();
        if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
            break;
        }
        if start > last {
            out.push((&text[last..start], false));
        }
        out.push((&text[start..end], true));
        last = end;
        from = end;
    }
    if last < text.len() {
        out.push((&text[last..], false));
    }
    out
}

fn image(ui: &mut egui::Ui, img: &ImageRef, ctx: &mut Ctx<'_>) {
    match resolve_link(&img.src, ctx.class, ctx.roots) {
        LinkTarget::File(path) if path.is_file() => {
            let uri = file_uri(&path);
            let mut image = egui::Image::new(&uri).fit_to_original_size(1.0);
            let max_w = ui.available_width().max(40.0);
            image = match (img.width, img.height) {
                (Some(w), Some(h)) => image.fit_to_exact_size(Vec2::new(w.min(max_w), h)),
                (Some(w), None) => image.max_width(w.min(max_w)),
                (None, Some(h)) => image.max_height(h).max_width(max_w),
                (None, None) => image.max_width(max_w),
            };
            match ui
                .ctx()
                .try_load_image(&uri, egui::load::SizeHint::Width(1024))
            {
                Ok(_) => {
                    ui.add(image);
                }
                Err(error) => {
                    let label = if img.alt.is_empty() {
                        img.src.as_str()
                    } else {
                        img.alt.as_str()
                    };
                    ui.label(
                        RichText::new(format!("[image: {label}]"))
                            .color(theme::TEXT_WEAK)
                            .size(11.0),
                    )
                    .on_hover_text(error.to_string());
                }
            }
        }
        _ => {
            let label = if img.alt.is_empty() {
                img.src.as_str()
            } else {
                img.alt.as_str()
            };
            ui.label(
                RichText::new(format!("[image: {label}]"))
                    .color(theme::TEXT_WEAK)
                    .size(11.0),
            );
        }
    }
}

/// egui_extras 0.37 strips `file://` and on Windows strips one leading slash, so
/// absolute drive paths must use the standard `file:///C:/...` form.
fn file_uri(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    let absolute = if path.starts_with('/') {
        path
    } else {
        format!("/{path}")
    };
    let escaped = absolute
        .replace('%', "%25")
        .replace(' ', "%20")
        .replace('#', "%23");
    format!("file://{escaped}")
}

fn package_content(ui: &mut egui::Ui, children: &[(String, String, String)], ctx: &mut Ctx<'_>) {
    egui::Frame::new()
        .stroke(Stroke::new(1.0, Color32::from_rgb(0xC0, 0xC0, 0xC0)))
        .show(ui, |ui| {
            egui::Grid::new(("package_content", ctx.class))
                .striped(true)
                .spacing(Vec2::new(12.0, 3.0))
                .show(ui, |ui| {
                    ui.label(RichText::new("Name").family(theme::bold()).size(BODY));
                    ui.label(
                        RichText::new("Description")
                            .family(theme::bold())
                            .size(BODY),
                    );
                    ui.end_row();
                    for (name, restriction, comment) in children {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                            let icon = if restriction == "package" {
                                egui_material_icons::icons::ICON_INVENTORY_2
                            } else {
                                egui_material_icons::icons::ICON_WIDGETS
                            };
                            paint_icon(ui, r.center(), icon, 13.0, theme::TEXT_WEAK);
                            let short = crate::library::short_name(name);
                            let link = ui
                                .add(
                                    egui::Label::new(
                                        RichText::new(short)
                                            .size(BODY)
                                            .color(theme::LINK)
                                            .underline(),
                                    )
                                    .sense(Sense::click()),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand);
                            if link.clicked() {
                                ctx.clicked = Some(format!("modelica://{name}"));
                            }
                        });
                        ui.label(RichText::new(comment).size(BODY));
                        ui.end_row();
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use super::{file_uri, split_find};
    use std::path::Path;

    #[test]
    fn file_uri_formats_unix_and_windows_paths() {
        assert_eq!(
            file_uri(Path::new("/tmp/a b/x.png")),
            "file:///tmp/a%20b/x.png"
        );
        assert_eq!(
            file_uri(Path::new(r"C:\Users\a b\x.png")),
            "file:///C:/Users/a%20b/x.png"
        );
    }

    #[test]
    fn find_splits_case_insensitively() {
        assert_eq!(
            split_find("The Modelica library", "modelica"),
            vec![("The ", false), ("Modelica", true), (" library", false)]
        );
        assert_eq!(split_find("abc", ""), vec![("abc", false)]);
        assert_eq!(
            split_find("aXa", "a"),
            vec![("a", true), ("X", false), ("a", true)]
        );
    }
}

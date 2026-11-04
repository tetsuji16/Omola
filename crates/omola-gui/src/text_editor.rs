//! Text editor with syntax highlighting, line numbers, and error navigation (G11).
//!
//! Uses `syntect` for Modelica syntax highlighting (via a similar language definition)
//! and integrates with egui for rendering.

use egui::{Align2, Color32, FontFamily, FontId, ScrollArea, Sense, Stroke, Ui, Vec2};
use std::path::PathBuf;
use syntect::{
    easy::HighlightLines,
    highlighting::{Style, ThemeSet},
    parsing::SyntaxSet,
};

/// Modelica syntax definition embedded as a `.sublime-syntax` compatible YAML.
// Modelica is Pascal/Ada-like; we base it on Ada for keywords and add Modelica-specific ones.
#[allow(dead_code)]
const MODELICA_SYNTAX: &str = r#"
name: Modelica
scope: source.modelica
file_extensions:
  - mo
  - mos
first_line_match: '(?i)^(model|package|block|connector|record|function|type|class)\b'
contexts:
  main:
    - match: '(?i)^\s*(model|package|block|connector|record|function|type|class|operator|record|connector|package|model|block|function|type|class|operator)\b'
      scope: keyword.definition.modelica
    - match: '(?i)\b(if|then|else|elseif|elsif|end|for|while|when|do|loop|repeat|until|return|break|continue|else|elseif|elsif|then|end)\b'
      scope: keyword.control.modelica
    - match: '(?i)\b(and|or|not|xor)\b'
      scope: keyword.operator.logical.modelica
    - match: '(?i)\b(real|integer|boolean|string|enumeration|type|extends|redeclare|replaceable|const|parameter|input|output|protected|public|private|outer|inner|final|partial|encapsulated|discrete|time|initial|terminal|der|pre|edge|change|sample|reinit|delay|smooth|noEvent|smooth|inStream|actualStream|hold|spatialDistribution|getInstanceName)\b'
      scope: storage.type.modelica
    - match: '(?i)\b(import|within|annotation|equation|algorithm|initial equation|initial algorithm|external|function|operator record|operator connector|operator block|operator model|operator type|operator class|operator)\b'
      scope: keyword.other.modelica
    - match: '//.*$'
      scope: comment.line.double-slash.modelica
    - match: '/\*'
      scope: punctuation.definition.comment.modelica
      push:
        - meta_scope: comment.block.modelica
        - match: '\*/'
          scope: punctuation.definition.comment.modelica
          pop: true
        - match: '[^*]'
    - match: '"'
      scope: punctuation.definition.string.begin.modelica
      push:
        - meta_scope: string.quoted.double.modelica
        - match: '\\.'
          scope: constant.character.escape.modelica
        - match: '"'
          scope: punctuation.definition.string.end.modelica
          pop: true
    - match: '\b([0-9]+\.[0-9]*([eE][+-]?[0-9]+)?|[0-9]+[eE][+-]?[0-9]+|[0-9]+\.[0-9]*|[0-9]+)\b'
      scope: constant.numeric.modelica
    - match: '\b(true|false)\b'
      scope: constant.language.boolean.modelica
    - match: '\b([a-zA-Z_][a-zA-Z0-9_]*)\s*(?=\()'
      scope: entity.name.function.modelica
    - match: '\b([a-zA-Z_][a-zA-Z0-9_]*)\b'
      scope: variable.other.modelica
    - match: '[:=<>+\-*/^.,;()\[\]{}]'
      scope: punctuation.modelica
"#;

/// Editor state for a single document.
#[derive(Debug, Clone, Default)]
pub struct EditorState {
    /// Current cursor position (byte offset from start).
    #[allow(dead_code)]
    pub cursor: usize,
    /// Selection range (start, end) in bytes; start <= end, None if no selection.
    #[allow(dead_code)]
    pub selection: Option<(usize, usize)>,
    /// Line number to scroll to / highlight (1-indexed).
    pub highlight_line: Option<usize>,
    /// Whether to show line numbers.
    pub show_line_numbers: bool,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            show_line_numbers: true,
            ..Default::default()
        }
    }
}

/// Syntax highlighter using syntect.
struct Highlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl Highlighter {
    fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        Self {
            syntax_set,
            theme_set,
        }
    }

    fn syntax_for_path(&self, path: Option<&PathBuf>) -> &syntect::parsing::SyntaxReference {
        if let Some(path) = path {
            if let Some(syntax) = self
                .syntax_set
                .find_syntax_by_extension(path.extension().and_then(|e| e.to_str()).unwrap_or(""))
            {
                return syntax;
            }
            // Fallback for Modelica files: use Ada syntax which is similar
            if let Some(syntax) = self.syntax_set.find_syntax_by_name("Ada") {
                return syntax;
            }
        }
        self.syntax_set.find_syntax_plain_text()
    }

    /// Highlight a line and return (styled segments, line text without newline).
    fn highlight_line(
        &self,
        syntax: &syntect::parsing::SyntaxReference,
        line: &str,
        theme: &syntect::highlighting::Theme,
    ) -> Vec<(Style, String)> {
        let mut highlighter = HighlightLines::new(syntax, theme);
        let regions = highlighter.highlight_line(line, &self.syntax_set).ok();
        if let Some(regions) = regions {
            regions
                .into_iter()
                .map(|(style, text)| (style, text.to_string()))
                .collect()
        } else {
            vec![(Style::default(), line.to_string())]
        }
    }
}

/// Draw the text editor with line numbers, syntax highlighting, and error navigation.
pub fn draw_text_editor(
    ui: &mut Ui,
    editor_state: &mut EditorState,
    text: &mut str,
    path: Option<&PathBuf>,
    error_lines: &[usize], // 1-indexed line numbers with errors
) -> bool {
    static HIGHLIGHTER: std::sync::OnceLock<Highlighter> = std::sync::OnceLock::new();
    let highlighter = HIGHLIGHTER.get_or_init(Highlighter::new);
    let syntax = highlighter.syntax_for_path(path);

    let theme_colors = highlighter
        .theme_set
        .themes
        .get("base16-ocean.dark")
        .or_else(|| highlighter.theme_set.themes.get("Monokai"))
        .or_else(|| highlighter.theme_set.themes.values().next())
        .expect("At least one theme");

    let font_id = FontId::monospace(13.0);
    let line_height = ui.ctx().fonts_mut(|f| f.row_height(&font_id));
    let char_width = ui.ctx().fonts_mut(|f| f.glyph_width(&font_id, ' '));

    let lines: Vec<&str> = text.lines().collect();
    let line_count = lines.len().max(1);
    let max_line_num_width = (line_count.to_string().len() as f32 + 2.0) * char_width;

    // Scroll area for the editor
    let available_height = ui.available_height();
    let _scroll_response = ScrollArea::both()
        .id_salt((
            "text_editor",
            path.map(|p| p.as_os_str()).unwrap_or_default(),
        ))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_min_height(available_height);
            ui.horizontal(|ui| {
                // Line numbers column
                if editor_state.show_line_numbers {
                    let line_num_width = max_line_num_width;
                    let (line_num_rect, _) = ui.allocate_exact_size(
                        Vec2::new(line_num_width, available_height),
                        Sense::hover(),
                    );
                    let line_num_painter = ui.painter_at(line_num_rect);
                    line_num_painter.rect_filled(line_num_rect, 0.0, crate::theme::BG);

                    for idx in 0..lines.len() {
                        let line_num = idx + 1;
                        let y = line_num_rect.top() + idx as f32 * line_height;
                        let is_error = error_lines.contains(&line_num);
                        let is_highlighted = editor_state.highlight_line == Some(line_num);

                        let color = if is_highlighted {
                            crate::theme::ACCENT
                        } else if is_error {
                            Color32::from_rgb(0xE0, 0x1B, 0x1B)
                        } else {
                            crate::theme::TEXT_WEAK
                        };

                        line_num_painter.text(
                            egui::pos2(line_num_rect.right() - 4.0, y + line_height / 2.0),
                            Align2::RIGHT_CENTER,
                            format!("{:>4}", line_num),
                            font_id.clone(),
                            color,
                        );

                        // Highlight background for error line
                        if is_error {
                            let error_rect = egui::Rect::from_min_size(
                                egui::pos2(line_num_rect.left(), y),
                                Vec2::new(line_num_rect.width(), line_height),
                            );
                            line_num_painter.rect_filled(
                                error_rect,
                                0.0,
                                Color32::from_rgba_unmultiplied(0xE0, 0x1B, 0x1B, 0x30),
                            );
                        }
                        if is_highlighted {
                            let highlight_rect = egui::Rect::from_min_size(
                                egui::pos2(line_num_rect.left(), y),
                                Vec2::new(line_num_rect.width(), line_height),
                            );
                            line_num_painter.rect_stroke(
                                highlight_rect,
                                0.0,
                                Stroke::new(1.0, crate::theme::ACCENT),
                                egui::StrokeKind::Inside,
                            );
                        }
                    }
                }

                // Editor text area
                let text_width = ui.available_width();
                let (text_rect, text_response) = ui.allocate_exact_size(
                    Vec2::new(text_width, available_height),
                    Sense::click_and_drag(),
                );
                let text_painter = ui.painter_at(text_rect);
                text_painter.rect_filled(text_rect, 0.0, Color32::WHITE);

                // Draw each line with syntax highlighting
                let mut cursor_y = text_rect.top();
                #[allow(clippy::needless_range_loop)]
                for idx in 0..lines.len() {
                    let line_num = idx + 1;
                    let is_error = error_lines.contains(&line_num);
                    let is_highlighted = editor_state.highlight_line == Some(line_num);

                    if is_error {
                        let error_rect = egui::Rect::from_min_size(
                            egui::pos2(text_rect.left(), cursor_y),
                            Vec2::new(text_rect.width(), line_height),
                        );
                        text_painter.rect_filled(
                            error_rect,
                            0.0,
                            Color32::from_rgba_unmultiplied(0xE0, 0x1B, 0x1B, 0x20),
                        );
                    }
                    if is_highlighted {
                        let highlight_rect = egui::Rect::from_min_size(
                            egui::pos2(text_rect.left(), cursor_y),
                            Vec2::new(text_rect.width(), line_height),
                        );
                        text_painter.rect_filled(
                            highlight_rect,
                            0.0,
                            Color32::from_rgba_unmultiplied(0x1F, 0x5F, 0xD6, 0x30),
                        );
                    }

                    // Syntax highlight the line
                    let highlighted = highlighter.highlight_line(syntax, lines[idx], theme_colors);
                    let mut x = text_rect.left() + 4.0;
                    for (style, segment) in highlighted {
                        let color = Color32::from_rgb(
                            style.foreground.r,
                            style.foreground.g,
                            style.foreground.b,
                        );
                        let font = if style
                            .font_style
                            .contains(syntect::highlighting::FontStyle::BOLD)
                        {
                            FontId::new(13.0, FontFamily::Monospace)
                        } else {
                            font_id.clone()
                        };
                        text_painter.text(
                            egui::pos2(x, cursor_y + line_height / 2.0),
                            Align2::LEFT_CENTER,
                            segment.clone(),
                            font,
                            color,
                        );
                        x += text_painter
                            .layout_no_wrap(segment, font_id.clone(), color)
                            .size()
                            .x;
                    }

                    cursor_y += line_height;
                }

                text_response
            });
        });

    // Handle keyboard shortcuts for error navigation
    if ui.input(|i| i.key_pressed(egui::Key::F8)) {
        // Next error
        if let Some(current) = editor_state.highlight_line {
            if let Some(next) = error_lines.iter().find(|&&l| l > current).copied() {
                editor_state.highlight_line = Some(next);
            } else if let Some(first) = error_lines.first().copied() {
                editor_state.highlight_line = Some(first);
            }
        } else if let Some(first) = error_lines.first().copied() {
            editor_state.highlight_line = Some(first);
        }
    }
    if ui.input(|i| i.key_pressed(egui::Key::F7)) {
        // Previous error
        if let Some(current) = editor_state.highlight_line {
            if let Some(prev) = error_lines.iter().rev().find(|&&l| l < current).copied() {
                editor_state.highlight_line = Some(prev);
            } else if let Some(last) = error_lines.last().copied() {
                editor_state.highlight_line = Some(last);
            }
        } else if let Some(last) = error_lines.last().copied() {
            editor_state.highlight_line = Some(last);
        }
    }

    // Ctrl+G for goto line
    if ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::G)) {
        // Could open a dialog here; for now just log
        // TODO: Add goto line dialog
    }

    // For now, we don't detect text changes here
    // The caller can compare text before/after
    false
}

/// Extract error line numbers from OMC error output.
/// Expected format: "file.mo:123:4: error: ..."
pub fn extract_error_lines(error_text: &str, current_file: Option<&PathBuf>) -> Vec<usize> {
    let mut lines = Vec::new();
    let current_file_name = current_file
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str());

    for line in error_text.lines() {
        // Match patterns like "file.mo:123:4: error:" or "file.mo:123: warning:"
        if let Some(colon1) = line.find(':') {
            let potential_file = &line[..colon1];
            let rest = &line[colon1 + 1..];

            // Check if this error is for the current file
            let is_current_file = current_file_name
                .map(|f| potential_file.ends_with(f))
                .unwrap_or(true);

            if is_current_file {
                if let Some(colon2) = rest.find(':') {
                    let line_str = &rest[..colon2];
                    if let Ok(line_num) = line_str.trim().parse::<usize>() {
                        if line_num > 0 {
                            lines.push(line_num);
                        }
                    }
                }
            }
        }
    }

    lines.sort();
    lines.dedup();
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_error_lines_basic() {
        let errors = r#"BouncingBall.mo:10:3: error: Unknown variable 'x'
BouncingBall.mo:20:1: warning: Unused variable 'y'
OtherModel.mo:5:2: error: Syntax error"#;
        let path = PathBuf::from("BouncingBall.mo");
        let result = extract_error_lines(errors, Some(&path));
        assert_eq!(result, vec![10, 20]);
    }

    #[test]
    fn extract_error_lines_no_file() {
        let errors = r#"BouncingBall.mo:10:3: error: Unknown variable 'x'
BouncingBall.mo:20:1: warning: Unused variable 'y'"#;
        let result = extract_error_lines(errors, None);
        assert_eq!(result, vec![10, 20]);
    }

    #[test]
    fn extract_error_lines_dedup() {
        let errors = r#"BouncingBall.mo:10:3: error: First
BouncingBall.mo:10:5: error: Second
BouncingBall.mo:20:1: error: Third"#;
        let path = PathBuf::from("BouncingBall.mo");
        let result = extract_error_lines(errors, Some(&path));
        assert_eq!(result, vec![10, 20]);
    }
}

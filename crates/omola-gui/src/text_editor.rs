//! Modelica text editor with syntax highlighting, line numbers and error navigation (G11, G23).
//!
//! The widget is an [`egui::TextEdit`] so that the cursor, selection, copy & paste, IME and
//! undo/redo all come from egui. Syntax colouring is supplied through the `layouter` hook:
//! a [`LayoutJob`] is rebuilt with `syntect` only when the text changes, and the result is
//! cached in [`EditorState`]. Line numbers, the error marker and the F7/F8 jumps are drawn
//! on top of the text.

use egui::text::{CCursor, CCursorRange, LayoutJob, TextFormat};
use egui::{Align, Align2, Color32, FontId, Rect, ScrollArea, Sense, Stroke, Ui, Vec2};
use std::path::PathBuf;
use std::sync::OnceLock;
use syntect::{
    easy::HighlightLines,
    highlighting::{FontStyle, Style, Theme, ThemeSet},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

/// Modelica syntax definition embedded as a `.sublime-syntax` compatible YAML.
///
/// `syntect`'s bundled set has neither Modelica nor Ada, so without this every `.mo` file
/// would be highlighted as plain text.
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

/// Font used by the editor.
fn editor_font() -> FontId {
    FontId::monospace(13.0)
}

/// Editor state for a single document.
///
/// Cursor, selection and undo history are owned by the `TextEdit` widget; only the syntax
/// cache and the navigation state live here.
#[derive(Debug, Clone, Default)]
pub struct EditorState {
    /// Line number to scroll to / highlight (1-indexed).
    pub highlight_line: Option<usize>,
    /// Whether to show line numbers.
    pub show_line_numbers: bool,
    /// Text the cached layout job was built from.
    cache_text: String,
    /// File the cached layout job was built for (the syntax depends on it).
    cache_path: Option<PathBuf>,
    /// Cached syntax-highlighted layout of [`Self::cache_text`].
    cache_job: Option<LayoutJob>,
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
    theme: Theme,
}

impl Highlighter {
    fn new() -> Self {
        let mut builder = SyntaxSet::load_defaults_newlines().into_builder();
        // A broken definition must not stop the editor; it would just fall back to plain text.
        if let Ok(modelica) =
            syntect::parsing::SyntaxDefinition::load_from_str(MODELICA_SYNTAX, true, None)
        {
            builder.add(modelica);
        }
        let syntax_set = builder.build();
        let theme_set = ThemeSet::load_defaults();
        // The workbench is light, so the dark default themes would be unreadable.
        let theme = ["InspiredGitHub", "base16-ocean.light", "Solarized (light)"]
            .iter()
            .find_map(|name| theme_set.themes.get(*name))
            .or_else(|| theme_set.themes.values().next())
            .cloned()
            .expect("At least one theme");
        Self { syntax_set, theme }
    }

    /// Syntax for a document: by extension, else Modelica (this editor only edits Modelica).
    fn syntax_for_path(&self, path: Option<&PathBuf>) -> &syntect::parsing::SyntaxReference {
        let extension = path
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
            .unwrap_or("mo");
        self.syntax_set
            .find_syntax_by_extension(extension)
            .or_else(|| self.syntax_set.find_syntax_by_extension("mo"))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text())
    }
}

fn highlighter() -> &'static Highlighter {
    static HIGHLIGHTER: OnceLock<Highlighter> = OnceLock::new();
    HIGHLIGHTER.get_or_init(Highlighter::new)
}

fn text_format(style: Style, font: &FontId) -> TextFormat {
    TextFormat {
        font_id: font.clone(),
        color: Color32::from_rgb(style.foreground.r, style.foreground.g, style.foreground.b),
        italics: style.font_style.contains(FontStyle::ITALIC),
        underline: Stroke::NONE,
        ..Default::default()
    }
}

/// Build the syntax-highlighted layout of `text`.
///
/// A single [`HighlightLines`] walks every line in order: creating one per line would lose the
/// parser state of multi-line comments and strings. The resulting `job.text` is always exactly
/// `text`, which the layout engine relies on.
pub fn highlight_job(text: &str, font: &FontId, path: Option<&PathBuf>) -> LayoutJob {
    let highlighter = highlighter();
    let syntax = highlighter.syntax_for_path(path);
    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    let mut lines = HighlightLines::new(syntax, &highlighter.theme);
    for line in LinesWithEndings::from(text) {
        match lines.highlight_line(line, &highlighter.syntax_set) {
            Ok(regions) => {
                for (style, segment) in regions {
                    job.append(segment, 0.0, text_format(style, font));
                }
            }
            Err(_) => job.append(
                line,
                0.0,
                TextFormat {
                    font_id: font.clone(),
                    color: crate::theme::TEXT,
                    ..Default::default()
                },
            ),
        }
    }
    job
}

/// Character index of the first character of `line` (1-indexed).
///
/// The index counts characters, not bytes, so that a Japanese comment before the cursor does
/// not shift the caret into the middle of a glyph.
pub fn line_start_char(text: &str, line: usize) -> usize {
    if line <= 1 {
        return 0;
    }
    let mut current = 1;
    for (index, c) in text.chars().enumerate() {
        if current == line {
            return index;
        }
        if c == '\n' {
            current += 1;
        }
    }
    text.chars().count()
}

/// Move the F7/F8 cursor over the error lines and return the line to reveal, if any.
fn navigate_errors(ui: &Ui, state: &mut EditorState, error_lines: &[usize]) -> Option<usize> {
    if error_lines.is_empty() {
        return None;
    }
    let next = ui.input(|i| i.key_pressed(egui::Key::F8));
    let previous = ui.input(|i| i.key_pressed(egui::Key::F7));
    if !next && !previous {
        return None;
    }
    let current = state.highlight_line;
    let target = if next {
        current
            .and_then(|c| error_lines.iter().find(|&&l| l > c).copied())
            .or_else(|| error_lines.first().copied())
    } else {
        current
            .and_then(|c| error_lines.iter().rev().find(|&&l| l < c).copied())
            .or_else(|| error_lines.last().copied())
    };
    state.highlight_line = target;
    target
}

/// Draw the text editor with line numbers, syntax highlighting and error navigation.
///
/// Returns `true` when the user changed `text`.
pub fn draw_text_editor(
    ui: &mut Ui,
    editor_state: &mut EditorState,
    text: &mut String,
    path: Option<&PathBuf>,
    error_lines: &[usize], // 1-indexed line numbers with errors
) -> bool {
    let font = editor_font();
    let row_height = ui.ctx().fonts_mut(|f| f.row_height(&font));
    let char_width = ui.ctx().fonts_mut(|f| f.glyph_width(&font, '0'));
    let line_count = text.lines().count().max(1);
    let show_numbers = editor_state.show_line_numbers;
    let gutter_width = if show_numbers {
        (line_count.to_string().len() as f32 + 2.0) * char_width
    } else {
        0.0
    };
    let jump = navigate_errors(ui, editor_state, error_lines);

    let mut changed = false;
    ScrollArea::both()
        .id_salt((
            "text_editor",
            path.map(|p| p.as_os_str()).unwrap_or_default(),
        ))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                let (gutter, _) = ui.allocate_exact_size(
                    Vec2::new(gutter_width, line_count as f32 * row_height),
                    Sense::hover(),
                );

                let output = {
                    // The layouter borrows the cache; keep it in its own scope so that
                    // `editor_state` is usable again once the widget has been drawn.
                    let state = &mut *editor_state;
                    let mut layouter = |ui: &Ui, buf: &dyn egui::TextBuffer, _wrap_width: f32| {
                        let current = buf.as_str();
                        let stale = state.cache_job.is_none()
                            || state.cache_text != current
                            || state.cache_path.as_ref() != path;
                        if stale {
                            state.cache_text = current.to_owned();
                            state.cache_path = path.cloned();
                            state.cache_job = Some(highlight_job(current, &font, path));
                        }
                        let mut job = state.cache_job.clone().unwrap_or_default();
                        job.wrap.max_width = f32::INFINITY;
                        ui.ctx().fonts_mut(|f| f.layout_job(job))
                    };
                    egui::TextEdit::multiline(text)
                        .code_editor()
                        .font(font.clone())
                        .desired_width(f32::INFINITY)
                        .desired_rows(line_count)
                        .frame(egui::Frame::NONE)
                        .margin(egui::Margin::ZERO)
                        .layouter(&mut layouter)
                        .show(ui)
                };
                changed = output.response.changed();
                let text_rect = output.response.rect;

                // Only the rows inside the viewport are painted.
                let clip = ui.clip_rect();
                let first_row =
                    ((clip.top() - gutter.top()) / row_height).floor().max(0.0) as usize;
                let last_row =
                    (((clip.bottom() - gutter.top()) / row_height).ceil() as usize).min(line_count);
                let painter = ui.painter_at(gutter);
                if show_numbers {
                    painter.rect_filled(gutter, 0.0, crate::theme::BG);
                }
                for row in first_row..last_row {
                    let line = row + 1;
                    let y = gutter.top() + row as f32 * row_height;
                    let is_error = error_lines.contains(&line);
                    let is_highlighted = editor_state.highlight_line == Some(line);
                    let color = if is_highlighted {
                        crate::theme::ACCENT
                    } else if is_error {
                        Color32::from_rgb(0xE0, 0x1B, 0x1B)
                    } else {
                        crate::theme::TEXT_WEAK
                    };
                    let row_rect = Rect::from_min_size(
                        egui::pos2(gutter.left(), y),
                        Vec2::new(gutter.width(), row_height),
                    );
                    if is_error || is_highlighted {
                        let fill = if is_error {
                            Color32::from_rgba_unmultiplied(0xE0, 0x1B, 0x1B, 0x30)
                        } else {
                            Color32::from_rgba_unmultiplied(0x1F, 0x5F, 0xD6, 0x30)
                        };
                        painter.rect_filled(row_rect, 0.0, fill);
                    }
                    if show_numbers {
                        painter.text(
                            egui::pos2(gutter.right() - 4.0, y + row_height / 2.0),
                            Align2::RIGHT_CENTER,
                            line.to_string(),
                            font.clone(),
                            color,
                        );
                    }
                    if is_highlighted && show_numbers {
                        painter.rect_stroke(
                            row_rect,
                            0.0,
                            Stroke::new(1.0, crate::theme::ACCENT),
                            egui::StrokeKind::Inside,
                        );
                    }
                }

                // F7/F8: reveal the line and put the caret at its first character.
                if let Some(line) = jump {
                    let y = text_rect.top() + (line - 1) as f32 * row_height;
                    let target = Rect::from_min_size(
                        egui::pos2(text_rect.left(), y),
                        Vec2::new(text_rect.width().max(1.0), row_height),
                    );
                    ui.scroll_to_rect(target, Some(Align::Center));
                    let id = output.response.id;
                    if let Some(mut edit_state) = egui::text_edit::TextEditState::load(ui.ctx(), id)
                    {
                        edit_state
                            .cursor
                            .set_char_range(Some(CCursorRange::one(CCursor::new(
                                line_start_char(text, line),
                            ))));
                        edit_state.store(ui.ctx(), id);
                    }
                }
            });
        });

    changed
}

/// Extract error line numbers from OMC error output.
/// Expected format: `file.mo:123:4: error: ...`
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

    fn format_at(job: &LayoutJob, byte: usize) -> &TextFormat {
        job.sections
            .iter()
            .find(|s| s.byte_range.start.0 <= byte && byte < s.byte_range.end.0)
            .map(|s| &s.format)
            .expect("byte index inside the job")
    }

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

    #[test]
    fn highlight_job_keeps_every_character() {
        let text = "model A\n  // comment\n  Real x = 1.0;\nend A;\n";
        let job = highlight_job(text, &editor_font(), None);
        assert_eq!(job.text, text);
    }

    #[test]
    fn highlight_job_colours_block_comment_continuation() {
        let text = "/* a\n b */ x";
        let job = highlight_job(text, &editor_font(), None);
        assert_eq!(job.text, text);
        let opener = format_at(&job, 0).color;
        // " b */" starts on the second line, inside the multi-line comment.
        let continuation = format_at(&job, text.find(" b").unwrap()).color;
        assert_eq!(opener, continuation);
        // ... and the text after the comment is no longer comment-coloured.
        let after = format_at(&job, text.find(" x").unwrap() + 1).color;
        assert_ne!(opener, after);
    }

    #[test]
    fn line_start_char_counts_characters() {
        let text = "model A // 日本語\n  Real x;\nend A;\n";
        assert_eq!(line_start_char(text, 1), 0);
        assert_eq!(
            line_start_char(text, 2),
            text.chars().position(|c| c == '\n').unwrap() + 1
        );
        assert_eq!(
            line_start_char(text, 3),
            text.chars().position(|c| c == '\n').unwrap() + 1 + "  Real x;\n".chars().count()
        );
        // Past the end: the last character index, never a panic.
        assert_eq!(line_start_char(text, 99), text.chars().count());
    }

    #[test]
    fn line_start_char_is_not_a_byte_offset() {
        let text = "// 日本語\nx";
        let byte_line_two = text.find('\n').unwrap() + 1;
        let char_line_two = line_start_char(text, 2);
        assert_eq!(char_line_two, "// 日本語\n".chars().count());
        assert_ne!(char_line_two, byte_line_two);
    }

    /// One headless frame: lay the editor out inside a fixed-size screen.
    fn frame(
        ctx: &egui::Context,
        events: Vec<egui::Event>,
        state: &mut EditorState,
        text: &mut String,
    ) -> bool {
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(800.0, 600.0),
            )),
            ..Default::default()
        };
        input.events = events;
        let mut changed = false;
        let mut output = ctx.run_ui(input, |ui| {
            changed = draw_text_editor(ui, state, text, None, &[]);
        });
        // No renderer is attached in a test, so the font atlas deltas must be discarded.
        output.textures_delta.clear();
        changed
    }

    fn click(pos: egui::Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ]
    }

    /// G23 regression: the Text layer must accept typed characters and report the change.
    #[test]
    fn typing_into_the_editor_changes_the_text() {
        let ctx = egui::Context::default();
        let mut state = EditorState::new();
        let mut text = String::from("model A\nend A;\n");
        let before = text.clone();

        // The first frame lays the widget out; only then can a click hit it and give it focus.
        frame(&ctx, vec![], &mut state, &mut text);
        frame(&ctx, click(egui::pos2(120.0, 20.0)), &mut state, &mut text);
        // The next frame types a character at the caret.
        let changed = frame(
            &ctx,
            vec![egui::Event::Text("x".into())],
            &mut state,
            &mut text,
        );

        assert!(changed, "typing must report a change");
        assert_ne!(text, before);
        assert_eq!(text.len(), before.len() + 1);
        assert!(text.contains('x'));
    }

    /// The caret of a `TextEdit` keeps focus between frames, so a second character also lands.
    #[test]
    fn two_typed_characters_are_both_kept() {
        let ctx = egui::Context::default();
        let mut state = EditorState::new();
        let mut text = String::from("model A\nend A;\n");
        frame(&ctx, vec![], &mut state, &mut text);
        frame(&ctx, click(egui::pos2(120.0, 20.0)), &mut state, &mut text);
        frame(
            &ctx,
            vec![egui::Event::Text("a".into())],
            &mut state,
            &mut text,
        );
        frame(
            &ctx,
            vec![egui::Event::Text("b".into())],
            &mut state,
            &mut text,
        );
        assert!(text.contains("ab"), "text was {text:?}");
    }
}

//! Modelica text editor with syntax highlighting, line numbers and error navigation (G11, G23).
//!
//! The widget is an [`egui::TextEdit`] so that the cursor, selection, copy & paste, IME and
//! undo/redo all come from egui. Syntax colouring is supplied through the `layouter` hook:
//! a [`LayoutJob`] is rebuilt from [`crate::highlight::tokenize`] only when the text changes,
//! and the result is cached in [`EditorState`]. Line numbers, the error marker and the
//! F7/F8 jumps are drawn on top of the text.

use egui::text::{CCursor, CCursorRange, LayoutJob, TextFormat};
use egui::{Align, Align2, Color32, FontId, Rect, ScrollArea, Sense, Stroke, Ui, Vec2};
use std::path::{Path, PathBuf};

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
    /// Line that was already scrolled to and given the caret.
    ///
    /// `highlight_line` can be set from outside (a click in the log, G25); the line is
    /// revealed once, not on every frame.
    revealed_line: Option<usize>,
    /// Whether to show line numbers.
    pub show_line_numbers: bool,
    /// Text the cached layout job was built from.
    cache_text: String,
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

/// Build the syntax-highlighted layout of `text`.
///
/// The tokenizer covers every byte exactly once, so the resulting `job.text` is always
/// exactly `text`, which the layout engine relies on.
pub fn highlight_job(text: &str, font: &FontId) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    for (range, kind) in crate::highlight::tokenize(text) {
        job.append(
            &text[range],
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: crate::theme::syntax_color(kind),
                ..Default::default()
            },
        );
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
    // F7/F8 move the highlight themselves; a line set from outside (a click in the log,
    // G25) is revealed once, when it changes.
    let jump = match navigate_errors(ui, editor_state, error_lines) {
        Some(line) => Some(line),
        None if editor_state.revealed_line != editor_state.highlight_line => {
            editor_state.highlight_line
        }
        None => None,
    };
    editor_state.revealed_line = jump.or(editor_state.revealed_line);

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
                        let stale = state.cache_job.is_none() || state.cache_text != current;
                        if stale {
                            state.cache_text = current.to_owned();
                            state.cache_job = Some(highlight_job(current, &font));
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

/// A source position reported by OMC, e.g. `[C:/u/A.mo:7:1-7:5:writable]`.
///
/// OMC 1.27 writes the position as a bracketed prefix on the message line. The path may
/// contain `:` (a Windows drive) and the range may span lines (`4:7-5:0`), so the text has
/// to be split from the right.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    pub file: String,
    pub line: usize,
    pub column: usize,
}

/// Find every `[<file>:<line>:<col>-<line>:<col>:<writable|readonly>]` in `text`.
///
/// Malformed brackets (`[`, a missing `]`, a non-numeric line) are skipped; this runs on
/// arbitrary log output and must never panic.
pub fn parse_omc_locations(text: &str) -> Vec<SourceLocation> {
    let mut locations = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        let Some(close) = after.find(']') else {
            break;
        };
        if let Some(location) = parse_bracketed_location(&after[..close]) {
            locations.push(location);
        }
        rest = &after[close + 1..];
    }
    locations
}

/// Parse the inside of one `[...]` group, without the brackets.
fn parse_bracketed_location(inner: &str) -> Option<SourceLocation> {
    // `:writable` / `:readonly` is what tells a location apart from any other `[...]`.
    let inner = inner
        .strip_suffix(":writable")
        .or_else(|| inner.strip_suffix(":readonly"))?;
    // The range is `<file>:<line>:<col>-<end line>:<end col>`. Only the start matters here,
    // and the path may itself contain `-`, so split on the last one.
    let (start, _end) = inner.rsplit_once('-')?;
    let mut parts = start.rsplitn(3, ':');
    let column: usize = parts.next()?.trim().parse().ok()?;
    let line: usize = parts.next()?.trim().parse().ok()?;
    let file = parts.next()?;
    if file.is_empty() || line == 0 || column == 0 {
        return None;
    }
    Some(SourceLocation {
        file: file.to_owned(),
        line,
        column,
    })
}

/// Line numbers of `locs` that belong to the document at `path`, sorted and without
/// duplicates.
///
/// A document that was never saved has no path; OMC then reports its text as
/// `<interactive>`, so those locations are the ones to mark.
pub fn lines_for_file(
    locs: &[SourceLocation],
    path: Option<&Path>,
    class: Option<&str>,
) -> Vec<usize> {
    let mut lines: Vec<usize> = locs
        .iter()
        .filter(|loc| location_is_for(&loc.file, path, class))
        .map(|loc| loc.line)
        .collect();
    lines.sort_unstable();
    lines.dedup();
    lines
}

/// Whether a location's `file` refers to the document at `path` (defined by `class`).
///
/// OMC echoes the path it was given, which may use either slash, another directory, and
/// on Windows lower case (`G25Test.mo` comes back as `g25test.mo`), so only the file name
/// is compared, case-insensitively on Windows.
///
/// A user model is not handed to OMC as the file the user opened: `backend::write_source`
/// writes the buffer to `<work root>/session-<pid>/run-<n>/<class>.mo` and OMC reports
/// errors against *that* copy, so the class name is matched as well.
pub fn location_is_for(file: &str, path: Option<&Path>, class: Option<&str>) -> bool {
    let name = path.and_then(|p| p.file_name()).and_then(|n| n.to_str());
    match name {
        Some(name) => {
            let same_name = match file_name_of(file) {
                Some(echoed) if cfg!(windows) => echoed.eq_ignore_ascii_case(name),
                Some(echoed) => echoed == name,
                None => false,
            };
            same_name || class.is_some_and(|class| is_scratch_copy_of(file, class))
        }
        None => file == "<interactive>" || !file.ends_with(".mo"),
    }
}

/// Whether `file` is the private copy of `class` written for OMC.
///
/// `backend::write_source` names the copy `result_stem(class)` = `class` with `.` → `_`,
/// inside `.../omola-gui/session-<pid>/run-<n>/`.
fn is_scratch_copy_of(file: &str, class: &str) -> bool {
    let normalized = file.replace('\\', "/");
    if !normalized.contains("/omola-gui/session-") || !normalized.contains("/run-") {
        return false;
    }
    let stem = Path::new(file).file_stem().and_then(|s| s.to_str());
    stem.is_some_and(|stem| stem == class.replace('.', "_"))
}

fn file_name_of(file: &str) -> Option<&str> {
    Path::new(file).file_name().and_then(|n| n.to_str())
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
    fn parse_omc_locations_reads_a_unix_path() {
        let text = "[/home/u/A.mo:12:3-12:20:writable] Error: Unknown variable x";
        assert_eq!(
            parse_omc_locations(text),
            vec![SourceLocation {
                file: "/home/u/A.mo".into(),
                line: 12,
                column: 3,
            }]
        );
    }

    #[test]
    fn parse_omc_locations_keeps_the_drive_letter() {
        // The drive letter's `:` must not be taken for the line separator.
        let text = r"[C:\Users\u\A.mo:7:1-7:5:writable] Error: something";
        let locations = parse_omc_locations(text);
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].file, r"C:\Users\u\A.mo");
        assert_eq!(locations[0].line, 7);
        assert_eq!(locations[0].column, 1);
    }

    #[test]
    fn parse_omc_locations_reads_what_omc_1_27_writes() {
        // Captured from `omc` 1.27.1 on Windows: forward slashes, and a range that
        // crosses a line boundary.
        let text = "[C:/Users/u/AppData/Local/Temp/omcprobe/Bad.mo:4:7-5:0:writable] Error: No viable alternative near token: ;";
        let locations = parse_omc_locations(text);
        assert_eq!(locations.len(), 1);
        assert_eq!(
            locations[0].file,
            "C:/Users/u/AppData/Local/Temp/omcprobe/Bad.mo"
        );
        assert_eq!(locations[0].line, 4);
        assert_eq!(locations[0].column, 7);
    }

    #[test]
    fn parse_omc_locations_finds_every_entry_and_skips_junk() {
        let text = "\
[<interactive>:4:7-5:0:writable] Error: first
no location here [
[not a location] Error: second
[A.mo:3:1-3:4:writable] Error: third
[A.mo:9:1-9:2:readonly] Error: fourth
[broken: x:1-2:writable] Error: fifth
[A.mo:1:1-1:2] Error: no suffix
";
        let lines: Vec<(String, usize, usize)> = parse_omc_locations(text)
            .into_iter()
            .map(|l| (l.file, l.line, l.column))
            .collect();
        assert_eq!(
            lines,
            vec![
                ("<interactive>".to_owned(), 4, 7),
                ("A.mo".to_owned(), 3, 1),
                ("A.mo".to_owned(), 9, 1),
            ]
        );
    }

    #[test]
    fn lines_for_file_matches_by_file_name() {
        let locations = parse_omc_locations(
            "[C:/work/BouncingBall.mo:12:3-12:4:writable] Error: a
[/home/u/Other.mo:3:1-3:2:writable] Error: b
[C:/work/BouncingBall.mo:12:9-12:10:writable] Error: c
[C:/work/BouncingBall.mo:7:1-7:2:writable] Error: d",
        );
        let path = PathBuf::from("/somewhere/else/BouncingBall.mo");
        assert_eq!(lines_for_file(&locations, Some(&path), None), vec![7, 12]);
        let other = PathBuf::from("/home/u/Other.mo");
        assert_eq!(lines_for_file(&locations, Some(&other), None), vec![3]);
        let missing = PathBuf::from("/tmp/Missing.mo");
        assert!(lines_for_file(&locations, Some(&missing), None).is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn location_is_for_ignores_the_case_on_windows() {
        // OMC 1.27 lower-cases the path it echoes: `G25Test.mo` came back as
        // `c:/users/.../g25test.mo` in a real run.
        let path = PathBuf::from("C:/Users/u/AppData/Local/Temp/G25Test.mo");
        assert!(location_is_for(
            "c:/users/u/appdata/local/temp/g25test.mo",
            Some(&path),
            None
        ));
        assert!(!location_is_for(
            "c:/users/u/appdata/local/temp/other.mo",
            Some(&path),
            None
        ));
    }

    #[test]
    fn a_scratch_copy_of_the_class_counts_as_the_document() {
        // A real Check with `G25Test.mo` open reported
        // `.../omola-gui/session-14972/run-4/BouncingBall.mo:3:39-3:42:writable`:
        // OMC sees the private copy, not the file the user opened.
        let locations = parse_omc_locations(
            "[C:/Users/u/AppData/Local/Temp/omola-gui/session-14972/run-4/BouncingBall.mo:3:39-3:42:writable] Error: x",
        );
        let path = PathBuf::from("C:/work/G25Test.mo");
        assert!(lines_for_file(&locations, Some(&path), None).is_empty());
        assert_eq!(
            lines_for_file(&locations, Some(&path), Some("BouncingBall")),
            vec![3]
        );
        assert!(lines_for_file(&locations, Some(&path), Some("Other")).is_empty());
        // A file that merely has the same name outside the scratch directory is not the copy.
        assert!(!location_is_for(
            "C:/work/BouncingBall.mo",
            Some(&path),
            Some("BouncingBall")
        ));
    }

    #[test]
    fn lines_for_file_without_a_path_uses_the_interactive_entries() {
        let locations = parse_omc_locations(
            "[<interactive>:4:7-5:0:writable] Error: a
[<interactive>:9:1-9:2:writable] Error: b
[C:/work/BouncingBall.mo:12:3-12:4:writable] Error: c",
        );
        // An unsaved model is reported as `<interactive>`; a real `.mo` path is not.
        assert_eq!(lines_for_file(&locations, None, None), vec![4, 9]);
    }

    #[test]
    fn highlight_job_keeps_every_character() {
        let text = "model A\n  // comment\n  Real x = 1.0;\nend A;\n";
        let job = highlight_job(text, &editor_font());
        assert_eq!(job.text, text);
    }

    #[test]
    fn highlight_job_colours_block_comment_continuation() {
        let text = "/* a\n b */ x";
        let job = highlight_job(text, &editor_font());
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

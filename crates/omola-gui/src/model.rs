//! Editor-side model state: the open Modelica document and a lightweight class outline.
//!
//! The outline scanner is a *display helper* for the package browser only. It is not a
//! Modelica parser; checking, translation and simulation are always delegated to OMC.

use crate::text_editor::EditorState;
use std::path::PathBuf;

/// Kinds of Modelica class definitions shown in the package browser.
pub const CLASS_KEYWORDS: &[&str] = &[
    "package",
    "model",
    "block",
    "connector",
    "record",
    "function",
    "type",
    "class",
    "operator",
];

/// One open `.mo` document in the edit window.
#[derive(Debug, Clone)]
pub struct Document {
    /// File on disk; `None` for an unsaved new model.
    pub path: Option<PathBuf>,
    pub text: String,
    pub dirty: bool,
    /// Editor state (cursor position, selection, syntax highlighting, etc.).
    pub editor: EditorState,
    /// The text before each model edit, oldest first (G34).
    pub undo: Vec<String>,
    /// The text each Undo removed, oldest first (G34).
    pub redo: Vec<String>,
}

/// How many text snapshots a document keeps for Undo.
pub const MAX_UNDO: usize = 100;

impl Document {
    pub fn new_untitled(text: impl Into<String>) -> Self {
        Self {
            path: None,
            text: text.into(),
            dirty: false,
            editor: EditorState::new(),
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn open(path: PathBuf) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(&path)?;
        Ok(Self {
            path: Some(path),
            text,
            dirty: false,
            editor: EditorState::new(),
            undo: Vec::new(),
            redo: Vec::new(),
        })
    }

    /// Remember the current text as the state Undo returns to, and drop the redo stack.
    ///
    /// Called right before a model edit replaces the text, so an Undo restores exactly what
    /// was on screen — including anything typed by hand since the last edit.
    pub fn push_undo(&mut self) {
        if self.undo.last() != Some(&self.text) {
            self.undo.push(self.text.clone());
            let excess = self.undo.len().saturating_sub(MAX_UNDO);
            self.undo.drain(..excess);
        }
        self.redo.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Move one snapshot from one stack to the other; `false` when there is nothing to move.
    pub fn step_history(&mut self, back: bool) -> bool {
        let (from, to) = if back {
            (&mut self.undo, &mut self.redo)
        } else {
            (&mut self.redo, &mut self.undo)
        };
        let Some(restored) = from.pop() else {
            return false;
        };
        let current = std::mem::replace(&mut self.text, restored);
        to.push(current);
        true
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        if let Some(path) = &self.path {
            std::fs::write(path, &self.text)?;
            self.dirty = false;
        }
        Ok(())
    }

    /// Title shown in the edit-window tab, e.g. `BouncingBall.mo*`.
    pub fn title(&self) -> String {
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Unnamed.mo".into());
        if self.dirty {
            format!("{name}*")
        } else {
            name
        }
    }
}

/// A class found by [`scan_classes`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassEntry {
    pub kind: String,
    /// Fully qualified name inside the file, e.g. `Pkg.Sub.Model`.
    pub qualified_name: String,
    pub depth: usize,
    /// Optional description string directly after the name.
    pub description: Option<String>,
    /// Byte range of the whole definition: from a leading `partial`/`encapsulated` prefix to
    /// the `;` of `end <Name>;` (inclusive), or to the `;` of a short definition
    /// (`type P = Real;`). Used to replace exactly one class in the buffer (G28).
    pub byte_range: std::ops::Range<usize>,
}

/// Prefixes that `list()` keeps in front of the class keyword, so a replaced range must
/// include them (`partial model X`).
const CLASS_PREFIXES: &[&str] = &["partial", "encapsulated", "expandable", "pure", "impure"];

/// Find class definitions (`model X`, `package P` ... `end P;`) with their nesting.
///
/// Comments and string literals are skipped. `end X;` pops the nesting stack.
pub fn scan_classes(text: &str) -> Vec<ClassEntry> {
    let tokens = tokenize(text);
    // A definition that has not reached its `end <Name>;` yet.
    struct Open {
        name: String,
        /// Index of its entry in `out`, whose `byte_range.end` is filled in on close.
        entry: usize,
    }
    let mut stack: Vec<Open> = Vec::new();
    let mut out: Vec<ClassEntry> = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i].text.as_str();
        // Skip prefixes like `partial`, `encapsulated`, `expandable`, `pure`, `impure`.
        if token == "end" {
            if let Some(next) = tokens.get(i + 1) {
                if let Some(closed) = stack.pop_if(|top| top.name == next.text) {
                    let semicolon = tokens.get(i + 2).filter(|t| t.text == ";");
                    out[closed.entry].byte_range.end = semicolon.map_or(next.end, |t| t.end);
                    i += if semicolon.is_some() { 3 } else { 2 };
                    continue;
                }
            }
            i += 1;
            continue;
        }
        if CLASS_KEYWORDS.contains(&token) {
            // `operator record X`, `operator function X`: take the last keyword.
            let mut kind = token.to_owned();
            let mut j = i + 1;
            while let Some(next) = tokens.get(j) {
                if CLASS_KEYWORDS.contains(&next.text.as_str()) {
                    kind = next.text.clone();
                    j += 1;
                } else {
                    break;
                }
            }
            let Some(name) = tokens.get(j) else { break };
            if !is_identifier(&name.text) {
                i = j;
                continue;
            }
            // `type X = Real(...)` and `model X = Y(...)` are short class definitions: no `end`.
            let short = tokens.get(j + 1).is_some_and(|t| t.text == "=");
            let description = tokens
                .get(j + 1)
                .and_then(|t| t.text.strip_prefix('"'))
                .map(|t| t.trim_end_matches('"').to_owned());
            // `partial model X`: `list()` keeps the prefix, so the range must start there.
            let mut start = tokens[i].start;
            let mut prefix = i;
            while prefix > 0 && CLASS_PREFIXES.contains(&tokens[prefix - 1].text.as_str()) {
                prefix -= 1;
                start = tokens[prefix].start;
            }
            let mut qualified: Vec<String> = stack.iter().map(|o| o.name.clone()).collect();
            qualified.push(name.text.clone());
            let end = if short {
                // A short definition has no `end`: it ends at the `;` that closes it.
                tokens[j + 1..]
                    .iter()
                    .find(|t| t.text == ";")
                    .map_or(text.len(), |t| t.end)
            } else {
                // Filled in when `end <Name>;` is seen; an unterminated class takes the rest.
                start
            };
            let entry = out.len();
            out.push(ClassEntry {
                kind,
                qualified_name: qualified.join("."),
                depth: stack.len(),
                description,
                byte_range: start..end,
            });
            if !short {
                stack.push(Open {
                    name: name.text.clone(),
                    entry,
                });
            }
            i = j + 1;
            continue;
        }
        i += 1;
    }
    for open in stack {
        out[open.entry].byte_range.end = text.len();
    }
    out
}

/// Replace the definition of `class` in `text` with `new_text`, re-applying the indentation of
/// a nested class.
///
/// Everything outside the class — the `within` clause, leading comments, other classes — is
/// left byte for byte unchanged. `new_text` is what OMC's `list()` returned, which is not
/// indented at all.
pub fn replace_class_text(text: &mut String, class: &str, new_text: &str) -> Result<(), String> {
    let entry = scan_classes(text)
        .into_iter()
        .find(|e| e.qualified_name == class && !e.byte_range.is_empty())
        .ok_or_else(|| format!("{class} is not defined in this document"))?;
    let indent = line_indent(text, entry.byte_range.start);
    let replacement = reindent(new_text, &indent);
    text.replace_range(entry.byte_range, &replacement);
    Ok(())
}

/// Leading whitespace of the line that contains byte `at`.
fn line_indent(text: &str, at: usize) -> String {
    let line_start = text[..at].rfind('\n').map_or(0, |i| i + 1);
    text[line_start..at]
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect()
}

/// Indent every line of `new_text` except the first, which already follows `indent`.
fn reindent(new_text: &str, indent: &str) -> String {
    if indent.is_empty() {
        return new_text.to_owned();
    }
    let mut out = String::with_capacity(new_text.len() + new_text.len() / 8);
    for (i, line) in new_text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
            if !line.trim().is_empty() {
                out.push_str(indent);
            }
        }
        out.push_str(line);
    }
    out
}

/// The class to check/simulate by default: the first non-package class, else the first class.
pub fn default_class(entries: &[ClassEntry]) -> Option<String> {
    entries
        .iter()
        .find(|e| e.kind != "package" && e.kind != "type" && e.kind != "function")
        .or_else(|| entries.first())
        .map(|e| e.qualified_name.clone())
}

fn is_identifier(token: &str) -> bool {
    let mut chars = token.chars();
    matches!(chars.next(), Some(c) if c == '_' || c.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// One lexeme and where it sits in the source text.
struct Token {
    text: String,
    start: usize,
    end: usize,
}

impl Token {
    fn new(text: &str, range: std::ops::Range<usize>) -> Self {
        Self {
            text: text[range.clone()].to_owned(),
            start: range.start,
            end: range.end,
        }
    }
}

/// Split into identifiers, string literals (kept with quotes) and single punctuation chars,
/// each with its byte range so [`scan_classes`] can report where a class starts and ends.
fn tokenize(text: &str) -> Vec<Token> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_whitespace() {
            i += 1;
        } else if c == b'/' && bytes.get(i + 1) == Some(&b'/') {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if c == b'/' && bytes.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
        } else if c == b'"' {
            let start = i;
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                if bytes[i] == b'\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(bytes.len());
            out.push(Token::new(text, start..i));
        } else if c == b'_' || c.is_ascii_alphanumeric() {
            let start = i;
            while i < bytes.len() && (bytes[i] == b'_' || bytes[i].is_ascii_alphanumeric()) {
                i += 1;
            }
            out.push(Token::new(text, start..i));
        } else {
            // One whole UTF-8 character, so every token boundary stays a char boundary.
            let len = text[i..].chars().next().map_or(1, char::len_utf8);
            out.push(Token::new(text, i..i + len));
            i += len;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_nested_classes_and_skips_comments_and_strings() {
        let text = r#"
package Lib "A library"
  // model Hidden
  model Tank "A tank"
    Real x;
    String s = "model NotAClass";
  equation
    der(x) = -x;
  end Tank;
  type Pressure = Real(unit="Pa");
  /* block AlsoHidden end AlsoHidden; */
  block Gain
  end Gain;
end Lib;
model Top
end Top;
"#;
        let names: Vec<_> = scan_classes(text)
            .into_iter()
            .map(|e| (e.kind, e.qualified_name, e.depth))
            .collect();
        assert_eq!(
            names,
            vec![
                ("package".into(), "Lib".into(), 0),
                ("model".into(), "Lib.Tank".into(), 1),
                ("type".into(), "Lib.Pressure".into(), 1),
                ("block".into(), "Lib.Gain".into(), 1),
                ("model".into(), "Top".into(), 0),
            ]
        );
    }

    #[test]
    fn default_class_prefers_models_over_packages() {
        let entries = scan_classes("package P model M end M; end P;");
        assert_eq!(default_class(&entries).as_deref(), Some("P.M"));
        assert_eq!(default_class(&[]), None);
    }

    #[test]
    fn reads_description_strings() {
        let entries = scan_classes("model A \"Desc\" end A;");
        assert_eq!(entries[0].description.as_deref(), Some("Desc"));
    }

    #[test]
    fn the_byte_range_covers_the_whole_definition() {
        let text = "// head\nmodel A\n  Real x;\nend A;\n\nmodel B\nend B;\n";
        let entries = scan_classes(text);
        assert_eq!(
            &text[entries[0].byte_range.clone()],
            "model A\n  Real x;\nend A;"
        );
        assert_eq!(&text[entries[1].byte_range.clone()], "model B\nend B;");
    }

    #[test]
    fn a_short_class_definition_ends_at_its_semicolon() {
        let text = "type P = Real(unit=\"Pa\");\nmodel M\nend M;\n";
        let entries = scan_classes(text);
        assert_eq!(
            &text[entries[0].byte_range.clone()],
            "type P = Real(unit=\"Pa\");"
        );
        assert_eq!(&text[entries[1].byte_range.clone()], "model M\nend M;");
    }

    #[test]
    fn replacing_one_class_leaves_the_rest_untouched() {
        let mut text =
            "// header\nmodel A\n  Real x;\nend A;\n\nmodel B\n  Real y;\nend B;\n".to_owned();
        replace_class_text(&mut text, "A", "model A\n  Real z;\nend A;").unwrap();
        assert_eq!(
            text,
            "// header\nmodel A\n  Real z;\nend A;\n\nmodel B\n  Real y;\nend B;\n"
        );
    }

    #[test]
    fn a_nested_class_keeps_its_indentation() {
        let mut text =
            "package P\n  model M\n    Real x;\n  end M;\n  model N\n  end N;\nend P;\n".to_owned();
        replace_class_text(&mut text, "P.M", "model M\n  Real x;\n  Real y;\nend M;").unwrap();
        assert_eq!(
            text,
            "package P\n  model M\n    Real x;\n    Real y;\n  end M;\n  model N\n  end N;\nend P;\n"
        );
    }

    #[test]
    fn a_partial_model_is_replaced_with_its_prefix() {
        let mut text = "partial model A\n  Real x;\nend A;\n".to_owned();
        replace_class_text(
            &mut text,
            "A",
            "partial model A\n  Real x;\n  Real y;\nend A;",
        )
        .unwrap();
        assert_eq!(text, "partial model A\n  Real x;\n  Real y;\nend A;\n");
    }

    #[test]
    fn replacing_a_class_that_is_not_there_is_an_error() {
        let mut text = "model A\nend A;\n".to_owned();
        assert!(replace_class_text(&mut text, "B", "model B\nend B;").is_err());
        assert_eq!(text, "model A\nend A;\n");
    }
}

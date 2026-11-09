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
    /// Whole-text snapshots used by graphical editing history (oldest first).
    pub undo: Vec<String>,
    pub redo: Vec<String>,
    /// Editor state (cursor position, selection, syntax highlighting, etc.).
    pub editor: EditorState,
}

impl Document {
    pub fn new_untitled(text: impl Into<String>) -> Self {
        Self {
            path: None,
            text: text.into(),
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            editor: EditorState::new(),
        }
    }

    pub fn open(path: PathBuf) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(&path)?;
        Ok(Self {
            path: Some(path),
            text,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            editor: EditorState::new(),
        })
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
}

/// Find class definitions (`model X`, `package P` ... `end P;`) with their nesting.
///
/// Comments and string literals are skipped. `end X;` pops the nesting stack.
pub fn scan_classes(text: &str) -> Vec<ClassEntry> {
    let tokens = tokenize(text);
    let mut stack: Vec<String> = Vec::new();
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i].as_str();
        // Skip prefixes like `partial`, `encapsulated`, `expandable`, `pure`, `impure`.
        if token == "end" {
            if let Some(next) = tokens.get(i + 1) {
                if stack.last().is_some_and(|top| top == next) {
                    stack.pop();
                    i += 2;
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
                if CLASS_KEYWORDS.contains(&next.as_str()) {
                    kind = next.clone();
                    j += 1;
                } else {
                    break;
                }
            }
            let Some(name) = tokens.get(j) else { break };
            if !is_identifier(name) {
                i = j;
                continue;
            }
            // `type X = Real(...)` and `model X = Y(...)` are short class definitions: no `end`.
            let short = tokens.get(j + 1).is_some_and(|t| t == "=");
            let description = tokens
                .get(j + 1)
                .and_then(|t| t.strip_prefix('"'))
                .map(|t| t.trim_end_matches('"').to_owned());
            let mut qualified = stack.clone();
            qualified.push(name.clone());
            out.push(ClassEntry {
                kind,
                qualified_name: qualified.join("."),
                depth: stack.len(),
                description,
            });
            if !short {
                stack.push(name.clone());
            }
            i = j + 1;
            continue;
        }
        i += 1;
    }
    out
}

/// Byte range of a class definition, including its declaration and matching `end`.
pub fn class_range(text: &str, qualified: &str) -> Option<std::ops::Range<usize>> {
    let toks = tokenize_spanned(text);
    let mut stack: Vec<(String, usize, String)> = Vec::new();
    let mut found = None;
    let mut i = 0;
    while i < toks.len() {
        let t = toks[i].0.as_str();
        if t == "end" {
            if let (Some((name, start, q)), Some(end_name), Some(semi)) =
                (stack.pop(), toks.get(i + 1), toks.get(i + 2))
            {
                if name == end_name.0 && semi.0 == ";" {
                    if q == qualified {
                        found = Some(start..semi.2);
                    }
                } else {
                    stack.push((name, start, q));
                }
            }
            i += 1;
            continue;
        }
        if CLASS_KEYWORDS.contains(&t) {
            let mut j = i + 1;
            while toks
                .get(j)
                .is_some_and(|x| CLASS_KEYWORDS.contains(&x.0.as_str()))
            {
                j += 1;
            }
            if let Some((name, _, _)) = toks.get(j) {
                if is_identifier(name) {
                    let q = stack
                        .iter()
                        .map(|x| x.0.as_str())
                        .chain(std::iter::once(name.as_str()))
                        .collect::<Vec<_>>()
                        .join(".");
                    let line_start = text[..toks[i].1].rfind('\n').map_or(0, |x| x + 1);
                    let short = toks.get(j + 1).is_some_and(|x| x.0 == "=");
                    if short {
                        if q == qualified {
                            let end = toks.iter().skip(j + 1).find(|x| x.0 == ";").map(|x| x.2)?;
                            found = Some(line_start..end);
                        }
                    } else {
                        stack.push((name.clone(), line_start, q));
                    }
                }
            }
        }
        i += 1;
    }
    found
}

/// Replace exactly one definition returned by OMC `list()`, preserving its original indentation.
pub fn replace_class_text(text: &str, qualified: &str, source: &str) -> Option<String> {
    let range = class_range(text, qualified)?;
    let original = &text[range.clone()];
    let indent = original
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .take_while(|c| c.is_whitespace())
        .collect::<String>();
    let replacement = source
        .lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("{indent}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut updated = text.to_owned();
    updated.replace_range(range, &replacement);
    Some(updated)
}

fn tokenize_spanned(text: &str) -> Vec<(String, usize, usize)> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let (start, c) = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '/' && chars.get(i + 1).is_some_and(|x| x.1 == '/') {
            while i < chars.len() && chars[i].1 != '\n' {
                i += 1;
            }
        } else if c == '/' && chars.get(i + 1).is_some_and(|x| x.1 == '*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i].1 == '*' && chars[i + 1].1 == '/') {
                i += 1;
            }
            i = (i + 2).min(chars.len());
        } else if c == '"' {
            i += 1;
            while i < chars.len() && chars[i].1 != '"' {
                if chars[i].1 == '\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(chars.len());
            let end = chars.get(i).map_or(text.len(), |x| x.0);
            out.push((text[start..end].into(), start, end));
        } else if c == '_' || c.is_ascii_alphanumeric() {
            i += 1;
            while i < chars.len() && (chars[i].1 == '_' || chars[i].1.is_ascii_alphanumeric()) {
                i += 1;
            }
            let end = chars.get(i).map_or(text.len(), |x| x.0);
            out.push((text[start..end].into(), start, end));
        } else {
            let end = start + c.len_utf8();
            out.push((c.to_string(), start, end));
            i += 1;
        }
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

/// Split into identifiers, string literals (kept with quotes) and single punctuation chars.
fn tokenize(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
        } else if c == '"' {
            let start = i;
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' {
                    i += 1;
                }
                i += 1;
            }
            i = (i + 1).min(chars.len());
            out.push(chars[start..i].iter().collect());
        } else if c == '_' || c.is_ascii_alphanumeric() {
            let start = i;
            while i < chars.len() && (chars[i] == '_' || chars[i].is_ascii_alphanumeric()) {
                i += 1;
            }
            out.push(chars[start..i].iter().collect());
        } else {
            out.push(c.to_string());
            i += 1;
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
    fn finds_top_level_and_nested_ranges_without_touching_comments() {
        let text =
            "// keep\npackage P\n  model M\n  end M;\n  model N end N;\nend P;\nmodel Q end Q;\n";
        let r = class_range(text, "P.M").unwrap();
        assert_eq!(&text[r], "  model M\n  end M;");
        let r = class_range(text, "Q").unwrap();
        assert_eq!(&text[r], "model Q end Q;");
    }

    #[test]
    fn replacement_preserves_comments_other_classes_and_nested_indent() {
        let before = "// header\npackage P\n  model M\n  end M;\nend P;\nmodel Q\nend Q;\n";
        let after = replace_class_text(before, "P.M", "model M\nReal x;\nend M;").unwrap();
        assert!(after.starts_with("// header\npackage P\n  model M\n  Real x;\n  end M;"));
        assert!(after.ends_with("end P;\nmodel Q\nend Q;\n"));
    }
}

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
    /// [`fingerprint`] of the file content when it was last read or written by Omola.
    /// `None` when the document was never read from or written to `path`.
    pub disk_stamp: Option<u64>,
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
            disk_stamp: None,
        }
    }

    pub fn open(path: PathBuf) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(&path)?;
        Ok(Self {
            path: Some(path),
            disk_stamp: Some(fingerprint(text.as_bytes())),
            text,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            editor: EditorState::new(),
        })
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        if let Some(path) = &self.path {
            write_atomic(path, &self.text)?;
            self.dirty = false;
            self.disk_stamp = Some(fingerprint(self.text.as_bytes()));
        }
        Ok(())
    }

    /// `true` when the file exists and its content differs from what Omola last read or
    /// wrote (issue G43). A missing file is not a conflict: saving simply recreates it.
    /// A file that exists but cannot be read counts as changed, so the user is asked.
    pub fn changed_on_disk(&self) -> bool {
        let Some(path) = &self.path else { return false };
        match std::fs::read(path) {
            Ok(bytes) => Some(fingerprint(&bytes)) != self.disk_stamp,
            Err(error) => error.kind() != std::io::ErrorKind::NotFound,
        }
    }

    /// Replace the buffer with the file content (the "Reload from disk" answer of G43).
    pub fn reload(&mut self) -> std::io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let text = std::fs::read_to_string(path)?;
        self.disk_stamp = Some(fingerprint(text.as_bytes()));
        self.text = text;
        self.dirty = false;
        self.undo.clear();
        self.redo.clear();
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

/// Fingerprint of file content, used to notice that another program changed a file.
///
/// Only compared inside one process, so the standard hasher is enough.
pub fn fingerprint(bytes: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

/// Write `text` to `path` through a temporary sibling file followed by a rename.
///
/// A crash, a full disk or a failed write therefore never leaves a half-written model
/// behind: the original file is only replaced once the new content is completely on disk
/// (`sync_all` before the rename). A symbolic link is followed (the link target is
/// replaced, not the link) and the permissions of an existing file are kept. A read-only
/// file is not replaced. The temporary file is created exclusively under a fresh name, so
/// an existing file or a link planted under that name is never written through.
pub fn write_atomic(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    let target = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let file_name = target.file_name().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no file name")
    })?;
    let existing = match std::fs::metadata(&target) {
        Ok(meta) => Some(meta.permissions()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    if existing.as_ref().is_some_and(|p| p.readonly()) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "the file is read-only",
        ));
    }
    let (tmp, file) = create_temporary_sibling(&target, file_name)?;
    let result = fill_and_replace(file, &tmp, &target, text, existing);
    if result.is_err() {
        // `tmp` was created by this call (`create_new`), so removing it is safe.
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Create `.<file>.omola-tmp-<pid>-<nanos>-<n>` next to `target`; never opens an existing file.
fn create_temporary_sibling(
    target: &std::path::Path,
    file_name: &std::ffi::OsStr,
) -> std::io::Result<(PathBuf, std::fs::File)> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    for attempt in 0..32u32 {
        let mut name = std::ffi::OsString::from(".");
        name.push(file_name);
        name.push(format!(
            ".omola-tmp-{}-{nanos}-{attempt}",
            std::process::id()
        ));
        let tmp = target.with_file_name(name);
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // Private until the content is complete; the final permissions are set later.
            options.mode(0o600);
        }
        match options.open(&tmp) {
            Ok(file) => return Ok((tmp, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not create a temporary file next to the target",
    ))
}

fn fill_and_replace(
    mut file: std::fs::File,
    tmp: &std::path::Path,
    target: &std::path::Path,
    text: &str,
    existing: Option<std::fs::Permissions>,
) -> std::io::Result<()> {
    use std::io::Write;
    file.write_all(text.as_bytes())?;
    if let Some(permissions) = existing {
        file.set_permissions(permissions)?;
    }
    file.sync_all()?;
    drop(file);
    std::fs::rename(tmp, target)?;
    // Make the rename itself durable. The new content is already in place, so a failure
    // here must not be reported as a failed save.
    #[cfg(unix)]
    if let Some(dir) = target.parent() {
        if let Ok(dir) = std::fs::File::open(dir) {
            let _ = dir.sync_all();
        }
    }
    Ok(())
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
    let within = within_prefix(&tokens);
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
            let mut qualified = within.clone();
            qualified.extend(stack.iter().cloned());
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
    let tokens: Vec<String> = toks.iter().map(|t| t.0.clone()).collect();
    let within = within_prefix(&tokens);
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
                    let q = within
                        .iter()
                        .map(String::as_str)
                        .chain(stack.iter().map(|x| x.0.as_str()))
                        .chain(std::iter::once(name.as_str()))
                        .collect::<Vec<_>>()
                        .join(".");
                    let mut first = i;
                    while first > 0
                        && matches!(
                            toks[first - 1].0.as_str(),
                            "partial" | "encapsulated" | "expandable" | "pure" | "impure"
                        )
                        && text[toks[first - 1].2..toks[first].1].trim().is_empty()
                    {
                        first -= 1;
                    }
                    let declaration_start = toks[first].1;
                    let line_start = text[..declaration_start].rfind('\n').map_or(0, |x| x + 1);
                    let line_start = if text[line_start..declaration_start].trim().is_empty() {
                        line_start
                    } else {
                        declaration_start
                    };
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

/// Parse the optional leading `within A.B;` of a stored definition.
fn within_prefix(tokens: &[String]) -> Vec<String> {
    if tokens.first().map(String::as_str) != Some("within") {
        return Vec::new();
    }
    let mut names = Vec::new();
    let mut i = 1;
    while let Some(token) = tokens.get(i) {
        if token == ";" {
            return names;
        }
        if !is_identifier(token) {
            return Vec::new();
        }
        names.push(token.clone());
        i += 1;
        match tokens.get(i).map(String::as_str) {
            Some(";") => return names,
            Some(".") => i += 1,
            _ => return Vec::new(),
        }
    }
    Vec::new()
}

#[cfg(test)]
#[test]
fn within_names_are_used_in_outline_and_edit_ranges() {
    let text = "within P.Sub;\nmodel M\n Real x;\nend M;\n";
    let classes = scan_classes(text);
    assert_eq!(classes[0].qualified_name, "P.Sub.M");
    let range = class_range(text, "P.Sub.M").unwrap();
    assert!(text[range].starts_with("model M"));
    assert_eq!(
        scan_classes("within; model Top end Top;")[0].qualified_name,
        "Top"
    );
    let same_line = "within P; model M end M;";
    let range = class_range(same_line, "P.M").unwrap();
    assert_eq!(&same_line[range], "model M end M;");
    for text in ["partial model M end M;", "within P; partial model M end M;"] {
        let name = if text.starts_with("within") {
            "P.M"
        } else {
            "M"
        };
        let replaced = replace_class_text(text, name, "partial model M end M;").unwrap();
        assert_eq!(replaced, text);
    }
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
    fn write_atomic_replaces_the_file_and_leaves_no_temporary_file() {
        let dir = std::env::temp_dir().join(format!("omola-atomic-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("M.mo");
        std::fs::write(&path, "model M end M;").unwrap();
        write_atomic(&path, "model M Real x; end M;").unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "model M Real x; end M;"
        );
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1, "temporary file left behind: {names:?}");
        // A new file in an existing directory works too.
        write_atomic(&dir.join("New.mo"), "model New end New;").unwrap();
        assert!(dir.join("New.mo").is_file());
        // A missing directory is an error and creates nothing.
        assert!(write_atomic(&dir.join("missing").join("X.mo"), "x").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_atomic_ignores_a_planted_temporary_name_and_keeps_read_only_files() {
        let dir = std::env::temp_dir().join(format!("omola-atomic-ro-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("M.mo");
        std::fs::write(&path, "old").unwrap();
        // The old, predictable temporary name; a file planted there must survive untouched.
        let planted = dir.join(format!(".M.mo.omola-tmp-{}", std::process::id()));
        std::fs::write(&planted, "planted").unwrap();
        write_atomic(&path, "new").unwrap();
        assert_eq!(std::fs::read_to_string(&planted).unwrap(), "planted");
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&path, permissions.clone()).unwrap();
        assert!(write_atomic(&path, "blocked").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        std::fs::set_permissions(&path, permissions).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn changed_on_disk_detects_an_external_write() {
        let dir = std::env::temp_dir().join(format!("omola-stamp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("M.mo");
        std::fs::write(&path, "model M end M;").unwrap();
        let mut doc = Document::open(path.clone()).unwrap();
        assert!(!doc.changed_on_disk());
        std::fs::write(&path, "model M Real x; end M;").unwrap();
        assert!(doc.changed_on_disk());
        doc.text = "model M Real y; end M;".into();
        doc.save().unwrap();
        assert!(!doc.changed_on_disk(), "our own save is not a conflict");
        std::fs::write(&path, "model M Real z; end M;").unwrap();
        doc.reload().unwrap();
        assert_eq!(doc.text, "model M Real z; end M;");
        assert!(!doc.changed_on_disk() && !doc.dirty);
        std::fs::remove_file(&path).unwrap();
        assert!(
            !doc.changed_on_disk(),
            "a deleted file is recreated by Save"
        );
        assert!(!Document::new_untitled("x").changed_on_disk());
        let _ = std::fs::remove_dir_all(&dir);
    }

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

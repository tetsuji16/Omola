//! Text layer tools: Go To Line and Replace (G45).

use super::*;

/// Text fields of the "Replace" dialog.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplaceDialog {
    pub find: String,
    pub replace: String,
    pub error: Option<String>,
}

/// Parse a 1-based line number and check it against the number of lines.
pub fn parse_goto_line(input: &str, line_count: usize) -> Result<usize, String> {
    let line_count = line_count.max(1);
    let line: usize = input
        .trim()
        .parse()
        .map_err(|_| "Enter a line number.".to_owned())?;
    if line == 0 || line > line_count {
        return Err(format!("The line must be between 1 and {line_count}."));
    }
    Ok(line)
}

/// Replace every occurrence of `needle` (case-sensitive). Returns the new text and the
/// number of replacements; an empty `needle` replaces nothing.
pub fn replace_all(text: &str, needle: &str, replacement: &str) -> (String, usize) {
    if needle.is_empty() {
        return (text.to_owned(), 0);
    }
    let count = text.matches(needle).count();
    (text.replace(needle, replacement), count)
}

impl OmolaApp {
    /// Document of the active class when it is a user model (library text is read-only).
    pub fn editable_text_doc(&self) -> Option<usize> {
        let class = self.active.as_ref()?;
        if self.is_user_class(class) {
            self.doc_of(class)
        } else {
            None
        }
    }

    pub fn open_goto_line_dialog(&mut self) {
        if self.editable_text_doc().is_some() {
            self.goto_line_dialog = Some(String::new());
        } else {
            self.info(LogTab::Syntax, "Go To works in a user model.".into());
        }
    }

    pub fn open_replace_dialog(&mut self) {
        if self.editable_text_doc().is_some() {
            self.replace_dialog = Some(ReplaceDialog::default());
        } else {
            self.info(LogTab::Syntax, "Replace works in a user model.".into());
        }
    }

    /// Show the Text layer with the caret on line `input` (1-based).
    pub fn goto_line(&mut self, input: &str) -> Result<usize, String> {
        let doc = self
            .editable_text_doc()
            .ok_or("Go To works in a user model.")?;
        let line = parse_goto_line(input, self.documents[doc].text.lines().count())?;
        self.select_tab(RibbonTab::Text);
        self.documents[doc].editor.highlight_line = Some(line);
        Ok(line)
    }

    /// Replace every occurrence of `find` in the active user document (one undo step).
    pub fn replace_in_active(&mut self, find: &str, replacement: &str) -> Result<usize, String> {
        let doc = self
            .editable_text_doc()
            .ok_or("Replace works in a user model.")?;
        if self.pending_edit.is_some() {
            return Err("A model edit is already running.".to_owned());
        }
        if find.is_empty() {
            return Err("Enter the text to find.".to_owned());
        }
        let (text, count) = replace_all(&self.documents[doc].text, find, replacement);
        if count == 0 {
            return Err(format!("\"{find}\" was not found."));
        }
        let before = std::mem::replace(&mut self.documents[doc].text, text);
        self.record_undo_snapshot(doc, before);
        self.user_text_changed(doc);
        self.info(
            LogTab::Syntax,
            format!("Replaced {count} occurrence(s) of \"{find}\"."),
        );
        Ok(count)
    }
}

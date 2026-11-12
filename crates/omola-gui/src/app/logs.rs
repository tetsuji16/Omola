//! The Logs window: bounded history, per-tab clearing and error locations.

use super::*;

impl OmolaApp {
    /// Re-parse the source positions of the current Check / Simulate when the log grew.
    ///
    /// Only the entries written since the current run started are looked at, so a fix and
    /// a re-check clear the markers of the previous run. Runs once per new log entry, not
    /// once per frame.
    pub(super) fn refresh_error_locations(&mut self) {
        let len = self.logs.len();
        if self.error_locations_log_len == Some(len) {
            return;
        }
        let from = match self.error_locations_log_len {
            // The pane was cleared (G22): parse everything that is left.
            Some(previous) if previous > len => 0,
            _ => self.error_from_log_index.min(len),
        };
        self.error_locations_log_len = Some(len);
        self.error_locations = self.logs[from..]
            .iter()
            .filter(|entry| {
                matches!(
                    entry.tab,
                    LogTab::Syntax | LogTab::Translation | LogTab::Simulation
                )
            })
            .flat_map(|entry| parse_omc_locations(&entry.text))
            .collect();
    }

    /// Forget the locations of the previous run when a new Check / Simulate starts.
    pub(super) fn start_error_scan(&mut self) {
        self.error_from_log_index = self.logs.len();
        self.error_locations_log_len = None;
        self.error_locations.clear();
    }

    /// Open the document a log location belongs to, show the Text layer and mark the line.
    ///
    /// G23 makes the editor scroll to `highlight_line` and put the caret on it.
    pub fn jump_to_location(&mut self, location: &SourceLocation) {
        // A user model is checked through a private copy named after its class, so the
        // class has to be part of the match.
        let document = (0..self.documents.len()).find(|&i| {
            let class = self.first_class_of(i);
            location_is_for(
                &location.file,
                self.documents[i].path.as_deref(),
                class.as_deref(),
            )
        });
        let class = document.and_then(|i| {
            self.user_classes
                .iter()
                .zip(&self.user_class_doc)
                .find(|(_, doc)| **doc == i)
                .map(|(class, _)| (i, class.qualified_name.clone()))
        });
        let Some((i, class)) = class else {
            let tab = self.log_tab;
            self.info(tab, format!("{} is not open.", location.file));
            return;
        };
        self.open_class(&class);
        self.select_tab(RibbonTab::Text);
        self.documents[i].editor.highlight_line = Some(location.line);
    }

    pub fn info(&mut self, tab: LogTab, text: String) {
        self.push_log(LogEntry {
            tab,
            level: LogLevel::Info,
            text,
        });
    }

    pub fn error(&mut self, tab: LogTab, text: String) {
        self.log_tab = tab;
        self.push_log(LogEntry {
            tab,
            level: LogLevel::Error,
            text,
        });
    }

    pub fn warn(&mut self, tab: LogTab, text: String) {
        self.push_log(LogEntry {
            tab,
            level: LogLevel::Warning,
            text,
        });
    }

    pub(super) fn push_log(&mut self, mut entry: LogEntry) {
        let char_count = entry.text.chars().count();
        if char_count > MAX_LOG_ENTRY_CHARS {
            let start_char = char_count - MAX_LOG_ENTRY_CHARS;
            let start_byte = entry
                .text
                .char_indices()
                .nth(start_char)
                .map_or(entry.text.len(), |(i, _)| i);
            let tail = entry.text.split_off(start_byte);
            entry.text = format!("... (earlier output truncated)\n{tail}");
        }
        self.logs.push(entry);
        self.error_locations_log_len = None;
        if self.logs.len() > MAX_LOG_ENTRIES {
            let excess = self.logs.len() - MAX_LOG_ENTRIES;
            self.logs.drain(..excess);
            self.error_from_log_index = self.error_from_log_index.saturating_sub(excess);
        }
    }

    pub fn clear_logs_for_tab(&mut self, tab: LogTab) {
        let old_start = self.error_from_log_index.min(self.logs.len());
        let retained_before_start = self.logs[..old_start]
            .iter()
            .filter(|entry| entry.tab != tab)
            .count();
        self.logs.retain(|entry| entry.tab != tab);
        self.error_from_log_index = retained_before_start;
        self.error_locations_log_len = None;
    }
}

//! Graphical edits through the OMC API and their undo/redo history.

use super::*;

impl OmolaApp {
    /// Apply graphical edits to a user-owned class through OMC.
    pub fn apply_edit(&mut self, class: &str, ops: Vec<EditOp>) {
        if !self.is_user_class(class) {
            return self.error(LogTab::Syntax, "Library classes are read-only".into());
        }
        if self.pending_edit.is_some() {
            return self.error(LogTab::Syntax, "A model edit is already running".into());
        }
        let Some(doc) = self.doc_of(class) else {
            return;
        };
        self.pending_edit = Some(class.to_owned());
        self.pending_edit_text = Some(self.documents[doc].text.clone());
        let libs = self.loaded_specs();
        self.worker.submit(Job::EditModel {
            doc_text: self.documents[doc].text.clone(),
            file_name: self.documents[doc]
                .path
                .as_ref()
                .map_or_else(String::new, |p| p.to_string_lossy().into_owned()),
            class: class.to_owned(),
            ops,
            libraries: libs,
        });
    }

    pub fn delete_selected_component(&mut self) {
        let Some(class) = self.active.clone() else {
            return;
        };
        if !self.is_user_class(&class) {
            return self.error(LogTab::Syntax, "Library classes are read-only".into());
        }
        let Some(name) = self.selected_component.clone() else {
            return;
        };
        let Some(Loadable::Ready(diagram)) = self.model_diagrams.get(&class) else {
            return;
        };
        let ops = crate::diagram::delete_component_ops(diagram, &name);
        if ops.len() > 1 {
            self.apply_edit(&class, ops);
            self.selected_component = None;
        }
    }

    pub(super) fn record_undo_snapshot(&mut self, doc: usize, text: String) {
        let Some(document) = self.documents.get_mut(doc) else {
            return;
        };
        if document.undo.last() != Some(&text) {
            document.undo.push(text);
            if document.undo.len() > 100 {
                document.undo.remove(0);
            }
        }
        document.redo.clear();
    }

    pub fn undo(&mut self) {
        self.restore_history(false);
    }

    pub fn redo(&mut self) {
        self.restore_history(true);
    }

    pub fn can_undo(&self) -> bool {
        self.active_doc()
            .is_some_and(|doc| !self.documents[doc].undo.is_empty())
    }

    pub fn can_redo(&self) -> bool {
        self.active_doc()
            .is_some_and(|doc| !self.documents[doc].redo.is_empty())
    }

    pub(super) fn restore_history(&mut self, redo: bool) {
        if self.pending_edit.is_some() {
            return;
        }
        let Some(class) = self.active.clone() else {
            return;
        };
        let Some(doc) = self.doc_of(&class) else {
            return;
        };
        let current = self.documents[doc].text.clone();
        let replacement = {
            let document = &mut self.documents[doc];
            if redo {
                if let Some(text) = document.redo.pop() {
                    document.undo.push(current);
                    Some(text)
                } else {
                    None
                }
            } else {
                if let Some(text) = document.undo.pop() {
                    document.redo.push(current);
                    Some(text)
                } else {
                    None
                }
            }
        };
        let Some(replacement) = replacement else {
            return;
        };
        self.documents[doc].text = replacement;
        self.user_text_changed(doc);
        self.model_diagrams.remove(&class);
        self.icons.remove(&class);
        if self
            .components
            .as_ref()
            .is_some_and(|(name, _)| name == &class)
        {
            self.components = None;
        }
        self.components_pending = None;
        self.request_diagram(&class);
        self.request_icon(&class);
        self.request_components(&class);
    }

    pub(super) fn handle_model_edited_event(&mut self, event: Event) {
        let Event::ModelEdited { class, result } = event else {
            return;
        };
        self.pending_edit = None;
        let submitted = self.pending_edit_text.take();
        match result {
            Ok(source) => {
                if let Some(doc) = self.doc_of(&class) {
                    if submitted.is_some_and(|text| text != self.documents[doc].text) {
                        // The result was computed from an older text: applying it would
                        // drop what was typed (or undone) in the meantime.
                        self.warn(
                            LogTab::Syntax,
                            format!(
                                "The text of {class} changed while the edit was running; \
                                 the edit was not applied."
                            ),
                        );
                    } else if let Some(updated) =
                        crate::model::replace_class_text(&self.documents[doc].text, &class, &source)
                    {
                        let old_text = self.documents[doc].text.clone();
                        self.record_undo_snapshot(doc, old_text);
                        self.documents[doc].text = updated;
                        self.user_text_changed(doc);
                        self.model_diagrams.remove(&class);
                        self.icons.remove(&class);
                        if self.components.as_ref().is_some_and(|(c, _)| c == &class) {
                            self.components = None;
                        }
                        self.components_pending = None;
                        self.request_diagram(&class);
                        self.request_icon(&class);
                        self.request_components(&class);
                    } else {
                        self.error(
                            LogTab::Syntax,
                            format!("Could not locate class {class} in the editor buffer"),
                        );
                    }
                }
            }
            Err(message) => self.error(LogTab::Syntax, message),
        }
    }
}

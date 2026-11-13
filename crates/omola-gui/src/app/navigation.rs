//! Model tabs, back/forward history, layers and the lazy per-class requests.

use super::*;

impl OmolaApp {
    /// Show a class in the edit window (new tab if needed) and record history.
    pub fn open_class(&mut self, class: &str) {
        self.navigate(class);
        if self
            .history
            .get(self.history_pos.wrapping_sub(1))
            .map(String::as_str)
            != Some(class)
            || self.history.is_empty()
        {
            self.history.truncate(self.history_pos);
            self.history.push(class.to_owned());
            self.history_pos = self.history.len();
        }
    }

    pub(super) fn navigate(&mut self, class: &str) {
        if !self.model_tabs.iter().any(|t| t == class) {
            // Replace the active tab (like single-clicking in the package browser),
            // unless it is a user model, which keeps its own tab.
            match self
                .active
                .as_ref()
                .and_then(|a| self.model_tabs.iter().position(|t| t == a))
            {
                Some(i) if !self.is_user_class(&self.model_tabs[i]) => {
                    self.model_tabs[i] = class.to_owned()
                }
                _ => self.model_tabs.push(class.to_owned()),
            }
        }
        let changed = self.active.as_deref() != Some(class);
        self.active = Some(class.to_owned());
        if changed {
            self.reset_diagram_view();
        }
        // Expand the package browser down to the class.
        let mut prefix = String::new();
        for part in class.split('.') {
            if !prefix.is_empty() {
                self.expanded.insert(prefix.clone());
                prefix.push('.');
            }
            prefix.push_str(part);
        }
        self.request_doc(class);
        self.request_components(class);
        if self.tab == RibbonTab::Text {
            self.request_text(class);
        }
    }

    pub fn open_in_new_tab(&mut self, class: &str) {
        if !self.model_tabs.iter().any(|t| t == class) {
            self.model_tabs.push(class.to_owned());
        }
        self.open_class(class);
    }

    pub fn close_tab(&mut self, class: &str) {
        self.model_tabs.retain(|t| t != class);
        if self.active.as_deref() == Some(class) {
            self.active = self.model_tabs.last().cloned();
        }
    }

    pub fn can_go_back(&self) -> bool {
        self.history_pos > 1
    }

    pub fn can_go_forward(&self) -> bool {
        self.history_pos < self.history.len()
    }

    pub fn go_back(&mut self) {
        if self.can_go_back() {
            self.history_pos -= 1;
            let class = self.history[self.history_pos - 1].clone();
            self.navigate(&class);
        }
    }

    pub fn go_forward(&mut self) {
        if self.can_go_forward() {
            self.history_pos += 1;
            let class = self.history[self.history_pos - 1].clone();
            self.navigate(&class);
        }
    }

    pub fn request_doc(&mut self, class: &str) {
        let stale = matches!(self.docs.get(class), Some(Loadable::Failed(_)))
            || self.doc_of(class).is_some_and(|i| self.documents[i].dirty);
        if self.docs.contains_key(class) && !stale {
            return;
        }
        if self
            .libraries
            .iter()
            .any(|l| matches!(l, LibraryState::Loading(_)))
            && !self.is_user_class(class)
            && self.restriction_of(class).is_none()
        {
            // Wait for the library index; re-requested when it arrives.
            return;
        }
        self.docs.insert(class.to_owned(), Loadable::Loading);
        self.worker.submit(Job::LoadClassDoc {
            class: class.to_owned(),
            context: self.context_for(class),
        });
    }

    /// Ask the worker for the component tree of `class` (once per class).
    ///
    /// This starts an OMC process, so it is only sent when the active class changes or
    /// after a successful Check - never per keystroke.
    pub fn request_components(&mut self, class: &str) {
        if self.components.as_ref().map(|(c, _)| c.as_str()) == Some(class)
            || self.components_pending.as_deref() == Some(class)
        {
            return;
        }
        self.components_pending = Some(class.to_owned());
        self.worker.submit(Job::LoadInstance {
            class: class.to_owned(),
            context: self.context_for(class),
        });
    }

    pub fn open_parameter_dialog(&mut self, class: &str, component: &str, component_type: &str) {
        let dialog = ParameterDialog {
            class: class.to_owned(),
            component: component.to_owned(),
            component_type: component_type.to_owned(),
            rows: None,
            values: Vec::new(),
            readonly: !self.is_user_class(class),
            error: None,
        };
        self.worker.submit(Job::LoadParameters {
            component_type: component_type.to_owned(),
            class: class.to_owned(),
            component: component.to_owned(),
            context: self.context_for(class),
        });
        self.parameter_dialog = Some(dialog);
    }

    /// Ask the worker for the Icon graphics of `class` (once per class).
    ///
    /// The Icon layer calls this on every frame it is visible, so an entry is added to
    /// [`Self::icons`] before the job is sent; a failed load is remembered as well and is
    /// only retried after the next Check.
    pub fn request_icon(&mut self, class: &str) {
        if self.icons.contains_key(class) {
            return;
        }
        self.icons.insert(class.to_owned(), Loadable::Loading);
        self.worker.submit(Job::LoadIcon {
            class: class.to_owned(),
            context: self.context_for(class),
        });
    }

    /// Ask the worker for the Diagram graphics of `class` (once per class).
    ///
    /// The Diagram layer calls this on every frame it is visible, so an entry is added to
    /// [`Self::model_diagrams`] before the job is sent; a failed load is remembered as well and is
    /// only retried after the next Check.
    pub fn request_diagram(&mut self, class: &str) {
        if self.model_diagrams.contains_key(class) {
            return;
        }
        self.model_diagrams
            .insert(class.to_owned(), Loadable::Loading);
        self.worker.submit(Job::LoadDiagram {
            class: class.to_owned(),
            context: self.context_for(class),
            cached_icons: self
                .icons
                .iter()
                .filter_map(|(name, icon)| match icon {
                    Loadable::Ready(icon) => Some((name.clone(), icon.clone())),
                    _ => None,
                })
                .collect(),
        });
    }

    pub fn request_text(&mut self, class: &str) {
        if self.is_user_class(class) || self.texts.contains_key(class) {
            return;
        }
        if self.restriction_of(class).as_deref() == Some("package") {
            self.texts.insert(
                class.to_owned(),
                Loadable::Failed(
                    "The Modelica text of a whole package is not shown; open one of its classes."
                        .into(),
                ),
            );
            return;
        }
        self.texts.insert(class.to_owned(), Loadable::Loading);
        self.worker.submit(Job::LoadClassText {
            class: class.to_owned(),
            context: self.context_for(class),
        });
    }

    /// The edit-window layer for the current ribbon tab.
    pub fn layer(&self) -> Layer {
        match self.tab {
            RibbonTab::Graphics | RibbonTab::Tools | RibbonTab::Simulation => self.graphics_layer,
            RibbonTab::Documentation => Layer::Documentation,
            RibbonTab::Text => Layer::Text,
        }
    }

    pub fn set_layer(&mut self, layer: Layer) {
        match layer {
            Layer::Icon | Layer::Diagram => {
                self.graphics_layer = layer;
                self.tab = RibbonTab::Graphics;
            }
            Layer::Documentation => self.tab = RibbonTab::Documentation,
            Layer::Text => {
                self.tab = RibbonTab::Text;
                if let Some(active) = self.active.clone() {
                    self.request_text(&active);
                }
            }
        }
    }

    /// Window title, e.g. `Modelica - Modelica (Read-Only) - [Documentation]`.
    pub fn window_title(&self) -> String {
        let layer = if self.tab == RibbonTab::Simulation {
            "Plot [1*]".to_owned()
        } else {
            self.layer().label().to_owned()
        };
        match &self.active {
            None => format!("Omola - Modelica Workbench - [{layer}]"),
            Some(class) => {
                let read_only = if self.is_user_class(class) {
                    ""
                } else {
                    " (Read-Only)"
                };
                format!(
                    "{} - {class}{read_only} - [{layer}]",
                    library::short_name(class)
                )
            }
        }
    }
}

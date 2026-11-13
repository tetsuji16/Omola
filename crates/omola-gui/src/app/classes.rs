//! Libraries, user documents and the class tree behind the Package Browser.

use super::*;

impl OmolaApp {
    pub fn load_library(&mut self, spec: LibrarySpec) {
        self.libraries.retain(|l| l.spec().name != spec.name);
        self.libraries.push(LibraryState::Loading(spec.clone()));
        self.worker.submit(Job::LoadLibrary(spec));
    }

    /// Install a library from the on-disk index cache without starting OMC.
    ///
    /// Returns `false` on a cache miss (or a stale/corrupt entry), so the caller can fall
    /// back to [`Self::load_library`]. `load_library` itself always queries OMC, which is
    /// what the Tools > Reload Libraries command wants.
    pub fn load_library_from_cache(&mut self, spec: LibrarySpec) -> bool {
        let Some(lib) = crate::cache::load(&self.cache_root, &spec) else {
            return false;
        };
        let (name, version, count, source) = (
            spec.name.clone(),
            lib.version.clone(),
            lib.classes.len(),
            lib.source_file.clone(),
        );
        self.libraries.retain(|l| l.spec().name != name);
        self.libraries.push(LibraryState::Loaded(lib));
        self.info(
            LogTab::Version,
            format!(
                "Loaded {name} {version} ({count} classes) from cache {}",
                source.display()
            ),
        );
        true
    }

    pub fn loaded_specs(&self) -> Vec<LibrarySpec> {
        self.libraries
            .iter()
            .filter_map(|l| match l {
                LibraryState::Loaded(lib) => Some(lib.spec.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn is_user_class(&self, class: &str) -> bool {
        self.user_classes.iter().any(|c| c.qualified_name == class)
    }

    /// Index of the document that defines a user class.
    pub fn doc_of(&self, class: &str) -> Option<usize> {
        self.user_classes
            .iter()
            .position(|c| c.qualified_name == class)
            .map(|i| self.user_class_doc[i])
    }

    /// The document of the active class, if it is a user class.
    pub fn active_doc(&self) -> Option<usize> {
        self.active.as_deref().and_then(|c| self.doc_of(c))
    }

    /// Context for OMC queries about `class`: loaded libraries plus the user buffer if needed.
    ///
    /// A user model may depend on classes declared in the user's *other* open documents
    /// (G53). Their editor buffers are appended to `user_source`, so the single private
    /// copy written for OMC defines the queried class together with its neighbours and the
    /// dependency resolves. The document of `class` itself is never duplicated; empty
    /// documents are skipped. Library classes carry no user source.
    pub fn context_for(&self, class: &str) -> ClassContext {
        let top = class.split('.').next().unwrap_or(class);
        let user = self.is_user_class(class);
        let mut libraries = self.loaded_specs();
        if !user {
            // Load only the library that owns the class (faster), plus its dependencies
            // which OMC resolves through `uses` annotations.
            libraries.retain(|l| l.name == top);
            if libraries.is_empty() {
                libraries.push(LibrarySpec::new(top, None));
            }
        }
        let user_source = user.then(|| {
            let own = self.doc_of(class);
            let mut text = own.map(|i| self.documents[i].text.clone()).unwrap_or_default();
            for (i, doc) in self.documents.iter().enumerate() {
                if Some(i) == own || doc.text.is_empty() {
                    continue;
                }
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&doc.text);
            }
            text
        });
        ClassContext {
            libraries,
            user_source,
        }
    }

    /// Restriction (`model`, `package`, ...) if known from an index.
    pub fn restriction_of(&self, class: &str) -> Option<String> {
        if let Some(c) = self.user_classes.iter().find(|c| c.qualified_name == class) {
            return Some(c.kind.clone());
        }
        self.libraries.iter().find_map(|l| match l {
            LibraryState::Loaded(lib) => lib.class(class).map(|c| c.restriction.clone()),
            _ => None,
        })
    }

    /// Children of a library or user class, as (qualified, restriction, comment).
    pub fn children_of(&self, class: &str) -> Vec<(String, String, String)> {
        if self.is_user_class(class) {
            return self
                .user_classes
                .iter()
                .filter(|c| library::parent_name(&c.qualified_name) == Some(class))
                .map(|c| {
                    (
                        c.qualified_name.clone(),
                        c.kind.clone(),
                        c.description.clone().unwrap_or_default(),
                    )
                })
                .collect();
        }
        self.libraries
            .iter()
            .find_map(|l| match l {
                LibraryState::Loaded(lib)
                    if class.split('.').next() == Some(lib.spec.name.as_str()) =>
                {
                    Some(
                        lib.children(class)
                            .into_iter()
                            .map(|c| {
                                (
                                    c.qualified_name.clone(),
                                    c.restriction.clone(),
                                    c.comment.clone(),
                                )
                            })
                            .collect(),
                    )
                }
                _ => None,
            })
            .unwrap_or_default()
    }

    /// Whether a class has direct children in the user outline or library index.
    pub fn has_children_of(&self, class: &str) -> bool {
        if self.is_user_class(class) {
            return self
                .user_classes
                .iter()
                .any(|c| library::parent_name(&c.qualified_name) == Some(class));
        }
        self.libraries.iter().any(|state| match state {
            LibraryState::Loaded(lib)
                if class.split('.').next() == Some(lib.spec.name.as_str()) =>
            {
                lib.has_children(class)
            }
            _ => false,
        })
    }

    /// Direct child names for rendering an expanded Package Browser node.
    pub fn child_names(&self, class: &str) -> Vec<&str> {
        if self.is_user_class(class) {
            return self
                .user_classes
                .iter()
                .filter(|c| library::parent_name(&c.qualified_name) == Some(class))
                .map(|c| c.qualified_name.as_str())
                .collect();
        }
        self.libraries
            .iter()
            .find_map(|state| match state {
                LibraryState::Loaded(lib)
                    if class.split('.').next() == Some(lib.spec.name.as_str()) =>
                {
                    Some(
                        lib.children_iter(class)
                            .map(|c| c.qualified_name.as_str())
                            .collect(),
                    )
                }
                _ => None,
            })
            .unwrap_or_default()
    }

    /// Borrowed restriction and comment for a Package Browser class.
    pub fn class_details(&self, class: &str) -> Option<(&str, &str)> {
        if let Some(c) = self.user_classes.iter().find(|c| c.qualified_name == class) {
            return Some((c.kind.as_str(), c.description.as_deref().unwrap_or("")));
        }
        self.libraries.iter().find_map(|state| match state {
            LibraryState::Loaded(lib) => lib
                .class(class)
                .map(|c| (c.restriction.as_str(), c.comment.as_str())),
            _ => None,
        })
    }

    /// Compute normalized Package Browser search matches, capped with one sentinel row.
    pub fn search_classes(&self, query: &str) -> Vec<String> {
        let mut hits = Vec::new();
        for c in &self.user_classes {
            if c.qualified_name.to_lowercase().contains(query) {
                hits.push(c.qualified_name.clone());
                if hits.len() > 500 {
                    return hits;
                }
            }
        }
        for state in &self.libraries {
            if let LibraryState::Loaded(lib) = state {
                for c in &lib.classes {
                    if library::short_name(&c.qualified_name)
                        .to_lowercase()
                        .contains(query)
                    {
                        hits.push(c.qualified_name.clone());
                        if hits.len() > 500 {
                            return hits;
                        }
                    }
                }
            }
        }
        hits
    }

    pub fn update_package_search_cache(&mut self) {
        let query = self.package_filter.trim().to_lowercase();
        if self
            .package_search_cache
            .as_ref()
            .is_none_or(|(cached, _)| cached != &query)
        {
            self.package_search_cache = Some((query.clone(), self.search_classes(&query)));
        }
    }
}

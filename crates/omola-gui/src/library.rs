//! Library and class-documentation queries against OMC, and the pure helpers around them.
//!
//! Every query prints explicit markers (`OMOLA_BEGIN key` / `OMOLA_END key`,
//! `OMOLA_CLASS\t...`) so the output can be parsed without guessing OMC's formatting.

use omola_core::{escape_modelica_string, validate_class_name, BackendError};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A library to load into OMC (`loadModel`). `version: None` means "any installed version".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibrarySpec {
    pub name: String,
    pub version: Option<String>,
}

impl LibrarySpec {
    pub fn new(name: &str, version: Option<&str>) -> Self {
        Self {
            name: name.into(),
            version: version.map(Into::into),
        }
    }
}

/// Libraries loaded at start-up, like a default Modelica tool installation.
pub fn default_libraries() -> Vec<LibrarySpec> {
    vec![
        LibrarySpec::new("Modelica", Some("4.1.0")),
        LibrarySpec::new("ModelicaReference", None),
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassEntry {
    pub qualified_name: String,
    pub restriction: String,
    pub comment: String,
}

impl ClassEntry {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn short_name(&self) -> &str {
        short_name(&self.qualified_name)
    }
}

pub fn short_name(qualified: &str) -> &str {
    qualified.rsplit('.').next().unwrap_or(qualified)
}

pub fn parent_name(qualified: &str) -> Option<&str> {
    qualified.rsplit_once('.').map(|(parent, _)| parent)
}

/// A loaded library: its index of classes in declaration order.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedLibrary {
    pub spec: LibrarySpec,
    pub version: String,
    /// `package.mo` (or single file) that defines the top-level class.
    pub source_file: PathBuf,
    /// All classes including the top-level one, in declaration order.
    pub classes: Vec<ClassEntry>,
    /// qualified name -> index into `classes` (the first occurrence wins).
    by_name: std::collections::HashMap<String, usize>,
    /// `children[i]` = indices of the direct children of `classes[i]`, in declaration order.
    children: Vec<Vec<usize>>,
}

impl LoadedLibrary {
    /// Build a library and its index.
    ///
    /// The index makes `class` / `children` / `has_children` O(1) instead of a linear
    /// scan over several thousand classes, which the Package Browser calls for every
    /// visible row of every frame (G18).
    pub fn new(
        spec: LibrarySpec,
        version: String,
        source_file: PathBuf,
        classes: Vec<ClassEntry>,
    ) -> Self {
        let mut by_name = std::collections::HashMap::with_capacity(classes.len());
        for (i, class) in classes.iter().enumerate() {
            by_name.entry(class.qualified_name.clone()).or_insert(i);
        }
        let mut children = vec![Vec::new(); classes.len()];
        for (i, class) in classes.iter().enumerate() {
            let parent = parent_name(&class.qualified_name).and_then(|p| by_name.get(p));
            if let Some(&p) = parent {
                children[p].push(i);
            }
        }
        Self {
            spec,
            version,
            source_file,
            classes,
            by_name,
            children,
        }
    }

    pub fn root(&self) -> Option<&ClassEntry> {
        self.classes.first()
    }

    /// Direct children of `parent` in declaration order.
    pub fn children(&self, parent: &str) -> Vec<&ClassEntry> {
        self.children_iter(parent).collect()
    }

    /// [`Self::children`] without the allocation, for the per-frame tree drawing.
    pub fn children_iter<'a>(
        &'a self,
        parent: &str,
    ) -> impl Iterator<Item = &'a ClassEntry> + 'a {
        let indices: &'a [usize] = self
            .by_name
            .get(parent)
            .map(|&p| self.children[p].as_slice())
            .unwrap_or(&[]);
        indices.iter().map(move |&i| &self.classes[i])
    }

    pub fn class(&self, qualified: &str) -> Option<&ClassEntry> {
        self.by_name.get(qualified).map(|&i| &self.classes[i])
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn has_children(&self, parent: &str) -> bool {
        self.by_name
            .get(parent)
            .is_some_and(|&p| !self.children[p].is_empty())
    }
}

fn load_statement(spec: &LibrarySpec, flag: &str) -> Result<String, BackendError> {
    validate_class_name(&spec.name)?;
    Ok(match &spec.version {
        Some(version) => {
            omola_core::validate_library_pins(&[(spec.name.clone(), version.clone())])?;
            format!(
                "{flag} := loadModel({}, {{\"{}\"}});\n",
                spec.name,
                escape_modelica_string(version)
            )
        }
        None => format!("{flag} := loadModel({});\n", spec.name),
    })
}

/// Load one library and print its full class index.
pub fn library_index_script(spec: &LibrarySpec) -> Result<String, BackendError> {
    let name = &spec.name;
    let mut script = load_statement(spec, "omolaLoaded")?;
    script.push_str(&format!(
        r#"if not omolaLoaded then
  print("OMOLA_LOAD_FAILED\n" + getErrorString() + "\n");
else
  print("OMOLA_BEGIN source\n" + getSourceFile({name}) + "\nOMOLA_END source\n");
  print("OMOLA_BEGIN version\n" + getVersion({name}) + "\nOMOLA_END version\n");
  print("OMOLA_CLASS\t{name}\t" + getClassRestriction({name}) + "\t" + getClassComment({name}) + "\n");
  for omolaClass in getClassNames({name}, recursive=true, qualified=true, sort=false) loop
    print("OMOLA_CLASS\t" + typeNameString(omolaClass) + "\t" + getClassRestriction(omolaClass) + "\t" + getClassComment(omolaClass) + "\n");
  end for;
end if;
"#
    ));
    Ok(script)
}

/// Parse the output of [`library_index_script`].
pub fn parse_library_index(spec: &LibrarySpec, output: &str) -> Result<LoadedLibrary, String> {
    if let Some(at) = output.find("OMOLA_LOAD_FAILED") {
        let detail = output[at + "OMOLA_LOAD_FAILED".len()..].trim();
        return Err(format!(
            "could not load library {}{}: {}",
            spec.name,
            spec.version
                .as_deref()
                .map(|v| format!(" {v}"))
                .unwrap_or_default(),
            if detail.is_empty() {
                "not installed"
            } else {
                detail
            }
        ));
    }
    let sections = parse_sections(output);
    let source_file = sections
        .get("source")
        .map(|s| PathBuf::from(s.trim()))
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or("OMC did not report the library source file")?;
    // `getClassNames(recursive=true)` also returns the top-level class itself, which the
    // script prints separately before the loop. Keep the first occurrence (the explicit
    // one, which carries the root's own comment) and drop later duplicates, so class
    // counts and search results do not list the root twice.
    let mut seen = std::collections::HashSet::new();
    let classes: Vec<ClassEntry> = output
        .lines()
        .filter_map(|line| line.strip_prefix("OMOLA_CLASS\t"))
        .filter_map(|rest| {
            let mut parts = rest.splitn(3, '\t');
            let qualified_name = parts.next()?.trim().to_owned();
            let restriction = parts.next()?.trim().to_owned();
            let comment = parts.next().unwrap_or("").trim().to_owned();
            (!qualified_name.is_empty()).then_some(ClassEntry {
                qualified_name,
                restriction,
                comment,
            })
        })
        .filter(|c| seen.insert(c.qualified_name.clone()))
        .collect();
    if classes.first().map(|c| c.qualified_name.as_str()) != Some(spec.name.as_str()) {
        return Err(format!("OMC returned no class index for {}", spec.name));
    }
    Ok(LoadedLibrary::new(
        spec.clone(),
        sections
            .get("version")
            .map(|v| v.trim().to_owned())
            .unwrap_or_default(),
        source_file,
        classes,
    ))
}

/// Documentation of one class.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ClassDoc {
    pub class: String,
    pub restriction: String,
    pub comment: String,
    pub info_html: String,
    pub revisions_html: String,
    /// Directory of the top-level library (or user file) for resolving `modelica://` URIs.
    pub roots: BTreeMap<String, PathBuf>,
    /// `experiment` annotation (or OMC defaults): start, stop, tolerance, intervals.
    pub experiment: Option<Experiment>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Experiment {
    pub start_time: f64,
    pub stop_time: f64,
    pub tolerance: f64,
    pub intervals: u32,
}

/// Parse `"0.0 3.0 1e-06 500"` printed by the class documentation script.
pub fn parse_experiment(text: &str) -> Option<Experiment> {
    let nums: Vec<f64> = text
        .split_whitespace()
        .map(|t| t.parse::<f64>())
        .collect::<Result<_, _>>()
        .ok()?;
    let [start_time, stop_time, tolerance, intervals] = nums[..] else {
        return None;
    };
    (stop_time.is_finite() && start_time.is_finite() && intervals >= 1.0).then_some(Experiment {
        start_time,
        stop_time,
        tolerance,
        intervals: intervals as u32,
    })
}

/// Script printing restriction, comment, and documentation of `class`.
/// `libraries` are loaded first; `user_file` (an editor copy) is loaded last.
pub fn class_doc_script(
    class: &str,
    libraries: &[LibrarySpec],
    user_file: Option<&Path>,
) -> Result<String, BackendError> {
    validate_class_name(class)?;
    let mut script = String::new();
    for (i, spec) in libraries.iter().enumerate() {
        script.push_str(&load_statement(spec, &format!("omolaLib{i}"))?);
    }
    if let Some(file) = user_file {
        script.push_str(&format!(
            "omolaUser := loadFile(\"{}\");\n",
            escape_modelica_string(&file.to_string_lossy())
        ));
    }
    let top = class.split('.').next().unwrap_or(class);
    script.push_str(&format!(
        r#"print("OMOLA_BEGIN root\n" + getSourceFile({top}) + "\nOMOLA_END root\n");
print("OMOLA_BEGIN restriction\n" + getClassRestriction({class}) + "\nOMOLA_END restriction\n");
print("OMOLA_BEGIN comment\n" + getClassComment({class}) + "\nOMOLA_END comment\n");
(omolaStart, omolaStop, omolaTol, omolaIntervals, omolaInterval) := getSimulationOptions({class});
print("OMOLA_BEGIN experiment\n" + String(omolaStart) + " " + String(omolaStop) + " " + String(omolaTol) + " " + String(omolaIntervals) + "\nOMOLA_END experiment\n");
omolaDocs := getDocumentationAnnotation({class});
print("OMOLA_BEGIN info\n" + omolaDocs[1] + "\nOMOLA_END info\n");
print("OMOLA_BEGIN revisions\n" + omolaDocs[2] + "\nOMOLA_END revisions\n");
"#
    ));
    Ok(script)
}

pub fn parse_class_doc(class: &str, output: &str) -> Result<ClassDoc, String> {
    let sections = parse_sections(output);
    let restriction = sections
        .get("restriction")
        .map(|s| s.trim().to_owned())
        .unwrap_or_default();
    if restriction.is_empty() {
        let hint: String = output.lines().rev().take(6).collect::<Vec<_>>().join(" | ");
        return Err(format!("class {class} was not found ({hint})"));
    }
    let mut roots = BTreeMap::new();
    if let Some(source) = sections.get("root").map(|s| s.trim()) {
        if !source.is_empty() {
            let top = class.split('.').next().unwrap_or(class).to_owned();
            if let Some(dir) = Path::new(source).parent() {
                roots.insert(top, dir.to_path_buf());
            }
        }
    }
    Ok(ClassDoc {
        class: class.into(),
        restriction,
        comment: sections
            .get("comment")
            .map(|s| s.trim().to_owned())
            .unwrap_or_default(),
        info_html: sections.get("info").cloned().unwrap_or_default(),
        revisions_html: sections.get("revisions").cloned().unwrap_or_default(),
        roots,
        experiment: sections.get("experiment").and_then(|e| parse_experiment(e)),
    })
}

/// Script printing the Modelica text of `class` (`list`).
pub fn class_text_script(class: &str, libraries: &[LibrarySpec]) -> Result<String, BackendError> {
    validate_class_name(class)?;
    let mut script = String::new();
    for (i, spec) in libraries.iter().enumerate() {
        script.push_str(&load_statement(spec, &format!("omolaLib{i}"))?);
    }
    script.push_str(&format!(
        "print(\"OMOLA_BEGIN text\\n\" + list({class}) + \"\\nOMOLA_END text\\n\");\n"
    ));
    Ok(script)
}

/// Script printing `getModelInstance(CLASS)` as JSON (with the user file loaded when given).
pub fn instance_script(
    class: &str,
    libraries: &[LibrarySpec],
    user_file: Option<&Path>,
) -> Result<String, BackendError> {
    validate_class_name(class)?;
    let mut script = String::new();
    for (i, spec) in libraries.iter().enumerate() {
        script.push_str(&load_statement(spec, &format!("omolaLib{i}"))?);
    }
    if let Some(file) = user_file {
        script.push_str(&format!(
            "omolaUser := loadFile(\"{}\");\n",
            escape_modelica_string(&file.to_string_lossy())
        ));
    }
    script.push_str(&format!(
        "print(\"OMOLA_BEGIN instance\\n\" + getModelInstance({class}, prettyPrint=true) + \"\\nOMOLA_END instance\\n\");\n"
    ));
    Ok(script)
}

/// Script printing `getModelInstanceAnnotation(CLASS, {"Icon"})` as JSON (G07).
///
/// Only the Icon annotation is asked for: the answer is a few KB instead of the ~75 KB of
/// a full `getModelInstance`, and OMC still nests the inherited annotations of the
/// `extends` chain into it (see `docs/omc-model-instance-json.md`).
pub fn icon_script(
    class: &str,
    libraries: &[LibrarySpec],
    user_file: Option<&Path>,
) -> Result<String, BackendError> {
    validate_class_name(class)?;
    let mut script = String::new();
    for (i, spec) in libraries.iter().enumerate() {
        script.push_str(&load_statement(spec, &format!("omolaLib{i}"))?);
    }
    if let Some(file) = user_file {
        script.push_str(&format!(
            "omolaUser := loadFile(\"{}\");\n",
            escape_modelica_string(&file.to_string_lossy())
        ));
    }
    script.push_str(&format!(
        "print(\"OMOLA_BEGIN icon\\n\" + getModelInstanceAnnotation({class}, {{\"Icon\"}}) + \"\\nOMOLA_END icon\\n\");\n"
    ));
    Ok(script)
}

/// Script running `checkModel` on a class from loaded libraries and/or a user file.
pub fn check_script(
    class: &str,
    libraries: &[LibrarySpec],
    user_file: Option<&Path>,
) -> Result<String, BackendError> {
    validate_class_name(class)?;
    let mut script = String::new();
    for (i, spec) in libraries.iter().enumerate() {
        script.push_str(&load_statement(spec, &format!("omolaLib{i}"))?);
    }
    if let Some(file) = user_file {
        script.push_str(&format!(
            "omolaUser := loadFile(\"{}\");\n",
            escape_modelica_string(&file.to_string_lossy())
        ));
    }
    script.push_str(&format!(
        "print(\"OMOLA_BEGIN check\\n\" + checkModel({class}) + \"\\nOMOLA_END check\\n\");\nprint(\"OMOLA_BEGIN errors\\n\" + getErrorString() + \"\\nOMOLA_END errors\\n\");\n"
    ));
    Ok(script)
}

/// Collect `OMOLA_BEGIN key` ... `OMOLA_END key` blocks (first occurrence of each key).
pub fn parse_sections(output: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut rest = output;
    while let Some(start) = rest.find("OMOLA_BEGIN ") {
        let after = &rest[start + "OMOLA_BEGIN ".len()..];
        let Some(eol) = after.find('\n') else { break };
        let key = after[..eol].trim().to_owned();
        let body_start = &after[eol + 1..];
        let end_marker = format!("OMOLA_END {key}");
        let Some(end) = body_start.find(&end_marker) else {
            break;
        };
        let body = body_start[..end]
            .strip_suffix('\n')
            .unwrap_or(&body_start[..end]);
        out.entry(key).or_insert_with(|| body.to_owned());
        rest = &body_start[end + end_marker.len()..];
    }
    out
}

/// What a documentation link points to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTarget {
    /// `modelica://A.B.C` without a path: another class.
    Class(String),
    /// A local file (resource inside a library, or `file://`).
    File(PathBuf),
    /// `http(s)://` and `mailto:`; opened in the system browser.
    External(String),
    Unsupported(String),
}

/// Resolve an `href`/`src` from the documentation of a class.
///
/// `modelica://Pkg.Sub/rel/path` maps to `<root of Pkg>/Sub/rel/path` (directory-structured
/// packages, as in the Modelica Standard Library). Relative paths resolve against the root
/// of the class's top-level package.
pub fn resolve_link(
    href: &str,
    current_class: &str,
    roots: &BTreeMap<String, PathBuf>,
) -> LinkTarget {
    let href = href.trim();
    let lower = href.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")
    {
        return LinkTarget::External(href.into());
    }
    if let Some(path) = href.strip_prefix("file://") {
        return LinkTarget::File(PathBuf::from(path));
    }
    if lower.starts_with("modelica://") {
        let rest = &href["modelica://".len()..];
        let (host, path) = match rest.find('/') {
            Some(i) => (&rest[..i], Some(&rest[i + 1..])),
            None => (rest, None),
        };
        let host = host.split('#').next().unwrap_or(host);
        match path.filter(|p| !p.is_empty()) {
            None => return LinkTarget::Class(host.into()),
            Some(path) => {
                let mut segments = host.split('.');
                let top = segments.next().unwrap_or_default();
                let Some(root) = roots.get(top) else {
                    return LinkTarget::Unsupported(href.into());
                };
                let mut file = root.clone();
                for segment in segments {
                    file.push(segment);
                }
                for part in path.split('/').filter(|p| !p.is_empty() && *p != ".") {
                    file.push(part);
                }
                return LinkTarget::File(file);
            }
        }
    }
    if href.contains("://") {
        return LinkTarget::Unsupported(href.into());
    }
    let top = current_class.split('.').next().unwrap_or(current_class);
    match roots.get(top) {
        Some(root) => LinkTarget::File(root.join(href)),
        None => LinkTarget::Unsupported(href.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msl() -> LibrarySpec {
        LibrarySpec::new("Modelica", Some("4.1.0"))
    }

    #[test]
    fn index_script_validates_and_mentions_markers() {
        let script = library_index_script(&msl()).unwrap();
        assert!(script.contains("loadModel(Modelica, {\"4.1.0\"})"));
        assert!(script.contains("OMOLA_CLASS"));
        assert!(library_index_script(&LibrarySpec::new("A\"); system(\"x", None)).is_err());
        assert!(library_index_script(&LibrarySpec::new("Modelica", Some("1\"}"))).is_err());
        let any = library_index_script(&LibrarySpec::new("ModelicaReference", None)).unwrap();
        assert!(any.contains("loadModel(ModelicaReference);"));
    }

    #[test]
    fn parses_index_ignoring_noise() {
        let output = "\"/tmp/x/\"\ntrue\nOMOLA_BEGIN source\n/lib/Modelica 4.1.0/package.mo\nOMOLA_END source\nOMOLA_BEGIN version\n4.1.0\nOMOLA_END version\nOMOLA_CLASS\tModelica\tpackage\tModelica Standard Library\nOMOLA_CLASS\tModelica.UsersGuide\tpackage\tUser's Guide\nOMOLA_CLASS\tModelica.Blocks\tpackage\tLibrary of basic input/output control blocks\nOMOLA_CLASS\tModelica.Blocks.Continuous\tpackage\t\n";
        let lib = parse_library_index(&msl(), output).unwrap();
        assert_eq!(lib.version, "4.1.0");
        assert_eq!(
            lib.source_file,
            PathBuf::from("/lib/Modelica 4.1.0/package.mo")
        );
        assert_eq!(lib.classes.len(), 4);
        let children: Vec<_> = lib
            .children("Modelica")
            .iter()
            .map(|c| c.short_name().to_owned())
            .collect();
        assert_eq!(children, ["UsersGuide", "Blocks"]);
        assert!(lib.has_children("Modelica.Blocks"));
        assert!(!lib.has_children("Modelica.UsersGuide"));
        assert_eq!(lib.class("Modelica.Blocks.Continuous").unwrap().comment, "");
    }

    #[test]
    fn duplicate_root_class_is_listed_once() {
        // `getClassNames(recursive=true)` returns the top-level class too, so real OMC output
        // contains the root twice: once from the explicit print, once from the loop.
        let output = "OMOLA_BEGIN source\n/lib/Modelica 4.1.0/package.mo\nOMOLA_END source\nOMOLA_BEGIN version\n4.1.0\nOMOLA_END version\nOMOLA_CLASS\tModelica\tpackage\tModelica Standard Library\nOMOLA_CLASS\tModelica\tpackage\tModelica Standard Library\nOMOLA_CLASS\tModelica.Blocks\tpackage\tLibrary of basic input/output control blocks\n";
        let lib = parse_library_index(&msl(), output).unwrap();
        assert_eq!(lib.classes.len(), 2);
        assert_eq!(
            lib.classes
                .iter()
                .filter(|c| c.qualified_name == "Modelica")
                .count(),
            1
        );
        assert_eq!(lib.children("Modelica").len(), 1);
    }

    #[test]
    fn reports_missing_library() {        let err =
            parse_library_index(&msl(), "OMOLA_LOAD_FAILED\nError: no such library\n").unwrap_err();
        assert!(err.contains("Modelica 4.1.0"), "{err}");
    }

    #[test]
    fn sections_keep_inner_text_verbatim() {
        let s = parse_sections("x\nOMOLA_BEGIN info\n<p>Error: a\n\"b\"</p>\nOMOLA_END info\nOMOLA_BEGIN comment\n\nOMOLA_END comment\n");
        assert_eq!(s["info"], "<p>Error: a\n\"b\"</p>");
        assert_eq!(s["comment"], "");
    }

    #[test]
    fn icon_script_asks_for_the_icon_annotation_only() {
        let script = icon_script("Modelica.Blocks.Continuous.FirstOrder", &[msl()], None).unwrap();
        assert!(script.contains("loadModel(Modelica, {\"4.1.0\"})"));
        assert!(script.contains(
            "getModelInstanceAnnotation(Modelica.Blocks.Continuous.FirstOrder, {\"Icon\"})"
        ));
        assert!(script.contains("OMOLA_BEGIN icon"));
        // The user file is loaded after the libraries, like the other scripts.
        let with_file = icon_script("M", &[], Some(Path::new("/tmp/M.mo"))).unwrap();
        assert!(with_file.contains("loadFile(\"/tmp/M.mo\")"));
        assert!(icon_script("A\"); system(\"x", &[], None).is_err());
    }

    #[test]
    fn class_doc_parses_and_records_root() {
        let out = "OMOLA_BEGIN root\n/msl/Modelica/package.mo\nOMOLA_END root\nOMOLA_BEGIN restriction\npackage\nOMOLA_END restriction\nOMOLA_BEGIN comment\nModelica Standard Library\nOMOLA_END comment\nOMOLA_BEGIN info\n<html><p>x</p></html>\nOMOLA_END info\nOMOLA_BEGIN revisions\n\nOMOLA_END revisions\n";
        let doc = parse_class_doc("Modelica", out).unwrap();
        assert_eq!(doc.comment, "Modelica Standard Library");
        assert_eq!(doc.roots["Modelica"], PathBuf::from("/msl/Modelica"));
        assert!(
            parse_class_doc("Nope", "OMOLA_BEGIN restriction\n\nOMOLA_END restriction\n").is_err()
        );
    }

    #[test]
    fn experiment_parsing() {
        let e = parse_experiment("0.0 10.0 1e-06 500").unwrap();
        assert_eq!(e.stop_time, 10.0);
        assert_eq!(e.intervals, 500);
        assert!(parse_experiment("0 1 2").is_none());
        assert!(parse_experiment("a b c d").is_none());
    }

    #[test]
    fn check_and_text_scripts_validate_names() {
        assert!(check_script("A;B", &[], None).is_err());
        assert!(class_text_script("Modelica.Blocks", &[msl()])
            .unwrap()
            .contains("list(Modelica.Blocks)"));
    }

    #[test]
    fn resolves_links() {
        let mut roots = BTreeMap::new();
        roots.insert("Modelica".to_owned(), PathBuf::from("/msl/Modelica"));
        assert_eq!(
            resolve_link(
                "modelica://Modelica/Resources/Images/a.png",
                "Modelica",
                &roots
            ),
            LinkTarget::File(PathBuf::from("/msl/Modelica/Resources/Images/a.png"))
        );
        assert_eq!(
            resolve_link(
                "modelica://Modelica.Blocks/Resources/b.svg",
                "Modelica",
                &roots
            ),
            LinkTarget::File(PathBuf::from("/msl/Modelica/Blocks/Resources/b.svg"))
        );
        assert_eq!(
            resolve_link(
                "modelica://Modelica.UsersGuide.Overview",
                "Modelica",
                &roots
            ),
            LinkTarget::Class("Modelica.UsersGuide.Overview".into())
        );
        assert_eq!(
            resolve_link("https://www.Modelica.org", "Modelica", &roots),
            LinkTarget::External("https://www.Modelica.org".into())
        );
        assert!(matches!(
            resolve_link("modelica://Other/x.png", "Modelica", &roots),
            LinkTarget::Unsupported(_)
        ));
    }

    // --- G18: class index ---

    fn entry(name: &str) -> ClassEntry {
        ClassEntry {
            qualified_name: name.into(),
            restriction: "package".into(),
            comment: String::new(),
        }
    }

    fn small_library() -> LoadedLibrary {
        LoadedLibrary::new(
            msl(),
            "4.1.0".into(),
            PathBuf::from("/lib/Modelica/package.mo"),
            vec![
                entry("Modelica"),
                entry("Modelica.Blocks"),
                entry("Modelica.Blocks.Continuous"),
                entry("Modelica.Blocks.Sources"),
                entry("Modelica.Electrical"),
            ],
        )
    }

    #[test]
    fn the_index_answers_class_and_children() {
        let lib = small_library();
        assert_eq!(
            lib.class("Modelica.Blocks.Sources").map(|c| c.short_name()),
            Some("Sources")
        );
        let children: Vec<&str> = lib
            .children("Modelica.Blocks")
            .iter()
            .map(|c| c.short_name())
            .collect();
        assert_eq!(children, ["Continuous", "Sources"], "declaration order");
        // The allocation-free iterator agrees with `children`.
        assert_eq!(lib.children_iter("Modelica.Blocks").count(), 2);
        assert_eq!(
            lib.children("Modelica")
                .iter()
                .map(|c| c.short_name())
                .collect::<Vec<_>>(),
            ["Blocks", "Electrical"]
        );
        assert!(lib.has_children("Modelica.Blocks"));
        assert!(!lib.has_children("Modelica.Blocks.Sources"));
        assert!(!lib.has_children("Modelica.Missing"));
    }

    #[test]
    fn unknown_names_are_empty_and_do_not_panic() {
        let lib = small_library();
        assert!(lib.class("Modelica.Nope").is_none());
        assert!(lib.children("Modelica.Nope").is_empty());
        assert!(lib.children_iter("Modelica.Nope").next().is_none());
        assert!(lib.children("").is_empty());
    }

    #[test]
    fn a_child_of_a_missing_parent_belongs_to_no_node() {
        // `getClassNames(recursive=true)` always lists the intermediate classes, but a
        // hand-written index may not: such a child must simply not be reachable.
        let lib = LoadedLibrary::new(
            msl(),
            "4.1.0".into(),
            PathBuf::from("/lib/Modelica/package.mo"),
            vec![entry("Modelica"), entry("Modelica.A.B")],
        );
        assert!(lib.children("Modelica").is_empty());
        assert!(lib.children("Modelica.A").is_empty());
        assert!(lib.class("Modelica.A.B").is_some());
        assert!(!lib.has_children("Modelica"));
    }
}

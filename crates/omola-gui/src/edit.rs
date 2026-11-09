//! Diagram edits as OpenModelica scripting API calls (G28).
//!
//! Omola has no Modelica parser. A diagram edit is therefore turned into an OMC script that
//! rewrites one class, and the rewritten class is read back with `list()` so the editor text
//! stays the single source of truth. Nothing here talks to OMC: [`edit_script`] is a pure
//! function from [`EditOp`]s to script text, which keeps the exact API calls testable without
//! a compiler.
//!
//! See `docs/issues/gui/G28-model-edit-backend.md`.

use std::fmt::Write as _;

/// Marker printed before the rewritten class text.
pub const BEGIN_MARKER: &str = "OMOLA_EDIT_BEGIN";
/// Marker printed after the rewritten class text.
pub const END_MARKER: &str = "OMOLA_EDIT_END";
/// Marker printed instead of the class text when an API call failed.
pub const FAIL_MARKER: &str = "OMOLA_EDIT_FAIL";

/// One edit of a class.
///
/// `SetPlacement` carries the component's type name instead of looking it up with
/// `getComponents`: the Diagram layer already knows it (`diagram::Component::type_name`), and
/// that saves a round trip whose answer would be a fragile string to parse.
#[allow(dead_code)] // the Diagram layer builds these from G29 on; until then only the tests do
#[derive(Debug, Clone, PartialEq)]
pub enum EditOp {
    /// Move/resize/rotate an existing component: rewrites its `Placement` annotation.
    SetPlacement {
        component: String,
        type_name: String,
        extent: [[f64; 2]; 2],
        origin: [f64; 2],
        rotation: f64,
    },
    /// Place a new component of an existing type.
    AddComponent {
        name: String,
        type_name: String,
        extent: [[f64; 2]; 2],
        origin: [f64; 2],
    },
    DeleteComponent {
        name: String,
    },
    RenameComponent {
        old: String,
        new: String,
    },
    /// Connect two connectors (`a.y` to `b.u`) with an explicit polyline.
    AddConnection {
        from: String,
        to: String,
        points: Vec<[f64; 2]>,
    },
    DeleteConnection {
        from: String,
        to: String,
    },
    /// `path = value` modifier; `value = None` removes the modifier again.
    SetModifier {
        path: String,
        value: Option<String>,
    },
}

impl EditOp {
    /// Short description used in the failure message, e.g. `addComponent(k)`.
    fn message(&self) -> String {
        match self {
            EditOp::SetPlacement { component, .. } => format!("updateComponent({component})"),
            EditOp::AddComponent { name, .. } => format!("addComponent({name})"),
            EditOp::DeleteComponent { name } => format!("deleteComponent({name})"),
            EditOp::RenameComponent { old, new } => {
                format!("renameComponentInClass({old} -> {new})")
            }
            EditOp::AddConnection { from, to, .. } => format!("addConnection({from} -> {to})"),
            EditOp::DeleteConnection { from, to } => format!("deleteConnection({from} -> {to})"),
            EditOp::SetModifier { path, .. } => format!("setElementModifierValue({path})"),
        }
    }

    /// The `omolaOk := <API call>;` line that performs this edit in `class`.
    fn statement(&self, class: &str) -> Result<String, String> {
        Ok(match self {
            EditOp::SetPlacement {
                component,
                type_name,
                extent,
                origin,
                rotation,
            } => {
                check_reference("component name", component)?;
                check_reference("type name", type_name)?;
                let placement = placement(*extent, *origin, *rotation)?;
                format!("omolaOk := updateComponent({component}, {type_name}, {class}, annotate={placement});")
            }
            EditOp::AddComponent {
                name,
                type_name,
                extent,
                origin,
            } => {
                check_reference("component name", name)?;
                check_reference("type name", type_name)?;
                let placement = placement(*extent, *origin, 0.0)?;
                format!(
                    "omolaOk := addComponent({name}, {type_name}, {class}, annotate={placement});"
                )
            }
            EditOp::DeleteComponent { name } => {
                check_reference("component name", name)?;
                format!("omolaOk := deleteComponent({name}, {class});")
            }
            EditOp::RenameComponent { old, new } => {
                check_identifier("component name", old)?;
                check_identifier("component name", new)?;
                format!("omolaOk := renameComponentInClass({class}, {old}, {new});")
            }
            EditOp::AddConnection { from, to, points } => {
                check_reference("connector", from)?;
                check_reference("connector", to)?;
                let line = line(points)?;
                format!("omolaOk := addConnection({from}, {to}, {class}, annotate={line});")
            }
            EditOp::DeleteConnection { from, to } => {
                check_reference("connector", from)?;
                check_reference("connector", to)?;
                format!("omolaOk := deleteConnection({from}, {to}, {class});")
            }
            EditOp::SetModifier { path, value } => {
                check_reference("modifier path", path)?;
                let code = match value {
                    Some(value) => format!("$Code(={})", modifier_value(value)?),
                    None => "$Code()".to_owned(),
                };
                format!("omolaOk := setElementModifierValue({class}, {path}, {code});")
            }
        })
    }
}

/// Script that applies `ops` to `class` and prints the rewritten class between the markers.
///
/// The caller prepends the `loadModel`/`loadFile` lines ([`crate::library::load_prefix`]);
/// everything here runs against the class as OMC currently holds it.
pub fn edit_script(class: &str, ops: &[EditOp]) -> Result<String, String> {
    check_reference("class name", class)?;
    if ops.is_empty() {
        return Err("no edit was requested".into());
    }
    let mut script = String::from("omolaEditOk := true;\nomolaEditMsg := \"\";\n");
    for op in ops {
        // Validation errors must surface before a single API call is made.
        let statement = op.statement(class)?;
        let message = omola_core::escape_modelica_string(&op.message());
        let _ = write!(
            script,
            "{statement}\n\
             if not omolaOk then\n  \
               omolaEditOk := false;\n  \
               omolaEditMsg := omolaEditMsg + \"{message}: \" + getErrorString();\n\
             end if;\n"
        );
    }
    let _ = write!(
        script,
        "if omolaEditOk then\n  \
           print(\"{BEGIN_MARKER}\\n\" + list({class}) + \"\\n{END_MARKER}\\n\");\n\
         else\n  \
           print(\"{FAIL_MARKER} \" + omolaEditMsg + \"\\n\");\n\
         end if;\n"
    );
    Ok(script)
}

/// Rewritten class text printed by [`edit_script`], or the OMC error message.
pub fn edited_text(output: &str) -> Result<String, String> {
    let begin = output.find(BEGIN_MARKER);
    if let Some(fail) = output.find(FAIL_MARKER) {
        // A failure marker inside the class text itself is not a failure, so it only counts
        // when it appears before the begin marker.
        if begin.is_none_or(|begin| fail < begin) {
            let message = output[fail + FAIL_MARKER.len()..].trim();
            return Err(if message.is_empty() {
                "OpenModelica refused the edit".into()
            } else {
                message.into()
            });
        }
    }
    let begin = begin.ok_or_else(|| "OpenModelica returned no edited class text".to_owned())?;
    let body = &output[begin + BEGIN_MARKER.len()..];
    let body = body.trim_start_matches('\n');
    let end = body
        .find(END_MARKER)
        .ok_or_else(|| "OpenModelica returned an incomplete class text".to_owned())?;
    Ok(body[..end].trim_end_matches('\n').to_owned())
}

/// Whether a class of this `restriction` can be dropped into a diagram (G30).
///
/// Packages, functions and types have no instance to place. Records do, even though the
/// Diagram layer draws nothing for them, so they are allowed.
pub fn draggable(restriction: &str) -> bool {
    !matches!(
        restriction.trim(),
        "package" | "function" | "operator function" | "type"
    )
}

/// A component name for `type_short` that is not in `existing` yet (G30).
///
/// The first letter is lowercased (`Constant` -> `constant`) and a number is appended when
/// the name is taken (`constant1`, `constant2`). Only the first character is touched, so
/// `PID` becomes `pID`; that differs from Dymola, which lowercases the whole acronym, and it
/// is fine as long as the result is a valid Modelica identifier.
pub fn unique_component_name(type_short: &str, existing: &[String]) -> String {
    let mut chars = type_short.chars();
    let base: String = match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => "component".to_owned(),
    };
    if !existing.iter().any(|name| name == &base) {
        return base;
    }
    // At most `existing.len()` numbers are taken, so one more always finds a free name.
    for suffix in 1..=existing.len() + 1 {
        let candidate = format!("{base}{suffix}");
        if !existing.iter().any(|name| name == &candidate) {
            return candidate;
        }
    }
    unreachable!("one of the first `existing.len() + 1` names is free")
}

/// Why a component name cannot be used, or `Ok(())` when it can (G31).
///
/// `existing` holds the other component names of the class; the old name of a rename must
/// be left out so the dialog's starting text is not reported as a clash.
pub fn check_component_name(name: &str, existing: &[String]) -> Result<(), &'static str> {
    if !crate::model::is_identifier(name) || crate::highlight::is_keyword(name) {
        return Err("Not a valid Modelica identifier");
    }
    if existing.iter().any(|other| other == name) {
        return Err("Name already used");
    }
    Ok(())
}

/// `Placement(visible=true, transformation=transformation(...))` for an annotation argument.
fn placement(extent: [[f64; 2]; 2], origin: [f64; 2], rotation: f64) -> Result<String, String> {
    let [[x1, y1], [x2, y2]] = extent;
    Ok(format!(
        "Placement(visible=true, transformation=transformation(origin={{{},{}}}, extent={{{{{},{}}},{{{},{}}}}}, rotation={}))",
        number(origin[0])?,
        number(origin[1])?,
        number(x1)?,
        number(y1)?,
        number(x2)?,
        number(y2)?,
        number(rotation)?
    ))
}

/// `Line(points={{x,y},{x,y},...})` for a connection annotation.
fn line(points: &[[f64; 2]]) -> Result<String, String> {
    if points.len() < 2 {
        return Err("a connection needs at least two points".into());
    }
    let mut text = String::new();
    for (i, [x, y]) in points.iter().enumerate() {
        if i > 0 {
            text.push(',');
        }
        let _ = write!(text, "{{{},{}}}", number(*x)?, number(*y)?);
    }
    Ok(format!("Line(points={{{text}}})"))
}

/// A `f64` as a Modelica number literal.
///
/// `format!("{}", x)` never uses an exponent, so `1e-5` would become `0.00001` and `1e-300`
/// a 300-digit string. Values outside `[1e-4, 1e16)` are therefore written as `1e-5`.
pub fn number(x: f64) -> Result<String, String> {
    if !x.is_finite() {
        return Err(format!("{x} cannot be written as a Modelica number"));
    }
    if x == 0.0 {
        // Also normalises -0.0, which `Display` would print as "-0".
        return Ok("0".into());
    }
    let magnitude = x.abs();
    if (1e-4..1e16).contains(&magnitude) {
        Ok(format!("{x}"))
    } else {
        Ok(format!("{x:e}"))
    }
}

/// Reject a `value` that would end the `$Code(...)` argument or the statement.
///
/// A modifier value is a Modelica expression, so it cannot be validated in general; but a
/// `;`, a line break or a `"` would let it escape the script line it is embedded in. Values
/// that are string literals need their own issue.
fn modifier_value(value: &str) -> Result<&str, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("a modifier value must not be empty".into());
    }
    if value.contains(';') || value.contains('\n') || value.contains('\r') || value.contains('"') {
        return Err(format!(
            "the value {value} must not contain ';', a line break or '\"'"
        ));
    }
    Ok(value)
}

fn check_identifier(kind: &str, name: &str) -> Result<(), String> {
    if is_reference_part(name) {
        Ok(())
    } else {
        Err(format!(
            "{kind} {name} is not a Modelica identifier (letters, digits and underscores)"
        ))
    }
}

/// A dotted reference such as `a.b` or `inertia1.flange_a[1]`.
fn check_reference(kind: &str, name: &str) -> Result<(), String> {
    if !name.is_empty() && name.split('.').all(is_reference_part) {
        Ok(())
    } else {
        Err(format!(
            "{kind} {name} is not a Modelica reference (dotted identifiers, optional [n])"
        ))
    }
}

fn is_reference_part(part: &str) -> bool {
    let bytes = part.as_bytes();
    let mut i = match bytes.first() {
        Some(b) if *b == b'_' || b.is_ascii_alphabetic() => 1,
        _ => return false,
    };
    while i < bytes.len() && (bytes[i] == b'_' || bytes[i].is_ascii_alphanumeric()) {
        i += 1;
    }
    while i < bytes.len() {
        if bytes[i] != b'[' {
            return false;
        }
        i += 1;
        let start = i;
        while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b',') {
            i += 1;
        }
        let subscript = &part[start..i];
        if subscript.is_empty() || subscript.starts_with(',') || subscript.ends_with(',') {
            return false;
        }
        if bytes.get(i) != Some(&b']') {
            return false;
        }
        i += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extent() -> [[f64; 2]; 2] {
        [[-10.0, -10.0], [10.0, 10.0]]
    }

    #[test]
    fn add_component_writes_a_placement_annotation() {
        let ops = [EditOp::AddComponent {
            name: "k".into(),
            type_name: "Modelica.Blocks.Sources.Constant".into(),
            extent: extent(),
            origin: [0.0, 0.0],
        }];
        let script = edit_script("A", &ops).unwrap();
        assert!(
            script.contains(
                "omolaOk := addComponent(k, Modelica.Blocks.Sources.Constant, A, \
                 annotate=Placement(visible=true, transformation=transformation(origin={0,0}, \
                 extent={{-10,-10},{10,10}}, rotation=0)));"
            ),
            "{script}"
        );
        assert!(script.contains("if not omolaOk then"));
        assert!(script.contains(r#"print("OMOLA_EDIT_BEGIN\n" + list(A) + "\nOMOLA_EDIT_END\n");"#));
        assert!(script.contains(r#"print("OMOLA_EDIT_FAIL " + omolaEditMsg + "\n");"#));
    }

    #[test]
    fn moving_a_component_rewrites_its_placement() {
        let ops = [EditOp::SetPlacement {
            component: "inertia1".into(),
            type_name: "Modelica.Mechanics.Rotational.Components.Inertia".into(),
            extent: [[-20.0, -15.0], [20.0, 15.0]],
            origin: [40.0, -12.5],
            rotation: 90.0,
        }];
        let script = edit_script("Drive", &ops).unwrap();
        assert!(
            script.contains(
                "omolaOk := updateComponent(inertia1, \
                 Modelica.Mechanics.Rotational.Components.Inertia, Drive, \
                 annotate=Placement(visible=true, transformation=transformation(origin={40,-12.5}, \
                 extent={{-20,-15},{20,15}}, rotation=90)));"
            ),
            "{script}"
        );
    }

    #[test]
    fn deleting_and_renaming_use_the_component_api() {
        let ops = [
            EditOp::DeleteComponent { name: "k".into() },
            EditOp::RenameComponent {
                old: "k".into(),
                new: "gain".into(),
            },
        ];
        let script = edit_script("A", &ops).unwrap();
        assert!(
            script.contains("omolaOk := deleteComponent(k, A);"),
            "{script}"
        );
        assert!(
            script.contains("omolaOk := renameComponentInClass(A, k, gain);"),
            "{script}"
        );
        // The failure message names the operation and keeps the API call out of the text.
        assert!(script.contains(r#""deleteComponent(k): ""#), "{script}");
    }

    #[test]
    fn a_connection_is_added_with_its_points() {
        let ops = [EditOp::AddConnection {
            from: "inertia1.flange_a".into(),
            to: "spring.flange_b".into(),
            points: vec![[-10.0, 0.0], [0.0, 0.0], [0.0, 12.5]],
        }];
        let script = edit_script("Drive", &ops).unwrap();
        assert!(
            script.contains(
                "omolaOk := addConnection(inertia1.flange_a, spring.flange_b, Drive, \
                 annotate=Line(points={{-10,0},{0,0},{0,12.5}}));"
            ),
            "{script}"
        );
        let none = [EditOp::AddConnection {
            from: "a.y".into(),
            to: "b.u".into(),
            points: vec![[0.0, 0.0]],
        }];
        assert!(edit_script("A", &none).is_err());
    }

    #[test]
    fn a_modifier_is_set_and_removed() {
        let set = [EditOp::SetModifier {
            path: "k.k".into(),
            value: Some("2.5".into()),
        }];
        let script = edit_script("A", &set).unwrap();
        assert!(
            script.contains("omolaOk := setElementModifierValue(A, k.k, $Code(=2.5));"),
            "{script}"
        );
        let removed = [EditOp::SetModifier {
            path: "k.k".into(),
            value: None,
        }];
        let script = edit_script("A", &removed).unwrap();
        assert!(
            script.contains("omolaOk := setElementModifierValue(A, k.k, $Code());"),
            "{script}"
        );
    }

    #[test]
    fn invalid_names_are_rejected() {
        for name in [
            "a;b", "x y", "", "q\"", "1abc", "a..b", "a[]", "a[1,]", "a[1",
        ] {
            let op = EditOp::DeleteComponent { name: name.into() };
            assert!(
                edit_script("A", &[op]).is_err(),
                "{name:?} must not reach the script"
            );
        }
        // A dotted reference and an array subscript are fine.
        let ok = EditOp::DeleteConnection {
            from: "bus[1].pin".into(),
            to: "ground.p".into(),
        };
        assert!(edit_script("A", &[ok]).is_ok());
    }

    #[test]
    fn a_modifier_value_must_not_break_the_script() {
        for value in ["1; deleteComponent(x, A)", "1\nend A;", "1\"2", "   "] {
            let op = EditOp::SetModifier {
                path: "k.k".into(),
                value: Some(value.into()),
            };
            assert!(
                edit_script("A", &[op]).is_err(),
                "{value:?} must be rejected"
            );
        }
    }

    #[test]
    fn numbers_use_modelica_literals() {
        assert_eq!(number(0.1).unwrap(), "0.1");
        assert_eq!(number(-10.0).unwrap(), "-10");
        assert_eq!(number(1e-5).unwrap(), "1e-5");
        assert_eq!(number(-2.5e-7).unwrap(), "-2.5e-7");
        assert_eq!(number(1e20).unwrap(), "1e20");
        assert_eq!(number(0.0).unwrap(), "0");
        assert_eq!(number(-0.0).unwrap(), "0");
        assert!(number(f64::NAN).is_err());
        assert!(number(f64::INFINITY).is_err());
        assert!(number(f64::NEG_INFINITY).is_err());
    }

    #[test]
    fn an_empty_edit_list_is_an_error() {
        assert!(edit_script("A", &[]).is_err());
        assert!(edit_script("A.B", &[EditOp::DeleteComponent { name: "k".into() }]).is_ok());
        assert!(edit_script("", &[EditOp::DeleteComponent { name: "k".into() }]).is_err());
    }

    #[test]
    fn the_edited_class_text_is_read_back() {
        let output =
            "omolaEditOk := true;\nOMOLA_EDIT_BEGIN\nmodel A\n  Real x;\nend A;\nOMOLA_EDIT_END\n";
        assert_eq!(edited_text(output).unwrap(), "model A\n  Real x;\nend A;");
    }

    #[test]
    fn a_failed_edit_reports_the_omc_error() {
        let output = "OMOLA_EDIT_FAIL addComponent(k): Error: Class A not found\n";
        let message = edited_text(output).unwrap_err();
        assert!(message.contains("Class A not found"), "{message}");
        assert!(edited_text("nothing at all").is_err());
        // A failure marker after the begin marker is part of the class text, not a failure.
        let inside = "OMOLA_EDIT_BEGIN\nmodel A\n  String s = \"OMOLA_EDIT_FAIL\";\nend A;\nOMOLA_EDIT_END\n";
        assert!(edited_text(inside).unwrap().contains("OMOLA_EDIT_FAIL"));
    }

    #[test]
    fn a_new_component_name_is_lowercase_and_unique() {
        let names = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(unique_component_name("Constant", &[]), "constant");
        assert_eq!(
            unique_component_name("Constant", &names(&["constant"])),
            "constant1"
        );
        assert_eq!(
            unique_component_name("Constant", &names(&["constant", "constant1"])),
            "constant2"
        );
        // A gap is filled instead of always growing.
        assert_eq!(
            unique_component_name("Gain", &names(&["gain", "gain2"])),
            "gain1"
        );
        // Only the first letter is lowercased.
        assert_eq!(unique_component_name("PID", &[]), "pID");
        assert_eq!(unique_component_name("", &[]), "component");
    }

    #[test]
    fn only_placeable_restrictions_can_be_dropped() {
        for restriction in [
            "model",
            "block",
            "class",
            "connector",
            "expandable connector",
            "record",
        ] {
            assert!(draggable(restriction), "{restriction} should be placeable");
        }
        for restriction in ["package", "function", "operator function", "type", " type "] {
            assert!(
                !draggable(restriction),
                "{restriction} should not be placeable"
            );
        }
    }

    #[test]
    fn a_component_name_must_be_a_free_identifier() {
        let taken = vec!["gain".to_string(), "k".to_string()];
        assert_eq!(check_component_name("abc", &taken), Ok(()));
        assert_eq!(check_component_name("_x1", &taken), Ok(()));
        for bad in ["1a", "a b", "", "'quoted'", "model"] {
            assert_eq!(
                check_component_name(bad, &taken),
                Err("Not a valid Modelica identifier"),
                "{bad} should be rejected"
            );
        }
        // `der` is a builtin operator, not a reserved word, so it is a legal name.
        assert_eq!(check_component_name("der", &taken), Ok(()));
        assert_eq!(
            check_component_name("gain", &taken),
            Err("Name already used")
        );
    }
}

//! Safe Modelica edit-script generation for graphical edits.

#[derive(Debug, Clone, PartialEq)]
pub enum EditOp {
    SetPlacement {
        component: String,
        type_name: String,
        extent: [[f64; 2]; 2],
        origin: [f64; 2],
        rotation: f64,
    },
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
    AddConnection {
        from: String,
        to: String,
        points: Vec<[f64; 2]>,
    },
    DeleteConnection {
        from: String,
        to: String,
    },
    SetModifier {
        path: String,
        value: Option<String>,
    },
    /// Store the simulation setup as the class's `experiment` annotation (G48).
    ///
    /// OMC replaces only the `experiment` element; other class annotations are kept.
    SetExperiment {
        start: f64,
        stop: f64,
        tolerance: f64,
        interval: f64,
    },
}

fn name(s: &str) -> bool {
    let mut expect_ident = true;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if expect_ident {
            if !(c == '_' || c.is_ascii_alphabetic()) {
                return false;
            }
            while chars
                .peek()
                .is_some_and(|c| *c == '_' || c.is_ascii_alphanumeric())
            {
                chars.next();
            }
            expect_ident = false;
        } else if c == '.' {
            expect_ident = true;
        } else if c == '[' {
            let mut digits = 0;
            while chars.peek().is_some_and(|c| c.is_ascii_digit()) {
                chars.next();
                digits += 1;
            }
            if digits == 0 || chars.next() != Some(']') {
                return false;
            }
        } else {
            return false;
        }
    }
    !expect_ident
}
fn n(v: f64) -> Result<String, String> {
    if !v.is_finite() {
        return Err("placement coordinates must be finite".into());
    }
    Ok(format!("{v}"))
}
fn xy(p: [f64; 2]) -> Result<String, String> {
    Ok(format!("{{{}, {}}}", n(p[0])?, n(p[1])?))
}
fn extent(e: [[f64; 2]; 2]) -> Result<String, String> {
    Ok(format!("{{{}, {}}}", xy(e[0])?, xy(e[1])?))
}
fn validate(s: &str) -> Result<(), String> {
    if name(s) {
        Ok(())
    } else {
        Err(format!("invalid Modelica name: {s}"))
    }
}

fn is_modelica_string_literal(value: &str) -> bool {
    let mut chars = value.chars().peekable();
    if chars.next() != Some('"') {
        return false;
    }
    let mut escaped = false;
    let mut closed = false;
    for c in chars.by_ref() {
        if escaped {
            if !matches!(c, '"' | '\\' | 'n' | 'r' | 't') {
                return false;
            }
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            closed = true;
            break;
        } else if matches!(c, '\n' | '\r') {
            return false;
        }
    }
    closed && !escaped && chars.next().is_none()
}

/// Build a checked OMC script. User strings are restricted to Modelica identifiers and expressions.
pub fn edit_script(class: &str, ops: &[EditOp]) -> Result<String, String> {
    validate(class)?;
    let mut s = String::new();
    for (i, op) in ops.iter().enumerate() {
        let (call, ok) = match op {
            EditOp::SetPlacement {
                component,
                type_name,
                extent: e,
                origin,
                rotation,
            } => {
                validate(component)?;
                validate(type_name)?;
                (format!("updateComponent({component}, {type_name}, {class}, annotate=Placement(visible=true, transformation=transformation(origin={}, extent={}, rotation={})))", xy(*origin)?, extent(*e)?, n(*rotation)?), format!("omolaEdit{i}") )
            }
            EditOp::AddComponent {
                name,
                type_name,
                extent: e,
                origin,
            } => {
                validate(name)?;
                validate(type_name)?;
                (format!("addComponent({name}, {type_name}, {class}, annotate=Placement(visible=true, transformation=transformation(origin={}, extent={}, rotation=0)))", xy(*origin)?, extent(*e)?), format!("omolaEdit{i}") )
            }
            EditOp::DeleteComponent { name } => {
                validate(name)?;
                (
                    format!("deleteComponent({name}, {class})"),
                    format!("omolaEdit{i}"),
                )
            }
            EditOp::RenameComponent { old, new } => {
                validate(old)?;
                validate(new)?;
                (
                    // Unlike add/delete/update APIs, OpenModelica returns the affected class
                    // paths (TypeName[:]) from renameComponentInClass. Reduce that result to
                    // a Boolean so the common edit-result check below accepts the call and
                    // still reports an empty result as a failed edit.
                    format!("size(renameComponentInClass({class}, {old}, {new}), 1) > 0"),
                    format!("omolaEdit{i}"),
                )
            }
            EditOp::AddConnection { from, to, points } => {
                validate(from)?;
                validate(to)?;
                let pts = points
                    .iter()
                    .map(|p| xy(*p))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ");
                (
                    format!(
                        "addConnection({from}, {to}, {class}, annotate=Line(points={{{pts}}}))"
                    ),
                    format!("omolaEdit{i}"),
                )
            }
            EditOp::DeleteConnection { from, to } => {
                validate(from)?;
                validate(to)?;
                (
                    format!("deleteConnection({from}, {to}, {class})"),
                    format!("omolaEdit{i}"),
                )
            }
            EditOp::SetModifier { path, value } => {
                validate(path)?;
                let code = match value {
                    Some(v) if is_modelica_string_literal(v) => format!("$Code(={v})"),
                    Some(v) if !v.contains([';', '\n', '\r', '"']) => format!("$Code(={v})"),
                    Some(_) => {
                        return Err(
                            "modifier value cannot contain semicolons, quotes, or newlines".into(),
                        )
                    }
                    None => {
                        return Err(
                            "clearing a modifier is unsupported; set the parameter's declared default explicitly".into(),
                        )
                    }
                };
                (
                    format!("setElementModifierValue({class}, {path}, {code})"),
                    format!("omolaEdit{i}"),
                )
            }
            EditOp::SetExperiment {
                start,
                stop,
                tolerance,
                interval,
            } => {
                let all = [*start, *stop, *tolerance, *interval];
                if all.iter().any(|v| !v.is_finite()) {
                    return Err("experiment values must be finite".into());
                }
                if stop <= start {
                    return Err("experiment StopTime must be greater than StartTime".into());
                }
                if *tolerance <= 0.0 || *interval <= 0.0 {
                    return Err("experiment Tolerance and Interval must be positive".into());
                }
                (
                    format!("addClassAnnotation({class}, annotate=experiment(StartTime={start}, StopTime={stop}, Tolerance={tolerance}, Interval={interval}))"),
                    format!("omolaEdit{i}"),
                )
            }
        };
        s.push_str(&format!("{ok} := {call};\nif not {ok} then print(\"OMOLA_EDIT_FAIL \" + getErrorString()); end if;\n"));
    }
    s.push_str(&format!(
        "print(\"OMOLA_EDIT_BEGIN\\n\" + list({class}) + \"\\nOMOLA_EDIT_END\\n\");\n"
    ));
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generates_all_operation_shapes_and_markers() {
        let ops = vec![
            EditOp::SetPlacement {
                component: "k".into(),
                type_name: "Modelica.Blocks.Sources.Constant".into(),
                extent: [[-10., -10.], [10., 10.]],
                origin: [0.1, -10.],
                rotation: 90.,
            },
            EditOp::AddComponent {
                name: "k".into(),
                type_name: "Modelica.Blocks.Sources.Constant".into(),
                extent: [[-10., -10.], [10., 10.]],
                origin: [1e-5, 0.],
            },
            EditOp::DeleteComponent { name: "k".into() },
            EditOp::RenameComponent {
                old: "k".into(),
                new: "k1".into(),
            },
            EditOp::AddConnection {
                from: "a.y".into(),
                to: "b.u".into(),
                points: vec![[0., 0.]],
            },
            EditOp::DeleteConnection {
                from: "a.y".into(),
                to: "b.u".into(),
            },
            EditOp::SetModifier {
                path: "k.k".into(),
                value: Some("2.5".into()),
            },
        ];
        let s = edit_script("Pkg.A", &ops).unwrap();
        for api in [
            "updateComponent",
            "addComponent",
            "deleteComponent",
            "renameComponentInClass",
            "addConnection",
            "deleteConnection",
            "setElementModifierValue",
        ] {
            assert!(s.contains(api));
        }
        assert!(s.contains("OMOLA_EDIT_BEGIN"));
        assert!(s.contains("1e-5") || s.contains("0.00001"));
    }
    #[test]
    fn rejects_invalid_inputs() {
        assert!(edit_script("A; system(1)", &[]).is_err());
        assert!(edit_script(
            "A",
            &[EditOp::DeleteComponent {
                name: "x; y".into()
            }]
        )
        .is_err());
        assert!(edit_script(
            "A",
            &[EditOp::SetModifier {
                path: "x.y".into(),
                value: Some("1; system(1)".into())
            }]
        )
        .is_err());
        assert!(edit_script(
            "A",
            &[EditOp::SetPlacement {
                component: "x".into(),
                type_name: "T".into(),
                extent: [[0., 0.], [1., 1.]],
                origin: [f64::NAN, 0.],
                rotation: 0.
            }]
        )
        .is_err());
    }

    #[test]
    fn rejects_modifier_removal_instead_of_emitting_invalid_omc_syntax() {
        assert!(edit_script(
            "A",
            &[EditOp::SetModifier {
                path: "plant.T".into(),
                value: None,
            }]
        )
        .unwrap_err()
        .contains("declared default"));
    }

    #[test]
    fn accepts_escaped_string_literals_but_rejects_trailing_script() {
        let op = EditOp::SetModifier {
            path: "plant.label".into(),
            value: Some("\"a\\\"b\\\\c; d\"".into()),
        };
        let script = edit_script("A", &[op]).unwrap();
        assert!(script.contains("$Code(=\"a\\\"b\\\\c; d\")"));
        assert!(edit_script(
            "A",
            &[EditOp::SetModifier {
                path: "plant.label".into(),
                value: Some("\"safe\"; print(1)".into()),
            }]
        )
        .is_err());
    }

    #[test]
    fn set_experiment_script_calls_add_class_annotation() {
        let script = edit_script(
            "Demo.M",
            &[EditOp::SetExperiment {
                start: 0.0,
                stop: 2.5,
                tolerance: 1e-6,
                interval: 0.005,
            }],
        )
        .unwrap();
        assert!(script.contains(
            "addClassAnnotation(Demo.M, annotate=experiment(StartTime=0, StopTime=2.5, Tolerance=0.000001, Interval=0.005))"
        ), "{script}");
        assert!(script.contains("OMOLA_EDIT_BEGIN"));
    }

    #[test]
    fn set_experiment_rejects_non_finite_and_reversed_times() {
        let op = |start, stop, tolerance, interval| EditOp::SetExperiment {
            start,
            stop,
            tolerance,
            interval,
        };
        for bad in [
            op(0.0, f64::NAN, 1e-6, 0.1),
            op(0.0, f64::INFINITY, 1e-6, 0.1),
            op(1.0, 1.0, 1e-6, 0.1),
            op(2.0, 1.0, 1e-6, 0.1),
            op(0.0, 1.0, 0.0, 0.1),
            op(0.0, 1.0, 1e-6, 0.0),
        ] {
            assert!(
                edit_script("M", std::slice::from_ref(&bad)).is_err(),
                "{bad:?}"
            );
        }
    }
}

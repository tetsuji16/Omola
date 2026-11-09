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
                    format!("renameComponentInClass({class}, {old}, {new})"),
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
}

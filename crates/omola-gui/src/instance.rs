//! Component tree of a class, read from OMC's `getModelInstance` JSON.
//!
//! OpenModelica 1.27.1 prints the instance as a JSON document whose structure is not
//! documented in the OpenModelica users guide. The parser here is written against the
//! real output captured in `tests/fixtures/instance_MSLFirstOrder.json` and
//! `tests/fixtures/instance_BouncingBall.json`; `docs/omc-model-instance-json.md`
//! records what those files contain.
//!
//! The tree is deliberately shallow (`max_depth`): the JSON inlines the full definition
//! of every component's class, so a deep tree would describe the whole Modelica Standard
//! Library instead of the model the user opened.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamRow {
    pub name: String,
    pub type_name: String,
    pub default_value: Option<String>,
    pub current_value: Option<String>,
    pub unit: String,
    pub comment: String,
    /// Modelica expressions to use for enumeration literals.
    pub choices: Vec<String>,
}

fn decode_modelica_string(value: String) -> String {
    let Some(inner) = value.strip_prefix('"').and_then(|s| s.strip_suffix('"')) else {
        return value;
    };
    let mut decoded = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') => decoded.push('"'),
                Some('\\') => decoded.push('\\'),
                Some('n') => decoded.push('\n'),
                Some('r') => decoded.push('\r'),
                Some('t') => decoded.push('\t'),
                Some(other) => {
                    decoded.push('\\');
                    decoded.push(other);
                }
                None => decoded.push('\\'),
            }
        } else {
            decoded.push(c);
        }
    }
    decoded
}

fn encode_modelica_string(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len() + 2);
    encoded.push('"');
    for c in value.chars() {
        match c {
            '"' => encoded.push_str("\\\""),
            '\\' => encoded.push_str("\\\\"),
            '\n' => encoded.push_str("\\n"),
            '\r' => encoded.push_str("\\r"),
            '\t' => encoded.push_str("\\t"),
            _ => encoded.push(c),
        }
    }
    encoded.push('"');
    encoded
}

pub fn parse_parameter_rows(type_json: &Value, component_json: Option<&Value>) -> Vec<ParamRow> {
    let modifiers = component_json.and_then(|c| c.get("modifiers"));
    let mut out = Vec::new();
    collect_parameters(
        type_json,
        modifiers,
        &mut out,
        &mut std::collections::HashSet::new(),
    );
    out
}

fn collect_parameters(
    node: &Value,
    modifiers: Option<&Value>,
    out: &mut Vec<ParamRow>,
    seen: &mut std::collections::HashSet<String>,
) {
    let Some(elements) = node.get("elements").and_then(Value::as_array) else {
        return;
    };
    for element in elements {
        match element.get("$kind").and_then(Value::as_str) {
            Some(EXTENDS) => {
                if let Some(base) = element.get("baseClass") {
                    collect_parameters(base, modifiers, out, seen);
                }
            }
            Some(COMPONENT) => {
                let Some(name) = element.get("name").and_then(Value::as_str) else {
                    continue;
                };
                if element
                    .pointer("/prefixes/variability")
                    .and_then(Value::as_str)
                    != Some("parameter")
                    || !seen.insert(name.to_owned())
                {
                    continue;
                }
                let Some(declared) = element.get("type") else {
                    continue;
                };
                let type_name = match declared {
                    Value::String(s) => s.clone(),
                    Value::Object(_) => declared
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    _ => continue,
                };
                let mut default_value = element
                    .get("modifiers")
                    .and_then(modifier_default)
                    .or_else(|| {
                        element
                            .pointer("/value/binding")
                            .and_then(binding_expression)
                    });
                let mut current_value = modifiers
                    .and_then(|m| m.get(name))
                    .and_then(binding_expression);
                if type_name == "String" {
                    default_value = default_value.map(decode_modelica_string);
                    current_value = current_value.map(decode_modelica_string);
                }
                let unit = element
                    .get("modifiers")
                    .and_then(|m| m.get("unit"))
                    .and_then(binding_expression)
                    .or_else(|| type_unit(declared))
                    .unwrap_or_default();
                let mut literals = Vec::new();
                collect_enum_choices(declared, &mut literals);
                let prefix = default_value
                    .as_deref()
                    .and_then(|v| v.rsplit_once('.').map(|(prefix, _)| format!("{prefix}.")))
                    .unwrap_or_default();
                let choices = literals
                    .into_iter()
                    .map(|literal| format!("{prefix}{literal}"))
                    .collect();
                let comment = element
                    .get("comment")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                out.push(ParamRow {
                    name: name.to_owned(),
                    type_name,
                    default_value,
                    current_value,
                    unit,
                    comment,
                    choices,
                });
            }
            _ => {}
        }
    }
}

fn modifier_default(value: &Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return (!s.is_empty()).then(|| s.to_owned());
    }
    value.get("binding").and_then(binding_expression)
}

fn binding_expression(value: &Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return Some(s.to_owned());
    }
    if let Some(b) = value.as_bool() {
        return Some(b.to_string());
    }
    if let Some(n) = value.as_number() {
        return Some(n.to_string());
    }
    if let Some(name) = value.get("name").and_then(Value::as_str) {
        return Some(name.to_owned());
    }
    None
}

fn type_unit(value: &Value) -> Option<String> {
    let elements = value.get("elements")?.as_array()?;
    for element in elements {
        if element.get("$kind").and_then(Value::as_str) == Some(EXTENDS) {
            if let Some(unit) = element
                .pointer("/modifiers/unit/$value")
                .and_then(Value::as_str)
            {
                if let Ok(decoded) = serde_json::from_str::<String>(unit) {
                    return Some(decoded);
                }
                return Some(unit.to_owned());
            }
            if let Some(base) = element.get("baseClass") {
                if let Some(unit) = type_unit(base) {
                    return Some(unit);
                }
            }
        }
    }
    None
}

fn collect_enum_choices(value: &Value, out: &mut Vec<String>) {
    let Some(elements) = value.get("elements").and_then(Value::as_array) else {
        return;
    };
    for element in elements {
        match element.get("$kind").and_then(Value::as_str) {
            Some(EXTENDS) => {
                if let Some(base) = element.get("baseClass") {
                    collect_enum_choices(base, out);
                }
            }
            Some(COMPONENT) if element.get("type").is_none() => {
                if let Some(name) = element.get("name").and_then(Value::as_str) {
                    out.push(name.to_owned());
                }
            }
            _ => {}
        }
    }
}

pub fn parameter_edits(
    component: &str,
    rows: &[ParamRow],
    values: &[String],
) -> Vec<crate::edit::EditOp> {
    rows.iter()
        .zip(values)
        .filter_map(|(row, draft)| {
            let draft = draft.trim();
            match (row.current_value.as_deref(), draft.is_empty()) {
                (None, true) => None,
                (Some(_), true) => {
                    row.default_value
                        .as_ref()
                        .map(|default| crate::edit::EditOp::SetModifier {
                            path: format!("{component}.{}", row.name),
                            value: Some(if row.type_name == "String" {
                                encode_modelica_string(default)
                            } else {
                                default.clone()
                            }),
                        })
                }
                (Some(current), false) if current == draft => None,
                (_, false) => Some(crate::edit::EditOp::SetModifier {
                    path: format!("{component}.{}", row.name),
                    value: Some(if row.type_name == "String" {
                        encode_modelica_string(draft)
                    } else {
                        draft.to_owned()
                    }),
                }),
            }
        })
        .collect()
}

/// Element `$kind` values that describe a component rather than a nested class.
const COMPONENT: &str = "component";
const EXTENDS: &str = "extends";

/// Types that never have an interesting component list of their own.
const BUILTIN_TYPES: &[&str] = &["Real", "Integer", "Boolean", "String", "enumeration"];

/// One row of the Component Browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentNode {
    /// Instance name, for example `firstOrder`.
    pub name: String,
    /// Declared type, for example `Modelica.Blocks.Continuous.FirstOrder` or `Real`.
    pub type_name: String,
    /// Description string of the component (empty when OMC has none).
    pub comment: String,
    /// `prefixes.variability` is `parameter` or `constant`.
    pub is_parameter: bool,
    pub children: Vec<ComponentNode>,
}

/// Return the first `{` ... last `}` slice of `output`, which is where OMC prints the
/// JSON document between its own status lines.
pub fn extract_json(output: &str) -> Option<&str> {
    let start = output.find('{')?;
    let end = output.rfind('}')?;
    (end > start).then(|| &output[start..=end])
}

/// Parse the components declared by a class, recursing at most `max_depth` levels into
/// the classes of the components themselves.
pub fn parse_components(json: &str, max_depth: usize) -> Result<Vec<ComponentNode>, String> {
    let root: Value = serde_json::from_str(json)
        .map_err(|error| format!("could not read the model instance JSON: {error}"))?;
    if !root.is_object() {
        return Err("the model instance JSON is not an object".into());
    }
    let mut components = Vec::new();
    collect(&root, max_depth, &mut components);
    Ok(components)
}

fn collect(node: &Value, depth: usize, out: &mut Vec<ComponentNode>) {
    let Some(elements) = node.get("elements").and_then(Value::as_array) else {
        return;
    };
    let mut seen = std::collections::HashSet::new();
    for element in elements {
        let mut found = Vec::new();
        match element.get("$kind").and_then(Value::as_str) {
            // `extends` elements are flattened into the parent: the JSON nests the base
            // class definition, so its components belong to the same level.
            Some(EXTENDS) => {
                if let Some(base) = element.get("baseClass") {
                    collect(base, depth, &mut found);
                }
            }
            Some(COMPONENT) => {
                if let Some(component) = component_node(element, depth) {
                    found.push(component);
                }
            }
            _ => {}
        }
        // The most derived definition comes first, so keep the first occurrence: a
        // subclass that redeclares a component wins over the inherited one.
        out.extend(found.into_iter().filter(|c| seen.insert(c.name.clone())));
    }
}

fn component_node(element: &Value, depth: usize) -> Option<ComponentNode> {
    let name = element.get("name")?.as_str()?.to_owned();
    // Enumeration literals are printed as components without a `type`; they are values,
    // not parts of the model, so they are skipped.
    let declared = element.get("type")?;
    let type_name = match declared {
        Value::String(name) => name.clone(),
        Value::Object(_) => declared.get("name")?.as_str()?.to_owned(),
        _ => return None,
    };
    let comment = element
        .get("comment")
        .and_then(Value::as_str)
        .or_else(|| declared.get("comment").and_then(Value::as_str))
        .unwrap_or_default()
        .to_owned();
    let variability = element
        .pointer("/prefixes/variability")
        .and_then(Value::as_str);
    let children = if depth > 1 && !BUILTIN_TYPES.contains(&type_name.as_str()) {
        let mut children = Vec::new();
        collect(declared, depth - 1, &mut children);
        children
    } else {
        Vec::new()
    };
    Some(ComponentNode {
        name,
        type_name,
        comment,
        is_parameter: matches!(variability, Some("parameter" | "constant")),
        children,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MSL_FIRST_ORDER: &str = include_str!("../tests/fixtures/instance_MSLFirstOrder.json");
    const BOUNCING_BALL: &str = include_str!("../tests/fixtures/instance_BouncingBall.json");

    fn names(nodes: &[ComponentNode]) -> Vec<&str> {
        nodes.iter().map(|n| n.name.as_str()).collect()
    }

    #[test]
    fn lists_the_top_level_components_of_a_model() {
        let components = parse_components(MSL_FIRST_ORDER, 4).unwrap();
        assert_eq!(names(&components), ["inputSignal", "plant"]);
        assert_eq!(
            components[1].type_name,
            "Modelica.Blocks.Continuous.FirstOrder"
        );
        // The component has no description of its own, so the class description is used.
        assert_eq!(
            components[1].comment,
            "First order transfer function block (= 1 pole)"
        );
        assert!(!components[1].is_parameter);
    }

    #[test]
    fn recurses_into_the_type_of_a_component() {
        let components = parse_components(MSL_FIRST_ORDER, 4).unwrap();
        let plant = &components[1];
        // FirstOrder's own parameters, plus the inherited output `y` from SO.
        let inner = names(&plant.children);
        assert!(inner.contains(&"k"), "{inner:?}");
        assert!(inner.contains(&"T"), "{inner:?}");
        assert!(inner.contains(&"y"), "{inner:?}");
        let k = plant.children.iter().find(|c| c.name == "k").unwrap();
        assert_eq!(k.type_name, "Real");
        assert!(k.is_parameter);
        assert_eq!(k.comment, "Gain");
        // `T : Modelica.Units.SI.Time` extends `Real`, so it has no components of its own.
        let t = plant.children.iter().find(|c| c.name == "T").unwrap();
        assert!(t.children.is_empty(), "{:?}", t.children);
    }

    #[test]
    fn stops_at_the_requested_depth() {
        let shallow = parse_components(MSL_FIRST_ORDER, 1).unwrap();
        assert_eq!(names(&shallow), ["inputSignal", "plant"]);
        assert!(shallow[1].children.is_empty());
    }

    #[test]
    fn marks_parameters_of_a_user_model() {
        let components = parse_components(BOUNCING_BALL, 4).unwrap();
        assert_eq!(
            names(&components),
            [
                "gravity",
                "restitution",
                "initialHeight",
                "height",
                "velocity"
            ]
        );
        let parameter = |name: &str| {
            components
                .iter()
                .find(|c| c.name == name)
                .unwrap()
                .is_parameter
        };
        assert!(parameter("gravity"));
        assert!(parameter("restitution"));
        assert!(parameter("initialHeight"));
        assert!(!parameter("height"));
        assert!(!parameter("velocity"));
        assert_eq!(components[3].comment, "Height above the ground (m)");
    }

    #[test]
    fn extracts_the_json_from_noisy_output() {
        let output = "true\ntrue\n{\"name\": \"M\", \"elements\": []}\n\"\"\n";
        assert_eq!(
            extract_json(output),
            Some("{\"name\": \"M\", \"elements\": []}")
        );
        assert_eq!(extract_json("no braces here"), None);
        assert_eq!(extract_json("} backwards {"), None);
    }

    #[test]
    fn rejects_broken_json_without_panicking() {
        assert!(parse_components("{not json", 4).is_err());
        assert!(parse_components("[]", 4).is_err());
        assert_eq!(parse_components("{}", 4).unwrap(), Vec::new());
    }

    #[test]
    fn parses_parameter_defaults_units_inherited_values_and_enumerations() {
        let root: Value = serde_json::from_str(MSL_FIRST_ORDER).unwrap();
        let plant = root["elements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["name"] == "plant")
            .unwrap();
        let rows = parse_parameter_rows(
            &plant["type"],
            Some(&serde_json::json!({"modifiers":{"T":"2"}})),
        );
        let k = rows.iter().find(|p| p.name == "k").unwrap();
        assert_eq!(k.default_value.as_deref(), Some("1"));
        let t = rows.iter().find(|p| p.name == "T").unwrap();
        assert_eq!(t.default_value.as_deref(), Some("0.25"));
        assert_eq!(t.current_value.as_deref(), Some("2"));
        assert_eq!(t.unit, "s");
        let init = rows.iter().find(|p| p.name == "initType").unwrap();
        assert!(init.choices.contains(&"Init.NoInit".to_owned()));
        assert!(rows.iter().all(|p| p.name != "NoInit"));
    }

    #[test]
    fn parameter_edits_only_include_changes_and_restore_defaults() {
        let rows = vec![
            ParamRow {
                name: "k".into(),
                type_name: "Real".into(),
                default_value: Some("1".into()),
                current_value: None,
                unit: String::new(),
                comment: String::new(),
                choices: vec![],
            },
            ParamRow {
                name: "T".into(),
                type_name: "Real".into(),
                default_value: Some("0.25".into()),
                current_value: Some("2".into()),
                unit: "s".into(),
                comment: String::new(),
                choices: vec![],
            },
        ];
        assert!(parameter_edits("plant", &rows, &[String::new(), "2".into()]).is_empty());
        let edits = parameter_edits("plant", &rows, &["3".into(), String::new()]);
        assert!(
            matches!(edits.as_slice(), [crate::edit::EditOp::SetModifier { path, value: Some(v) }, crate::edit::EditOp::SetModifier { path: reset, value: Some(default) }] if path == "plant.k" && v == "3" && reset == "plant.T" && default == "0.25")
        );
    }

    #[test]
    fn string_parameter_edits_escape_literals_and_restore_the_default() {
        let rows = [ParamRow {
            name: "label".into(),
            type_name: "String".into(),
            default_value: Some("default".into()),
            current_value: Some("old".into()),
            unit: String::new(),
            comment: String::new(),
            choices: Vec::new(),
        }];
        let edited = parameter_edits("plant", &rows, &["a\"b\\c".into()]);
        assert!(
            matches!(edited.as_slice(), [crate::edit::EditOp::SetModifier { path, value: Some(v) }] if path == "plant.label" && v == "\"a\\\"b\\\\c\"")
        );
        let reset = parameter_edits("plant", &rows, &[String::new()]);
        assert!(
            matches!(reset.as_slice(), [crate::edit::EditOp::SetModifier { path, value: Some(v) }] if path == "plant.label" && v == "\"default\"")
        );
    }
}

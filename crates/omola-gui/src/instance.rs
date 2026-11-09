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

/// One row of the parameter dialog (G33).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamRow {
    pub name: String,
    pub type_name: String,
    /// Default the class declares, shown as a hint when the row has no value.
    pub default: String,
    /// Value the component's modifier gives it now, empty when there is none.
    pub original: String,
    /// Text being edited in the dialog; starts as [`Self::original`].
    pub value: String,
    pub unit: String,
    pub comment: String,
    /// Enumeration literals when the type is an enumeration, empty otherwise.
    pub choices: Vec<String>,
    pub is_boolean: bool,
}

/// The parameters of a class, with the values `modifiers` gives them (G33).
///
/// `modifiers` is the component's modifier list from the class instance JSON, as
/// `(name, text)` pairs; a parameter without one shows its default as a hint only.
pub fn parse_parameters(
    json: &str,
    modifiers: &[(String, String)],
) -> Result<Vec<ParamRow>, String> {
    let root: Value = serde_json::from_str(json)
        .map_err(|error| format!("could not read the parameter JSON: {error}"))?;
    if !root.is_object() {
        return Err("the parameter JSON is not an object".into());
    }
    let mut found = Vec::new();
    collect_parameters(&root, 0, &mut found);
    let mut seen = std::collections::HashSet::new();
    let mut rows: Vec<ParamRow> = Vec::new();
    for element in found {
        let Some(name) = element.get("name").and_then(Value::as_str) else {
            continue;
        };
        if !seen.insert(name.to_owned()) {
            continue;
        }
        let declared = element.get("type").cloned().unwrap_or(Value::Null);
        let type_name = match &declared {
            Value::String(name) => name.clone(),
            Value::Object(_) => declared
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            _ => String::new(),
        };
        let choices = enum_literals(&declared);
        let default = element
            .pointer("/value/binding")
            .and_then(value_text)
            .or_else(|| element.get("modifiers").and_then(value_text))
            .unwrap_or_default();
        let mut original = modifiers
            .iter()
            .find(|(other, _)| other == name)
            .map(|(_, text)| text.clone())
            .unwrap_or_default();
        // An enumeration modifier is written `Init.NoInit`; the dialog works with the
        // literal alone and puts the prefix back when it writes the modifier.
        if !choices.is_empty() {
            original = original.rsplit('.').next().unwrap_or(&original).to_owned();
        }
        rows.push(ParamRow {
            name: name.to_owned(),
            type_name: type_name.clone(),
            default,
            value: original.clone(),
            original,
            unit: unit_of(element, &declared),
            comment: element
                .get("comment")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            choices,
            is_boolean: type_name == "Boolean",
        });
    }
    Ok(rows)
}

/// The `modifiers` of one component in a class instance JSON, as `(name, text)` pairs.
pub fn component_modifiers(json: &str, component: &str) -> Result<Vec<(String, String)>, String> {
    let root: Value = serde_json::from_str(json)
        .map_err(|error| format!("could not read the model instance JSON: {error}"))?;
    let mut elements = Vec::new();
    collect_elements(&root, 0, &mut elements);
    let element = elements
        .into_iter()
        .find(|element| element.get("name").and_then(Value::as_str) == Some(component))
        .ok_or_else(|| format!("{component} is not part of the model"))?;
    Ok(element
        .get("modifiers")
        .and_then(Value::as_object)
        .map(|modifiers| {
            modifiers
                .iter()
                .filter_map(|(name, value)| Some((name.clone(), value_text(value)?)))
                .collect()
        })
        .unwrap_or_default())
}

/// Every `component` element of a class, with `extends` flattened (G33).
fn collect_elements<'a>(node: &'a Value, depth: usize, out: &mut Vec<&'a Value>) {
    if depth > 8 {
        return;
    }
    let Some(elements) = node.get("elements").and_then(Value::as_array) else {
        return;
    };
    for element in elements {
        match element.get("$kind").and_then(Value::as_str) {
            Some(EXTENDS) => {
                if let Some(base) = element.get("baseClass") {
                    collect_elements(base, depth + 1, out);
                }
            }
            Some(COMPONENT) => out.push(element),
            _ => {}
        }
    }
}

fn collect_parameters<'a>(node: &'a Value, depth: usize, out: &mut Vec<&'a Value>) {
    if depth > 8 {
        return;
    }
    let Some(elements) = node.get("elements").and_then(Value::as_array) else {
        return;
    };
    for element in elements {
        match element.get("$kind").and_then(Value::as_str) {
            Some(EXTENDS) => {
                if let Some(base) = element.get("baseClass") {
                    collect_parameters(base, depth + 1, out);
                }
            }
            Some(COMPONENT) => {
                // `constant` is a value, not something a user tunes per component.
                let variability = element
                    .pointer("/prefixes/variability")
                    .and_then(Value::as_str);
                if variability == Some("parameter") && element.get("type").is_some() {
                    out.push(element);
                }
            }
            _ => {}
        }
    }
}

/// Enumeration literals of a type, empty when it is not an enumeration (G33).
fn enum_literals(declared: &Value) -> Vec<String> {
    let Some(elements) = declared.get("elements").and_then(Value::as_array) else {
        return Vec::new();
    };
    let is_enum = elements.iter().any(|element| {
        element.get("$kind").and_then(Value::as_str) == Some(EXTENDS)
            && element.get("baseClass").and_then(Value::as_str) == Some("enumeration")
    });
    if !is_enum {
        return Vec::new();
    }
    elements
        .iter()
        .filter(|element| element.get("$kind").and_then(Value::as_str) == Some(COMPONENT))
        .filter_map(|element| element.get("name").and_then(Value::as_str))
        .map(str::to_owned)
        .collect()
}

/// Unit of a parameter: its own `unit` modifier, else the one its type declares (G33).
fn unit_of(element: &Value, declared: &Value) -> String {
    element
        .pointer("/modifiers/unit")
        .and_then(value_text)
        .or_else(|| type_unit(declared, 0))
        .unwrap_or_default()
}

fn type_unit(class: &Value, depth: usize) -> Option<String> {
    if depth > 6 {
        return None;
    }
    for element in class.get("elements")?.as_array()? {
        if element.get("$kind").and_then(Value::as_str) != Some(EXTENDS) {
            continue;
        }
        if let Some(unit) = element.pointer("/modifiers/unit").and_then(value_text) {
            return Some(unit);
        }
        if let Some(base) = element.get("baseClass") {
            if let Some(unit) = type_unit(base, depth + 1) {
                return Some(unit);
            }
        }
    }
    None
}

/// Display text of a JSON value: numbers without a pointless `.0`, strings without their
/// quotes, enumerations by their last name segment, modifier records by their `$value`.
fn value_text(value: &Value) -> Option<String> {
    match value {
        Value::Number(number) => Some(number_text(number.as_f64()?)),
        Value::String(text) => Some(text.trim_matches('"').to_owned()),
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Object(object) => {
            if object.get("$kind").and_then(Value::as_str) == Some("enum") {
                let name = object.get("name").and_then(Value::as_str)?;
                return Some(name.rsplit('.').next().unwrap_or(name).to_owned());
            }
            object.get("$value").and_then(value_text)
        }
        _ => None,
    }
}

fn number_text(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
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

    /// The `type` object of `plant`, which is the `FirstOrder` class definition.
    fn first_order() -> serde_json::Value {
        let root: serde_json::Value = serde_json::from_str(MSL_FIRST_ORDER).unwrap();
        root["elements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|element| element["name"] == "plant")
            .unwrap()["type"]
            .clone()
    }

    #[test]
    fn lists_the_parameters_of_a_block() {
        let class = first_order();
        let rows = parse_parameters(&class.to_string(), &[]).unwrap();
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["k", "T", "initType", "y_start"]);
        let k = &rows[0];
        assert_eq!(k.type_name, "Real");
        assert_eq!(k.default, "1");
        assert_eq!(k.comment, "Gain");
        assert!(k.choices.is_empty());
        let t = &rows[1];
        assert_eq!(t.default, "0.25");
        // The unit comes from `Modelica.Units.SI.Time`, not from the parameter itself.
        assert_eq!(t.unit, "s");
        // An enumeration lists its literals and keeps only the literal part of its default.
        let init = &rows[2];
        assert_eq!(
            init.choices,
            ["NoInit", "SteadyState", "InitialState", "InitialOutput"]
        );
        assert_eq!(init.default, "NoInit");
        assert!(!rows[3].is_boolean);
    }

    #[test]
    fn a_modifier_overrides_the_default_of_a_parameter() {
        let class = first_order();
        let modifiers = vec![
            ("T".to_owned(), "2".to_owned()),
            ("initType".to_owned(), "Init.SteadyState".to_owned()),
        ];
        let rows = parse_parameters(&class.to_string(), &modifiers).unwrap();
        let t = rows.iter().find(|r| r.name == "T").unwrap();
        assert_eq!(t.original, "2");
        assert_eq!(t.value, "2");
        assert_eq!(t.default, "0.25");
        // The enumeration keeps the literal, so the combo box can select it.
        let init = rows.iter().find(|r| r.name == "initType").unwrap();
        assert_eq!(init.original, "SteadyState");
        // `k` has no modifier, so it only has a default.
        let k = rows.iter().find(|r| r.name == "k").unwrap();
        assert!(k.original.is_empty());
    }

    #[test]
    fn the_modifiers_of_a_component_are_read_from_the_model() {
        let modifiers = component_modifiers(MSL_FIRST_ORDER, "plant").unwrap();
        assert_eq!(
            modifiers,
            vec![
                ("T".to_owned(), "0.25".to_owned()),
                ("k".to_owned(), "1".to_owned()),
            ]
        );
        assert!(component_modifiers(MSL_FIRST_ORDER, "nope").is_err());
        assert!(component_modifiers("{not json", "plant").is_err());
    }
}

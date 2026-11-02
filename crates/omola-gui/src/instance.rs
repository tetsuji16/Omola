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
}

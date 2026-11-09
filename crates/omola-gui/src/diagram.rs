//! Read-only rendering of Modelica `annotation(Diagram(graphics={...}))` and component placement (G12).
//!
//! The diagram data comes from `getModelInstance` (instance JSON) and
//! `getModelInstanceAnnotation(..., {"Icon"})` for each component type.
//!
//! Placement coordinate transform follows Modelica 3.6 section 18.6.1:
//! `p' = R(rotation) * scale * p + origin`, where `scale` maps the icon's
//! `extent` rectangle to the placement's `extent` rectangle (handling flip via
//! `extent.x1 > extent.x2` or `extent.y1 > extent.y2`).

use crate::graphics::{Extent, Graphic, Icon, Shape, Style};
use serde_json::Value;

/// Parsed placement of a component in the diagram.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    pub extent: Extent,
    pub origin: [f64; 2],
    pub rotation_deg: f64,
}

impl Default for Placement {
    fn default() -> Self {
        Self {
            extent: Extent::DEFAULT,
            origin: [0.0, 0.0],
            rotation_deg: 0.0,
        }
    }
}

/// A component placed on the diagram with its icon.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedComponent {
    pub name: String,
    pub type_name: String,
    pub placement: Placement,
    pub icon: Option<Icon>,
    pub connectors: Vec<ConnectorPin>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConnectorPin {
    pub name: String,
    pub extent: Extent,
}

/// A connection between two components' connectors.
#[derive(Debug, Clone, PartialEq)]
pub struct Connection {
    pub from: String,
    pub to: String,
    pub points: Vec<[f64; 2]>,
    pub color: [u8; 3],
}

/// The complete diagram data for one class.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagram {
    pub coordinate_extent: Extent,
    pub own_graphics: Vec<Graphic>,
    pub components: Vec<PlacedComponent>,
    pub connections: Vec<Connection>,
}

/// Connections whose endpoints belong to `component`, avoiding prefix collisions (c1 vs c10).
pub fn connections_of(diagram: &Diagram, component: &str) -> Vec<(String, String)> {
    let owns = |name: &str| name == component || name.starts_with(&format!("{component}."));
    diagram
        .connections
        .iter()
        .filter(|c| owns(&c.from) || owns(&c.to))
        .map(|c| (c.from.clone(), c.to.clone()))
        .collect()
}

pub fn delete_component_ops(diagram: &Diagram, component: &str) -> Vec<crate::edit::EditOp> {
    let mut ops = connections_of(diagram, component)
        .into_iter()
        .map(|(from, to)| crate::edit::EditOp::DeleteConnection { from, to })
        .collect::<Vec<_>>();
    ops.push(crate::edit::EditOp::DeleteComponent {
        name: component.into(),
    });
    ops
}

pub fn move_component_ops(
    diagram: &Diagram,
    component: &str,
    placement: &Placement,
) -> Vec<crate::edit::EditOp> {
    let mut ops = Vec::new();
    if let Some(c) = diagram.components.iter().find(|c| c.name == component) {
        ops.push(crate::edit::EditOp::SetPlacement {
            component: c.name.clone(),
            type_name: c.type_name.clone(),
            extent: [
                [placement.extent.x1, placement.extent.y1],
                [placement.extent.x2, placement.extent.y2],
            ],
            origin: placement.origin,
            rotation: placement.rotation_deg,
        });
        for connection in &diagram.connections {
            let from = connection.from == component
                || connection.from.starts_with(&format!("{component}."));
            let to =
                connection.to == component || connection.to.starts_with(&format!("{component}."));
            if !from && !to {
                continue;
            }
            let mut moved = c.clone();
            moved.placement = placement.clone();
            let a = endpoint_position(diagram, &connection.from, Some((&moved, component)))
                .or_else(|| connection.points.first().copied());
            let b = endpoint_position(diagram, &connection.to, Some((&moved, component)))
                .or_else(|| connection.points.last().copied());
            let (Some(a), Some(b)) = (a, b) else {
                continue;
            };
            let points = crate::diagram_view::orthogonal_route(a, b);
            ops.push(crate::edit::EditOp::DeleteConnection {
                from: connection.from.clone(),
                to: connection.to.clone(),
            });
            ops.push(crate::edit::EditOp::AddConnection {
                from: connection.from.clone(),
                to: connection.to.clone(),
                points,
            });
        }
    }
    ops
}

fn endpoint_position(
    diagram: &Diagram,
    endpoint: &str,
    moved: Option<(&PlacedComponent, &str)>,
) -> Option<[f64; 2]> {
    let (component_name, pin_name) = endpoint.split_once('.')?;
    let comp = if let Some((c, _name)) = moved.filter(|(_, name)| *name == component_name) {
        c
    } else {
        diagram
            .components
            .iter()
            .find(|c| c.name == component_name)?
    };
    let pin = comp.connectors.iter().find(|p| p.name == pin_name)?;
    Some(crate::diagram_view::pin_position(comp, pin))
}

pub fn new_connection_op(
    diagram: &Diagram,
    from: String,
    to: String,
    points: Vec<[f64; 2]>,
) -> Option<crate::edit::EditOp> {
    let (from_component, from_pin) = from.split_once('.')?;
    let (to_component, to_pin) = to.split_once('.')?;
    if from == to || from_component == to_component {
        return None;
    }
    if diagram
        .connections
        .iter()
        .any(|c| (c.from == from && c.to == to) || (c.from == to && c.to == from))
    {
        return None;
    }
    if endpoint_position(diagram, &from, None).is_none()
        || endpoint_position(diagram, &to, None).is_none()
        || from_pin.is_empty()
        || to_pin.is_empty()
    {
        return None;
    }
    Some(crate::edit::EditOp::AddConnection { from, to, points })
}

impl Default for Diagram {
    fn default() -> Self {
        Self {
            coordinate_extent: Extent::DEFAULT,
            own_graphics: Vec::new(),
            components: Vec::new(),
            connections: Vec::new(),
        }
    }
}

/// Parse the diagram from instance JSON + icon annotations.
pub fn parse_diagram(
    instance_json: &Value,
    icons: &BTreeMap<String, Icon>,
) -> Result<Diagram, String> {
    let mut diagram = Diagram::default();

    // Parse the class's own Diagram annotation graphics
    if let Some(annotation) = instance_json.get("annotation") {
        if let Some(diagram_ann) = annotation.get("Diagram") {
            if let Some(graphics) = diagram_ann.get("graphics").and_then(|g| g.as_array()) {
                for entry in graphics {
                    if let Ok(Some(graphic)) = parse_graphic(entry) {
                        diagram.own_graphics.push(graphic);
                    }
                }
            }
            if let Some(extent) = diagram_ann
                .get("coordinateSystem")
                .and_then(|c| c.get("extent"))
                .and_then(extent_from_value)
            {
                diagram.coordinate_extent = extent;
            }
        }
    }

    // Parse components and their placements
    if let Some(elements) = instance_json.get("elements").and_then(|e| e.as_array()) {
        for element in elements {
            if element.get("$kind").and_then(|k| k.as_str()) != Some("component") {
                continue;
            }
            let name = element
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_owned();
            let type_name = element
                .get("typeName")
                .and_then(|n| n.as_str())
                .or_else(|| {
                    element
                        .get("type")
                        .and_then(|t| t.get("name"))
                        .and_then(|n| n.as_str())
                })
                .unwrap_or("")
                .to_owned();

            // Parse placement annotation
            let placement = if let Some(ann) = element.get("annotation") {
                parse_placement(ann)
            } else {
                Placement::default()
            };

            let icon = icons.get(&type_name).cloned();
            let mut connectors = Vec::new();
            collect_connector_pins(element.get("type").unwrap_or(&Value::Null), &mut connectors);

            diagram.components.push(PlacedComponent {
                name,
                type_name,
                placement,
                icon,
                connectors,
            });
        }
    }

    // Parse connections
    if let Some(connections) = instance_json.get("connections").and_then(|c| c.as_array()) {
        for conn in connections {
            // Real OMC format: {lhs: {parts: [...]}, rhs: {...}, annotation: {Line: {points, color}}}
            let cref = |v: &Value| -> Option<String> {
                v.get("parts")?.as_array().map(|parts| {
                    parts
                        .iter()
                        .filter_map(|part| part.get("name").and_then(|n| n.as_str()))
                        .collect::<Vec<_>>()
                        .join(".")
                })
            };
            let from_owned = conn
                .get("from")
                .and_then(|v| v.as_str())
                .map(|v| v.to_owned())
                .or_else(|| conn.get("lhs").and_then(cref));
            let to_owned = conn
                .get("to")
                .and_then(|v| v.as_str())
                .map(|v| v.to_owned())
                .or_else(|| conn.get("rhs").and_then(cref));
            if let (Some(from), Some(to)) = (from_owned, to_owned) {
                {
                    let line = conn.get("annotation").and_then(|a| a.get("Line"));
                    let points = line
                        .and_then(|l| l.get("points"))
                        .or_else(|| conn.get("points"))
                        .and_then(|p| p.as_array())
                        .and_then(|arr| {
                            let pts: Vec<[f64; 2]> =
                                arr.iter().filter_map(point_from_value).collect();
                            if pts.len() == arr.len() && !pts.is_empty() {
                                Some(pts)
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default();

                    let color = line
                        .and_then(|l| l.get("color"))
                        .or_else(|| conn.get("color"))
                        .and_then(|c| c.as_array())
                        .and_then(|arr| {
                            if arr.len() >= 3 {
                                Some([
                                    arr[0].as_f64().unwrap_or(0.0) as u8,
                                    arr[1].as_f64().unwrap_or(0.0) as u8,
                                    arr[2].as_f64().unwrap_or(0.0) as u8,
                                ])
                            } else {
                                None
                            }
                        })
                        .unwrap_or([0, 0, 0]);

                    diagram.connections.push(Connection {
                        from: from.to_owned(),
                        to: to.to_owned(),
                        points,
                        color,
                    });
                }
            }
        }
    }

    Ok(diagram)
}

fn collect_connector_pins(class: &Value, pins: &mut Vec<ConnectorPin>) {
    let Some(elements) = class.get("elements").and_then(Value::as_array) else {
        return;
    };
    for element in elements {
        match element.get("$kind").and_then(Value::as_str) {
            Some("component")
                if element
                    .get("type")
                    .and_then(|t| t.get("restriction"))
                    .and_then(Value::as_str)
                    .is_some_and(|r| r == "connector" || r == "expandable connector") =>
            {
                if let (Some(name), Some(annotation)) = (
                    element.get("name").and_then(Value::as_str),
                    element.get("annotation"),
                ) {
                    if annotation.get("Placement").is_some() {
                        pins.push(ConnectorPin {
                            name: name.to_owned(),
                            extent: parse_placement(annotation).extent,
                        });
                    }
                }
            }
            Some("extends") => {
                collect_connector_pins(element.get("baseClass").unwrap_or(&Value::Null), pins)
            }
            _ => {}
        }
    }
}

/// Parse a placement annotation into a `Placement` struct.
fn parse_placement(annotation: &Value) -> Placement {
    let mut placement = Placement::default();

    // Look for Placement in the annotation
    if let Some(placement_ann) = annotation.get("Placement") {
        if let Some(trans) = placement_ann.get("transformation") {
            if let Some(extent) = trans.get("extent").and_then(extent_from_value) {
                placement.extent = extent;
            }
            if let Some(origin) = trans.get("origin").and_then(point_from_value) {
                placement.origin = origin;
            }
            if let Some(rot) = trans.get("rotation").and_then(|v| v.as_f64()) {
                placement.rotation_deg = rot;
            }
        }
    }

    placement
}

/// Transform a point from icon coordinate system to parent diagram coordinate system.
///
/// Modelica 3.6 section 18.6.1 formula:
/// ```
/// p_diagram = R(rotation) * S * p_icon + origin
/// ```
/// where `S` is the scaling matrix that maps the icon's extent to the placement's extent:
/// ```
/// S = [ (extent_x2 - extent_x1) / (icon_x2 - icon_x1),  0                                ]
///     [ 0,                                              (extent_y2 - extent_y1) / (icon_y2 - icon_y1) ]
/// ```
/// Note: If `extent.x1 > extent.x2`, the shape is mirrored horizontally.
/// If `extent.y1 > extent.y2`, the shape is mirrored vertically.
pub fn placement_transform(
    p_icon: [f64; 2],
    icon_extent: &Extent,
    placement: &Placement,
) -> [f64; 2] {
    let icon_w = icon_extent.x2 - icon_extent.x1;
    let icon_h = icon_extent.y2 - icon_extent.y1;
    if icon_w.abs() < 1e-9 || icon_h.abs() < 1e-9 {
        return placement.origin;
    }
    let sx = (placement.extent.x2 - placement.extent.x1) / icon_w;
    let sy = (placement.extent.y2 - placement.extent.y1) / icon_h;
    let scaled = [p_icon[0] * sx, p_icon[1] * sy];
    let (sin, cos) = placement.rotation_deg.to_radians().sin_cos();
    let rx = cos * scaled[0] - sin * scaled[1] + placement.origin[0];
    let ry = sin * scaled[0] + cos * scaled[1] + placement.origin[1];
    [rx, ry]
}

/// Bounding box of a placed component in diagram coordinates.
///
/// Modelica 3.6 §18.6.1.1 maps an icon point to `origin + R * S * p_icon`, where `S`
/// takes the icon extent onto the placement extent. The four corners of the placement
/// extent therefore land on `origin + R * corner`; their axis-aligned box is the area the
/// view has to draw, hit-test and put the label under.
///
/// This is also what makes an omitted `origin` behave: the specification says it defaults
/// to `{0, 0}` and the `extent` is then already in the parent's coordinates, so the box is
/// the extent itself (`tests/fixtures/instance_cauer.json` has such a component, `G`).
pub fn component_bounds(placement: &Placement) -> Extent {
    let corners = [
        [placement.extent.x1, placement.extent.y1],
        [placement.extent.x2, placement.extent.y1],
        [placement.extent.x2, placement.extent.y2],
        [placement.extent.x1, placement.extent.y2],
    ];
    let mut x1 = f64::INFINITY;
    let mut y1 = f64::INFINITY;
    let mut x2 = f64::NEG_INFINITY;
    let mut y2 = f64::NEG_INFINITY;
    for corner in corners {
        // `S` is the identity here: the placement extent is mapped onto itself.
        let q = placement_transform(corner, &placement.extent, placement);
        x1 = x1.min(q[0]);
        y1 = y1.min(q[1]);
        x2 = x2.max(q[0]);
        y2 = y2.max(q[1]);
    }
    Extent { x1, y1, x2, y2 }
}

/// Parse a graphic entry (Rectangle, Line, Polygon, Ellipse, Text, Bitmap).
fn parse_graphic(entry: &Value) -> Result<Option<Graphic>, String> {
    let name = entry
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or("graphic without a name")?;
    let elements = entry
        .get("elements")
        .and_then(|e| e.as_array())
        .ok_or_else(|| format!("{name} without elements"))?;
    let visible = elements
        .first()
        .ok_or_else(|| format!("{name} without visible"))?;
    if !is_visible(visible) || is_conditional(visible) {
        return Ok(None);
    }
    let origin = elements
        .get(1)
        .and_then(point_from_value)
        .unwrap_or([0.0, 0.0]);
    let rotation_deg = elements.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0);

    let shape = match name {
        "Rectangle" => {
            let extent = elements
                .get(9)
                .and_then(extent_from_value)
                .ok_or("Rectangle without extent")?;
            let radius = elements.get(10).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let style = style(elements, 3, 4, 5, 6, 7);
            Shape::Rectangle {
                extent,
                radius,
                style,
            }
        }
        "Line" => {
            let points = elements
                .get(3)
                .and_then(|v| v.as_array())
                .and_then(|arr| {
                    let pts: Vec<[f64; 2]> = arr.iter().filter_map(point_from_value).collect();
                    if pts.len() == arr.len() && !pts.is_empty() {
                        Some(pts)
                    } else {
                        None
                    }
                })
                .ok_or("Line without points")?;
            let style = style(elements, 4, 4, 5, 5, 6);
            Shape::Line { points, style }
        }
        "Polygon" => {
            let points = elements
                .get(8)
                .and_then(|v| v.as_array())
                .and_then(|arr| {
                    let pts: Vec<[f64; 2]> = arr.iter().filter_map(point_from_value).collect();
                    if pts.len() == arr.len() && !pts.is_empty() {
                        Some(pts)
                    } else {
                        None
                    }
                })
                .ok_or("Polygon without points")?;
            let style = style(elements, 3, 4, 5, 6, 7);
            Shape::Polygon { points, style }
        }
        "Ellipse" => {
            let extent = elements
                .get(8)
                .and_then(extent_from_value)
                .ok_or("Ellipse without extent")?;
            let start_angle = elements.get(9).and_then(|v| v.as_f64()).unwrap_or(0.0);
            let end_angle = elements.get(10).and_then(|v| v.as_f64()).unwrap_or(360.0);
            let style = style(elements, 3, 4, 5, 6, 7);
            Shape::Ellipse {
                extent,
                start_angle,
                end_angle,
                style,
            }
        }
        "Text" => {
            let extent = elements
                .get(8)
                .and_then(extent_from_value)
                .ok_or("Text without extent")?;
            let text = elements
                .get(9)
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_owned();
            let color = elements
                .get(11)
                .and_then(rgb_from_value)
                .or_else(|| elements.get(3).and_then(rgb_from_value))
                .unwrap_or([0, 0, 0]);
            Shape::Text {
                extent,
                text,
                color,
            }
        }
        "Bitmap" => return Ok(None),
        other => return Err(format!("unsupported graphic {other}")),
    };

    Ok(Some(Graphic {
        shape,
        origin,
        rotation_deg,
    }))
}

/// `lineColor`, `fillColor`, `linePattern`, `fillPattern`, `lineThickness`.
fn style(
    elements: &[Value],
    line_idx: usize,
    fill_idx: usize,
    pattern_idx: usize,
    fill_pattern_idx: usize,
    thickness_idx: usize,
) -> Style {
    let filled = match elements
        .get(fill_pattern_idx)
        .and_then(enum_name_from_value)
    {
        Some("None") | None => None,
        Some(_) => elements.get(fill_idx).and_then(rgb_from_value),
    };
    Style {
        line: elements
            .get(line_idx)
            .and_then(rgb_from_value)
            .unwrap_or([0, 0, 0]),
        fill: filled,
        thickness: elements
            .get(thickness_idx)
            .and_then(|v| v.as_f64())
            .unwrap_or(0.25),
        pattern: line_pattern(elements.get(pattern_idx).and_then(enum_name_from_value)),
    }
}

fn enum_name_from_value(v: &Value) -> Option<&str> {
    v.get("name")
        .and_then(|n| n.as_str())
        .map(|name| name.rsplit('.').next().unwrap_or(name))
}

fn line_pattern(name: Option<&str>) -> crate::graphics::LinePattern {
    match name {
        Some("Dash") => crate::graphics::LinePattern::Dash,
        Some("Dot") => crate::graphics::LinePattern::Dot,
        Some("DashDot") => crate::graphics::LinePattern::DashDot,
        Some("DashDotDot") => crate::graphics::LinePattern::DashDotDot,
        _ => crate::graphics::LinePattern::Solid,
    }
}

fn is_visible(v: &Value) -> bool {
    v.as_bool().unwrap_or(false)
}

fn is_conditional(v: &Value) -> bool {
    v.get("$kind").and_then(|k| k.as_str()) == Some("cref")
}

fn point_from_value(v: &Value) -> Option<[f64; 2]> {
    let arr = v.as_array()?;
    Some([arr.first()?.as_f64()?, arr.get(1)?.as_f64()?])
}

fn extent_from_value(v: &Value) -> Option<Extent> {
    let arr = v.as_array()?;
    let p = point_from_value(arr.first()?)?;
    let q = point_from_value(arr.get(1)?)?;
    Some(Extent {
        x1: p[0],
        y1: p[1],
        x2: q[0],
        y2: q[1],
    })
}

fn rgb_from_value(v: &Value) -> Option<[u8; 3]> {
    let arr = v.as_array()?;
    if arr.len() < 3 {
        return None;
    }
    let mut out = [0u8; 3];
    for (slot, raw) in out.iter_mut().zip(arr.iter()) {
        let n = raw.as_f64()?;
        if n < 0.0 {
            return None;
        }
        *slot = n.clamp(0.0, 255.0) as u8;
    }
    Some(out)
}

use std::collections::BTreeMap;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delete_ops_remove_only_component_connections_before_component() {
        let d = Diagram {
            connections: vec![
                Connection {
                    from: "c1.y".into(),
                    to: "g.u".into(),
                    points: vec![],
                    color: [0; 3],
                },
                Connection {
                    from: "c10.y".into(),
                    to: "g2.u".into(),
                    points: vec![],
                    color: [0; 3],
                },
                Connection {
                    from: "c1.p[2]".into(),
                    to: "g.u".into(),
                    points: vec![],
                    color: [0; 3],
                },
            ],
            ..Diagram::default()
        };
        assert_eq!(
            connections_of(&d, "c1"),
            vec![
                ("c1.y".into(), "g.u".into()),
                ("c1.p[2]".into(), "g.u".into())
            ]
        );
        let ops = delete_component_ops(&d, "c1");
        assert_eq!(ops.len(), 3);
        assert!(
            matches!(ops.last(),Some(crate::edit::EditOp::DeleteComponent{name}) if name=="c1")
        );
        assert!(matches!(
            ops[0],
            crate::edit::EditOp::DeleteConnection { .. }
        ));
    }
    use crate::graphics::Extent;

    #[test]
    fn placement_transform_identity() {
        let icon_extent = Extent::DEFAULT;
        let placement = Placement::default();
        let p = placement_transform([100.0, 100.0], &icon_extent, &placement);
        assert!(
            (p[0] - 100.0).abs() < 1e-9 && (p[1] - 100.0).abs() < 1e-9,
            "{p:?}"
        );
    }

    #[test]
    fn placement_transform_with_origin() {
        let icon_extent = Extent::DEFAULT;
        let placement = Placement {
            origin: [50.0, -20.0],
            ..Default::default()
        };
        let p = placement_transform([0.0, 0.0], &icon_extent, &placement);
        assert!(
            (p[0] - 50.0).abs() < 1e-9 && (p[1] + 20.0).abs() < 1e-9,
            "{p:?}"
        );
    }

    #[test]
    fn placement_transform_rotation_90() {
        let icon_extent = Extent::DEFAULT;
        let placement = Placement {
            rotation_deg: 90.0,
            ..Default::default()
        };
        // (100, 0) in icon coords -> (0, 100) in placement coords
        let p = placement_transform([100.0, 0.0], &icon_extent, &placement);
        assert!(p[0].abs() < 1e-9 && (p[1] - 100.0).abs() < 1e-9, "{p:?}");
    }

    #[test]
    fn placement_transform_x_flip() {
        let icon_extent = Extent::DEFAULT;
        let placement = Placement {
            extent: Extent {
                x1: 10.0,
                x2: -10.0,
                y1: -10.0,
                y2: 10.0,
            },
            ..Default::default()
        };
        // (100, 0) in icon coords -> x maps to left edge of flipped placement (-10) per issue G12
        let p = placement_transform([100.0, 0.0], &icon_extent, &placement);
        assert!((p[0] + 10.0).abs() < 1e-9, "{p:?}");
    }

    #[test]
    fn placement_transform_y_flip() {
        let icon_extent = Extent::DEFAULT;
        let placement = Placement {
            extent: Extent {
                x1: -10.0,
                x2: 10.0,
                y1: 10.0,
                y2: -10.0,
            },
            ..Default::default()
        };
        // (0, 100) in icon coords -> y maps to bottom edge of flipped placement (-10) per issue G12
        let p = placement_transform([0.0, 100.0], &icon_extent, &placement);
        assert!((p[1] + 10.0).abs() < 1e-9, "{p:?}");
    }

    fn cauer() -> Diagram {
        let json: Value =
            serde_json::from_str(include_str!("../tests/fixtures/instance_cauer.json")).unwrap();
        parse_diagram(&json, &BTreeMap::new()).unwrap()
    }

    fn pid() -> Diagram {
        let json: Value =
            serde_json::from_str(include_str!("../tests/fixtures/instance_pid.json")).unwrap();
        parse_diagram(&json, &BTreeMap::new()).unwrap()
    }

    fn assert_bounds(p: &Placement, expected: (f64, f64, f64, f64)) {
        let b = component_bounds(p);
        let got = (b.x1, b.y1, b.x2, b.y2);
        assert!(
            (got.0 - expected.0).abs() < 1e-9
                && (got.1 - expected.1).abs() < 1e-9
                && (got.2 - expected.2).abs() < 1e-9
                && (got.3 - expected.3).abs() < 1e-9,
            "bounds {got:?} != {expected:?}"
        );
    }

    #[test]
    fn component_bounds_without_origin_is_the_extent() {
        // `G` in the Cauer fixture has `extent=[[-10,-90],[10,-70]]` and no `origin`, so
        // the box is that extent in the diagram's coordinates (not the origin).
        let diagram = cauer();
        let g = diagram
            .components
            .iter()
            .find(|c| c.name == "G")
            .expect("G");
        assert_eq!(g.placement.origin, [0.0, 0.0]);
        assert_bounds(&g.placement, (-10.0, -90.0, 10.0, -70.0));
    }

    #[test]
    fn component_bounds_of_the_pid_controller() {
        // `PI` in the PID fixture: `extent=[[-56,-20],[-36,0]]`, no `origin`.
        let diagram = pid();
        let pi = diagram
            .components
            .iter()
            .find(|c| c.name == "PI")
            .expect("PI");
        assert_bounds(&pi.placement, (-56.0, -20.0, -36.0, 0.0));
    }

    #[test]
    fn component_bounds_applies_origin_and_rotation() {
        // Cauer `C1`: origin [-60,-20], extent [-10,-10]..[10,10], rotation 270.
        let diagram = cauer();
        let c1 = diagram
            .components
            .iter()
            .find(|c| c.name == "C1")
            .expect("C1");
        assert_bounds(&c1.placement, (-70.0, -30.0, -50.0, -10.0));
    }

    #[test]
    fn component_bounds_of_a_rotated_rectangle() {
        // A non-square box shows the rotation: 20 high becomes 20 wide at 90 degrees.
        let placement = Placement {
            extent: Extent {
                x1: 0.0,
                y1: 0.0,
                x2: 10.0,
                y2: 20.0,
            },
            origin: [100.0, 0.0],
            rotation_deg: 90.0,
        };
        assert_bounds(&placement, (80.0, 0.0, 100.0, 10.0));
    }

    #[test]
    fn parse_diagram_fixture_cauer_low_pass() {
        let json: Value =
            serde_json::from_str(include_str!("../tests/fixtures/instance_cauer.json")).unwrap();
        let icons = BTreeMap::new();
        let d = parse_diagram(&json, &icons).unwrap();
        assert_eq!(d.components.len(), 18);
        assert!(d.components.iter().any(|c| c.name == "R1"));
        assert_eq!(d.connections.len(), 16);
        assert!(d
            .connections
            .iter()
            .all(|c| !c.from.is_empty() && !c.to.is_empty()));
        assert!(!d.own_graphics.is_empty());
        assert_eq!(d.coordinate_extent.x1, -120.0);
        assert_eq!(d.coordinate_extent.x2, 120.0);
    }

    #[test]
    fn parse_diagram_fixture_pid_controller() {
        let json: Value =
            serde_json::from_str(include_str!("../tests/fixtures/instance_pid.json")).unwrap();
        let icons = BTreeMap::new();
        let d = parse_diagram(&json, &icons).unwrap();
        assert_eq!(d.components.len(), 10);
        assert_eq!(d.connections.len(), 9);
        assert!(!d.own_graphics.is_empty());
        assert_eq!(d.coordinate_extent.x1, -100.0);
        assert_eq!(d.coordinate_extent.x2, 100.0);
    }

    #[test]
    fn parse_connector_pins_from_instance_fixture() {
        let json: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/instance_MSLFirstOrder.json"
        ))
        .unwrap();
        let d = parse_diagram(&json, &BTreeMap::new()).unwrap();
        let component = d
            .components
            .iter()
            .find(|c| c.connectors.iter().any(|p| p.name == "u"))
            .unwrap();
        assert!(component.connectors.iter().any(|p| p.name == "u"));
        assert!(component.connectors.iter().any(|p| p.name == "y"));
        let u = component.connectors.iter().find(|p| p.name == "u").unwrap();
        assert_eq!(
            (u.extent.x1, u.extent.y1, u.extent.x2, u.extent.y2),
            (-140.0, -20.0, -100.0, 20.0)
        );
    }

    #[test]
    fn moving_component_edits_placement_and_attached_connection_together() {
        let pin = || ConnectorPin {
            name: "p".into(),
            extent: Extent {
                x1: 0.0,
                y1: 0.0,
                x2: 0.0,
                y2: 0.0,
            },
        };
        let component = |name: &str, origin| PlacedComponent {
            name: name.into(),
            type_name: "C".into(),
            placement: Placement {
                origin,
                ..Placement::default()
            },
            icon: None,
            connectors: vec![pin()],
        };
        let d = Diagram {
            components: vec![component("a", [0.0, 0.0]), component("b", [10.0, 0.0])],
            connections: vec![Connection {
                from: "a.p".into(),
                to: "b.p".into(),
                points: vec![[0.0, 0.0], [10.0, 0.0]],
                color: [0; 3],
            }],
            ..Diagram::default()
        };
        let placement = Placement {
            origin: [4.0, 0.0],
            ..Placement::default()
        };
        let ops = move_component_ops(&d, "a", &placement);
        assert!(matches!(ops.as_slice(), [
            crate::edit::EditOp::SetPlacement { component, .. },
            crate::edit::EditOp::DeleteConnection { from, to },
            crate::edit::EditOp::AddConnection { from: added_from, to: added_to, points },
        ] if component == "a" && from == "a.p" && to == "b.p" && added_from == from && added_to == to && points[0] == [4.0, 0.0] && points[1] == [10.0, 0.0]));
    }

    #[test]
    fn duplicate_or_same_component_connections_are_rejected() {
        let pin = |name: &str| ConnectorPin {
            name: name.into(),
            extent: Extent {
                x1: 0.0,
                y1: 0.0,
                x2: 0.0,
                y2: 0.0,
            },
        };
        let component = |name: &str| PlacedComponent {
            name: name.into(),
            type_name: "C".into(),
            placement: Placement::default(),
            icon: None,
            connectors: vec![pin("p"), pin("q")],
        };
        let d = Diagram {
            components: vec![component("a"), component("b")],
            connections: vec![Connection {
                from: "a.p".into(),
                to: "b.p".into(),
                points: vec![],
                color: [0; 3],
            }],
            ..Diagram::default()
        };
        assert!(new_connection_op(&d, "a.p".into(), "a.q".into(), vec![]).is_none());
        assert!(new_connection_op(&d, "b.p".into(), "a.p".into(), vec![]).is_none());
        assert!(new_connection_op(&d, "a.p".into(), "b.q".into(), vec![]).is_some());
    }

    #[test]
    fn parse_diagram_with_components_and_connections() {
        let instance_json = serde_json::json!({
            "name": "Test",
            "restriction": "model",
            "elements": [
                {
                    "$kind": "component",
                    "name": "resistor1",
                    "typeName": "Modelica.Electrical.Analog.Basic.Resistor",
                    "annotation": {
                        "Placement": {
                            "transformation": {
                                "extent": [[-50, -50], [50, 50]],
                                "origin": [0, 0],
                                "rotation": 0
                            }
                        }
                    }
                },
                {
                    "$kind": "component",
                    "name": "capacitor1",
                    "typeName": "Modelica.Electrical.Analog.Basic.Capacitor",
                    "annotation": {
                        "Placement": {
                            "transformation": {
                                "extent": [[-50, -50], [50, 50]],
                                "origin": [100, 0],
                                "rotation": 90
                            }
                        }
                    }
                }
            ],
            "connections": [
                {
                    "from": "resistor1.p",
                    "to": "capacitor1.p",
                    "points": [[0, 0], [50, 0], [100, 50]],
                    "color": [0, 0, 255]
                }
            ],
            "annotation": {
                "Diagram": {
                    "coordinateSystem": {
                        "extent": [[-100, -100], [200, 100]]
                    },
                    "graphics": [
                        {
                            "$kind": "record",
                            "name": "Rectangle",
                            "elements": [
                                true,
                                [0, 0],
                                0,
                                [0, 0, 0],
                                [255, 255, 255],
                                { "$kind": "enum", "name": "LinePattern.Solid", "index": 2 },
                                { "$kind": "enum", "name": "FillPattern.Solid", "index": 2 },
                                0.25,
                                [],
                                [[-100, -100], [100, 100]],
                                0
                            ]
                        }
                    ]
                }
            }
        });

        let icons = BTreeMap::new();
        let diagram = parse_diagram(&instance_json, &icons).expect("parse failed");

        assert_eq!(diagram.components.len(), 2);
        assert_eq!(diagram.connections.len(), 1);
        assert_eq!(diagram.own_graphics.len(), 1);
        assert_eq!(diagram.coordinate_extent.x1, -100.0);
        assert_eq!(diagram.coordinate_extent.x2, 200.0);

        // Check component 1 placement
        let r1 = &diagram.components[0];
        assert_eq!(r1.name, "resistor1");
        assert_eq!(r1.placement.origin, [0.0, 0.0]);
        assert_eq!(r1.placement.extent.x1, -50.0);
        assert_eq!(r1.placement.extent.x2, 50.0);

        // Check component 2 placement with rotation
        let c1 = &diagram.components[1];
        assert_eq!(c1.name, "capacitor1");
        assert_eq!(c1.placement.origin, [100.0, 0.0]);
        assert_eq!(c1.placement.rotation_deg, 90.0);

        // Check connection
        let conn = &diagram.connections[0];
        assert_eq!(conn.from, "resistor1.p");
        assert_eq!(conn.to, "capacitor1.p");
        assert_eq!(conn.points.len(), 3);
    }
}

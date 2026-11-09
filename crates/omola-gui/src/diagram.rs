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

/// A connector pin of a placed component, in the component's icon coordinates (G32).
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectorPin {
    pub name: String,
    pub extent: Extent,
}

impl ConnectorPin {
    /// Centre of the pin, in icon coordinates.
    pub fn centre(&self) -> [f64; 2] {
        [
            (self.extent.x1 + self.extent.x2) / 2.0,
            (self.extent.y1 + self.extent.y2) / 2.0,
        ]
    }
}

/// A component placed on the diagram with its icon.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedComponent {
    pub name: String,
    pub type_name: String,
    pub placement: Placement,
    pub icon: Option<Icon>,
    /// Connector pins of the component's type, in icon coordinates (G32).
    pub connectors: Vec<ConnectorPin>,
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
            let connectors = element
                .get("type")
                .map(parse_connectors)
                .unwrap_or_default();

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

/// Connector pins declared by a class, including the ones it inherits (G32).
///
/// `getModelInstance` puts a component's type under `type`, and inherited elements under
/// `type.elements[].$kind == "extends".baseClass`. A derived class declares its own
/// connectors first, so the first occurrence of a name wins.
fn parse_connectors(class: &Value) -> Vec<ConnectorPin> {
    fn walk(class: &Value, depth: usize, out: &mut Vec<ConnectorPin>) {
        if depth > 8 {
            return;
        }
        let Some(elements) = class.get("elements").and_then(|e| e.as_array()) else {
            return;
        };
        for element in elements {
            match element.get("$kind").and_then(|k| k.as_str()) {
                Some("extends") => {
                    if let Some(base) = element.get("baseClass") {
                        walk(base, depth + 1, out);
                    }
                }
                Some("component") => {
                    // A builtin type is a string and has no restriction.
                    let Some(kind) = element.get("type") else {
                        continue;
                    };
                    let restriction = kind
                        .get("restriction")
                        .and_then(|r| r.as_str())
                        .unwrap_or_default();
                    if !matches!(restriction, "connector" | "expandable connector") {
                        continue;
                    }
                    let Some(name) = element.get("name").and_then(|n| n.as_str()) else {
                        continue;
                    };
                    let Some(extent) = element
                        .get("annotation")
                        .and_then(|a| a.get("Placement"))
                        .and_then(|p| p.get("transformation"))
                        .and_then(|t| t.get("extent"))
                        .and_then(extent_from_value)
                    else {
                        continue;
                    };
                    if !out.iter().any(|pin| pin.name == name) {
                        out.push(ConnectorPin {
                            name: name.to_owned(),
                            extent,
                        });
                    }
                }
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(class, 0, &mut out);
    out
}

/// Diagram coordinates of a connector pin (G32).
///
/// The pin extent is in the component's icon coordinates, so it goes through the same
/// `origin + R * S * p` transform as the icon itself, with `S` taken from the icon's
/// coordinate system.
pub fn pin_position(component: &PlacedComponent, pin: &ConnectorPin) -> [f64; 2] {
    let icon_extent = component
        .icon
        .as_ref()
        .map_or(Extent::DEFAULT, |icon| icon.coordinate_extent);
    placement_transform(pin.centre(), &icon_extent, &component.placement)
}

/// Diagram coordinates of the connector named by a reference such as `gain1.u` (G32).
pub fn connector_position(diagram: &Diagram, cref: &str) -> Option<[f64; 2]> {
    let (component_name, pin_name) = cref.split_once('.')?;
    let component = diagram
        .components
        .iter()
        .find(|c| c.name == component_name)?;
    // `c1.p[2]` names one element of the array connector `p`.
    let pin_name = pin_name.split('[').next().unwrap_or(pin_name);
    let pin = component.connectors.iter().find(|p| p.name == pin_name)?;
    Some(pin_position(component, pin))
}

/// Whether the connector reference `cref` (`gain1.u`) belongs to `component` (`gain1`).
///
/// `c1` owns `c1.p[2]` but not `c10.out`.
pub fn connector_belongs_to(cref: &str, component: &str) -> bool {
    cref.strip_prefix(component)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
}

/// The pin closest to `p` within `tolerance`, as `(component index, pin index)` (G32).
pub fn hit_pin(diagram: &Diagram, p: [f64; 2], tolerance: f64) -> Option<(usize, usize)> {
    let mut best: Option<(f64, (usize, usize))> = None;
    for (ci, component) in diagram.components.iter().enumerate() {
        for (pi, pin) in component.connectors.iter().enumerate() {
            let q = pin_position(component, pin);
            let d = point_distance(p, q);
            if d <= tolerance && best.is_none_or(|(closest, _)| d < closest) {
                best = Some((d, (ci, pi)));
            }
        }
    }
    best.map(|(_, hit)| hit)
}

/// Three-segment right-angled route from `a` to `b` (G32).
///
/// The middle x is the midpoint snapped to the 2-unit grid, so a route runs horizontally,
/// then vertically, then horizontally again. Points at the same height need one segment.
pub fn orthogonal_route(a: [f64; 2], b: [f64; 2]) -> Vec<[f64; 2]> {
    if (a[1] - b[1]).abs() < 1e-9 {
        return vec![a, b];
    }
    let mx = crate::diagram_view::snap((a[0] + b[0]) / 2.0, crate::diagram_view::GRID);
    vec![a, [mx, a[1]], [mx, b[1]], b]
}

/// Distance between two points.
fn point_distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

/// Distance from `p` to the segment `a`-`b` (G32).
pub fn point_segment_distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length_sq = dx * dx + dy * dy;
    if length_sq < 1e-18 {
        return point_distance(p, a);
    }
    let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / length_sq).clamp(0.0, 1.0);
    point_distance(p, [a[0] + t * dx, a[1] + t * dy])
}

/// Index of the connection whose polyline passes within `tolerance` of `p` (G32).
pub fn hit_connection(diagram: &Diagram, p: [f64; 2], tolerance: f64) -> Option<usize> {
    let mut best: Option<(f64, usize)> = None;
    for (index, conn) in diagram.connections.iter().enumerate() {
        for pair in conn.points.windows(2) {
            let d = point_segment_distance(p, pair[0], pair[1]);
            if d <= tolerance && best.is_none_or(|(closest, _)| d < closest) {
                best = Some((d, index));
            }
        }
    }
    best.map(|(_, index)| index)
}

/// New route for `conn` after `component` moved to `placement` (G32).
///
/// Both ends are looked up as pins; the moved component uses the new placement, the other
/// one keeps its loaded placement. If a pin cannot be found the old points are kept, which
/// is what a connection to a component without a known icon needs.
pub fn rerouted_points(
    diagram: &Diagram,
    conn: &Connection,
    component: &str,
    placement: &Placement,
) -> Vec<[f64; 2]> {
    let position = |cref: &str| -> Option<[f64; 2]> {
        let (component_name, pin_name) = cref.split_once('.')?;
        let found = diagram
            .components
            .iter()
            .find(|c| c.name == component_name)?;
        let pin_name = pin_name.split('[').next().unwrap_or(pin_name);
        let pin = found.connectors.iter().find(|p| p.name == pin_name)?;
        let effective = if component_name == component {
            placement
        } else {
            &found.placement
        };
        let icon_extent = found
            .icon
            .as_ref()
            .map_or(Extent::DEFAULT, |icon| icon.coordinate_extent);
        Some(placement_transform(pin.centre(), &icon_extent, effective))
    };
    match (position(&conn.from), position(&conn.to)) {
        (Some(a), Some(b)) => orthogonal_route(a, b),
        _ => conn.points.clone(),
    }
}

/// Index of the topmost component whose box contains `p` (diagram coordinates).
///
/// Components are painted in order, so the last one containing the point is the one the
/// user sees on top and therefore the one a click selects (G29).
pub fn hit_component(diagram: &Diagram, p: [f64; 2]) -> Option<usize> {
    diagram
        .components
        .iter()
        .enumerate()
        .rev()
        .find(|(_, c)| {
            let b = component_bounds(&c.placement);
            (b.x1..=b.x2).contains(&p[0]) && (b.y1..=b.y2).contains(&p[1])
        })
        .map(|(index, _)| index)
}

/// The connections that touch `component`, as `(from, to)` pairs (G31).
///
/// A connector belongs to the component when the name matches exactly or is followed by a
/// dot, so `c1` matches `c1.p[2]` but not `c10.out`. Deleting a component has to delete
/// these first: `deleteComponent` does not remove `connect` statements.
pub fn connections_of(diagram: &Diagram, component: &str) -> Vec<(String, String)> {
    diagram
        .connections
        .iter()
        .filter(|c| {
            connector_belongs_to(&c.from, component) || connector_belongs_to(&c.to, component)
        })
        .map(|c| (c.from.clone(), c.to.clone()))
        .collect()
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

    fn placed(name: &str, origin: [f64; 2], extent: Extent, rotation_deg: f64) -> PlacedComponent {
        PlacedComponent {
            name: name.into(),
            type_name: "T".into(),
            placement: Placement {
                extent,
                origin,
                rotation_deg,
            },
            icon: None,
            connectors: Vec::new(),
        }
    }

    fn square() -> Extent {
        Extent {
            x1: -10.0,
            y1: -10.0,
            x2: 10.0,
            y2: 10.0,
        }
    }

    #[test]
    fn a_click_picks_the_component_that_is_drawn_last() {
        let diagram = Diagram {
            components: vec![
                placed("first", [0.0, 0.0], square(), 0.0),
                placed("second", [5.0, 5.0], square(), 0.0),
            ],
            ..Diagram::default()
        };
        // Both boxes contain (4, 4); the later one is on top.
        assert_eq!(hit_component(&diagram, [4.0, 4.0]), Some(1));
        // Only the first box reaches down here (the second starts at -5).
        assert_eq!(hit_component(&diagram, [-8.0, -8.0]), Some(0));
        // Outside every box.
        assert_eq!(hit_component(&diagram, [100.0, 100.0]), None);
        assert_eq!(hit_component(&Diagram::default(), [0.0, 0.0]), None);
    }

    #[test]
    fn a_click_on_a_rotated_component_uses_its_rotated_box() {
        // 20x40 at 90 degrees is 40 wide and 20 high, centred on the origin.
        let tall = Extent {
            x1: -10.0,
            y1: -20.0,
            x2: 10.0,
            y2: 20.0,
        };
        let diagram = Diagram {
            components: vec![placed("tall", [0.0, 0.0], tall, 90.0)],
            ..Diagram::default()
        };
        assert_eq!(hit_component(&diagram, [15.0, 0.0]), Some(0));
        assert_eq!(hit_component(&diagram, [0.0, 15.0]), None);
    }

    fn connected(from: &str, to: &str) -> Connection {
        Connection {
            from: from.into(),
            to: to.into(),
            points: vec![[0.0, 0.0], [1.0, 1.0]],
            color: [0, 0, 0],
        }
    }

    #[test]
    fn a_component_owns_its_connections_but_not_similar_names() {
        let diagram = Diagram {
            connections: vec![
                connected("c1.p", "c10.p"),
                connected("c10.n", "c1.n[2]"),
                connected("c2.p", "c10.n"),
            ],
            ..Diagram::default()
        };
        assert_eq!(
            connections_of(&diagram, "c1"),
            vec![
                ("c1.p".to_owned(), "c10.p".to_owned()),
                ("c10.n".to_owned(), "c1.n[2]".to_owned()),
            ]
        );
        assert_eq!(
            connections_of(&diagram, "c10"),
            vec![
                ("c1.p".to_owned(), "c10.p".to_owned()),
                ("c10.n".to_owned(), "c1.n[2]".to_owned()),
                ("c2.p".to_owned(), "c10.n".to_owned()),
            ]
        );
        // `c1` is not a prefix match for `c10`, and an unconnected component has nothing.
        assert!(connections_of(&diagram, "c3").is_empty());
        assert!(connections_of(&Diagram::default(), "c1").is_empty());
    }

    #[test]
    fn the_pins_of_a_component_come_from_its_type() {
        let json: Value =
            serde_json::from_str(include_str!("../tests/fixtures/instance_pid.json")).unwrap();
        let diagram = parse_diagram(&json, &BTreeMap::new()).unwrap();
        let pi = diagram
            .components
            .iter()
            .find(|c| c.name == "PI")
            .expect("PI");
        let names: Vec<&str> = pi.connectors.iter().map(|p| p.name.as_str()).collect();
        // Three inherited from `SVcontrol`, one declared by `LimPID` itself.
        assert_eq!(names, ["u_s", "u_m", "y", "u_ff"]);
        // The icon is 20 units wide, so the pins at -120 and +110 land at -12 and 11.
        assert_eq!(connector_position(&diagram, "PI.u_s"), Some([-12.0, 0.0]));
        assert_eq!(connector_position(&diagram, "PI.y"), Some([11.0, 0.0]));
        // A name that is not a pin, and a component that is not there.
        assert_eq!(connector_position(&diagram, "PI.k"), None);
        assert_eq!(connector_position(&diagram, "nope.y"), None);
    }

    #[test]
    fn a_pin_is_found_within_the_tolerance() {
        let json: Value =
            serde_json::from_str(include_str!("../tests/fixtures/instance_pid.json")).unwrap();
        let diagram = parse_diagram(&json, &BTreeMap::new()).unwrap();
        let pi = diagram
            .components
            .iter()
            .position(|c| c.name == "PI")
            .expect("PI");
        let (component, pin) = hit_pin(&diagram, [11.0, 0.5], 1.0).expect("pin y");
        assert_eq!(component, pi);
        assert_eq!(diagram.components[component].connectors[pin].name, "y");
        assert_eq!(hit_pin(&diagram, [11.0, 30.0], 1.0), None);
        assert_eq!(hit_pin(&Diagram::default(), [0.0, 0.0], 5.0), None);
    }

    #[test]
    fn a_route_has_three_segments_unless_the_ends_share_a_height() {
        assert_eq!(
            orthogonal_route([0.0, 0.0], [10.0, 0.0]),
            vec![[0.0, 0.0], [10.0, 0.0]]
        );
        let route = orthogonal_route([0.0, 0.0], [10.0, 6.0]);
        assert_eq!(route.len(), 4);
        assert_eq!(route[0], [0.0, 0.0]);
        // The midpoint 5 is snapped to the 2-unit grid.
        assert_eq!(route[1], [6.0, 0.0]);
        assert_eq!(route[2], [6.0, 6.0]);
        assert_eq!(route[3], [10.0, 6.0]);
        // The other direction turns at the same x.
        assert_eq!(orthogonal_route([10.0, 6.0], [0.0, 0.0])[1], [6.0, 6.0]);
    }

    #[test]
    fn the_distance_to_a_segment_is_measured_to_the_line() {
        let (a, b) = ([0.0, 0.0], [10.0, 0.0]);
        assert_eq!(point_segment_distance([5.0, 0.0], a, b), 0.0);
        assert_eq!(point_segment_distance([5.0, 3.0], a, b), 3.0);
        // Past the end the nearest point is the end point itself.
        assert_eq!(point_segment_distance([13.0, 4.0], a, b), 5.0);
        // A zero-length segment is a point.
        assert_eq!(point_segment_distance([3.0, 4.0], a, a), 5.0);
    }

    #[test]
    fn a_connection_is_hit_within_the_tolerance() {
        let diagram = Diagram {
            connections: vec![connected("a.p", "b.n")],
            ..Diagram::default()
        };
        // `connected` runs from (0,0) to (1,1).
        assert_eq!(hit_connection(&diagram, [0.5, 0.5], 0.1), Some(0));
        assert_eq!(hit_connection(&diagram, [0.5, 1.0], 0.1), None);
        assert_eq!(hit_connection(&Diagram::default(), [0.0, 0.0], 5.0), None);
    }

    fn pinned(name: &str, origin: [f64; 2], pins: &[(&str, Extent)]) -> PlacedComponent {
        PlacedComponent {
            name: name.into(),
            type_name: "T".into(),
            placement: Placement {
                extent: square(),
                origin,
                rotation_deg: 0.0,
            },
            icon: None,
            connectors: pins
                .iter()
                .map(|(name, extent)| ConnectorPin {
                    name: (*name).to_owned(),
                    extent: *extent,
                })
                .collect(),
        }
    }

    fn right_pin() -> Extent {
        Extent {
            x1: 90.0,
            y1: -10.0,
            x2: 110.0,
            y2: 10.0,
        }
    }

    fn left_pin() -> Extent {
        Extent {
            x1: -110.0,
            y1: -10.0,
            x2: -90.0,
            y2: 10.0,
        }
    }

    #[test]
    fn moving_a_component_reroutes_its_connections() {
        let diagram = Diagram {
            components: vec![
                pinned("a", [0.0, 0.0], &[("p", right_pin())]),
                pinned("b", [40.0, 0.0], &[("n", left_pin())]),
            ],
            connections: vec![Connection {
                from: "a.p".into(),
                to: "b.n".into(),
                points: vec![[10.0, 0.0], [30.0, 0.0]],
                color: [0, 0, 0],
            }],
            ..Diagram::default()
        };
        let moved = Placement {
            extent: square(),
            origin: [0.0, 20.0],
            rotation_deg: 0.0,
        };
        let points = rerouted_points(&diagram, &diagram.connections[0], "a", &moved);
        // The end on `a` follows the new placement, the other end stays put.
        assert_eq!(points.first(), Some(&[10.0, 20.0]));
        assert_eq!(points.last(), Some(&[30.0, 0.0]));
        // Without a known pin the old route is kept.
        let unknown = Connection {
            from: "a.missing".into(),
            to: "b.n".into(),
            points: vec![[1.0, 1.0], [2.0, 2.0]],
            color: [0, 0, 0],
        };
        assert_eq!(
            rerouted_points(&diagram, &unknown, "a", &moved),
            unknown.points
        );
    }
}

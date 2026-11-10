//! Read-only rendering of Modelica `annotation(Icon(graphics={...}))` (G07).
//!
//! The shapes come from OMC as `getModelInstanceAnnotation(Class, {"Icon"})`, whose
//! `graphics` entries are *positional records* (`{"$kind":"record","name":"Rectangle",
//! "elements":[...]}`) rather than objects with named keys. The field order is the one of
//! Modelica 3.6 section 18.6 and is written down next to every parser below; see
//! `docs/omc-model-instance-json.md` for the captured output.
//!
//! Nothing here edits the model: the Icon layer only paints.
//!
//! Simplifications (deliberate, see issue G07):
//! - Cylinder fills are shaded; other non-solid fill patterns are simplified to solid fills.
//! - A shape whose `visible` is a conditional expression (`useHeatPort` etc.) is skipped:
//!   its condition would need the parameter values, and the default of every MSL model
//!   is `false`, so skipping matches what OMEdit shows out of the box.
//! - `Line` arrow heads are not drawn.
//! - `Bitmap` is not drawn.
//! - `%name` is replaced by the class name; other `%parameter` placeholders are left as
//!   they are.

use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape as EguiShape, Stroke, Vec2};

/// A Modelica rectangle in the icon coordinate system (y points up).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Extent {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl Extent {
    pub const DEFAULT: Extent = Extent {
        x1: -100.0,
        y1: -100.0,
        x2: 100.0,
        y2: 100.0,
    };

    fn center(&self) -> [f64; 2] {
        [(self.x1 + self.x2) / 2.0, (self.y1 + self.y2) / 2.0]
    }
}

/// Stroke and fill of one graphic.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// Line colour as RGB, `None` when the annotation did not give one.
    pub line: [u8; 3],
    /// Fill colour; `None` means "not filled" (`FillPattern.None` or no `fillColor`).
    pub fill: Option<[u8; 3]>,
    pub fill_pattern: FillPattern,
    /// Modelica `lineThickness` (0.25 by default).
    pub thickness: f64,
    /// Modelica `LinePattern` (Solid, Dash, Dot, ...).
    pub pattern: LinePattern,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            line: [0, 0, 0],
            fill: None,
            fill_pattern: FillPattern::Solid,
            thickness: 0.25,
            pattern: LinePattern::Solid,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillPattern {
    Solid,
    HorizontalCylinder,
    VerticalCylinder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinePattern {
    Solid,
    Dash,
    Dot,
    DashDot,
    DashDotDot,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Line {
        points: Vec<[f64; 2]>,
        style: Style,
    },
    Polygon {
        points: Vec<[f64; 2]>,
        style: Style,
    },
    Rectangle {
        extent: Extent,
        radius: f64,
        style: Style,
    },
    Ellipse {
        extent: Extent,
        start_angle: f64,
        end_angle: f64,
        style: Style,
    },
    Text {
        extent: Extent,
        text: String,
        color: [u8; 3],
    },
}

/// One entry of `Icon.graphics`, with the placement attributes every shape shares.
#[derive(Debug, Clone, PartialEq)]
pub struct Graphic {
    pub shape: Shape,
    pub origin: [f64; 2],
    pub rotation_deg: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Icon {
    pub coordinate_extent: Extent,
    pub graphics: Vec<Graphic>,
    /// Graphics that were dropped because their annotation could not be read.
    pub skipped: Vec<String>,
}

impl Icon {
    /// Number of shapes actually painted.
    // Only the tests read this today; the Diagram layer (G12) will size its canvas with it.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.graphics.len()
    }

    pub fn is_empty(&self) -> bool {
        self.graphics.is_empty()
    }

    /// Shapes of a given kind, for tests and for the Diagram layer (G12).
    #[allow(dead_code)]
    pub fn shapes(&self) -> impl Iterator<Item = &Shape> {
        self.graphics.iter().map(|g| &g.shape)
    }
}

// ------------------------------------------------------------------ JSON helpers

fn number(v: &serde_json::Value) -> Option<f64> {
    v.as_f64()
}

fn point(v: &serde_json::Value) -> Option<[f64; 2]> {
    let a = v.as_array()?;
    Some([number(a.first()?)?, number(a.get(1)?)?])
}

fn points(v: &serde_json::Value) -> Option<Vec<[f64; 2]>> {
    // DynamicSelect's first argument is the static representation used in the editor.
    if v.get("$kind").and_then(serde_json::Value::as_str) == Some("call")
        && v.get("name").and_then(serde_json::Value::as_str) == Some("DynamicSelect")
    {
        return points(v.get("arguments")?.as_array()?.first()?);
    }
    let a = v.as_array()?;
    let out: Vec<[f64; 2]> = a.iter().filter_map(point).collect();
    // A partially readable point list would silently distort the shape.
    (out.len() == a.len() && !out.is_empty()).then_some(out)
}

fn extent(v: &serde_json::Value) -> Option<Extent> {
    let a = v.as_array()?;
    let p = point(a.first()?)?;
    let q = point(a.get(1)?)?;
    Some(Extent {
        x1: p[0],
        y1: p[1],
        x2: q[0],
        y2: q[1],
    })
}

/// `[r, g, b]` with values 0..=255. `[-1, -1, -1]` (Modelica's "use the default") is `None`.
fn rgb(v: &serde_json::Value) -> Option<[u8; 3]> {
    let a = v.as_array()?;
    let mut out = [0u8; 3];
    for (slot, raw) in out.iter_mut().zip(a.iter()) {
        let n = number(raw)?;
        if n < 0.0 {
            return None;
        }
        *slot = n.clamp(0.0, 255.0) as u8;
    }
    (a.len() >= 3).then_some(out)
}

/// Name after the dot of an enumeration value (`FillPattern.Solid` -> `Solid`).
fn enum_name(v: &serde_json::Value) -> Option<&str> {
    let name = v.get("name")?.as_str()?;
    Some(name.rsplit('.').next().unwrap_or(name))
}

/// `visible` is `true`, `false`, or a `cref` (a condition such as `useHeatPort`).
fn is_visible(v: &serde_json::Value) -> bool {
    v.as_bool().unwrap_or(false)
}

fn is_conditional(v: &serde_json::Value) -> bool {
    v.get("$kind").and_then(|k| k.as_str()) == Some("cref")
}

fn field(e: &[serde_json::Value], i: usize) -> Option<&serde_json::Value> {
    e.get(i)
}

fn line_pattern(v: Option<&serde_json::Value>) -> LinePattern {
    match v.and_then(enum_name) {
        Some("Dash") => LinePattern::Dash,
        Some("Dot") => LinePattern::Dot,
        Some("DashDot") => LinePattern::DashDot,
        Some("DashDotDot") => LinePattern::DashDotDot,
        _ => LinePattern::Solid,
    }
}

/// `lineColor`, `fillColor`, `linePattern`, `fillPattern`, `lineThickness` live at the
/// same indices for every shape with a style (4, 5, 5/6, 6/7, 7/8).
fn style(
    e: &[serde_json::Value],
    line: usize,
    fill: usize,
    pattern: usize,
    fill_pattern: usize,
    thickness: usize,
) -> Style {
    let filled = match field(e, fill_pattern).and_then(enum_name) {
        // Hatching is simplified to a solid fill (issue G07).
        Some("None") | None => None,
        Some(_) => field(e, fill).and_then(rgb),
    };
    Style {
        line: field(e, line).and_then(rgb).unwrap_or([0, 0, 0]),
        fill: filled,
        fill_pattern: match field(e, fill_pattern).and_then(enum_name) {
            Some("HorizontalCylinder") => FillPattern::HorizontalCylinder,
            Some("VerticalCylinder") => FillPattern::VerticalCylinder,
            _ => FillPattern::Solid,
        },
        thickness: field(e, thickness).and_then(number).unwrap_or(0.25),
        pattern: line_pattern(field(e, pattern)),
    }
}

// ------------------------------------------------------------------ parsing

/// Parse the `annotation.Icon` of a `getModelInstanceAnnotation` answer.
///
/// Inherited icon graphics are painted first, followed by graphics declared by the class.
pub fn parse_icon(json: &serde_json::Value) -> Result<Icon, String> {
    let icon = find_icon(json).ok_or("the class declares no Icon annotation")?;
    Ok(icon)
}

fn find_icon(node: &serde_json::Value) -> Option<Icon> {
    let mut combined: Option<Icon> = None;
    for element in node
        .get("elements")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        if element.get("$kind").and_then(|k| k.as_str()) == Some("extends") {
            if let Some(base) = element.get("baseClass").and_then(find_icon) {
                if let Some(icon) = &mut combined {
                    icon.graphics.extend(base.graphics);
                    icon.skipped.extend(base.skipped);
                } else {
                    combined = Some(base);
                }
            }
        }
    }
    if let Some(own_node) = node.get("annotation").and_then(|a| a.get("Icon")) {
        let own = parse_icon_node(own_node);
        if let Some(icon) = &mut combined {
            if own_node
                .get("coordinateSystem")
                .and_then(|c| c.get("extent"))
                .is_some()
            {
                icon.coordinate_extent = own.coordinate_extent;
            }
            icon.graphics.extend(own.graphics);
            icon.skipped.extend(own.skipped);
        } else {
            combined = Some(own);
        }
    }
    combined
}

fn parse_icon_node(icon: &serde_json::Value) -> Icon {
    let coordinate_extent = icon
        .get("coordinateSystem")
        .and_then(|c| c.get("extent"))
        .and_then(extent)
        .unwrap_or(Extent::DEFAULT);
    let mut graphics = Vec::new();
    let mut skipped = Vec::new();
    if let Some(list) = icon.get("graphics").and_then(|g| g.as_array()) {
        for entry in list {
            match parse_graphic(entry) {
                Ok(Some(graphic)) => graphics.push(graphic),
                Ok(None) => {}
                Err(message) => skipped.push(message),
            }
        }
    }
    Icon {
        coordinate_extent,
        graphics,
        skipped,
    }
}

/// `Ok(None)` for graphics that are intentionally not painted (hidden, conditional,
/// `Bitmap`), `Err` for a malformed annotation.
fn parse_graphic(entry: &serde_json::Value) -> Result<Option<Graphic>, String> {
    let name = entry
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or("graphic without a name")?;
    let elements = entry
        .get("elements")
        .and_then(|e| e.as_array())
        .ok_or_else(|| format!("{name} without elements"))?;
    let visible = field(elements, 0).ok_or_else(|| format!("{name} without visible"))?;
    if !is_visible(visible) || is_conditional(visible) {
        return Ok(None);
    }
    let origin = field(elements, 1).and_then(point).unwrap_or([0.0, 0.0]);
    let rotation_deg = field(elements, 2).and_then(number).unwrap_or(0.0);
    let shape = match name {
        // Rectangle(visible, origin, rotation, lineColor, fillColor, linePattern,
        //           fillPattern, lineThickness, borderPattern, extent, radius)
        "Rectangle" => Shape::Rectangle {
            extent: field(elements, 9)
                .and_then(extent)
                .ok_or("Rectangle without extent")?,
            radius: field(elements, 10).and_then(number).unwrap_or(0.0),
            style: style(elements, 3, 4, 5, 6, 7),
        },
        // Line(visible, origin, rotation, points, color, pattern, thickness, arrow,
        //      arrowSize, smooth)
        "Line" => Shape::Line {
            points: field(elements, 3)
                .and_then(points)
                .ok_or("Line without points")?,
            style: style(elements, 4, 4, 5, 5, 6),
        },
        // Polygon(visible, origin, rotation, lineColor, fillColor, linePattern,
        //         fillPattern, lineThickness, points, smooth)
        "Polygon" => Shape::Polygon {
            points: field(elements, 8)
                .and_then(points)
                .ok_or("Polygon without points")?,
            style: style(elements, 3, 4, 5, 6, 7),
        },
        // Ellipse(visible, origin, rotation, lineColor, fillColor, linePattern,
        //         fillPattern, lineThickness, extent, startAngle, endAngle, closure)
        "Ellipse" => Shape::Ellipse {
            extent: field(elements, 8)
                .and_then(extent)
                .ok_or("Ellipse without extent")?,
            start_angle: field(elements, 9).and_then(number).unwrap_or(0.0),
            end_angle: field(elements, 10).and_then(number).unwrap_or(360.0),
            style: style(elements, 3, 4, 5, 6, 7),
        },
        // Text(visible, origin, rotation, lineColor, fillColor, linePattern,
        //      fillPattern, lineThickness, extent, textString, fontSize, textColor,
        //      horizontalAlignment, string, textStyle)
        "Text" => Shape::Text {
            extent: field(elements, 8)
                .and_then(extent)
                .ok_or("Text without extent")?,
            text: field(elements, 9)
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_owned(),
            color: field(elements, 11)
                .and_then(rgb)
                .or_else(|| field(elements, 3).and_then(rgb))
                .unwrap_or([0, 0, 0]),
        },
        // Bitmaps are out of scope (issue G07).
        "Bitmap" => return Ok(None),
        other => return Err(format!("unsupported graphic {other}")),
    };
    Ok(Some(Graphic {
        shape,
        origin,
        rotation_deg,
    }))
}

// ------------------------------------------------------------------ placement

/// Apply a graphic's `origin` and `rotation` to a point of its own coordinate system.
///
/// Modelica rotates the shape around the origin of the coordinate system and then
/// translates it by `origin`, i.e. `p' = R(rotation) * p + origin` (section 18.6).
pub fn transform(p: [f64; 2], origin: [f64; 2], rotation_deg: f64) -> [f64; 2] {
    let (sin, cos) = rotation_deg.to_radians().sin_cos();
    [
        cos * p[0] - sin * p[1] + origin[0],
        sin * p[0] + cos * p[1] + origin[1],
    ]
}

/// Modelica coordinates to screen coordinates: y is flipped and the icon is scaled to
/// fit `rect` while keeping its aspect ratio, centred.
pub fn to_screen(p: [f64; 2], icon_extent: &Extent, rect: Rect) -> Pos2 {
    let (w, h) = (
        icon_extent.x2 - icon_extent.x1,
        icon_extent.y2 - icon_extent.y1,
    );
    if w.abs() < 1e-9 || h.abs() < 1e-9 {
        return rect.center();
    }
    let scale = (rect.width() / w.abs() as f32).min(rect.height() / h.abs() as f32);
    let c = icon_extent.center();
    let center = rect.center();
    Pos2::new(
        center.x + (p[0] - c[0]) as f32 * scale,
        center.y - (p[1] - c[1]) as f32 * scale,
    )
}

/// Paint `icon` into `rect`; `name` replaces `%name` in `Text` shapes.
pub fn paint_icon(painter: &Painter, rect: Rect, icon: &Icon, name: &str) {
    let extent = &icon.coordinate_extent;
    let scale = {
        let (w, h) = (extent.x2 - extent.x1, extent.y2 - extent.y1);
        if w.abs() < 1e-9 || h.abs() < 1e-9 {
            1.0
        } else {
            (rect.width() / w.abs() as f32).min(rect.height() / h.abs() as f32)
        }
    };
    let place =
        |p: [f64; 2], g: &Graphic| to_screen(transform(p, g.origin, g.rotation_deg), extent, rect);
    for graphic in &icon.graphics {
        match &graphic.shape {
            Shape::Line { points, style } => {
                let pts: Vec<Pos2> = points.iter().map(|p| place(*p, graphic)).collect();
                paint_polyline(painter, &pts, style, scale, false);
            }
            Shape::Polygon { points, style } => {
                let pts: Vec<Pos2> = points.iter().map(|p| place(*p, graphic)).collect();
                if let Some(fill) = style.fill {
                    painter.add(EguiShape::convex_polygon(
                        pts.clone(),
                        color(fill),
                        Stroke::NONE,
                    ));
                }
                // Concave polygons are not filled correctly by `convex_polygon`; the outline
                // is always drawn as a closed path so they still read as a polygon.
                paint_polyline(painter, &pts, style, scale, true);
            }
            Shape::Rectangle {
                extent,
                radius,
                style,
            } => {
                // Four corners as a polygon so that `rotation` works for rectangles too.
                let corners = [
                    [extent.x1, extent.y1],
                    [extent.x2, extent.y1],
                    [extent.x2, extent.y2],
                    [extent.x1, extent.y2],
                ];
                let pts: Vec<Pos2> = corners.iter().map(|p| place(*p, graphic)).collect();
                paint_rectangle_fill(painter, *extent, style, |p| place(p, graphic));
                if *radius > 0.0 {
                    // Rounded corners: draw the outline as a rounded rectangle when the
                    // shape is not rotated, otherwise fall back to the sharp outline.
                    let a = place([extent.x1, extent.y1], graphic);
                    let b = place([extent.x2, extent.y2], graphic);
                    let r = Rect::from_two_pos(a, b);
                    let radius_px = (*radius as f32 * scale).min(r.width().min(r.height()) / 2.0);
                    painter.rect_stroke(
                        r,
                        radius_px,
                        stroke(style, scale),
                        egui::StrokeKind::Middle,
                    );
                } else {
                    paint_polyline(painter, &pts, style, scale, true);
                }
            }
            Shape::Ellipse {
                extent,
                start_angle,
                end_angle,
                style,
            } => {
                let pts = ellipse_points(*extent, *start_angle, *end_angle)
                    .into_iter()
                    .map(|p| place(p, graphic))
                    .collect::<Vec<_>>();
                let full = (end_angle - start_angle).abs() >= 359.999;
                if full {
                    if let Some(fill) = style.fill {
                        painter.add(EguiShape::convex_polygon(
                            pts.clone(),
                            color(fill),
                            Stroke::NONE,
                        ));
                    }
                }
                paint_polyline(painter, &pts, style, scale, full);
            }
            Shape::Text {
                extent,
                text,
                color: text_color,
            } => {
                let a = place([extent.x1, extent.y1], graphic);
                let b = place([extent.x2, extent.y2], graphic);
                let box_rect = Rect::from_two_pos(a, b);
                let label = text.replace("%name", name);
                if label.trim().is_empty() {
                    continue;
                }
                let size = (box_rect.height() * 0.9).clamp(8.0, 48.0);
                painter.text(
                    box_rect.center(),
                    Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(size),
                    color(*text_color),
                );
            }
        }
    }
}

fn color(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// Fill a rectangle in its own coordinates, before applying its placement and rotation.
/// Cylinder fills fade from the line colour at both edges to the fill colour at the centre.
pub fn paint_rectangle_fill(
    painter: &Painter,
    extent: Extent,
    style: &Style,
    place: impl Fn([f64; 2]) -> Pos2,
) {
    if let Some(mesh) = rectangle_fill_mesh(extent, style, place) {
        painter.add(EguiShape::mesh(mesh));
    }
}

fn rectangle_fill_mesh(
    extent: Extent,
    style: &Style,
    place: impl Fn([f64; 2]) -> Pos2,
) -> Option<egui::Mesh> {
    let fill = style.fill?;
    let Extent { x1, y1, x2, y2 } = extent;
    let mut mesh = egui::Mesh::default();
    let edge = color(style.line);
    let middle = color(fill);
    let (positions, colors) = match style.fill_pattern {
        FillPattern::Solid => (
            vec![[x1, y1], [x2, y1], [x2, y2], [x1, y2]],
            vec![middle; 4],
        ),
        FillPattern::HorizontalCylinder => {
            let y = (y1 + y2) / 2.0;
            (
                vec![[x1, y1], [x2, y1], [x1, y], [x2, y], [x1, y2], [x2, y2]],
                vec![edge, edge, middle, middle, edge, edge],
            )
        }
        FillPattern::VerticalCylinder => {
            let x = (x1 + x2) / 2.0;
            (
                vec![[x1, y1], [x1, y2], [x, y1], [x, y2], [x2, y1], [x2, y2]],
                vec![edge, edge, middle, middle, edge, edge],
            )
        }
    };
    for (point, color) in positions.into_iter().zip(colors) {
        mesh.colored_vertex(place(point), color);
    }
    if style.fill_pattern == FillPattern::Solid {
        mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    } else {
        mesh.indices
            .extend_from_slice(&[0, 1, 2, 1, 3, 2, 2, 3, 4, 3, 5, 4]);
    }
    Some(mesh)
}

/// `lineThickness` is in the 200-unit icon coordinate system; keep 1..8 px on screen.
fn stroke(style: &Style, scale: f32) -> Stroke {
    Stroke::new(
        (style.thickness as f32 * scale).clamp(1.0, 8.0),
        color(style.line),
    )
}

fn paint_polyline(painter: &Painter, pts: &[Pos2], style: &Style, scale: f32, closed: bool) {
    if pts.len() < 2 {
        return;
    }
    let mut path = pts.to_vec();
    if closed {
        path.push(pts[0]);
    }
    let s = stroke(style, scale);
    let shape = match style.pattern {
        LinePattern::Solid => EguiShape::line(path, s),
        LinePattern::Dash => EguiShape::Vec(EguiShape::dashed_line(&path, s, 4.0, 3.0)),
        LinePattern::Dot => EguiShape::Vec(EguiShape::dashed_line(&path, s, 1.5, 3.0)),
        LinePattern::DashDot => EguiShape::Vec(EguiShape::dashed_line(&path, s, 6.0, 3.0)),
        LinePattern::DashDotDot => EguiShape::Vec(EguiShape::dashed_line(&path, s, 8.0, 3.0)),
    };
    painter.add(shape);
}

/// 48-segment approximation of (a part of) an ellipse inside `extent`, y up.
fn ellipse_points(extent: Extent, start_angle: f64, end_angle: f64) -> Vec<[f64; 2]> {
    const SEGMENTS: usize = 48;
    let cx = (extent.x1 + extent.x2) / 2.0;
    let cy = (extent.y1 + extent.y2) / 2.0;
    let rx = (extent.x2 - extent.x1) / 2.0;
    let ry = (extent.y2 - extent.y1) / 2.0;
    let sweep = (end_angle - start_angle).clamp(-360.0, 360.0);
    let steps = ((SEGMENTS as f64 * sweep.abs() / 360.0).ceil() as usize).max(2);
    (0..=steps)
        .map(|i| {
            let angle = start_angle + sweep * i as f64 / steps as f64;
            let (sin, cos) = angle.to_radians().sin_cos();
            [cx + rx * cos, cy + ry * sin]
        })
        .collect()
}

/// Size of the square sheet the Icon layer paints into, for callers that need the same
/// geometry as [`paint_icon`].
pub fn sheet(rect: Rect) -> Rect {
    let side = (rect.width().min(rect.height()) - 40.0).max(100.0);
    Rect::from_center_size(rect.center(), Vec2::splat(side))
}

#[cfg(test)]
mod tests {
    #[test]
    fn dynamic_select_points_use_the_static_graphic() {
        let value = serde_json::json!({"$kind":"call", "name":"DynamicSelect", "arguments":[[[-80,-80],[80,80]], {"$kind":"if"}]});
        assert_eq!(
            super::points(&value),
            Some(vec![[-80.0, -80.0], [80.0, 80.0]])
        );
    }
    use super::*;

    fn fixture(json: &str) -> serde_json::Value {
        serde_json::from_str(json).expect("fixture is valid JSON")
    }

    fn parse(json: &str) -> Icon {
        parse_icon(&fixture(json)).expect("fixture has an Icon")
    }

    fn kinds(icon: &Icon) -> Vec<&'static str> {
        icon.shapes()
            .map(|s| match s {
                Shape::Line { .. } => "Line",
                Shape::Polygon { .. } => "Polygon",
                Shape::Rectangle { .. } => "Rectangle",
                Shape::Ellipse { .. } => "Ellipse",
                Shape::Text { .. } => "Text",
            })
            .collect()
    }

    #[test]
    fn transform_rotates_counter_clockwise() {
        let p = transform([10.0, 0.0], [0.0, 0.0], 90.0);
        assert!((p[0]).abs() < 1e-9 && (p[1] - 10.0).abs() < 1e-9, "{p:?}");
        // Then it translates by `origin`.
        let q = transform([10.0, 0.0], [5.0, -2.0], 90.0);
        assert!(
            (q[0] - 5.0).abs() < 1e-9 && (q[1] - 8.0).abs() < 1e-9,
            "{q:?}"
        );
        // 180 degrees mirrors both axes.
        let r = transform([3.0, 4.0], [0.0, 0.0], 180.0);
        assert!(
            (r[0] + 3.0).abs() < 1e-9 && (r[1] + 4.0).abs() < 1e-9,
            "{r:?}"
        );
    }

    #[test]
    fn to_screen_flips_y_and_centres_a_square() {
        let rect = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::splat(200.0));
        let e = Extent::DEFAULT;
        let tl = to_screen([-100.0, 100.0], &e, rect);
        let br = to_screen([100.0, -100.0], &e, rect);
        assert!((tl.x - rect.left()).abs() < 1e-3, "{tl:?}");
        assert!((tl.y - rect.top()).abs() < 1e-3, "{tl:?}");
        assert!((br.x - rect.right()).abs() < 1e-3, "{br:?}");
        assert!((br.y - rect.bottom()).abs() < 1e-3, "{br:?}");
        // The centre of the icon lands on the centre of the rectangle.
        let c = to_screen([0.0, 0.0], &e, rect);
        assert!((c.x - rect.center().x).abs() < 1e-3 && (c.y - rect.center().y).abs() < 1e-3);
        // A non-square rectangle keeps the aspect ratio: the icon is centred with margins.
        let wide = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0));
        let l = to_screen([-100.0, 0.0], &e, wide);
        let r = to_screen([100.0, 0.0], &e, wide);
        assert!(
            ((r.x - l.x) - 200.0).abs() < 1e-3,
            "icon did not stay square"
        );
        assert!((l.x - 100.0).abs() < 1e-3 && (r.x - 300.0).abs() < 1e-3);
    }

    #[test]
    fn first_order_icon_has_lines_polygons_and_text() {
        let icon = parse(include_str!("../tests/fixtures/icon_FirstOrder.json"));
        assert_eq!(
            kinds(&icon),
            [
                "Rectangle",
                "Text",
                "Line",
                "Polygon",
                "Line",
                "Polygon",
                "Line",
                "Text",
                "Text"
            ]
        );
        assert_eq!(icon.coordinate_extent, Extent::DEFAULT);
        assert!(icon.skipped.is_empty(), "{:?}", icon.skipped);
        // The base block frame and instance name remain visible under the class graphics.
        let texts: Vec<_> = icon
            .shapes()
            .filter_map(|s| match s {
                Shape::Text { text, color, .. } => Some((text.clone(), *color)),
                _ => None,
            })
            .collect();
        assert_eq!(texts[0].0, "%name");
        assert_eq!(texts[1], ("PT1".into(), [192, 192, 192]));
        // `textColor = {-1,-1,-1}` means "use lineColor", which is black here.
        assert_eq!(texts[2], ("T=%T".into(), [0, 0, 0]));
        // The step response curve is the only shape with a non-zero origin.
        let shifted = icon
            .graphics
            .iter()
            .filter(|g| g.origin != [0.0, 0.0])
            .collect::<Vec<_>>();
        assert_eq!(shifted.len(), 1);
        assert_eq!(shifted[0].origin, [-26.667, 6.667]);
        // ... and it is a Bezier curve, which only changes `smooth` (not drawn).
        match &shifted[0].shape {
            Shape::Line { points, .. } => assert_eq!(points.len(), 3),
            other => panic!("expected a Line, got {other:?}"),
        }
    }

    #[test]
    fn resistor_icon_has_a_filled_rectangle_and_lines() {
        let icon = parse(include_str!("../tests/fixtures/icon_Resistor.json"));
        // The `useHeatPort` line between the two texts is conditional and is dropped.
        assert_eq!(kinds(&icon), ["Rectangle", "Line", "Line", "Text", "Text"]);
        let rect = icon.shapes().next().expect("a rectangle");
        match rect {
            Shape::Rectangle {
                extent,
                style,
                radius,
            } => {
                assert_eq!(*radius, 0.0);
                assert_eq!(style.line, [0, 0, 255]);
                assert_eq!(style.fill, Some([255, 255, 255]));
                assert_eq!(extent.x1, -70.0);
                assert_eq!(extent.y2, -30.0);
            }
            other => panic!("expected a Rectangle, got {other:?}"),
        }
        // Nothing left is dashed: the dotted heat port line was the only one.
        assert!(icon.graphics.iter().all(|g| !matches!(
            &g.shape,
            Shape::Line { style, .. } if style.pattern != LinePattern::Solid
        )));
        // The label text carries `%name`.
        assert!(icon
            .shapes()
            .any(|s| matches!(s, Shape::Text { text, .. } if text == "%name")));
    }

    #[test]
    fn inertia_icon_retains_cylinder_fill_patterns() {
        let icon = parse(include_str!("../tests/fixtures/icon_Inertia.json"));
        assert_eq!(icon.len(), 15);
        // The final rectangle only has a `FillPattern.None` outline on top of the body.
        let filled: Vec<_> = icon
            .graphics
            .iter()
            .filter_map(|g| match &g.shape {
                Shape::Rectangle { style, radius, .. } => Some((style.fill, *radius)),
                _ => None,
            })
            .collect();
        assert_eq!(
            filled,
            [
                (Some([192, 192, 192]), 0.0),
                (Some([192, 192, 192]), 0.0),
                (Some([255, 255, 255]), 10.0),
                (None, 10.0),
            ]
        );
        assert_eq!(icon.graphics.iter().filter(|g| matches!(&g.shape,
            Shape::Rectangle { style, .. } if style.fill.is_some() && style.fill_pattern == FillPattern::HorizontalCylinder
        )).count(), 3);
        // Two texts: `%name` and `J=%J`.
        let texts: Vec<_> = icon
            .shapes()
            .filter_map(|s| match s {
                Shape::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, ["%name", "J=%J"]);
    }

    #[test]
    fn cylinder_fill_uses_line_color_at_edges_and_fill_color_at_center() {
        let style = Style {
            line: [64, 64, 64],
            fill: Some([255, 255, 255]),
            fill_pattern: FillPattern::HorizontalCylinder,
            ..Style::default()
        };
        let mesh = rectangle_fill_mesh(Extent::DEFAULT, &style, |p| {
            Pos2::new(p[0] as f32, p[1] as f32)
        })
        .unwrap();
        assert_eq!(mesh.vertices.len(), 6);
        assert_eq!(mesh.vertices[0].color, Color32::from_rgb(64, 64, 64));
        assert_eq!(mesh.vertices[2].color, Color32::WHITE);
        assert_eq!(mesh.vertices[4].color, Color32::from_rgb(64, 64, 64));
        assert_eq!(mesh.indices.len(), 12);
    }

    #[test]
    fn inherited_icon_is_found_through_extends() {
        let icon = parse(include_str!("../tests/fixtures/icon_InheritedBlock.json"));
        // The class itself has no annotation; the Icon of Modelica.Blocks.Icons.Block is
        // two `extends` levels down.
        assert_eq!(kinds(&icon), ["Rectangle", "Text"]);
        let first = icon.shapes().next().expect("the block frame");
        match first {
            Shape::Rectangle { extent, style, .. } => {
                assert_eq!(style.line, [0, 0, 127]);
                assert_eq!(*extent, Extent::DEFAULT);
            }
            other => panic!("expected a Rectangle, got {other:?}"),
        }
    }

    #[test]
    fn a_class_without_icon_is_reported_not_panicked() {
        let error = parse_icon(&fixture(r#"{"name":"M","restriction":"model"}"#))
            .expect_err("no Icon annotation");
        assert!(error.contains("Icon"), "{error}");
    }

    #[test]
    fn broken_graphics_are_skipped_and_listed() {
        let json = fixture(
            r#"{"name":"M","restriction":"model","annotation":{"Icon":{"graphics":[
                {"$kind":"record","name":"Line","elements":[true,[0,0],0]},
                {"$kind":"record","name":"Spline","elements":[true,[0,0],0]},
                {"$kind":"record","name":"Line","elements":[true,[0,0],0,[[0,0],[1,1]],[0,0,0],
                    {"$kind":"enum","name":"LinePattern.Solid","index":2},0.25,[],3,
                    {"$kind":"enum","name":"Smooth.None","index":1}]}
            ]}}}"#,
        );
        let icon = parse_icon(&json).expect("an Icon");
        assert_eq!(icon.len(), 1, "only the complete line survives");
        assert_eq!(icon.skipped.len(), 2, "{:?}", icon.skipped);
        assert!(
            icon.skipped[0].contains("Line without points"),
            "{:?}",
            icon.skipped
        );
        assert!(
            icon.skipped[1].contains("unsupported graphic Spline"),
            "{:?}",
            icon.skipped
        );
    }

    #[test]
    fn hidden_shapes_are_dropped_and_conditional_ones_too() {
        let line = r#"{"$kind":"record","name":"Line","elements":[VIS,[0,0],0,[[0,0],[1,1]],
            [0,0,0],{"$kind":"enum","name":"LinePattern.Solid","index":2},0.25,[],3,
            {"$kind":"enum","name":"Smooth.None","index":1}]}"#;
        let json = fixture(&format!(
            r#"{{"name":"M","restriction":"model","annotation":{{"Icon":{{"graphics":[
                {}, {}, {}
            ]}}}}}}"#,
            line.replace("VIS", "false"),
            line.replace(
                "VIS",
                r#"{"$kind":"cref","parts":[{"name":"useHeatPort"}]}"#
            ),
            line.replace("VIS", "true"),
        ));
        let icon = parse_icon(&json).expect("an Icon");
        assert_eq!(icon.len(), 1);
        assert!(icon.skipped.is_empty(), "{:?}", icon.skipped);
    }

    #[test]
    fn text_colour_falls_back_to_the_line_colour() {
        let json = fixture(
            r#"{"name":"M","restriction":"model","annotation":{"Icon":{"graphics":[
                {"$kind":"record","name":"Text","elements":[true,[0,0],0,[0,0,255],[0,0,0],
                    {"$kind":"enum","name":"LinePattern.Solid","index":2},
                    {"$kind":"enum","name":"FillPattern.None","index":1},0.25,
                    [[-10,-10],[10,10]],"hi",0,[-1,-1,-1],"",[],
                    {"$kind":"enum","name":"TextAlignment.Center","index":2}]}
            ]}}}"#,
        );
        let icon = parse_icon(&json).expect("an Icon");
        assert!(
            matches!(icon.shapes().next(), Some(Shape::Text { color, .. }) if *color == [0, 0, 255])
        );
    }

    #[test]
    fn ellipse_points_span_the_extent() {
        let pts = ellipse_points(Extent::DEFAULT, 0.0, 360.0);
        assert_eq!(pts.len(), 49);
        let (first, last) = (pts.first().unwrap(), pts.last().unwrap());
        assert!(
            (first[0] - last[0]).abs() < 1e-9 && (first[1] - last[1]).abs() < 1e-9,
            "a full ellipse closes: {first:?} vs {last:?}"
        );
        let xs: Vec<f64> = pts.iter().map(|p| p[0]).collect();
        let ys: Vec<f64> = pts.iter().map(|p| p[1]).collect();
        assert!(xs.iter().cloned().fold(f64::MIN, f64::max) - 100.0 < 1e-9);
        assert!(ys.iter().cloned().fold(f64::MIN, f64::max) - 100.0 < 1e-9);
        // A half ellipse from 0 to 180 degrees only covers the upper half.
        let half = ellipse_points(Extent::DEFAULT, 0.0, 180.0);
        assert!(half.iter().all(|p| p[1] >= -1e-9));
    }
}

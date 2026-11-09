//! Read-only diagram painting (G12).
use crate::diagram::{component_bounds, placement_transform, Diagram, PlacedComponent, Placement};
use crate::graphics::{Extent, Graphic, Shape, Style};
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape as EguiShape, Stroke, Vec2};

pub fn pin_position(component: &PlacedComponent, pin: &crate::diagram::ConnectorPin) -> [f64; 2] {
    let local = [
        (pin.extent.x1 + pin.extent.x2) / 2.0,
        (pin.extent.y1 + pin.extent.y2) / 2.0,
    ];
    let icon_extent = component
        .icon
        .as_ref()
        .map(|i| i.coordinate_extent)
        .unwrap_or(Extent::DEFAULT);
    crate::diagram::placement_transform(local, &icon_extent, &component.placement)
}

pub fn hit_pin(diagram: &Diagram, p: [f64; 2], tolerance: f64) -> Option<(usize, usize)> {
    diagram
        .components
        .iter()
        .enumerate()
        .rev()
        .find_map(|(ci, c)| {
            c.connectors.iter().enumerate().find_map(|(pi, pin)| {
                let at = pin_position(c, pin);
                ((at[0] - p[0]).hypot(at[1] - p[1]) <= tolerance).then_some((ci, pi))
            })
        })
}

pub fn orthogonal_route(a: [f64; 2], b: [f64; 2]) -> Vec<[f64; 2]> {
    if (a[1] - b[1]).abs() < 1e-9 {
        return vec![a, b];
    }
    let mx = snap((a[0] + b[0]) / 2.0, 2.0);
    vec![a, [mx, a[1]], [mx, b[1]], b]
}

pub fn point_segment_distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let d = ab[0] * ab[0] + ab[1] * ab[1];
    if d <= f64::EPSILON {
        return (p[0] - a[0]).hypot(p[1] - a[1]);
    }
    let t = (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / d).clamp(0.0, 1.0);
    (p[0] - (a[0] + t * ab[0])).hypot(p[1] - (a[1] + t * ab[1]))
}

pub fn hit_connection(diagram: &Diagram, p: [f64; 2], tolerance: f64) -> Option<usize> {
    diagram
        .connections
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, c)| {
            c.points
                .windows(2)
                .any(|s| point_segment_distance(p, s[0], s[1]) <= tolerance)
                .then_some(i)
        })
}

pub fn to_screen(p: [f64; 2], extent: &Extent, rect: Rect, zoom: f32, pan: Vec2) -> Pos2 {
    let (w, h) = (extent.x2 - extent.x1, extent.y2 - extent.y1);
    if w.abs() < 1e-9 || h.abs() < 1e-9 {
        return rect.center() + pan;
    }
    let base = (rect.width() / w.abs() as f32).min(rect.height() / h.abs() as f32);
    let scale = base * zoom;
    let cx = (extent.x1 + extent.x2) / 2.0;
    let cy = (extent.y1 + extent.y2) / 2.0;
    let c = rect.center() + pan;
    Pos2::new(
        c.x + (p[0] - cx) as f32 * scale,
        c.y - (p[1] - cy) as f32 * scale,
    )
}

/// Convert a screen position back into diagram coordinates (Modelica y points upward).
pub fn to_diagram(pos: Pos2, extent: &Extent, rect: Rect, zoom: f32, pan: Vec2) -> [f64; 2] {
    let (w, h) = (extent.x2 - extent.x1, extent.y2 - extent.y1);
    if w.abs() < 1e-9 || h.abs() < 1e-9 {
        return [0.0, 0.0];
    }
    let scale = (rect.width() / w.abs() as f32).min(rect.height() / h.abs() as f32) * zoom;
    if scale.abs() < 1e-9 {
        return [0.0, 0.0];
    }
    let center = rect.center() + pan;
    let cx = (extent.x1 + extent.x2) / 2.0;
    let cy = (extent.y1 + extent.y2) / 2.0;
    [
        cx + ((pos.x - center.x) / scale) as f64,
        cy - ((pos.y - center.y) / scale) as f64,
    ]
}
fn scale_of(extent: &Extent, rect: Rect, zoom: f32) -> f32 {
    let (w, h) = (extent.x2 - extent.x1, extent.y2 - extent.y1);
    if w.abs() < 1e-9 || h.abs() < 1e-9 {
        return zoom;
    }
    (rect.width() / w.abs() as f32).min(rect.height() / h.abs() as f32) * zoom
}
fn col(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}
fn stroke(style: &Style, scale: f32) -> Stroke {
    Stroke::new(
        (style.thickness as f32 * scale).clamp(1.0, 8.0),
        col(style.line),
    )
}
fn poly(painter: &Painter, pts: &[Pos2], style: &Style, scale: f32, closed: bool) {
    if pts.len() < 2 {
        return;
    }
    let mut path = pts.to_vec();
    if closed {
        path.push(pts[0]);
    }
    let s = stroke(style, scale);
    let shape = match style.pattern {
        crate::graphics::LinePattern::Solid => EguiShape::line(path, s),
        crate::graphics::LinePattern::Dash => {
            EguiShape::Vec(EguiShape::dashed_line(&path, s, 4.0, 3.0))
        }
        crate::graphics::LinePattern::Dot => {
            EguiShape::Vec(EguiShape::dashed_line(&path, s, 1.5, 3.0))
        }
        crate::graphics::LinePattern::DashDot => {
            EguiShape::Vec(EguiShape::dashed_line(&path, s, 6.0, 3.0))
        }
        crate::graphics::LinePattern::DashDotDot => {
            EguiShape::Vec(EguiShape::dashed_line(&path, s, 8.0, 3.0))
        }
    };
    painter.add(shape);
}
fn ellipse_pts(extent: Extent, start: f64, end: f64) -> Vec<[f64; 2]> {
    let cx = (extent.x1 + extent.x2) / 2.0;
    let cy = (extent.y1 + extent.y2) / 2.0;
    let rx = (extent.x2 - extent.x1) / 2.0;
    let ry = (extent.y2 - extent.y1) / 2.0;
    let sweep = (end - start).clamp(-360.0, 360.0);
    let steps = 48;
    (0..=steps)
        .map(|i| {
            let a = (start + sweep * i as f64 / steps as f64).to_radians();
            [cx + rx * a.cos(), cy + ry * a.sin()]
        })
        .collect()
}
#[allow(clippy::too_many_arguments)]
fn paint_graphics(
    painter: &Painter,
    graphics: &[Graphic],
    extent: &Extent,
    rect: Rect,
    zoom: f32,
    pan: Vec2,
    place: &dyn Fn([f64; 2], &Graphic) -> [f64; 2],
    component_name: &str,
    type_name: &str,
    max_text_width: Option<f32>,
) {
    let scale = scale_of(extent, rect, zoom);
    for g in graphics {
        match &g.shape {
            Shape::Line { points, style } => {
                let pts: Vec<Pos2> = points
                    .iter()
                    .map(|p| to_screen(place(*p, g), extent, rect, zoom, pan))
                    .collect();
                poly(painter, &pts, style, scale, false);
            }
            Shape::Polygon { points, style } => {
                let pts: Vec<Pos2> = points
                    .iter()
                    .map(|p| to_screen(place(*p, g), extent, rect, zoom, pan))
                    .collect();
                if let Some(fill) = style.fill {
                    painter.add(EguiShape::convex_polygon(
                        pts.clone(),
                        col(fill),
                        Stroke::NONE,
                    ));
                }
                poly(painter, &pts, style, scale, true);
            }
            Shape::Rectangle {
                extent: ext,
                radius,
                style,
            } => {
                let corners = [
                    [ext.x1, ext.y1],
                    [ext.x2, ext.y1],
                    [ext.x2, ext.y2],
                    [ext.x1, ext.y2],
                ];
                let pts: Vec<Pos2> = corners
                    .iter()
                    .map(|p| to_screen(place(*p, g), extent, rect, zoom, pan))
                    .collect();
                if let Some(fill) = style.fill {
                    painter.add(EguiShape::convex_polygon(
                        pts.clone(),
                        col(fill),
                        Stroke::NONE,
                    ));
                }
                if *radius > 0.0 {
                    let a = to_screen(place([ext.x1, ext.y1], g), extent, rect, zoom, pan);
                    let b = to_screen(place([ext.x2, ext.y2], g), extent, rect, zoom, pan);
                    let r = Rect::from_two_pos(a, b);
                    let rp = (*radius as f32 * scale).min(r.width().min(r.height()) / 2.0);
                    painter.rect_stroke(r, rp, stroke(style, scale), egui::StrokeKind::Middle);
                } else {
                    poly(painter, &pts, style, scale, true);
                }
            }
            Shape::Ellipse {
                extent: ext,
                start_angle,
                end_angle,
                style,
            } => {
                let pts: Vec<Pos2> = ellipse_pts(*ext, *start_angle, *end_angle)
                    .into_iter()
                    .map(|p| to_screen(place(p, g), extent, rect, zoom, pan))
                    .collect();
                let full = (end_angle - start_angle).abs() >= 359.999;
                if full {
                    if let Some(fill) = style.fill {
                        painter.add(EguiShape::convex_polygon(
                            pts.clone(),
                            col(fill),
                            Stroke::NONE,
                        ));
                    }
                }
                poly(painter, &pts, style, scale, full);
            }
            Shape::Text {
                extent: ext,
                text,
                color: tc,
            } => {
                let a = to_screen(place([ext.x1, ext.y1], g), extent, rect, zoom, pan);
                let b = to_screen(place([ext.x2, ext.y2], g), extent, rect, zoom, pan);
                let br = Rect::from_two_pos(a, b);
                let label = text
                    .replace("%name", component_name)
                    .replace("%class", type_name);
                if label.trim().is_empty() {
                    continue;
                }
                let mut size = (br.height() * 0.9).clamp(8.0, 48.0);
                let width = br.width().min(max_text_width.unwrap_or(br.width()));
                let measured = painter
                    .layout_no_wrap(label.clone(), FontId::proportional(size), col(*tc))
                    .size()
                    .x;
                if measured > width && measured > 0.0 {
                    size = (size * width / measured).max(6.0);
                }
                painter.text(
                    br.center(),
                    Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(size),
                    col(*tc),
                );
            }
        }
    }
}
pub fn paint_diagram(painter: &Painter, rect: Rect, diagram: &Diagram, zoom: f32, pan: Vec2) {
    let extent = &diagram.coordinate_extent;
    let scale = scale_of(extent, rect, zoom);
    paint_graphics(
        painter,
        &diagram.own_graphics,
        extent,
        rect,
        zoom,
        pan,
        &|p, g| crate::graphics::transform(p, g.origin, g.rotation_deg),
        "",
        "",
        None,
    );
    for conn in &diagram.connections {
        let pts: Vec<Pos2> = conn
            .points
            .iter()
            .map(|p| to_screen(*p, extent, rect, zoom, pan))
            .collect();
        if pts.len() >= 2 {
            let st = Style {
                line: conn.color,
                fill: None,
                thickness: 0.25,
                pattern: crate::graphics::LinePattern::Solid,
            };
            poly(painter, &pts, &st, scale, false);
        }
    }
    for comp in &diagram.components {
        if let Some(icon) = &comp.icon {
            let icon_extent = icon.coordinate_extent;
            let placement = &comp.placement;
            let component_rect = bounds_screen_rect(placement, extent, rect, zoom, pan);
            paint_graphics(
                painter,
                &icon.graphics,
                extent,
                rect,
                zoom,
                pan,
                &|p, _g| placement_transform(p, &icon_extent, placement),
                &comp.name,
                &comp.type_name,
                Some(component_rect.width()),
            );
        } else {
            // No icon: the placement box is all there is to draw. It already includes the
            // `origin` (and the rotation), so the box must not be offset again.
            let r = bounds_screen_rect(&comp.placement, extent, rect, zoom, pan);
            painter.rect_stroke(
                r,
                0.0,
                Stroke::new(1.0, Color32::BLACK),
                egui::StrokeKind::Inside,
            );
        }
        let has_name_text = comp.icon.as_ref().is_some_and(|icon| {
            icon.graphics
                .iter()
                .any(|g| matches!(&g.shape, Shape::Text { text, .. } if text.contains("%name")))
        });
        if !has_name_text {
            let r = bounds_screen_rect(&comp.placement, extent, rect, zoom, pan);
            painter.text(
                r.center(),
                Align2::CENTER_CENTER,
                comp.name.clone(),
                FontId::proportional(11.0),
                Color32::BLACK,
            );
        }
        for pin in &comp.connectors {
            let p = to_screen(pin_position(comp, pin), extent, rect, zoom, pan);
            painter.circle_filled(p, 2.5, Color32::from_rgb(0x20, 0x60, 0xA0));
        }
    }
}
pub fn component_screen_rect(
    diagram: &Diagram,
    rect: Rect,
    zoom: f32,
    pan: Vec2,
    comp: &PlacedComponent,
) -> Rect {
    bounds_screen_rect(&comp.placement, &diagram.coordinate_extent, rect, zoom, pan)
}

/// Component at a diagram coordinate; later-painted components win when boxes overlap.
pub fn hit_component(diagram: &Diagram, p: [f64; 2]) -> Option<usize> {
    diagram
        .components
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, comp)| {
            let b = component_bounds(&comp.placement);
            (p[0] >= b.x1 && p[0] <= b.x2 && p[1] >= b.y1 && p[1] <= b.y2).then_some(i)
        })
}

pub fn snap(v: f64, grid: f64) -> f64 {
    if !grid.is_finite() || grid <= 0.0 {
        return v;
    }
    (v / grid).round() * grid
}

pub fn placement_edit(
    comp: &PlacedComponent,
    delta: [f64; 2],
    rotation_delta: f64,
) -> (Placement, crate::edit::EditOp) {
    let mut placement = comp.placement.clone();
    placement.origin[0] += delta[0];
    placement.origin[1] += delta[1];
    placement.rotation_deg = (placement.rotation_deg + rotation_delta).rem_euclid(360.0);
    let op = crate::edit::EditOp::SetPlacement {
        component: comp.name.clone(),
        type_name: comp.type_name.clone(),
        extent: [
            [placement.extent.x1, placement.extent.y1],
            [placement.extent.x2, placement.extent.y2],
        ],
        origin: placement.origin,
        rotation: placement.rotation_deg,
    };
    (placement, op)
}

/// Screen rectangle of a placement's bounding box.
///
/// `component_bounds` already applies `origin` and `rotation`, so nothing is added here;
/// doing so would move a component twice.
fn bounds_screen_rect(
    placement: &Placement,
    extent: &Extent,
    rect: Rect,
    zoom: f32,
    pan: Vec2,
) -> Rect {
    let b = component_bounds(placement);
    let pts = [[b.x1, b.y1], [b.x2, b.y1], [b.x2, b.y2], [b.x1, b.y2]];
    let screen: Vec<Pos2> = pts
        .iter()
        .map(|p| to_screen(*p, extent, rect, zoom, pan))
        .collect();
    let min = Pos2::new(
        screen.iter().map(|p| p.x).fold(f32::INFINITY, f32::min),
        screen.iter().map(|p| p.y).fold(f32::INFINITY, f32::min),
    );
    let max = Pos2::new(
        screen.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max),
        screen.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max),
    );
    Rect::from_min_max(min, max)
}

#[cfg(test)]
mod edit_tests {
    use super::*;
    use crate::diagram::{Diagram, PlacedComponent, Placement};
    use crate::graphics::Extent;

    #[test]
    fn screen_diagram_conversion_round_trips_zoom_and_pan() {
        let ex = Extent::DEFAULT;
        let rect = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(800.0, 600.0));
        for (zoom, pan) in [
            (1.0, Vec2::ZERO),
            (2.0, Vec2::new(40.0, -15.0)),
            (0.4, Vec2::new(-8.0, 21.0)),
        ] {
            let p = [23.5, -41.25];
            let got = to_diagram(to_screen(p, &ex, rect, zoom, pan), &ex, rect, zoom, pan);
            assert!((got[0] - p[0]).abs() < 1e-4 && (got[1] - p[1]).abs() < 1e-4);
        }
    }
    #[test]
    fn hit_prefers_later_components_and_handles_rotated_bounds() {
        let mut d = Diagram::default();
        let p = Placement {
            extent: Extent {
                x1: -10.0,
                y1: -5.0,
                x2: 10.0,
                y2: 5.0,
            },
            origin: [0.0, 0.0],
            rotation_deg: 90.0,
        };
        d.components.push(PlacedComponent {
            name: "first".into(),
            type_name: "A".into(),
            placement: p.clone(),
            icon: None,
            connectors: vec![],
        });
        d.components.push(PlacedComponent {
            name: "last".into(),
            type_name: "A".into(),
            placement: p,
            icon: None,
            connectors: vec![],
        });
        assert_eq!(hit_component(&d, [0.0, 9.0]), Some(1));
        assert_eq!(hit_component(&d, [20.0, 20.0]), None);
    }
    #[test]
    fn snap_handles_negative_and_ties() {
        assert_eq!(snap(-2.4, 2.0), -2.0);
        assert_eq!(snap(-3.0, 2.0), -4.0);
        assert_eq!(snap(3.0, 2.0), 4.0);
    }
    #[test]
    fn drag_produces_one_placement_edit_and_rotation_is_counterclockwise_step() {
        let comp = PlacedComponent {
            name: "k".into(),
            type_name: "T".into(),
            placement: Placement::default(),
            icon: None,
            connectors: vec![],
        };
        let (p, op) = placement_edit(&comp, [2.0, -4.0], 90.0);
        assert_eq!(p.origin, [2.0, -4.0]);
        assert_eq!(p.rotation_deg, 90.0);
        assert!(matches!(op,crate::edit::EditOp::SetPlacement{component,..} if component=="k"));
    }

    #[test]
    fn route_hits_and_pin_positions_follow_component_placement() {
        assert_eq!(
            orthogonal_route([1.0, 2.0], [9.0, 2.0]),
            vec![[1.0, 2.0], [9.0, 2.0]]
        );
        assert_eq!(
            orthogonal_route([9.0, 8.0], [-1.0, -2.0]),
            vec![[9.0, 8.0], [4.0, 8.0], [4.0, -2.0], [-1.0, -2.0]]
        );
        assert_eq!(
            point_segment_distance([2.0, 3.0], [0.0, 0.0], [4.0, 0.0]),
            3.0
        );
        assert_eq!(
            point_segment_distance([8.0, 0.0], [0.0, 0.0], [4.0, 0.0]),
            4.0
        );
        let pin = crate::diagram::ConnectorPin {
            name: "p".into(),
            extent: Extent {
                x1: 0.0,
                y1: 0.0,
                x2: 0.0,
                y2: 0.0,
            },
        };
        let mut c = PlacedComponent {
            name: "a".into(),
            type_name: "C".into(),
            placement: Placement {
                origin: [20.0, 10.0],
                ..Placement::default()
            },
            icon: None,
            connectors: vec![pin],
        };
        let expected = pin_position(&c, &c.connectors[0]);
        assert_eq!(expected, [20.0, 10.0]);
        let mut d = Diagram {
            components: vec![c.clone()],
            ..Diagram::default()
        };
        d.connections.push(crate::diagram::Connection {
            from: "a.p".into(),
            to: "b.p".into(),
            points: vec![[20.0, 10.0], [30.0, 10.0]],
            color: [0; 3],
        });
        assert_eq!(hit_pin(&d, expected, 0.1), Some((0, 0)));
        assert_eq!(hit_connection(&d, [25.0, 13.0], 3.1), Some(0));
        assert_eq!(hit_connection(&d, [25.0, 13.0], 2.9), None);
        c.placement.origin[0] += 2.0;
        assert_eq!(pin_position(&c, &c.connectors[0]), [22.0, 10.0]);
    }
}

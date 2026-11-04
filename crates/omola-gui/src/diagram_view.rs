//! Read-only diagram painting (G12).
use crate::diagram::{placement_transform, Diagram, PlacedComponent};
use crate::graphics::{Extent, Graphic, Shape, Style};
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape as EguiShape, Stroke, Vec2};

fn to_screen(p: [f64; 2], extent: &Extent, rect: Rect, zoom: f32, pan: Vec2) -> Pos2 {
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
    class_name: &str,
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
                let label = text.replace("%name", class_name);
                if label.trim().is_empty() {
                    continue;
                }
                let size = (br.height() * 0.9).clamp(8.0, 48.0);
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
            paint_graphics(
                painter,
                &icon.graphics,
                extent,
                rect,
                zoom,
                pan,
                &|p, _g| placement_transform(p, &icon_extent, placement),
                &comp.type_name,
            );
        } else {
            let a = to_screen(
                [comp.placement.extent.x1, comp.placement.extent.y1],
                extent,
                rect,
                zoom,
                pan,
            );
            let b = to_screen(
                [comp.placement.extent.x2, comp.placement.extent.y2],
                extent,
                rect,
                zoom,
                pan,
            );
            let r = Rect::from_two_pos(a, b);
            painter.rect_stroke(
                r,
                0.0,
                Stroke::new(1.0, Color32::BLACK),
                egui::StrokeKind::Inside,
            );
        }
        let center = to_screen(comp.placement.origin, extent, rect, zoom, pan);
        let half_h =
            ((comp.placement.extent.y2 - comp.placement.extent.y1).abs() / 2.0) as f32 * scale;
        painter.text(
            Pos2::new(center.x, center.y + half_h + 8.0),
            Align2::CENTER_TOP,
            comp.name.clone(),
            FontId::proportional(11.0),
            Color32::BLACK,
        );
    }
}
pub fn component_screen_rect(
    diagram: &Diagram,
    rect: Rect,
    zoom: f32,
    pan: Vec2,
    comp: &PlacedComponent,
) -> Rect {
    let extent = &diagram.coordinate_extent;
    let pts = [
        [comp.placement.extent.x1, comp.placement.extent.y1],
        [comp.placement.extent.x2, comp.placement.extent.y1],
        [comp.placement.extent.x2, comp.placement.extent.y2],
        [comp.placement.extent.x1, comp.placement.extent.y2],
    ];
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

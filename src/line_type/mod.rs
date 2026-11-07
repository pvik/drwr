use std::f32::consts::FRAC_PI_2;

use egui::emath::GuiRounding as _;
use egui::epaint::EllipseShape;
use egui::{Color32, Mesh, Painter, Pos2, Rangef, Rect, Stroke, StrokeKind, Vec2, pos2, vec2};
use graphml::yed::LineType;

/// The line types the side panel offers, in its order.
pub const ALL: [LineType; 4] = [
    LineType::Line,
    LineType::Dashed,
    LineType::Dotted,
    LineType::DashedDotted,
];

/// More would take too long to paint, e.g. round a node of an absurd size.
const MAX_PERIODS: f32 = 10_000.0;

/// How a line type reads in the side panel. Unknown ones read as written.
pub fn name(line_type: &LineType) -> &str {
    match line_type {
        LineType::Line => "Solid",
        LineType::Dashed => "Dashed",
        LineType::Dotted => "Dotted",
        LineType::DashedDotted => "Dash-dotted",
        LineType::Other(keyword) => keyword,
    }
}

/// Paints `rect`'s outline in `stroke` and `line_type`, centered on it.
pub fn paint_rect_outline(painter: &Painter, rect: Rect, stroke: Stroke, line_type: &LineType) {
    let Some(pattern) = pattern(line_type) else {
        painter.rect_stroke(rect, 0.0, stroke, StrokeKind::Middle);
        return;
    };
    // Where egui puts a solid outline.
    let mut sides = [rect.min.x, rect.min.y, rect.max.x, rect.max.y];
    for side in &mut sides {
        stroke.round_center_to_pixel(painter.pixels_per_point(), side);
    }
    let [left, top, right, bottom] = sides;
    let half = stroke.width / 2.0;
    // The top and bottom take the corners.
    let (upper, lower) = (top + half, (bottom - half).max(top + half));
    let strips = [
        [pos2(left - half, top), pos2(right + half, top)],
        [pos2(right, upper), pos2(right, lower)],
        [pos2(right + half, bottom), pos2(left - half, bottom)],
        [pos2(left, lower), pos2(left, upper)],
    ];
    paint_dashes(painter, &strips, stroke, pattern);
}

/// Paints the outline of the ellipse filling `rect` in `stroke` and
/// `line_type`, centered on it.
pub fn paint_ellipse_outline(painter: &Painter, rect: Rect, stroke: Stroke, line_type: &LineType) {
    let (center, radius) = (rect.center(), rect.size() / 2.0);
    let Some(pattern) = pattern(line_type) else {
        // egui strokes an ellipse outside its outline, so the outline is
        // pulled in by half the width. A stroke wider than that covers it.
        let half = Vec2::splat(stroke.width / 2.0);
        let shape = if radius.min_elem() > half.x {
            EllipseShape::stroke(center, radius - half, stroke)
        } else {
            EllipseShape::filled(center, radius + half, stroke.color)
        };
        painter.add(shape);
        return;
    };
    // Along the polygon egui draws the ellipse as, so that the dashes lie
    // where its solid outline does.
    let points = ellipse_points(center, radius, painter.pixels_per_point());
    let next = points.iter().cycle().skip(1);
    let strips: Vec<[Pos2; 2]> = points.iter().zip(next).map(|(&a, &b)| [a, b]).collect();
    paint_dashes(painter, &strips, stroke, pattern);
}

/// The corners of the polygon egui draws an ellipse as, clockwise from its
/// top. Empty if it has no width or height, as egui draws none then.
fn ellipse_points(center: Pos2, radius: Vec2, pixels_per_point: f32) -> Vec<Pos2> {
    if radius.x <= 0.0 || radius.y <= 0.0 {
        return Vec::new();
    }
    // Like egui, at least 8 points a quarter, more for big ellipses, eased
    // towards the tight bends.
    let max_radius = (radius.max_elem() * pixels_per_point) as u32;
    let num_points = u32::max(8, max_radius / 16);
    let ratio = ((radius.y / radius.x) * 0.5).clamp(0.0, 1.0);
    let quarter: Vec<Vec2> = (1..num_points)
        .map(|i| {
            let percent = i as f32 / num_points as f32;
            let eased = 2.0 * (percent - percent.powf(2.0)) * ratio + percent.powf(2.0);
            let t = eased * FRAC_PI_2;
            vec2(radius.x * t.cos(), radius.y * t.sin())
        })
        .collect();
    let mut points = Vec::with_capacity(4 * num_points as usize);
    points.push(center + vec2(0.0, -radius.y));
    points.extend(quarter.iter().rev().map(|p| center + vec2(p.x, -p.y)));
    points.push(center + vec2(radius.x, 0.0));
    points.extend(quarter.iter().map(|p| center + *p));
    points.push(center + vec2(0.0, radius.y));
    points.extend(quarter.iter().rev().map(|p| center + vec2(-p.x, p.y)));
    points.push(center + vec2(-radius.x, 0.0));
    points.extend(quarter.iter().map(|p| center - *p));
    points
}

/// Paints a line across `x` at `y` in `stroke` and `line_type`.
pub fn paint_hline(painter: &Painter, x: Rangef, y: f32, stroke: Stroke, line_type: &LineType) {
    let Some(pattern) = pattern(line_type) else {
        painter.hline(x, y, stroke);
        return;
    };
    let mut y = y;
    stroke.round_center_to_pixel(painter.pixels_per_point(), &mut y);
    paint_dashes(
        painter,
        &[[pos2(x.min, y), pos2(x.max, y)]],
        stroke,
        pattern,
    );
}

/// The lengths of a line type's dashes and the gaps after them, in line
/// widths. `None` for solid lines, which unknown types are drawn as.
fn pattern(line_type: &LineType) -> Option<&'static [[f32; 2]]> {
    match line_type {
        LineType::Line | LineType::Other(_) => None,
        LineType::Dashed => Some(&[[4.0, 2.0]]),
        LineType::Dotted => Some(&[[1.0, 1.0]]),
        LineType::DashedDotted => Some(&[[4.0, 2.0], [1.0, 2.0]]),
    }
}

/// Paints dashes of `pattern` along `strips`, as if they were one line. Each
/// strip is `stroke` wide, from one end of its middle to the other.
fn paint_dashes(painter: &Painter, strips: &[[Pos2; 2]], stroke: Stroke, pattern: &[[f32; 2]]) {
    if stroke.is_empty() {
        return;
    }
    let pixels_per_point = painter.pixels_per_point();
    // As faint as egui paints solid lines thinner than a pixel.
    let color = stroke
        .color
        .gamma_multiply((stroke.width * pixels_per_point).min(1.0));
    // At least a pixel thick, as egui draws thinner lines.
    let half = stroke.width.max(1.0 / pixels_per_point) / 2.0;
    // At least a point, so that thin lines still show their dots.
    let unit = stroke.width.max(1.0);
    let mut mesh = Mesh::default();
    for dash in dashes(strips, pattern, unit) {
        let length: f32 = dash.iter().map(|[from, to]| from.distance(*to)).sum();
        let curved = dash
            .iter()
            .any(|[from, to]| from.x != to.x && from.y != to.y);
        // A dot on a curve is square whichever way the curve runs, and
        // snapped to pixels like the rects below, so none fall between them.
        // Dots are a unit long, where the shortest dash is four.
        if curved && length <= 1.5 * unit {
            let (from, to) = (dash[0][0], dash[dash.len() - 1][1]);
            let rect = Rect::from_center_size(from.lerp(to, 0.5), Vec2::splat(unit));
            mesh.add_colored_rect(rect.round_to_pixels(pixels_per_point), color);
            continue;
        }
        for [from, to] in dash {
            let across = (to - from).normalized().rot90() * half;
            if from.x == to.x || from.y == to.y {
                // Rects snapped to pixels, unlike egui's, whose feathered line
                // caps would blur small dots into a solid line.
                let rect = Rect::from_two_pos(from - across, to + across);
                mesh.add_colored_rect(rect.round_to_pixels(pixels_per_point), color);
            } else {
                // Along a curve, which can't be snapped.
                let corners = [from - across, to - across, from + across, to + across];
                add_quad(&mut mesh, corners, color);
            }
        }
    }
    painter.add(mesh);
}

/// Adds a quad with `corners` in the order [`Mesh::add_colored_rect`] uses.
fn add_quad(mesh: &mut Mesh, corners: [Pos2; 4], color: Color32) {
    let index = mesh.vertices.len() as u32;
    for corner in corners {
        mesh.colored_vertex(corner, color);
    }
    mesh.add_triangle(index, index + 1, index + 2);
    mesh.add_triangle(index + 2, index + 1, index + 3);
}

/// The dashes of `pattern` laid along `strips` in order, as if they were one
/// line, each as the pieces of the strips it covers. `pattern` has the
/// lengths of dashes and the gaps after them, in `unit`s.
fn dashes(strips: &[[Pos2; 2]], pattern: &[[f32; 2]], unit: f32) -> Vec<Vec<[Pos2; 2]>> {
    let lengths: Vec<f32> = strips.iter().map(|[from, to]| from.distance(*to)).collect();
    let period = unit * pattern.iter().flatten().sum::<f32>();
    let periods = (lengths.iter().sum::<f32>() / period).ceil();
    // Also catches lengths that aren't finite.
    if !(0.0..=MAX_PERIODS).contains(&periods) {
        return vec![strips.to_vec()];
    }
    let mut dashes: Vec<Vec<[Pos2; 2]>> = Vec::new();
    let mut offset = 0.0;
    // Kept from one strip to the next, so a dash crossing between them is cut
    // at the same point on both.
    let mut period_start = 0.0;
    for (&[from, to], &length) in strips.iter().zip(&lengths) {
        let end = offset + length;
        let direction = (to - from).normalized();
        let point = |distance: f32| from + direction * (distance - offset);
        let mut start = period_start;
        while start < end {
            period_start = start;
            for &[dash, gap] in pattern {
                let (a, b) = (start.max(offset), end.min(start + unit * dash));
                if a < b {
                    let piece = [
                        if a == offset { from } else { point(a) },
                        if b == end { to } else { point(b) },
                    ];
                    match dashes.last_mut() {
                        // A dash begun on an earlier strip runs on.
                        Some(last) if start < offset => last.push(piece),
                        _ => dashes.push(vec![piece]),
                    }
                }
                start += unit * (dash + gap);
            }
        }
        offset = end;
    }
    dashes
}

#[cfg(test)]
mod tests;

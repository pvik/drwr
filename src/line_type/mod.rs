use egui::emath::GuiRounding as _;
use egui::{Mesh, Painter, Pos2, Rangef, Rect, Stroke, StrokeKind, pos2};
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
pub fn paint_outline(painter: &Painter, rect: Rect, stroke: Stroke, line_type: &LineType) {
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

/// Paints dashes of `pattern` along horizontal and vertical `strips`, as if
/// they were one line. Each strip is `stroke` wide, from one end of its
/// middle to the other.
fn paint_dashes(painter: &Painter, strips: &[[Pos2; 2]], stroke: Stroke, pattern: &[[f32; 2]]) {
    if stroke.is_empty() {
        return;
    }
    let pixels_per_point = painter.pixels_per_point();
    // As faint as egui paints solid lines thinner than a pixel.
    let color = stroke
        .color
        .gamma_multiply((stroke.width * pixels_per_point).min(1.0));
    let half = stroke.width / 2.0;
    // At least a point, so that thin lines still show their dots.
    let unit = stroke.width.max(1.0);
    // Rects snapped to pixels, unlike egui's, whose feathered line caps would
    // blur small dots into a solid line.
    let mut mesh = Mesh::default();
    for [from, to] in dashes(strips, pattern, unit) {
        let across = (to - from).normalized().rot90() * half;
        let rect = Rect::from_two_pos(from - across, to + across);
        mesh.add_colored_rect(rect.round_to_pixels(pixels_per_point), color);
    }
    painter.add(mesh);
}

/// The pieces of `strips` that dashes of `pattern` cover, laid along them in
/// order as if they were one line. `pattern` has the lengths of dashes and the
/// gaps after them, in `unit`s.
fn dashes(strips: &[[Pos2; 2]], pattern: &[[f32; 2]], unit: f32) -> Vec<[Pos2; 2]> {
    let lengths: Vec<f32> = strips.iter().map(|[from, to]| from.distance(*to)).collect();
    let period = unit * pattern.iter().flatten().sum::<f32>();
    let periods = (lengths.iter().sum::<f32>() / period).ceil();
    // Also catches lengths that aren't finite.
    if !(0.0..=MAX_PERIODS).contains(&periods) {
        return strips.to_vec();
    }
    let mut pieces = Vec::new();
    let mut offset = 0.0;
    for (&[from, to], &length) in strips.iter().zip(&lengths) {
        let end = offset + length;
        let direction = (to - from).normalized();
        let point = |distance: f32| from + direction * (distance - offset);
        let mut start = (offset / period).floor() * period;
        while start < end {
            for &[dash, gap] in pattern {
                let (a, b) = (start.max(offset), end.min(start + unit * dash));
                if a < b {
                    pieces.push([point(a), point(b)]);
                }
                start += unit * (dash + gap);
            }
        }
        offset = end;
    }
    pieces
}

#[cfg(test)]
mod tests;

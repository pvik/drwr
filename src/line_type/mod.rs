use std::f32::consts::FRAC_PI_2;
use std::iter;

use egui::emath::GuiRounding as _;
use egui::epaint::PathShape;
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
/// As many as egui gives an ellipse 32,000 pixels across. More would take
/// too long to paint for one of an absurd size.
const MAX_QUARTER_POINTS: u32 = 1_000;
/// How far a curved dash's corners may reach, in half widths.
const MAX_MITER: f32 = 2.0;
/// Corners rounded by less, in pixels, look square, and would be drawn from
/// points too close together to turn accurately.
const MIN_CORNER_RADIUS: f32 = 0.5;

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
    let Rect { min, max } = snap_outline(rect, stroke, painter.pixels_per_point());
    let [left, top, right, bottom] = [min.x, min.y, max.x, max.y];
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
/// `line_type`, centered on [`ellipse_points`].
pub fn paint_ellipse_outline(painter: &Painter, rect: Rect, stroke: Stroke, line_type: &LineType) {
    if rect.size().min_elem() == 0.0 {
        // A flat ellipse is a line, which yEd draws in every line type.
        paint_rect_outline(painter, rect, stroke, line_type);
        return;
    }
    let points = ellipse_points(rect, painter.pixels_per_point());
    paint_outline(painter, points, stroke, line_type);
}

/// Paints the outline of the polygon with corners `points` in `stroke` and
/// `line_type`, centered on it.
pub fn paint_outline(painter: &Painter, points: Vec<Pos2>, stroke: Stroke, line_type: &LineType) {
    let Some(pattern) = pattern(line_type) else {
        painter.add(PathShape::closed_line(points, stroke));
        return;
    };
    let next = points.iter().cycle().skip(1);
    let strips: Vec<[Pos2; 2]> = points.iter().zip(next).map(|(&a, &b)| [a, b]).collect();
    paint_dashes(painter, &strips, stroke, pattern);
}

/// Paints a line through `points` in `stroke` and `line_type`.
pub fn paint_polyline(painter: &Painter, points: &[Pos2], stroke: Stroke, line_type: &LineType) {
    let Some(pattern) = pattern(line_type) else {
        painter.add(PathShape::line(points.to_vec(), stroke));
        return;
    };
    let half = stroke.width.max(1.0 / painter.pixels_per_point()) / 2.0;
    let mut strips: Vec<[Pos2; 2]> = points
        .windows(2)
        .filter(|pair| pair[0] != pair[1])
        .map(|pair| [pair[0], pair[1]])
        .collect();
    // Like a rect's outline, at square corners the strip before takes the
    // corner, so the strips neither overlap nor leave a notch.
    for i in 1..strips.len() {
        let before = (strips[i - 1][1] - strips[i - 1][0]).normalized();
        let after = (strips[i][1] - strips[i][0]).normalized();
        let level = |v: Vec2| v.x == 0.0 || v.y == 0.0;
        if level(before) && level(after) && before.dot(after) == 0.0 {
            strips[i - 1][1] += before * half;
            strips[i][0] += after * half;
        }
    }
    paint_dashes(painter, &strips, stroke, pattern);
}

/// The corners of the polygon `rect` is drawn as with its corners rounded to
/// `radius`, but by no more than half of each side, like Java, which yEd is
/// written in. Its sides are where egui puts `rect`'s solid outline in
/// `stroke`. It runs counterclockwise from the top of the left side, where
/// Java starts it. Empty if the corners would look square.
pub fn rounded_rect_points(
    rect: Rect,
    radius: f32,
    stroke: Stroke,
    pixels_per_point: f32,
) -> Vec<Pos2> {
    let rect = snap_outline(rect, stroke, pixels_per_point);
    let radius = Vec2::splat(radius).min(rect.size() / 2.0);
    // Also catches inverted rects.
    if !(rect.is_finite() && radius.min_elem() * pixels_per_point >= MIN_CORNER_RADIUS) {
        return Vec::new();
    }
    let quarter = quarter_points(radius, pixels_per_point);
    // The corners' centers, kept from crossing by rounding errors.
    let (min, max) = (rect.min + radius, rect.max - radius);
    let max = max.max(min);
    let [left, top, right, bottom] = [rect.min.x, rect.min.y, rect.max.x, rect.max.y];
    let mut points = Vec::with_capacity(4 * (quarter.len() + 2));
    add_side(&mut points, pos2(left, min.y), pos2(left, max.y));
    let corner = pos2(min.x, max.y);
    points.extend(quarter.iter().map(|p| corner + vec2(-p.x, p.y)));
    add_side(&mut points, pos2(min.x, bottom), pos2(max.x, bottom));
    points.extend(quarter.iter().rev().map(|p| max + *p));
    add_side(&mut points, pos2(right, max.y), pos2(right, min.y));
    let corner = pos2(max.x, min.y);
    points.extend(quarter.iter().map(|p| corner + vec2(p.x, -p.y)));
    add_side(&mut points, pos2(max.x, top), pos2(min.x, top));
    points.extend(quarter.iter().rev().map(|p| min - *p));
    points
}

/// Adds the ends of a side, which are the same if it's rounded all the way.
fn add_side(points: &mut Vec<Pos2>, from: Pos2, to: Pos2) {
    points.push(from);
    if to != from {
        points.push(to);
    }
}

/// Where egui puts `rect`'s solid outline in `stroke`, or its fill if
/// `stroke` has no width.
fn snap_outline(rect: Rect, stroke: Stroke, pixels_per_point: f32) -> Rect {
    if stroke.width <= 0.0 {
        return rect.round_to_pixels(pixels_per_point);
    }
    let Rect { mut min, mut max } = rect;
    for side in [&mut min.x, &mut min.y, &mut max.x, &mut max.y] {
        stroke.round_center_to_pixel(pixels_per_point, side);
    }
    Rect::from_min_max(min, max)
}

/// The corners of the polygon the ellipse filling `rect` is drawn as,
/// clockwise from its right end, where Java, which yEd is written in, starts
/// them. Empty if it has no width or height.
pub fn ellipse_points(rect: Rect, pixels_per_point: f32) -> Vec<Pos2> {
    let (center, radius) = (rect.center(), rect.size() / 2.0);
    if !(rect.is_finite() && radius.min_elem() > 0.0) {
        return Vec::new();
    }
    let quarter = quarter_points(radius, pixels_per_point);
    let mut points = Vec::with_capacity(4 * (quarter.len() + 1));
    points.push(center + vec2(radius.x, 0.0));
    points.extend(quarter.iter().map(|p| center + *p));
    points.push(center + vec2(0.0, radius.y));
    points.extend(quarter.iter().rev().map(|p| center + vec2(-p.x, p.y)));
    points.push(center + vec2(-radius.x, 0.0));
    points.extend(quarter.iter().map(|p| center - *p));
    points.push(center + vec2(0.0, -radius.y));
    points.extend(quarter.iter().rev().map(|p| center + vec2(p.x, -p.y)));
    points
}

/// The points between the ends of a quarter of the ellipse of `radius`
/// around zero, from its right end down. Like egui, at least 8 make up a
/// quarter, more for big ellipses, eased towards the tight bends.
fn quarter_points(radius: Vec2, pixels_per_point: f32) -> Vec<Vec2> {
    let max_radius = (radius.max_elem() * pixels_per_point) as u32;
    let num_points = (max_radius / 16).clamp(8, MAX_QUARTER_POINTS);
    let ratio = ((radius.y / radius.x) * 0.5).clamp(0.0, 1.0);
    (1..num_points)
        .map(|i| {
            let percent = i as f32 / num_points as f32;
            let eased = 2.0 * (percent - percent.powf(2.0)) * ratio + percent.powf(2.0);
            let t = eased * FRAC_PI_2;
            vec2(radius.x * t.cos(), radius.y * t.sin())
        })
        .collect()
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
/// widths, or points for lines thinner than that. `None` for solid lines,
/// which unknown types are drawn as.
pub fn pattern(line_type: &LineType) -> Option<&'static [[f32; 2]]> {
    match line_type {
        LineType::Line | LineType::Other(_) => None,
        LineType::Dashed => Some(&[[4.0, 2.0]]),
        LineType::Dotted => Some(&[[1.0, 1.0]]),
        LineType::DashedDotted => Some(&[[4.0, 2.0], [1.0, 2.0]]),
    }
}

/// The part of a dash on one strip.
#[derive(Clone, Copy, Debug)]
struct Piece {
    line: [Pos2; 2],
    /// The strip's, which a tiny piece couldn't give accurately.
    direction: Vec2,
    /// Whether it carries on the dash of the piece before, from the strip
    /// before.
    runs_on: bool,
    /// Whether its dash is a dot, a unit long.
    dot: bool,
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
    let straight = |piece: &Piece| piece.direction.x == 0.0 || piece.direction.y == 0.0;
    let mut mesh = Mesh::default();
    for dash in dashes(strips, pattern, unit).chunk_by(|_, next| next.runs_on) {
        if dash.iter().all(straight) {
            // Rects snapped to pixels, unlike egui's, whose feathered line
            // caps would blur small dots into a solid line.
            for piece in dash {
                let [from, to] = piece.line;
                let across = piece.direction.rot90() * half;
                let rect = Rect::from_two_pos(from - across, to + across);
                mesh.add_colored_rect(rect.round_to_pixels(pixels_per_point), color);
            }
        } else if dash[0].dot {
            // Square whichever way the curve runs, and on whole pixels, as a
            // rotated quad a pixel wide can light none.
            let (from, to) = (dash[0].line[0], dash[dash.len() - 1].line[1]);
            let square = pixel_square(from.lerp(to, 0.5), 2.0 * half, pixels_per_point);
            mesh.add_colored_rect(square, color);
        } else {
            add_curved_dash(&mut mesh, dash, half, 1.0 / pixels_per_point, color);
        }
    }
    painter.add(mesh);
}

/// A square around `center`, as near `side` wide as whole pixels allow, and
/// on them.
fn pixel_square(center: Pos2, side: f32, pixels_per_point: f32) -> Rect {
    let side = (side * pixels_per_point).round().max(1.0);
    let min = (center * pixels_per_point - Vec2::splat(side / 2.0)).round();
    Rect::from_min_size(min, Vec2::splat(side)) / pixels_per_point
}

/// Adds `dash`, `half` wide either side of its pieces, which it joins with
/// miters, and with its sides faded out over `feathering` like egui's lines.
fn add_curved_dash(mesh: &mut Mesh, dash: &[Piece], half: f32, feathering: f32, color: Color32) {
    let index = mesh.vertices.len() as u32;
    let (inner, outer) = (half - feathering / 2.0, half + feathering / 2.0);
    let (first, last) = (dash[0], dash[dash.len() - 1]);
    let corners = dash.windows(2).map(|pair| {
        let across = (pair[0].direction + pair[1].direction).rot90() / 2.0;
        (pair[1].line[0], across / across.length_sq())
    });
    let points = iter::once((first.line[0], first.direction.rot90()))
        .chain(corners)
        .chain(iter::once((last.line[1], last.direction.rot90())));
    for (point, across) in points {
        // Capped, as a sharp bend in a line would spike.
        let length = across.length();
        let across = if length > MAX_MITER {
            across * (MAX_MITER / length)
        } else {
            across
        };
        mesh.colored_vertex(point + across * outer, Color32::TRANSPARENT);
        mesh.colored_vertex(point + across * inner, color);
        mesh.colored_vertex(point - across * inner, color);
        mesh.colored_vertex(point - across * outer, Color32::TRANSPARENT);
    }
    for i in 0..dash.len() as u32 {
        let (a, b) = (index + 4 * i, index + 4 * (i + 1));
        for side in 0..3 {
            mesh.add_triangle(a + side, a + side + 1, b + side);
            mesh.add_triangle(a + side + 1, b + side + 1, b + side);
        }
    }
}

/// The dashes of `pattern` laid along `strips` in order, as if they were one
/// line, as the pieces of the strips they cover. `pattern` has the lengths
/// of dashes and the gaps after them, in `unit`s.
fn dashes(strips: &[[Pos2; 2]], pattern: &[[f32; 2]], unit: f32) -> Vec<Piece> {
    let lengths: Vec<f32> = strips.iter().map(|[from, to]| from.distance(*to)).collect();
    let period = unit * pattern.iter().flatten().sum::<f32>();
    let periods = (lengths.iter().sum::<f32>() / period).ceil();
    // Also catches lengths that aren't finite.
    if !(0.0..=MAX_PERIODS).contains(&periods) {
        let solid = strips.iter().enumerate().map(|(i, &line)| Piece {
            line,
            direction: (line[1] - line[0]).normalized(),
            runs_on: i > 0,
            dot: false,
        });
        return solid.collect();
    }
    let mut pieces = Vec::new();
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
                    pieces.push(Piece {
                        line: [
                            if a == offset { from } else { point(a) },
                            if b == end { to } else { point(b) },
                        ],
                        direction,
                        runs_on: start < offset,
                        dot: dash <= 1.0,
                    });
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

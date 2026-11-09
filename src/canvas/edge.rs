//! Where edges run on screen, and how they're painted.

use egui::epaint::PathShape;
use egui::{Painter, Pos2, Stroke};
use graphml::yed::{Arrow, Arrows, LineType};

use crate::line_type;

/// yEd's standard arrowhead, a triangle with its back pushed in, in line
/// widths: how long it is, half how wide, and how far its notch, where the
/// line ends, is from its tip. Lines 1 point wide get yEd's size.
const ARROW_LENGTH: f32 = 10.0;
const ARROW_HALF_WIDTH: f32 = 4.0;
const ARROW_NOTCH: f32 = 7.0;

/// What cuts a line where it leaves a node.
pub trait Cut {
    /// Whether `pos` is inside, where the line isn't drawn.
    fn inside(&self, pos: Pos2) -> bool;

    /// Where a line from `from`, inside, to `to`, outside, leaves.
    fn exit(&self, from: Pos2, to: Pos2) -> Pos2 {
        bisect(|pos| self.inside(pos), from, to)
    }
}

impl<F: Fn(Pos2) -> bool> Cut for F {
    fn inside(&self, pos: Pos2) -> bool {
        self(pos)
    }
}

/// `points` cut where they leave the nodes `source` and `target`. Nodes that
/// overlap leave no line between them, so then it's left whole.
pub fn clip(points: Vec<Pos2>, source: impl Cut, target: impl Cut) -> Vec<Pos2> {
    let clipped = clip_start(&points, source).and_then(|points| {
        let reversed: Vec<Pos2> = points.into_iter().rev().collect();
        let mut clipped = clip_start(&reversed, target)?;
        clipped.reverse();
        Some(clipped)
    });
    clipped.unwrap_or(points)
}

/// `points` from where they leave `node`, as they are if they start outside
/// it, or `None` if they never leave it.
fn clip_start(points: &[Pos2], node: impl Cut) -> Option<Vec<Pos2>> {
    if !node.inside(points[0]) {
        return Some(points.to_vec());
    }
    let out = (1..points.len()).find(|&i| !node.inside(points[i]))?;
    let mut clipped = vec![node.exit(points[out - 1], points[out])];
    clipped.extend_from_slice(&points[out..]);
    Some(clipped)
}

/// Where the line from `from`, inside an outline `inside` tells the inside
/// of, to `to`, outside it, leaves it. Bisected, as it can be any shape.
pub fn bisect(inside: impl Fn(Pos2) -> bool, mut from: Pos2, mut to: Pos2) -> Pos2 {
    for _ in 0..20 {
        let middle = from.lerp(to, 0.5);
        if inside(middle) {
            from = middle;
        } else {
            to = middle;
        }
    }
    to
}

/// The index of the segment of `path` nearest `pos`, and how far it is.
pub fn nearest_segment(path: &[Pos2], pos: Pos2) -> Option<(usize, f32)> {
    let distances = path.windows(2).map(|segment| {
        let (from, to) = (segment[0], segment[1]);
        let along = to - from;
        let t = if along.length_sq() > 0.0 {
            ((pos - from).dot(along) / along.length_sq()).clamp(0.0, 1.0)
        } else {
            0.0
        };
        pos.distance(from + along * t)
    });
    distances
        .enumerate()
        .min_by(|(_, a), (_, b)| a.total_cmp(b))
}

/// Paints a line along `path` in `stroke` and `line_type`, with `arrows` at
/// its ends sized to its width.
pub fn paint(
    painter: &Painter,
    mut path: Vec<Pos2>,
    stroke: Stroke,
    line_type: &LineType,
    arrows: &Arrows,
) {
    path.dedup();
    if path.len() < 2 || stroke.width <= 0.0 || stroke.color.a() == 0 {
        return;
    }
    snap(&mut path, stroke, painter.pixels_per_point());
    let (line, heads) = with_arrows(path, stroke.width, arrows);
    if line.len() >= 2 {
        line_type::paint_polyline(painter, &line, stroke, line_type);
    }
    for head in heads {
        painter.add(PathShape::convex_polygon(head, stroke.color, Stroke::NONE));
    }
}

/// `path`, a line `width` wide, cut back to the notches of the arrowheads
/// `arrows` puts on its ends, and the outlines of those, the target's first.
pub fn with_arrows(
    mut path: Vec<Pos2>,
    width: f32,
    arrows: &Arrows,
) -> (Vec<Pos2>, Vec<Vec<Pos2>>) {
    // Both are placed before the line is cut back to their notches, which can
    // leave nothing of a short line.
    let target = arrowhead(&path, &arrows.target, width);
    path.reverse();
    let source = arrowhead(&path, &arrows.source, width);
    if source.is_some() {
        cut_end(&mut path, ARROW_NOTCH * width);
    }
    path.reverse();
    if target.is_some() {
        cut_end(&mut path, ARROW_NOTCH * width);
    }
    (path, [target, source].into_iter().flatten().collect())
}

/// The outline of `arrow` at the end of `path`, a line `width` wide, from
/// its tip.
fn arrowhead(path: &[Pos2], arrow: &Arrow, width: f32) -> Option<Vec<Pos2>> {
    if *arrow == Arrow::None {
        return None;
    }
    let tip = *path.last()?;
    let from = *path.iter().rev().find(|&&point| point != tip)?;
    let back = (from - tip).normalized();
    let across = back.rot90() * ARROW_HALF_WIDTH * width;
    let corner = tip + back * ARROW_LENGTH * width;
    let notch = tip + back * ARROW_NOTCH * width;
    let mut outline = vec![tip, corner + across, notch, corner - across];
    // egui fans the fill out from the first point, which only the tip sees all
    // of, and reverses a path wound the other way.
    if signed_area(&outline) < 0.0 {
        outline.swap(1, 3);
    }
    Some(outline)
}

/// Twice the area of `polygon`, positive if egui fills it as wound.
fn signed_area(polygon: &[Pos2]) -> f32 {
    let previous = polygon.iter().cycle().skip(polygon.len() - 1);
    previous
        .zip(polygon)
        .map(|(a, b)| a.x * b.y - b.x * a.y)
        .sum()
}

/// Cuts `distance` off the end of `path`, leaving a point if it's shorter.
fn cut_end(path: &mut Vec<Pos2>, mut distance: f32) {
    while path.len() >= 2 {
        let (from, to) = (path[path.len() - 2], path[path.len() - 1]);
        let length = from.distance(to);
        if length > distance {
            *path.last_mut().expect("checked") = to + (from - to) * (distance / length);
            return;
        }
        distance -= length;
        path.pop();
    }
}

/// Moves the level and upright segments of `path` to where egui puts solid
/// lines in `stroke`, so they're sharp.
fn snap(path: &mut [Pos2], stroke: Stroke, pixels_per_point: f32) {
    let original = path.to_vec();
    for (i, segment) in original.windows(2).enumerate() {
        let (from, to) = (segment[0], segment[1]);
        if from.x == to.x {
            for point in &mut path[i..i + 2] {
                stroke.round_center_to_pixel(pixels_per_point, &mut point.x);
            }
        } else if from.y == to.y {
            for point in &mut path[i..i + 2] {
                stroke.round_center_to_pixel(pixels_per_point, &mut point.y);
            }
        }
    }
}

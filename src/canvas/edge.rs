//! Where edges run on screen, and how they're painted.

use egui::epaint::PathShape;
use egui::{Painter, Pos2, Stroke};
use graphml::yed::{Arrow, Arrows, LineType};

use crate::line_type;

/// yEd's standard arrowhead, a triangle with its back pushed in, in graph
/// points: how long it is, half how wide, and how far its notch, where the
/// line ends, is from its tip.
const ARROW_LENGTH: f32 = 10.0;
const ARROW_HALF_WIDTH: f32 = 4.0;
const ARROW_NOTCH: f32 = 7.0;

/// `points` cut where they leave the outlines `source` and `target` tell the
/// inside of. Nodes that overlap leave no line between them, so then it's
/// left whole.
pub fn clip(
    points: Vec<Pos2>,
    source: impl Fn(Pos2) -> bool,
    target: impl Fn(Pos2) -> bool,
) -> Vec<Pos2> {
    let clipped = clip_start(&points, source).and_then(|points| {
        let reversed: Vec<Pos2> = points.into_iter().rev().collect();
        let mut clipped = clip_start(&reversed, target)?;
        clipped.reverse();
        Some(clipped)
    });
    clipped.unwrap_or(points)
}

/// `points` from where they leave the outline `inside` tells the inside of,
/// as they are if they start outside it, or `None` if they never leave it.
fn clip_start(points: &[Pos2], inside: impl Fn(Pos2) -> bool) -> Option<Vec<Pos2>> {
    if !inside(points[0]) {
        return Some(points.to_vec());
    }
    let out = (1..points.len()).find(|&i| !inside(points[i]))?;
    let (mut from, mut to) = (points[out - 1], points[out]);
    // Bisected, as the outline can be any shape.
    for _ in 0..20 {
        let middle = from.lerp(to, 0.5);
        if inside(middle) {
            from = middle;
        } else {
            to = middle;
        }
    }
    let mut clipped = vec![to];
    clipped.extend_from_slice(&points[out..]);
    Some(clipped)
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
/// its ends, at `zoom` screen points a graph point.
pub fn paint(
    painter: &Painter,
    mut path: Vec<Pos2>,
    stroke: Stroke,
    line_type: &LineType,
    arrows: &Arrows,
    zoom: f32,
) {
    path.dedup();
    if path.len() < 2 || stroke.color.a() == 0 {
        return;
    }
    if stroke.width > 0.0 {
        snap(&mut path, stroke, painter.pixels_per_point());
    }
    let mut heads = Vec::new();
    if arrows.target != Arrow::None {
        heads.extend(arrowhead(&mut path, zoom));
    }
    if arrows.source != Arrow::None {
        path.reverse();
        heads.extend(arrowhead(&mut path, zoom));
        path.reverse();
    }
    if path.len() >= 2 && stroke.width > 0.0 {
        line_type::paint_polyline(painter, &path, stroke, line_type);
    }
    for head in heads {
        painter.add(PathShape::convex_polygon(head, stroke.color, Stroke::NONE));
    }
}

/// The outline of an arrowhead at the end of `path`, from its tip, which
/// cuts the line back to its notch.
fn arrowhead(path: &mut Vec<Pos2>, zoom: f32) -> Option<Vec<Pos2>> {
    let tip = *path.last()?;
    let from = *path.iter().rev().find(|&&point| point != tip)?;
    let back = (from - tip).normalized();
    let across = back.rot90() * ARROW_HALF_WIDTH * zoom;
    let corner = tip + back * ARROW_LENGTH * zoom;
    let notch = tip + back * ARROW_NOTCH * zoom;
    let mut outline = vec![tip, corner + across, notch, corner - across];
    // egui fans the fill out from the first point, which only the tip sees all
    // of, and reverses a path wound the other way.
    if signed_area(&outline) < 0.0 {
        outline.swap(1, 3);
    }
    cut_end(path, ARROW_NOTCH * zoom);
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

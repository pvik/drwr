//! Works out the bends draw.io's edge styles draw a line with, since files
//! only keep the points a user placed.

use graphml::yed::{Point, Rect};

/// Where a line meets a node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct End {
    pub bounds: Rect,
    /// Where on the node the line is fixed to, if it is (draw.io's
    /// `exitX`/`entryX`, or a loose end); else it floats.
    pub point: Option<Point>,
}

/// How a line is routed: draw.io's `edgeStyle`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Router {
    Straight,
    /// `orthogonalEdgeStyle` and `segmentEdgeStyle`.
    Orthogonal,
    /// `elbowEdgeStyle`, its bend `vertical` or not.
    Elbow {
        vertical: bool,
    },
    /// `entityRelationEdgeStyle`, leaving its nodes `segment` long.
    EntityRelation {
        segment: f64,
    },
    /// `sequenceEdgeStyle`: a level message between lifelines.
    Sequence,
}

/// A routed line: where it leaves and enters its nodes (points inside or on
/// them, which yEd keeps as ports), and the bends in between.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Path {
    pub source: Point,
    pub target: Point,
    pub bends: Vec<Point>,
}

/// How far orthogonal lines go straight out of a fixed point before turning,
/// draw.io's `jettySize` for 1 pt lines with default arrows.
const JETTY: f64 = 20.0;

/// Routes a line from `source` to `target` through the points the user
/// placed, `hints`.
pub(crate) fn route(router: Router, source: &End, target: &End, hints: &[Point]) -> Path {
    if router == Router::Straight {
        return Path {
            source: source.point.unwrap_or(center(source.bounds)),
            target: target.point.unwrap_or(center(target.bounds)),
            bends: hints.to_vec(),
        };
    }
    let mut bends = if source.bounds == target.bounds && hints.is_empty() {
        self_loop(source.bounds)
    } else {
        match router {
            Router::Straight => unreachable!("handled above"),
            Router::Orthogonal if hints.is_empty() => orthogonal(source, target),
            Router::Orthogonal => segments(source, target, hints),
            Router::Elbow { vertical } => elbow(source, target, hints.first().copied(), vertical),
            Router::EntityRelation { segment } => entity_relation(source, target, segment),
            Router::Sequence => sequence(source, target, hints),
        }
    };
    let source_port = port(source, bends.first().copied(), target);
    let target_port = port(target, bends.last().copied(), source);
    let mut line = Vec::with_capacity(bends.len() + 2);
    line.push(source_port);
    line.append(&mut bends);
    line.push(target_port);
    simplify(&mut line);
    let bends = match line.len() {
        0..=2 => Vec::new(),
        n => line[1..n - 1].to_vec(),
    };
    Path {
        source: source_port,
        target: target_port,
        bends,
    }
}

fn center(rect: Rect) -> Point {
    Point {
        x: rect.x + rect.width / 2.0,
        y: rect.y + rect.height / 2.0,
    }
}

fn contains(rect: Rect, point: Point) -> bool {
    within(point.x, rect.x, rect.x + rect.width) && within(point.y, rect.y, rect.y + rect.height)
}

fn within(value: f64, start: f64, end: f64) -> bool {
    value >= start && value <= end
}

/// The rect routing goes by: a fixed point stands in for its node.
fn routing_rect(end: &End) -> Rect {
    match end.point {
        Some(point) => Rect {
            x: point.x,
            y: point.y,
            width: 0.0,
            height: 0.0,
        },
        None => end.bounds,
    }
}

/// Where a line whose first bend (from this end) is `next` meets the node:
/// its fixed point, or where draw.io's orthogonal perimeter projection
/// puts it, as a point on the node's middle line.
fn port(end: &End, next: Option<Point>, other: &End) -> Point {
    if let Some(point) = end.point {
        return point;
    }
    let bounds = end.bounds;
    let mid = center(bounds);
    let next = next.unwrap_or(match other.point {
        Some(point) => point,
        None => center(other.bounds),
    });
    let across = within(next.y, bounds.y, bounds.y + bounds.height);
    let down = within(next.x, bounds.x, bounds.x + bounds.width);
    match (across, down) {
        (true, false) => Point {
            x: mid.x,
            y: next.y,
        },
        (false, true) => Point {
            x: next.x,
            y: mid.y,
        },
        _ => mid,
    }
}

/// Drops repeated bends and those in the middle of a straight run.
fn simplify(bends: &mut Vec<Point>) {
    const NEAR: f64 = 0.5;
    bends.dedup_by(|b, a| (a.x - b.x).abs() < NEAR && (a.y - b.y).abs() < NEAR);
    let mut i = 1;
    while i + 1 < bends.len() {
        let (a, b, c) = (bends[i - 1], bends[i], bends[i + 1]);
        let level = (a.y - b.y).abs() < NEAR && (b.y - c.y).abs() < NEAR;
        let upright = (a.x - b.x).abs() < NEAR && (b.x - c.x).abs() < NEAR;
        if level || upright {
            bends.remove(i);
        } else {
            i += 1;
        }
    }
}

/// A loop out of the right side of a node and back.
fn self_loop(bounds: Rect) -> Vec<Point> {
    let x = bounds.x + bounds.width + JETTY;
    let quarter = bounds.height / 4.0;
    let mid = center(bounds);
    vec![
        Point {
            x,
            y: mid.y - quarter,
        },
        Point {
            x,
            y: mid.y + quarter,
        },
    ]
}

/// A side of a node, by the direction a line leaves it in.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Side {
    West,
    North,
    East,
    South,
}

impl Side {
    fn horizontal(self) -> bool {
        matches!(self, Side::West | Side::East)
    }

    fn step(self) -> (f64, f64) {
        match self {
            Side::West => (-1.0, 0.0),
            Side::North => (0.0, -1.0),
            Side::East => (1.0, 0.0),
            Side::South => (0.0, 1.0),
        }
    }

    /// The middle of this side of `rect`.
    fn middle(self, rect: Rect) -> Point {
        let mid = center(rect);
        match self {
            Side::West => Point { x: rect.x, ..mid },
            Side::North => Point { y: rect.y, ..mid },
            Side::East => Point {
                x: rect.x + rect.width,
                ..mid
            },
            Side::South => Point {
                y: rect.y + rect.height,
                ..mid
            },
        }
    }
}

/// The side a fixed point lies on, within a point of it.
fn side_of(end: &End) -> Option<Side> {
    let point = end.point?;
    let b = end.bounds;
    let near = |a: f64, b: f64| (a - b).abs() <= 1.0;
    if near(point.x, b.x) {
        Some(Side::West)
    } else if near(point.x, b.x + b.width) {
        Some(Side::East)
    } else if near(point.y, b.y) {
        Some(Side::North)
    } else if near(point.y, b.y + b.height) {
        Some(Side::South)
    } else {
        None
    }
}

/// An orthogonal line without placed points. Like draw.io, it leaves sideways
/// and enters from above or below when the nodes are apart both ways (one
/// bend), else it runs from side to facing side (two bends, or none).
fn orthogonal(source: &End, target: &End) -> Vec<Point> {
    let (s, t) = (source.bounds, target.bounds);
    let left = s.x - (t.x + t.width);
    let right = t.x - (s.x + s.width);
    let above = s.y - (t.y + t.height);
    let below = t.y - (s.y + s.height);
    let across = if left >= right {
        Side::West
    } else {
        Side::East
    };
    let down = if above >= below {
        Side::North
    } else {
        Side::South
    };
    let facing = |side: Side| match side {
        Side::West => Side::East,
        Side::East => Side::West,
        Side::North => Side::South,
        Side::South => Side::North,
    };
    let (default_out, default_in) = if left.max(right) > 0.0 && above.max(below) > 0.0 {
        (across, facing(down))
    } else if above.max(below) > 0.0 {
        (down, facing(down))
    } else if left.max(right) > 0.0 {
        (across, facing(across))
    } else {
        // Overlapping nodes: straight from middle to middle.
        return Vec::new();
    };
    let out = side_of(source).unwrap_or(default_out);
    let entry = side_of(target).unwrap_or(default_in);
    let start = source.point.unwrap_or_else(|| out.middle(s));
    let end = target.point.unwrap_or_else(|| entry.middle(t));
    connect(start, out, end, entry, s, t)
}

/// Bends joining `start`, leaving it towards `out`, to `end`, entered from
/// its `entry` side.
fn connect(start: Point, out: Side, end: Point, entry: Side, s: Rect, t: Rect) -> Vec<Point> {
    let (o, e) = (out.step(), entry.step());
    let ahead = |from: Point, to: Point, (dx, dy): (f64, f64)| {
        (to.x - from.x) * dx + (to.y - from.y) * dy > 0.0
    };
    let facing = o == (-e.0, -e.1);
    let at = |x: f64, y: f64| Point { x, y };
    if out.horizontal() != entry.horizontal() {
        // One bend, where a level and an upright run meet.
        let corner = match out.horizontal() {
            true => at(end.x, start.y),
            false => at(start.x, end.y),
        };
        if ahead(start, corner, o) && ahead(end, corner, e) {
            return vec![corner];
        }
    } else if facing && ahead(start, end, o) {
        // Facing sides: turn halfway across the gap between the nodes.
        return match out.horizontal() {
            true => {
                let gap = gap_middle(s.x, s.x + s.width, t.x, t.x + t.width);
                let x = gap.unwrap_or((start.x + end.x) / 2.0);
                vec![at(x, start.y), at(x, end.y)]
            }
            false => {
                let gap = gap_middle(s.y, s.y + s.height, t.y, t.y + t.height);
                let y = gap.unwrap_or((start.y + end.y) / 2.0);
                vec![at(start.x, y), at(end.x, y)]
            }
        };
    }
    let out_point = at(start.x + o.0 * JETTY, start.y + o.1 * JETTY);
    let in_point = at(end.x + e.0 * JETTY, end.y + e.1 * JETTY);
    if out.horizontal() != entry.horizontal() {
        // Out of both nodes, then one turn between.
        return match out.horizontal() {
            true => vec![out_point, at(out_point.x, in_point.y), in_point],
            false => vec![out_point, at(in_point.x, out_point.y), in_point],
        };
    }
    if o == e {
        // Both leave the same way: round the far side of both.
        return match out.horizontal() {
            true => {
                let x = match o.0 > 0.0 {
                    true => out_point.x.max(in_point.x),
                    false => out_point.x.min(in_point.x),
                };
                vec![at(x, start.y), at(x, end.y)]
            }
            false => {
                let y = match o.1 > 0.0 {
                    true => out_point.y.max(in_point.y),
                    false => out_point.y.min(in_point.y),
                };
                vec![at(start.x, y), at(end.x, y)]
            }
        };
    }
    // Facing away from each other: out, across between the nodes (or past
    // them), and back in.
    match out.horizontal() {
        true => {
            let gap = gap_middle(s.y, s.y + s.height, t.y, t.y + t.height);
            let y = gap.unwrap_or((s.y + s.height).max(t.y + t.height) + JETTY);
            vec![out_point, at(out_point.x, y), at(in_point.x, y), in_point]
        }
        false => {
            let gap = gap_middle(s.x, s.x + s.width, t.x, t.x + t.width);
            let x = gap.unwrap_or((s.x + s.width).max(t.x + t.width) + JETTY);
            vec![out_point, at(x, out_point.y), at(x, in_point.y), in_point]
        }
    }
}

/// The middle of the gap between two spans, if they don't overlap.
fn gap_middle(a_start: f64, a_end: f64, b_start: f64, b_end: f64) -> Option<f64> {
    if a_end < b_start {
        Some((a_end + b_start) / 2.0)
    } else if b_end < a_start {
        Some((b_end + a_start) / 2.0)
    } else {
        None
    }
}

/// An orthogonal line through placed points, as draw.io's segment connector
/// reads them: the first point fixes the first free segment, and each point
/// after it one more segment, alternating across and down.
fn segments(source: &End, target: &End, hints: &[Point]) -> Vec<Point> {
    const TOLERANCE: f64 = 1.0;
    let start = source.point.unwrap_or(center(source.bounds));
    let end = target.point.unwrap_or(center(target.bounds));
    let mut hints = hints.to_vec();
    let snap = |hint: &mut Point, to: Point| {
        if (hint.x - to.x).abs() < TOLERANCE {
            hint.x = to.x;
        }
        if (hint.y - to.y).abs() < TOLERANCE {
            hint.y = to.y;
        }
    };
    snap(&mut hints[0], start);
    let last = hints.len() - 1;
    snap(&mut hints[last], end);

    // Whether the segment through the first point runs across, decided by
    // which channel of either node the points line up with.
    let mut horizontal = true;
    for (i, end_of_line) in [source, target].into_iter().enumerate() {
        let hint = if i == 0 {
            hints[0]
        } else {
            hints[hints.len() - 1]
        };
        let (level, upright) = match end_of_line.point {
            Some(point) => (point.y == hint.y, point.x == hint.x),
            None => {
                let b = end_of_line.bounds;
                (
                    within(hint.y, b.y, b.y + b.height),
                    within(hint.x, b.x, b.x + b.width),
                )
            }
        };
        if i == 0 && level && upright {
            if end_of_line.point.is_some() && hints.len() > 1 {
                hints.remove(0);
            }
            continue;
        }
        if level || upright {
            horizontal = if i == 1 && hints.len() % 2 == 1 {
                upright
            } else {
                level
            };
            break;
        }
    }

    let mut points = Vec::new();
    let mut at = start;
    let first = hints[0];
    if horizontal {
        if needs_turn(source, first, true) {
            points.push(Point {
                x: at.x,
                y: first.y,
            });
        }
        at.y = first.y;
    } else {
        if needs_turn(source, first, false) {
            points.push(Point {
                x: first.x,
                y: at.y,
            });
        }
        at.x = first.x;
    }
    for hint in &hints {
        horizontal = !horizontal;
        if horizontal {
            at.y = hint.y;
        } else {
            at.x = hint.x;
        }
        points.push(at);
    }
    let last = hints[hints.len() - 1];
    if horizontal && needs_turn(target, last, true) {
        points.push(Point {
            x: end.x,
            y: last.y,
        });
    } else if !horizontal && needs_turn(target, last, false) {
        points.push(Point {
            x: last.x,
            y: end.y,
        });
    }
    if source.point.is_none() {
        let inside = points.iter().take_while(|p| contains(source.bounds, **p));
        let count = inside.count();
        points.drain(..count);
    }
    if target.point.is_none() {
        while points.last().is_some_and(|p| contains(target.bounds, *p)) {
            points.pop();
        }
    }
    if let (Some(fixed), Some(last)) = (target.point, points.last())
        && (fixed.x - last.x).abs() <= TOLERANCE
        && (fixed.y - last.y).abs() <= TOLERANCE
    {
        points.pop();
    }
    points
}

/// Whether reaching the level (`across`) or upright run through `hint` from
/// `end` takes a turn: a fixed point off its line, or a node outside its span.
fn needs_turn(end: &End, hint: Point, across: bool) -> bool {
    let b = end.bounds;
    match (end.point, across) {
        (Some(fixed), true) => fixed.y != hint.y,
        (Some(fixed), false) => fixed.x != hint.x,
        (None, true) => !within(hint.y, b.y, b.y + b.height),
        (None, false) => !within(hint.x, b.x, b.x + b.width),
    }
}

/// draw.io's elbow connector: one upright bend run halfway between the
/// nodes, or through the placed point, or a level one if `vertical`.
fn elbow(source: &End, target: &End, hint: Option<Point>, vertical: bool) -> Vec<Point> {
    let (s, t) = (source.bounds, target.bounds);
    let (upright_forced, level_forced) = match hint {
        Some(hint) => {
            let left = s.x.min(t.x);
            let right = (s.x + s.width).max(t.x + t.width);
            let top = s.y.min(t.y);
            let bottom = (s.y + s.height).max(t.y + t.height);
            (
                hint.y < top || hint.y > bottom,
                hint.x < left || hint.x > right,
            )
        }
        None => {
            let left = s.x.max(t.x);
            let right = (s.x + s.width).min(t.x + t.width);
            let upright = left == right;
            let top = s.y.max(t.y);
            let bottom = (s.y + s.height).min(t.y + t.height);
            (upright, !upright && top == bottom)
        }
    };
    let (s, t) = (routing_rect(source), routing_rect(target));
    let outside = |point: Point| !contains(t, point) && !contains(s, point);
    let mut bends = Vec::new();
    if !level_forced && (upright_forced || vertical) {
        // Top to bottom: a level run between the nodes.
        let top = s.y.max(t.y);
        let bottom = (s.y + s.height).min(t.y + t.height);
        let in_span = |hint: Point, r: Rect| within(hint.x, r.x, r.x + r.width);
        let mut x = match hint {
            Some(hint) if in_span(hint, s) => hint.x,
            _ => center(s).x,
        };
        let y = hint.map_or((top + bottom) / 2.0, |hint| hint.y);
        if outside(Point { x, y }) {
            bends.push(Point { x, y });
        }
        x = match hint {
            Some(hint) if in_span(hint, t) => hint.x,
            _ => center(t).x,
        };
        if outside(Point { x, y }) {
            bends.push(Point { x, y });
        }
        if bends.len() == 1 {
            let x = match hint {
                Some(hint) => hint.x,
                None => (s.x.max(t.x) + (s.x + s.width).min(t.x + t.width)) / 2.0,
            };
            if hint.is_none() || outside(Point { x, y }) {
                bends.push(Point { x, y });
            }
        }
    } else {
        // Side to side: an upright run between the nodes.
        let left = s.x.max(t.x);
        let right = (s.x + s.width).min(t.x + t.width);
        let x = hint.map_or((left + right) / 2.0, |hint| hint.x);
        let in_span = |hint: Point, r: Rect| within(hint.y, r.y, r.y + r.height);
        let y1 = match hint {
            Some(hint) if in_span(hint, s) => hint.y,
            _ => center(s).y,
        };
        let y2 = match hint {
            Some(hint) if in_span(hint, t) => hint.y,
            _ => center(t).y,
        };
        if outside(Point { x, y: y1 }) {
            bends.push(Point { x, y: y1 });
        }
        if outside(Point { x, y: y2 }) {
            bends.push(Point { x, y: y2 });
        }
        if bends.len() == 1 {
            let y = match hint {
                Some(hint) => hint.y,
                None => (s.y.max(t.y) + (s.y + s.height).min(t.y + t.height)) / 2.0,
            };
            if hint.is_none() || outside(Point { x, y }) {
                bends.push(Point { x, y });
            }
        }
    }
    bends
}

/// draw.io's entity relation connector: out of the sides facing each other,
/// `segment` straight, then across.
fn entity_relation(source: &End, target: &End, segment: f64) -> Vec<Point> {
    let (s, t) = (source.bounds, target.bounds);
    let source_left = t.x + t.width < s.x;
    let target_left = s.x + s.width < t.x;
    let (s, t) = (routing_rect(source), routing_rect(target));
    let x0 = if source_left { s.x } else { s.x + s.width };
    let xe = if target_left { t.x } else { t.x + t.width };
    let (y0, ye) = (center(s).y, center(t).y);
    let step = |left: bool| if left { -segment } else { segment };
    let departure = Point {
        x: x0 + step(source_left),
        y: y0,
    };
    let arrival = Point {
        x: xe + step(target_left),
        y: ye,
    };
    if source_left == target_left {
        let x = if source_left {
            x0.min(xe) - segment
        } else {
            x0.max(xe) + segment
        };
        vec![Point { x, y: y0 }, Point { x, y: ye }]
    } else if (departure.x < arrival.x) == source_left {
        let mid = (y0 + ye) / 2.0;
        vec![
            departure,
            Point {
                x: departure.x,
                y: mid,
            },
            Point {
                x: arrival.x,
                y: mid,
            },
            arrival,
        ]
    } else {
        vec![departure, arrival]
    }
}

/// draw.io's sequence message: level at its placed point's height, or a
/// fixed end's, within the span both ends share. More points route by hand.
fn sequence(source: &End, target: &End, hints: &[Point]) -> Vec<Point> {
    if hints.len() > 1 {
        return hints.to_vec();
    }
    let (s, t) = (source.bounds, target.bounds);
    let y = match (source.point, target.point) {
        (Some(_), Some(_)) => return Vec::new(),
        (Some(fixed), None) | (None, Some(fixed)) => fixed.y,
        (None, None) => {
            let top = s.y.max(t.y);
            let bottom = (s.y + s.height).min(t.y + t.height);
            if top > bottom {
                return Vec::new();
            }
            let middle = (center(s).y + center(t).y) / 2.0;
            hints
                .first()
                .map_or(middle, |hint| hint.y)
                .clamp(top, bottom)
        }
    };
    let x = (center(s).x + center(t).x) / 2.0;
    vec![Point { x, y }]
}

#[cfg(test)]
mod tests;

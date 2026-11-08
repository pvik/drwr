use super::{Path, Point, Rect};

/// The route yEd places an edge's labels along: from where the edge leaves
/// its source node's bounds, through its bends, to where it enters its
/// target node's bounds.
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    pub points: Vec<Point>,
    /// The bounds of the source node.
    pub source: Rect,
    pub target: Rect,
}

impl Route {
    /// The route of an edge along `path`, from a node with the bounds
    /// `source` to one with the bounds `target`. Nodes that overlap leave no
    /// route between them, so then it runs whole from port to port.
    pub fn new(path: &Path, source: Rect, target: Rect) -> Self {
        let port = |node: &Rect, offset: Point| Point {
            x: node.x + node.width / 2.0 + offset.x,
            y: node.y + node.height / 2.0 + offset.y,
        };
        let mut points = Vec::with_capacity(path.points.len() + 2);
        points.push(port(&source, path.source));
        points.extend_from_slice(&path.points);
        points.push(port(&target, path.target));
        let clipped = clip_start(&points, &source).and_then(|points| {
            let reversed: Vec<Point> = points.into_iter().rev().collect();
            let mut clipped = clip_start(&reversed, &target)?;
            clipped.reverse();
            Some(clipped)
        });
        Route {
            points: clipped.unwrap_or(points),
            source,
            target,
        }
    }
}

/// `points` from where they leave `node`, or `None` if they never do.
fn clip_start(points: &[Point], node: &Rect) -> Option<Vec<Point>> {
    if !inside(node, points[0]) {
        return Some(points.to_vec());
    }
    let out = (1..points.len()).find(|&i| !inside(node, points[i]))?;
    let mut clipped = vec![exit(node, points[out - 1], points[out])];
    clipped.extend_from_slice(&points[out..]);
    Some(clipped)
}

/// Whether `point` is inside `rect`, not on its sides.
fn inside(rect: &Rect, point: Point) -> bool {
    point.x > rect.x
        && point.x < rect.x + rect.width
        && point.y > rect.y
        && point.y < rect.y + rect.height
}

/// Where the line from `from`, inside `rect`, to `to`, outside it, leaves it.
fn exit(rect: &Rect, from: Point, to: Point) -> Point {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let along = |start: f64, delta: f64, min: f64, max: f64| {
        if delta > 0.0 {
            (max - start) / delta
        } else if delta < 0.0 {
            (min - start) / delta
        } else {
            f64::INFINITY
        }
    };
    let x = along(from.x, dx, rect.x, rect.x + rect.width);
    let y = along(from.y, dy, rect.y, rect.y + rect.height);
    let t = x.min(y).clamp(0.0, 1.0);
    Point {
        x: from.x + dx * t,
        y: from.y + dy * t,
    }
}

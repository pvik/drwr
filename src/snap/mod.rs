//! Snapping what the canvas drags into line with the nodes around it, as
//! yEd's snap lines do, and the guides showing what it lines up with.

use graphml::yed::{Point, Rect};

/// Lines closer than this, in graph points, count as one.
const SAME: f64 = 1e-6;

/// An axis, which the lines snapped along it cross.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}

impl Axis {
    pub const BOTH: [Axis; 2] = [Axis::X, Axis::Y];

    fn other(self) -> Axis {
        match self {
            Axis::X => Axis::Y,
            Axis::Y => Axis::X,
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// A line of a box across an axis: its left or top side, its middle, or its
/// right or bottom side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Start,
    Middle,
    End,
}

impl Side {
    const ALL: [Side; 3] = [Side::Start, Side::Middle, Side::End];
}

/// A line of something dragged, which may snap onto lines of what's around.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Feature {
    axis: Axis,
    /// Where it crosses its axis.
    at: f64,
    kind: Kind,
    /// What it spans along the other axis.
    span: [f64; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    /// A side or middle of a box, which snaps onto the same of a node, or a
    /// side onto the opposite side of a node beside it, which it touches then.
    Box(Side),
    /// A point, which snaps onto any side or middle, and onto the points.
    Point,
}

impl Feature {
    /// The sides and middles of `rect`.
    pub fn box_lines(rect: &Rect) -> [Feature; 6] {
        std::array::from_fn(|i| Feature::side(rect, Axis::BOTH[i / 3], Side::ALL[i % 3]))
    }

    /// The `side` of `rect` across `axis`.
    pub fn side(rect: &Rect, axis: Axis, side: Side) -> Feature {
        Feature {
            axis,
            at: line(rect, axis, side),
            kind: Kind::Box(side),
            span: span(rect, axis.other()),
        }
    }

    /// The lines through `point` across each axis.
    pub fn point(point: Point) -> [Feature; 2] {
        Axis::BOTH.map(|axis| Feature {
            axis,
            at: coordinate(point, axis),
            kind: Kind::Point,
            span: [coordinate(point, axis.other()); 2],
        })
    }
}

/// A line drawn to show that dragged things line up along it: across `axis`
/// at `at`, spanning `from` to `to` along the other axis, in graph points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Guide {
    pub axis: Axis,
    pub at: f64,
    pub from: f64,
    pub to: f64,
}

/// What dragged things snap onto: the sides and middles of nodes, and
/// points, such as the neighbours of a dragged bend.
#[derive(Clone, Debug)]
pub struct Targets {
    /// The nodes' bounds, and whether their sides count, besides their
    /// middles.
    nodes: Vec<(Rect, bool)>,
    points: Vec<Point>,
    /// How near, in graph points, a line snaps onto another.
    reach: f64,
}

impl Targets {
    /// Targets that lines within `reach` graph points snap onto.
    pub fn new(reach: f64) -> Self {
        Targets {
            nodes: Vec::new(),
            points: Vec::new(),
            reach,
        }
    }

    /// Adds a node with bounds `rect`.
    pub fn add_node(&mut self, rect: Rect) {
        self.nodes.push((rect, true));
    }

    /// Adds the middles of a node with bounds `rect`, but not its sides.
    pub fn add_middles(&mut self, rect: Rect) {
        self.nodes.push((rect, false));
    }

    pub fn add_point(&mut self, point: Point) {
        self.points.push(point);
    }

    /// How far to move along each axis for the nearest of `features` within
    /// reach to snap onto a line it may snap onto, if any is within reach.
    pub fn snap(&self, features: &[Feature]) -> [Option<f64>; 2] {
        Axis::BOTH.map(|axis| {
            let along = features.iter().filter(|feature| feature.axis == axis);
            along
                .flat_map(|feature| self.lines(feature).map(|(at, _)| at - feature.at))
                .filter(|shift| shift.abs() <= self.reach)
                .min_by(|a, b| a.abs().total_cmp(&b.abs()))
        })
    }

    /// The guides along the lines that `features` lie on, each spanning the
    /// features and what they line up with.
    pub fn guides(&self, features: &[Feature]) -> Vec<Guide> {
        let mut guides: Vec<Guide> = features
            .iter()
            .flat_map(|feature| {
                let on = self
                    .lines(feature)
                    .filter(|(at, _)| (at - feature.at).abs() <= SAME);
                on.map(|(_, [from, to])| Guide {
                    axis: feature.axis,
                    at: feature.at,
                    from: from.min(feature.span[0]),
                    to: to.max(feature.span[1]),
                })
            })
            .collect();
        guides.sort_by(|a, b| (a.axis.index().cmp(&b.axis.index())).then(a.at.total_cmp(&b.at)));
        // One guide per line, over all that lines up along it.
        guides.dedup_by(|next, kept| {
            let same = next.axis == kept.axis && (next.at - kept.at).abs() <= SAME;
            if same {
                kept.from = kept.from.min(next.from);
                kept.to = kept.to.max(next.to);
            }
            same
        });
        guides
    }

    /// The lines `feature` may snap onto, each with what its node or point
    /// spans along the other axis.
    fn lines(&self, feature: &Feature) -> impl Iterator<Item = (f64, [f64; 2])> {
        let axis = feature.axis;
        let nodes = self.nodes.iter().flat_map(move |&(rect, sides)| {
            let across = span(&rect, axis.other());
            let beside = feature.span[0] <= across[1] && across[0] <= feature.span[1];
            Side::ALL.into_iter().filter_map(move |side| {
                let snaps = match feature.kind {
                    Kind::Box(own) => {
                        own == side || beside && own != Side::Middle && side != Side::Middle
                    }
                    Kind::Point => sides || side == Side::Middle,
                };
                snaps.then(|| (line(&rect, axis, side), across))
            })
        });
        let points = (self.points.iter())
            .filter(move |_| feature.kind == Kind::Point)
            .map(move |&point| {
                let across = coordinate(point, axis.other());
                (coordinate(point, axis), [across; 2])
            });
        nodes.chain(points)
    }
}

/// Where the `side` of `rect` crosses `axis`. Its middle is where the canvas
/// puts a node's, to the bit, so ports snapped there have no offset.
fn line(rect: &Rect, axis: Axis, side: Side) -> f64 {
    let (start, size) = match axis {
        Axis::X => (rect.x, rect.width),
        Axis::Y => (rect.y, rect.height),
    };
    match side {
        Side::Start => start,
        Side::Middle => start + size / 2.0,
        Side::End => start + size,
    }
}

/// Where `rect` starts and ends along `axis`.
fn span(rect: &Rect, axis: Axis) -> [f64; 2] {
    [line(rect, axis, Side::Start), line(rect, axis, Side::End)]
}

fn coordinate(point: Point, axis: Axis) -> f64 {
    match axis {
        Axis::X => point.x,
        Axis::Y => point.y,
    }
}

#[cfg(test)]
mod tests;

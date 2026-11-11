//! Snapping what the canvas drags into line with the nodes around it, or
//! into equal gaps with them, as yEd's snap lines do, and the guides showing
//! what it lines up with.

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

/// A line of something dragged, which may snap onto lines of what's around,
/// or the whole of it along an axis, which may snap into equal gaps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Feature {
    axis: Axis,
    /// Where it crosses its axis, or where a block starts along it.
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
    /// A block ending at `end`, which snaps halfway between the nearest nodes
    /// either side of it, or as far past the nearest as that is past the
    /// next, as yEd does.
    Block { end: f64 },
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

    /// The box around `rects` as one block along each axis, none without
    /// `rects`.
    pub fn block(rects: &[Rect]) -> Vec<Feature> {
        let around = |axis: Axis| {
            let spans = rects.iter().map(|rect| span(rect, axis));
            spans.reduce(|a, b| [a[0].min(b[0]), a[1].max(b[1])])
        };
        let blocks = Axis::BOTH.into_iter().filter_map(|axis| {
            let [at, end] = around(axis)?;
            let span = around(axis.other())?;
            let kind = Kind::Block { end };
            Some(Feature {
                axis,
                at,
                kind,
                span,
            })
        });
        blocks.collect()
    }

    fn moved(mut self, by: f64) -> Feature {
        self.at += by;
        if let Kind::Block { end } = &mut self.kind {
            *end += by;
        }
        self
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

/// Equal gaps between things side by side along `axis`, which yEd shows
/// with dimension lines: each from where one thing ends to where the next
/// starts along the axis, in graph points. All the things overlap `band`
/// across the axis, and `start` is the least coordinate across it any has,
/// such as the top of the highest in a row.
#[derive(Clone, Debug, PartialEq)]
pub struct EqualGaps {
    pub axis: Axis,
    pub gaps: Vec<[f64; 2]>,
    pub band: [f64; 2],
    pub start: f64,
}

impl EqualGaps {
    /// Adds the gap from `from` to `node`, `way` of it, if it's as wide as
    /// the others and `node` overlaps the band, returning whether it did.
    fn take(&mut self, from: &Extent, node: &Extent, way: Way) -> bool {
        let gap = from.gap_to(node, way);
        let as_wide = |other: &[f64; 2]| (length(gap) - length(*other)).abs() <= SAME;
        let fits = length(gap) > SAME && self.gaps.first().is_none_or(as_wide);
        match overlap(self.band, node.across) {
            Some(band) if fits => {
                self.band = band;
                self.start = self.start.min(node.across[0]);
                self.gaps.push(gap);
                true
            }
            _ => false,
        }
    }
}

/// What dragged things snap onto: the sides and middles of nodes, the gaps
/// between nodes, and points, such as the neighbours of a dragged bend.
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
    /// reach to snap onto a line, or into equal gaps, if any is within reach.
    pub fn snap(&self, features: &[Feature]) -> [Option<f64>; 2] {
        Axis::BOTH.map(|axis| {
            let along = features.iter().filter(|feature| feature.axis == axis);
            along
                .flat_map(|feature| self.shifts(feature))
                .filter(|shift| shift.abs() <= self.reach)
                .min_by(|a, b| a.abs().total_cmp(&b.abs()))
        })
    }

    /// The equal gaps that the blocks of `features` make with the nodes
    /// beside them.
    pub fn equal_gaps(&self, features: &[Feature]) -> Vec<EqualGaps> {
        (features.iter())
            .flat_map(|feature| self.equal_gaps_of(feature))
            .collect()
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
                    Kind::Block { .. } => false,
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

    /// How far `feature` may move along its axis to snap.
    fn shifts(&self, feature: &Feature) -> Vec<f64> {
        if !matches!(feature.kind, Kind::Block { .. }) {
            return self.lines(feature).map(|(at, _)| at - feature.at).collect();
        }
        // Moving may bring other nodes beside it, so it has to end up in
        // equal gaps.
        let spaced = |shift: &f64| {
            shift.abs() <= self.reach && !self.equal_gaps_of(&feature.moved(*shift)).is_empty()
        };
        self.spacing_shifts(feature)
            .into_iter()
            .filter(spaced)
            .collect()
    }

    /// How far `block` would move along its axis to be halfway between the
    /// nearest nodes either side of it, or as far past the nearest as that
    /// is past the next.
    fn spacing_shifts(&self, block: &Feature) -> Vec<f64> {
        let Some(me) = Extent::of(block) else {
            return Vec::new();
        };
        let row = self.row(block.axis, &me);
        let before = nearest(&row, &me, Way::Before);
        let after = nearest(&row, &me, Way::After);
        let mut shifts = Vec::new();
        if let (Some((_, left)), Some((_, right))) = (before, after) {
            shifts.push((length(right) - length(left)) / 2.0);
        }
        for (way, next) in [(Way::Before, before), (Way::After, after)] {
            if let Some((node, gap)) = next
                && let Some((_, wanted)) = nearest(&row, &node, way)
            {
                let wider = length(wanted) - length(gap);
                shifts.push(if way == Way::Before { wider } else { -wider });
            }
        }
        shifts
    }

    /// The equal gaps `block` makes with the nodes beside it along its axis,
    /// on along them while the gaps stay equal: one run through it if it's
    /// halfway between two, else a run each way with at least two gaps.
    fn equal_gaps_of(&self, block: &Feature) -> Vec<EqualGaps> {
        let Some(me) = Extent::of(block) else {
            return Vec::new();
        };
        let row = self.row(block.axis, &me);
        let new = || EqualGaps {
            axis: block.axis,
            gaps: Vec::new(),
            band: me.across,
            start: me.across[0],
        };
        let go_on = |run: &mut EqualGaps, mut from: Extent, way: Way| {
            while let Some((node, _)) = nearest(&row, &from, way)
                && run.take(&from, &node, way)
            {
                from = node;
            }
        };
        let mut runs = Vec::new();
        let mut halfway = new();
        if let Some((left, _)) = nearest(&row, &me, Way::Before)
            && let Some((right, _)) = nearest(&row, &me, Way::After)
            && halfway.take(&me, &left, Way::Before)
            && halfway.take(&me, &right, Way::After)
        {
            go_on(&mut halfway, left, Way::Before);
            go_on(&mut halfway, right, Way::After);
            runs.push(halfway);
        } else {
            for way in [Way::Before, Way::After] {
                let mut run = new();
                go_on(&mut run, me, way);
                if run.gaps.len() >= 2 {
                    runs.push(run);
                }
            }
        }
        for run in &mut runs {
            run.gaps.sort_by(|a, b| a[0].total_cmp(&b[0]));
        }
        runs
    }

    /// The nodes beside `block` along `axis`: those overlapping it across.
    fn row(&self, axis: Axis, block: &Extent) -> Vec<Extent> {
        let nodes = self.nodes.iter().filter(|(_, sides)| *sides);
        let extents = nodes.map(|(rect, _)| Extent {
            along: span(rect, axis),
            across: span(rect, axis.other()),
        });
        let beside = |node: &Extent| overlap(node.across, block.across).is_some();
        extents.filter(beside).collect()
    }
}

/// What a node or block spans along an axis and across it.
#[derive(Clone, Copy, Debug)]
struct Extent {
    along: [f64; 2],
    across: [f64; 2],
}

impl Extent {
    fn of(block: &Feature) -> Option<Extent> {
        match block.kind {
            Kind::Block { end } => Some(Extent {
                along: [block.at, end],
                across: block.span,
            }),
            _ => None,
        }
    }

    /// The gap along the axis from it to `other`, `way` of it.
    fn gap_to(&self, other: &Extent, way: Way) -> [f64; 2] {
        match way {
            Way::Before => [other.along[1], self.along[0]],
            Way::After => [self.along[1], other.along[0]],
        }
    }
}

/// Towards the start of an axis, or its end.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Way {
    Before,
    After,
}

/// The node of `row` nearest `from` of those wholly `way` of it, with the
/// gap between them.
fn nearest(row: &[Extent], from: &Extent, way: Way) -> Option<(Extent, [f64; 2])> {
    let gaps = row.iter().map(|node| (*node, from.gap_to(node, way)));
    gaps.filter(|(_, gap)| length(*gap) >= -SAME)
        .min_by(|(_, a), (_, b)| length(*a).total_cmp(&length(*b)))
}

/// Where `a` and `b` overlap, if by more than [`SAME`].
fn overlap(a: [f64; 2], b: [f64; 2]) -> Option<[f64; 2]> {
    let both = [a[0].max(b[0]), a[1].min(b[1])];
    (length(both) > SAME).then_some(both)
}

fn length([start, end]: [f64; 2]) -> f64 {
    end - start
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

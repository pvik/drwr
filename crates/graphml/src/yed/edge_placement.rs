use std::f64::consts::TAU;

use super::codec::{get_or, set};
use super::placement::inner_mut;
use super::{Point, Rect, Route};
use crate::{Error, xml::Element};

/// How far yEd keeps edge labels from the nodes their edge joins, as fitted
/// to the bounds yEd saved for real labels.
const NODE_DISTANCE: f64 = 5.0;
/// Labels nearer their edge than this are put on it.
const ON_EDGE: f64 = 0.5;

/// A placement by yEd's Smart model for edge labels, a `custom` one: on or
/// beside a segment of the edge's [`Route`]. Along the segment, a `ratio` in
/// 0..=1 puts the label's center that far between where the label keeps
/// [`NODE_DISTANCE`] from the nodes, and other ratios that many points before
/// or after those.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SmartEdge {
    /// From the source, or from the target if negative, so -1 is the last.
    segment: i32,
    ratio: f64,
    distance: f64,
    side: Side,
    /// Whether `distance` is to the label's center, or else to its side.
    to_center: bool,
    /// In radians, clockwise.
    angle: f64,
}

/// Where a label is across its segment, looking from the edge's source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Left,
    Right,
    /// On the edge.
    Center,
}

impl Side {
    fn keyword(self) -> &'static str {
        match self {
            Side::Left => "left",
            Side::Right => "right",
            Side::Center => "center",
        }
    }
}

/// A segment of a route, from `start` along `direction` for `length`.
struct Segment {
    start: Point,
    direction: Point,
    length: f64,
}

impl Segment {
    fn new(route: &Route, index: usize) -> Self {
        let (start, end) = (route.points[index], route.points[index + 1]);
        let (dx, dy) = (end.x - start.x, end.y - start.y);
        let length = dx.hypot(dy);
        let direction = if length > 0.0 {
            Point {
                x: dx / length,
                y: dy / length,
            }
        } else {
            Point { x: 1.0, y: 0.0 }
        };
        Segment {
            start,
            direction,
            length,
        }
    }

    /// To the left, looking along the segment, with y pointing down.
    fn left(&self) -> Point {
        Point {
            x: self.direction.y,
            y: -self.direction.x,
        }
    }

    /// The point `along` the segment and `across` it, to its left.
    fn at(&self, along: f64, across: f64) -> Point {
        let left = self.left();
        Point {
            x: self.start.x + self.direction.x * along + left.x * across,
            y: self.start.y + self.direction.y * along + left.y * across,
        }
    }

    /// How far from its start, and how far to its left, `point` is.
    fn coordinates(&self, point: Point) -> (f64, f64) {
        let (dx, dy) = (point.x - self.start.x, point.y - self.start.y);
        let left = self.left();
        (
            dx * self.direction.x + dy * self.direction.y,
            dx * left.x + dy * left.y,
        )
    }

    /// How far `point` is from the segment.
    fn distance(&self, point: Point) -> f64 {
        let (along, across) = self.coordinates(point);
        let beyond = if along < 0.0 {
            -along
        } else {
            (along - self.length).max(0.0)
        };
        beyond.hypot(across)
    }
}

impl Default for SmartEdge {
    /// On the middle of the first segment, like yEd's new labels.
    fn default() -> Self {
        SmartEdge {
            segment: 0,
            ratio: 0.5,
            distance: 30.0,
            side: Side::Center,
            to_center: true,
            angle: 0.0,
        }
    }
}

impl SmartEdge {
    /// Reads the Smart model and its parameter in `label`, if it has them.
    pub(crate) fn read(label: &Element) -> Result<Option<Self>, Error> {
        let inner = |outer, inner| label.child(outer).and_then(|outer| outer.child(inner));
        let model = inner("LabelModel", "SmartEdgeLabelModel");
        let parameter = inner("ModelParameter", "SmartEdgeLabelModelParameter");
        let (Some(_), Some(parameter)) = (model, parameter) else {
            return Ok(None);
        };
        let side = match parameter.attr("position") {
            Some("left") => Side::Left,
            Some("right") => Side::Right,
            Some("center") | None => Side::Center,
            Some(_) => return Ok(None),
        };
        let default = SmartEdge::default();
        Ok(Some(SmartEdge {
            segment: get_or(parameter, "segment", default.segment)?,
            ratio: get_or(parameter, "ratio", default.ratio)?,
            distance: get_or(parameter, "distance", default.distance)?,
            side,
            to_center: get_or(parameter, "distanceToCenter", default.to_center)?,
            angle: get_or(parameter, "angle", default.angle)?,
        }))
    }

    pub(crate) fn write(&self, label: &mut Element) {
        set(label, "modelName", &"custom".to_owned());
        label.attributes.remove("modelPosition");
        let model = inner_mut(label, "LabelModel", "SmartEdgeLabelModel");
        if model.attributes.is_empty() {
            for (name, value) in [
                ("autoRotationEnabled", "false"),
                ("defaultAngle", "0.0"),
                ("defaultDistance", "10.0"),
            ] {
                model.set_attr(name, value);
            }
        }
        let parameter = inner_mut(label, "ModelParameter", "SmartEdgeLabelModelParameter");
        set(parameter, "angle", &self.angle);
        set(parameter, "distance", &self.distance);
        set(parameter, "distanceToCenter", &self.to_center);
        set(parameter, "position", &self.side.keyword().to_owned());
        set(parameter, "ratio", &self.ratio);
        set(parameter, "segment", &self.segment);
    }

    /// The bounds of a label `width` by `height` placed so on `route`, or
    /// `None` if it's turned, which this library doesn't place.
    pub(crate) fn place(&self, route: &Route, width: f64, height: f64) -> Option<Rect> {
        let turns = self.angle / TAU;
        if (turns - turns.round()).abs() > 1e-9 {
            return None;
        }
        let segments = route.points.len().checked_sub(1).filter(|&n| n > 0)?;
        let index = self.index(segments);
        let segment = Segment::new(route, index);
        let across = self.across(&segment, width, height);
        let (min, max) = slide(route, &segment, index, across, width, height);
        let along = if self.ratio < 0.0 {
            min + self.ratio
        } else if self.ratio > 1.0 {
            max + self.ratio
        } else {
            min + self.ratio * (max - min)
        };
        let center = segment.at(along, across);
        Some(Rect {
            x: center.x - width / 2.0,
            y: center.y - height / 2.0,
            width,
            height,
        })
    }

    /// The placement that puts a label `width` by `height` with its center
    /// at `center` on `route`, by the segment nearest it. Keeps what `old`
    /// says where the label's position doesn't.
    pub(crate) fn through(
        route: &Route,
        center: Point,
        width: f64,
        height: f64,
        old: Option<SmartEdge>,
    ) -> Self {
        let old = old.unwrap_or_default();
        let Some(segments) = route.points.len().checked_sub(1).filter(|&n| n > 0) else {
            return old;
        };
        let distance = |index: &usize| Segment::new(route, *index).distance(center);
        let index = (0..segments)
            .min_by(|a, b| distance(a).total_cmp(&distance(b)))
            .expect("routes have a segment");
        let segment = Segment::new(route, index);
        let (along, across) = segment.coordinates(center);
        let (side, distance) = if across.abs() < ON_EDGE {
            (Side::Center, old.distance)
        } else if across > 0.0 {
            (Side::Left, across)
        } else {
            (Side::Right, -across)
        };
        let across = if side == Side::Center { 0.0 } else { across };
        let (min, max) = slide(route, &segment, index, across, width, height);
        let ratio = if along < min {
            along - min
        } else if along > max {
            // Ratios up to 1 are relative, so a label less than a point past
            // the end goes to the end.
            (along - max).max(1.0)
        } else if max > min {
            (along - min) / (max - min)
        } else {
            0.5
        };
        SmartEdge {
            segment: index as i32,
            ratio,
            distance,
            side,
            to_center: true,
            angle: old.angle,
        }
    }

    /// The same placement once the bend at `bend` is removed from a route of
    /// `segments` segments, which merges the segments either side of it.
    pub(crate) fn without_bend(self, bend: usize, segments: usize) -> Self {
        let index = self.index(segments);
        let index = if index > bend { index - 1 } else { index };
        let segment = if self.segment >= 0 {
            index as i32
        } else {
            index as i32 - (segments as i32 - 1)
        };
        SmartEdge { segment, ..self }
    }

    /// The index of the segment among `segments`, from the source.
    fn index(&self, segments: usize) -> usize {
        let index = if self.segment >= 0 {
            i64::from(self.segment)
        } else {
            segments as i64 + i64::from(self.segment)
        };
        index.clamp(0, segments as i64 - 1) as usize
    }

    /// How far left of `segment` a label `width` by `height` goes.
    fn across(&self, segment: &Segment, width: f64, height: f64) -> f64 {
        let left = segment.left();
        let extent = width / 2.0 * left.x.abs() + height / 2.0 * left.y.abs();
        let distance = if self.to_center {
            self.distance
        } else {
            self.distance + extent
        };
        match self.side {
            Side::Left => distance,
            Side::Right => -distance,
            Side::Center => 0.0,
        }
    }
}

/// How far along `segment`, the one at `index` on `route`, the center of a
/// label `width` by `height` and `across` to its left can be while keeping
/// [`NODE_DISTANCE`] from the nodes its ends meet: from and to where.
fn slide(
    route: &Route,
    segment: &Segment,
    index: usize,
    across: f64,
    width: f64,
    height: f64,
) -> (f64, f64) {
    let last = route.points.len() - 2;
    let backwards = Point {
        x: -segment.direction.x,
        y: -segment.direction.y,
    };
    let min = if index == 0 {
        let center = segment.at(0.0, across);
        clearance(&route.source, center, segment.direction, width, height)
    } else {
        0.0
    };
    let max = if index == last {
        let center = segment.at(segment.length, across);
        segment.length - clearance(&route.target, center, backwards, width, height)
    } else {
        segment.length
    };
    if max < min {
        let middle = (min + max) / 2.0;
        (middle, middle)
    } else {
        (min, max)
    }
}

/// How far a label `width` by `height` centered at `center` must move in
/// `direction` to keep [`NODE_DISTANCE`] from `node`; 0 if it does already.
fn clearance(node: &Rect, center: Point, direction: Point, width: f64, height: f64) -> f64 {
    let (grow_x, grow_y) = (width / 2.0 + NODE_DISTANCE, height / 2.0 + NODE_DISTANCE);
    let (min_x, max_x) = (node.x - grow_x, node.x + node.width + grow_x);
    let (min_y, max_y) = (node.y - grow_y, node.y + node.height + grow_y);
    let inside = center.x > min_x && center.x < max_x && center.y > min_y && center.y < max_y;
    if !inside {
        return 0.0;
    }
    let exit = |start: f64, delta: f64, min: f64, max: f64| {
        if delta > 0.0 {
            (max - start) / delta
        } else if delta < 0.0 {
            (min - start) / delta
        } else {
            f64::INFINITY
        }
    };
    let x = exit(center.x, direction.x, min_x, max_x);
    let y = exit(center.y, direction.y, min_y, max_y);
    x.min(y)
}

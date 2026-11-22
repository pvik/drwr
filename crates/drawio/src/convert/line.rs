//! Lines: their ends, route, arrowheads and labels.

use graphml::{
    Edge, KeyDomain,
    yed::{self, Arrow, Arrows, EdgeGraphics, EdgeKind, Fill, NodeGraphics, NodeKind, Point, Rect},
};

use super::{
    Converter, Placed, add_properties, center,
    label::{place, point_on},
    rect, stroke, turn,
};
use crate::{
    route::{self, End, Router},
    style::Style,
};

/// The size of the nodes added at loose line ends.
const ANCHOR_SIZE: f64 = 2.0;

/// A line, converted once what it ends on is.
pub(super) enum Line {
    Waiting(Pending),
    Converting,
    /// Its edge, unless it's hidden or ends on nothing shown.
    Done(Option<Box<Edge>>),
}

/// An edge cell, with its parent's top-left corner and style.
pub(super) struct Pending {
    pub index: usize,
    pub origin: Point,
    pub parent_style: Option<Style>,
}

/// Where a line ends, before any node is added for it.
enum LineEnd {
    On(Terminal),
    Loose(Point),
}

/// A line's end: the node and where on it.
struct Terminal {
    node: String,
    node_bounds: Rect,
    end: End,
}

/// What draw.io works out where lines are fixed to a vertex from.
#[derive(Clone, Debug)]
pub(super) struct Frame {
    /// The vertex's bounds, unturned.
    bounds: Rect,
    /// Whether its `direction` is north or south.
    upright: bool,
    /// Clockwise quarter turns of its fixed points by `direction`.
    quarters: u8,
    flip_h: bool,
    flip_v: bool,
    perimeter: Option<Perimeter>,
    /// The turn its node shows, in degrees.
    pub rotation: f64,
}

/// The outlines draw.io moves fixed points onto, by `perimeter`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Perimeter {
    Rectangle,
    Ellipse,
    Rhombus,
}

impl Frame {
    pub fn new(style: &Style, bounds: Rect) -> Self {
        let direction = style.get("direction");
        let quarters = match direction {
            _ if !style.flag_or("anchorPointDirection", true) => 0,
            Some("south") => 1,
            Some("west") => 2,
            Some("north") => 3,
            _ => 0,
        };
        let perimeter = style.get("perimeter").map(|perimeter| match perimeter {
            "ellipsePerimeter" => Perimeter::Ellipse,
            "rhombusPerimeter" => Perimeter::Rhombus,
            _ => Perimeter::Rectangle,
        });
        Frame {
            bounds,
            upright: matches!(direction, Some("north" | "south")),
            quarters,
            flip_h: style.flag("flipH"),
            flip_v: style.flag("flipV"),
            perimeter,
            rotation: 0.0,
        }
    }
}

impl Converter<'_> {
    /// Converts `self.lines[line]`, unless it's converted or underway.
    pub(super) fn convert_line(&mut self, line: usize) {
        let pending = match std::mem::replace(&mut self.lines[line], Line::Converting) {
            Line::Waiting(pending) => pending,
            other => {
                self.lines[line] = other;
                return;
            }
        };
        let edge = self.edge(pending).map(Box::new);
        self.lines[line] = Line::Done(edge);
    }

    /// The edge for the edge cell `pending.index`, unless it's hidden or an
    /// end is on nothing shown.
    fn edge(&mut self, pending: Pending) -> Option<Edge> {
        let cells = self.cells;
        let cell = &cells[pending.index];
        if !cell.visible {
            self.notes.hidden_lines += 1;
            return None;
        }
        let style = Style::new(&cell.style, true, pending.parent_style.as_ref());
        let g = &cell.geometry;
        let shift = |p: Point| Point {
            x: p.x + pending.origin.x,
            y: p.y + pending.origin.y,
        };
        let source = self.end(
            cell.source.as_deref(),
            g.source_point.map(shift),
            &style,
            true,
        );
        let target = self.end(
            cell.target.as_deref(),
            g.target_point.map(shift),
            &style,
            false,
        );
        let (Some(source), Some(target)) = (source, target) else {
            self.notes.unplaced_lines += 1;
            return None;
        };
        let (source, target) = (self.terminal(source), self.terminal(target));
        let hints: Vec<Point> = g.points.iter().copied().map(shift).collect();
        let path = if loops(&style, &source, &target, &hints) {
            let segment = style.number("segment", self.grid_size);
            let direction = style.get("direction").unwrap_or("west");
            route::self_loop(
                &source.end,
                &target.end,
                hints.first().copied(),
                segment,
                direction,
            )
        } else {
            route::route(router(&style), &source.end, &target.end, &hints)
        };
        let relative = |port: Point, node: Rect| Point {
            x: port.x - (node.x + node.width / 2.0),
            y: port.y - (node.y + node.height / 2.0),
        };
        let mut graphics = EdgeGraphics::new(EdgeKind::PolyLine);
        graphics.path = yed::Path {
            source: relative(path.source, source.node_bounds),
            target: relative(path.target, target.node_bounds),
            points: path.bends,
        };
        graphics.line = stroke(&style);
        graphics.arrows = Arrows {
            source: arrow(style.get("startArrow"), style.get("startFill") != Some("0")),
            target: arrow(style.get("endArrow"), style.get("endFill") != Some("0")),
        };
        if style.flag("curved") {
            self.notes.curved += 1;
        }
        let along = yed::Route::new(&graphics.path, source.node_bounds, target.node_bounds);
        // Where lines ending on this one end.
        let middle = point_on(&along, 0.0, 0.0, None);
        self.anchors.insert(pending.index, middle);
        if let Some(label) = self.label(cell, &style) {
            let anchor = point_on(&along, g.x, g.y, g.offset);
            graphics.labels.push(place(label, &along, anchor, &style));
        }
        for index in self.children_of(pending.index) {
            let child = &cells[index];
            if !child.vertex || !child.visible {
                continue;
            }
            let cg = &child.geometry;
            let anchor = match cg.relative {
                true => point_on(&along, cg.x, cg.y, cg.offset),
                false => shift(Point { x: cg.x, y: cg.y }),
            };
            let child_style = Style::new(&child.style, false, Some(&style));
            if cg.width > 0.0 && cg.height > 0.0 {
                // A shape on the line rather than a label: its corner is there.
                let bounds = rect(anchor.x, anchor.y, cg.width, cg.height);
                self.visit_vertex(index, &child_style, bounds);
                continue;
            }
            self.anchors.insert(index, anchor);
            if let Some(label) = self.label(child, &child_style) {
                graphics
                    .labels
                    .push(place(label, &along, anchor, &child_style));
            }
        }
        let mut edge = Edge::new(source.node, target.node);
        add_properties(&self.doc.keys, KeyDomain::Edge, &mut edge.data, cell);
        self.keys.set_edge_graphics(&mut edge, &graphics);
        Some(edge)
    }

    /// Where a line ends: on the cell with id `id`, or loose at `point` if
    /// there's no such cell. `None` if that cell isn't shown.
    fn end(
        &mut self,
        id: Option<&str>,
        point: Option<Point>,
        style: &Style,
        source: bool,
    ) -> Option<LineEnd> {
        let Some(&index) = id.and_then(|id| self.index.get(id)) else {
            return point.map(LineEnd::Loose);
        };
        let mut terminal = self.stand_ins.get(&index).copied().unwrap_or(index);
        if !self.placed.contains_key(&terminal) && !self.anchors.contains_key(&terminal) {
            self.convert_containing_lines(terminal);
            terminal = self.stand_ins.get(&index).copied().unwrap_or(index);
        }
        if let Some(placed) = self.placed.get(&terminal) {
            return Some(LineEnd::On(Terminal {
                node: placed.node.clone(),
                node_bounds: placed.node_bounds,
                end: End {
                    bounds: placed.bounds,
                    point: constraint(style, source, placed),
                    loose: false,
                },
            }));
        }
        self.anchors.get(&terminal).copied().map(LineEnd::Loose)
    }

    /// Places an edge terminal or one of its descendants by resolving its
    /// containing lines from outermost to innermost.
    fn convert_containing_lines(&mut self, index: usize) {
        let ancestors = std::iter::successors(Some(index), |&cell| self.parent_of(cell));
        // Broken parent links can form a cycle.
        let lines: Vec<_> = ancestors
            .take(self.cells.len())
            .filter(|&cell| self.cells[cell].edge)
            .collect();
        for cell in lines.into_iter().rev() {
            if let Some(&line) = self.line_of.get(&cell) {
                self.convert_line(line);
            }
        }
    }

    /// The terminal for a line end, adding a small node for a loose one.
    fn terminal(&mut self, end: LineEnd) -> Terminal {
        let point = match end {
            LineEnd::On(terminal) => return terminal,
            LineEnd::Loose(point) => point,
        };
        self.notes.loose_ends += 1;
        let bounds = rect(
            point.x - ANCHOR_SIZE / 2.0,
            point.y - ANCHOR_SIZE / 2.0,
            ANCHOR_SIZE,
            ANCHOR_SIZE,
        );
        let mut graphics = NodeGraphics::new(NodeKind::Shape(yed::ShapeType::Rectangle));
        graphics.geometry = bounds;
        graphics.fill = Fill::default();
        graphics.border.color = None;
        Terminal {
            node: self.push_node(&graphics),
            node_bounds: bounds,
            end: End {
                bounds,
                point: Some(point),
                loose: true,
            },
        }
    }
}

/// Whether draw.io draws the line as a loop: one from a node back to it,
/// with fewer than two placed points.
fn loops(style: &Style, source: &Terminal, target: &Terminal, hints: &[Point]) -> bool {
    let fixed = source.end.point.is_some() || target.end.point.is_some();
    source.node == target.node && hints.len() < 2 && !(style.flag("orthogonalLoop") && fixed)
}

/// Where draw.io fixes a line's end on `vertex` by its `exitX`/`entryX`
/// constraint, if it has one: turned and flipped with the vertex, then moved
/// onto its outline.
fn constraint(style: &Style, source: bool, vertex: &Placed) -> Option<Point> {
    let [x, y, dx, dy, perimeter] = match source {
        true => ["exitX", "exitY", "exitDx", "exitDy", "exitPerimeter"],
        false => ["entryX", "entryY", "entryDx", "entryDy", "entryPerimeter"],
    };
    let (fx, fy) = (style.parsed(x)?, style.parsed(y)?);
    let frame = &vertex.frame;
    let mid = center(frame.bounds);
    let mut bounds = frame.bounds;
    let (mut flip_h, mut flip_v) = (frame.flip_h, frame.flip_v);
    if frame.upright {
        (bounds.width, bounds.height) = (bounds.height, bounds.width);
        bounds.x = mid.x - bounds.width / 2.0;
        bounds.y = mid.y - bounds.height / 2.0;
        (flip_h, flip_v) = (flip_v, flip_h);
    }
    let mut point = Point {
        x: bounds.x + fx * bounds.width + style.number(dx, 0.0),
        y: bounds.y + fy * bounds.height + style.number(dy, 0.0),
    };
    if flip_h {
        point.x = 2.0 * mid.x - point.x;
    }
    if flip_v {
        point.y = 2.0 * mid.y - point.y;
    }
    point = turn(point, mid, f64::from(frame.quarters) * 90.0);
    if let Some(outline) = frame.perimeter.filter(|_| style.flag_or(perimeter, true)) {
        point = onto(outline, frame.bounds, point);
    }
    Some(turn(point, mid, frame.rotation))
}

/// Where draw.io's perimeter `outline` of a vertex in `bounds` meets the line
/// from its center towards `point`.
fn onto(outline: Perimeter, bounds: Rect, point: Point) -> Point {
    let mid = center(bounds);
    let (a, b) = (bounds.width / 2.0, bounds.height / 2.0);
    if a == 0.0 || b == 0.0 {
        // The perimeter has collapsed to a line or a point.
        return Point {
            x: point.x.max(bounds.x).min(bounds.x + bounds.width),
            y: point.y.max(bounds.y).min(bounds.y + bounds.height),
        };
    }
    let (dx, dy) = (point.x - mid.x, point.y - mid.y);
    // How far along the line from the center the outline is.
    let along = match outline {
        Perimeter::Ellipse => {
            // In whole points, as draw.io does.
            let (dx, dy) = (dx.trunc(), dy.trunc());
            if dx == 0.0 && dy == 0.0 {
                return point;
            }
            let t = a * b / (b * b * dx * dx + a * a * dy * dy).sqrt();
            return Point {
                x: mid.x + t * dx,
                y: mid.y + t * dy,
            };
        }
        // Straight at a corner, the bottom one for the center itself.
        Perimeter::Rhombus if dx == 0.0 => {
            let y = if dy < 0.0 { mid.y - b } else { mid.y + b };
            return Point { x: mid.x, y };
        }
        Perimeter::Rhombus if dy == 0.0 => {
            let x = if dx < 0.0 { mid.x - a } else { mid.x + a };
            return Point { x, y: mid.y };
        }
        Perimeter::Rhombus => {
            let along = (dx.abs() / a + dy.abs() / b).recip();
            if along > 1.0 {
                // A point inside doesn't reach it, so draw.io takes the center.
                return mid;
            }
            along
        }
        Perimeter::Rectangle if dx == 0.0 && dy == 0.0 => {
            return Point {
                x: mid.x + a,
                y: mid.y,
            };
        }
        Perimeter::Rectangle => (dx.abs() / a).max(dy.abs() / b).recip(),
    };
    Point {
        x: mid.x + dx * along,
        y: mid.y + dy * along,
    }
}

fn router(style: &Style) -> Router {
    if style.flag("noEdgeStyle") {
        return Router::Straight;
    }
    match style.get("edgeStyle") {
        Some("orthogonalEdgeStyle" | "segmentEdgeStyle") => Router::Orthogonal,
        Some("elbowEdgeStyle") => Router::Elbow {
            vertical: style.is("elbow", "vertical"),
        },
        Some("sideToSideEdgeStyle") => Router::Elbow { vertical: false },
        Some("topToBottomEdgeStyle") => Router::Elbow { vertical: true },
        Some("entityRelationEdgeStyle") => Router::EntityRelation {
            segment: style.number("segment", 30.0),
        },
        Some("sequenceEdgeStyle") => Router::Sequence,
        _ => Router::Straight,
    }
}

/// The yEd arrowhead closest to draw.io's marker `name`, filled or not.
fn arrow(name: Option<&str>, filled: bool) -> Arrow {
    let Some(name) = name else {
        return Arrow::None;
    };
    let other = |keyword: &str| Arrow::Other(keyword.to_owned());
    match name {
        "classic" | "classicThin" if filled => Arrow::Standard,
        "block" | "blockThin" | "doubleBlock" | "mermaidExtension" if filled => Arrow::Delta,
        "classic" | "classicThin" | "block" | "blockThin" | "doubleBlock" | "mermaidExtension" => {
            Arrow::WhiteDelta
        }
        "open" | "openThin" | "async" | "openAsync" => Arrow::Plain,
        "oval" | "circle" | "circlePlus" if filled => Arrow::Circle,
        "sysMLFound" | "sysMLLost" => Arrow::Circle,
        "oval" | "circle" | "circlePlus" | "halfCircle" | "sysMLPackCont" | "sysMLProvInt"
        | "sysMLReqInt" => Arrow::TransparentCircle,
        // yEd has no square, so a box gets the four-sided diamond.
        "diamond" | "diamondThin" | "mermaidDiamond" | "box" if filled => Arrow::Diamond,
        "diamond" | "diamondThin" | "mermaidDiamond" | "box" => Arrow::WhiteDiamond,
        "dash" | "baseDash" => Arrow::Dash,
        "cross" | "sysMLx" => Arrow::SkewedDash,
        "ERone" => other("crows_foot_one"),
        "ERmandOne" => other("crows_foot_one_mandatory"),
        "ERmany" => other("crows_foot_many"),
        "ERoneToMany" => other("crows_foot_many_mandatory"),
        "ERzeroToMany" | "manyOptional" => other("crows_foot_many_optional"),
        "ERzeroToOne" => other("crows_foot_one_optional"),
        // draw.io draws no marker it doesn't know.
        _ => Arrow::None,
    }
}

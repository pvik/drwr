//! Converts a page's cells to GraphML with yEd graphics.

mod notes;

use std::collections::{HashMap, HashSet};

use base64::Engine;
use graphml::{
    Data, Edge, GraphMl, KeyDomain, Node,
    yed::{
        self, Alignment, Arrow, Arrows, Color, EdgeGraphics, EdgeKind, Fill, Font, FontStyle, Keys,
        Label, LabelPosition, LineType, NodeGraphics, NodeKind, Point, Rect, ShapeType, Stroke,
    },
};

use crate::{
    model::{Cell, Model},
    route::{self, End, Router},
    shape::{self, Shape},
    style::Style,
    text,
};
use notes::Notes;

/// A page converted to GraphML.
#[derive(Clone, Debug)]
pub struct Import {
    pub graphml: GraphMl,
    /// The corner radius of each round box, by node id, as draw.io rounds
    /// them; yEd's round rectangles have a fixed 4 pt radius.
    pub corner_radii: Vec<(String, f64)>,
    /// What couldn't be carried over, a sentence each.
    pub notes: Vec<String>,
}

pub(crate) fn convert(model: &Model) -> Import {
    let mut converter = Converter::new(model);
    converter.run();
    Import {
        graphml: converter.doc,
        corner_radii: converter.corner_radii,
        notes: converter.notes.sentences(),
    }
}

/// The size of the nodes added at loose line ends.
const ANCHOR_SIZE: f64 = 2.0;

struct Converter<'a> {
    cells: &'a [Cell],
    /// Cell indexes by id.
    index: HashMap<&'a str, usize>,
    /// Cell indexes by their parent's id, in document order.
    children: HashMap<&'a str, Vec<usize>>,
    /// The ids lines end on.
    terminals: HashSet<&'a str>,
    doc: GraphMl,
    keys: Keys,
    /// The node each drawn vertex became, by cell index.
    placed: HashMap<usize, Placed>,
    /// The collapsed container standing in for each cell hidden in it.
    stand_ins: HashMap<usize, usize>,
    /// Lines to convert once every vertex has its node.
    edges: Vec<Pending>,
    corner_radii: Vec<(String, f64)>,
    notes: Notes,
    node_count: usize,
    edge_count: usize,
}

/// Where a vertex went.
#[derive(Clone, Debug)]
struct Placed {
    node: String,
    /// The bounds of the node lines end on.
    node_bounds: Rect,
    /// The vertex's own bounds, which draw.io routes lines by.
    bounds: Rect,
}

/// An edge cell, with its parent's top-left corner and style.
struct Pending {
    index: usize,
    origin: Point,
    parent_style: Option<Style>,
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

impl<'a> Converter<'a> {
    fn new(model: &'a Model) -> Self {
        let cells = model.cells.as_slice();
        let mut index = HashMap::new();
        let mut children: HashMap<&str, Vec<usize>> = HashMap::new();
        let mut terminals = HashSet::new();
        for (i, cell) in cells.iter().enumerate() {
            index.entry(cell.id.as_str()).or_insert(i);
            if let Some(parent) = &cell.parent {
                children.entry(parent.as_str()).or_default().push(i);
            }
            if cell.edge {
                terminals.extend(cell.source.as_deref());
                terminals.extend(cell.target.as_deref());
            }
        }
        let mut doc = yed::new_document();
        let keys = Keys::ensure(&mut doc);
        Converter {
            cells,
            index,
            children,
            terminals,
            doc,
            keys,
            placed: HashMap::new(),
            stand_ins: HashMap::new(),
            edges: Vec::new(),
            corner_radii: Vec::new(),
            notes: Notes::default(),
            node_count: 0,
            edge_count: 0,
        }
    }

    fn children_of(&self, index: usize) -> Vec<usize> {
        let id = self.cells[index].id.as_str();
        self.children.get(id).cloned().unwrap_or_default()
    }

    fn run(&mut self) {
        let roots = (0..self.cells.len()).filter(|&i| {
            let parent = self.cells[i].parent.as_deref();
            parent.is_none_or(|parent| !self.index.contains_key(parent))
        });
        for root in roots.collect::<Vec<_>>() {
            if self.is_drawn(root) {
                self.visit(root, Point::default(), None);
                continue;
            }
            for layer in self.children_of(root) {
                if self.is_drawn(layer) {
                    self.visit(layer, Point::default(), None);
                } else if !self.cells[layer].visible {
                    self.notes.hidden_layers += 1;
                } else {
                    for child in self.children_of(layer) {
                        self.visit(child, Point::default(), None);
                    }
                }
            }
        }
        for pending in std::mem::take(&mut self.edges) {
            self.edge(pending);
        }
    }

    fn is_drawn(&self, index: usize) -> bool {
        self.cells[index].vertex || self.cells[index].edge
    }

    /// Converts a cell and what's in it, `origin` being its parent's top-left
    /// corner.
    fn visit(&mut self, index: usize, origin: Point, parent: Option<(&Style, Rect)>) {
        let cells = self.cells;
        let cell = &cells[index];
        if cell.edge {
            self.edges.push(Pending {
                index,
                origin,
                parent_style: parent.map(|(style, _)| style.clone()),
            });
            return;
        }
        if !cell.vertex {
            for child in self.children_of(index) {
                self.visit(child, origin, parent);
            }
            return;
        }
        if !cell.visible {
            self.notes.hidden_shapes += 1;
            return;
        }
        let style = Style::new(&cell.style, false, parent.map(|(style, _)| style));
        let g = &cell.geometry;
        let offset = g.offset.unwrap_or_default();
        let bounds = match parent {
            Some((_, p)) if g.relative => Rect {
                x: p.x + g.x * p.width + offset.x,
                y: p.y + g.y * p.height + offset.y,
                width: g.width,
                height: g.height,
            },
            _ => Rect {
                x: origin.x + g.x,
                y: origin.y + g.y,
                width: g.width,
                height: g.height,
            },
        };
        self.vertex(index, &style, bounds);
        let inside = self.children_of(index);
        if cell.collapsed {
            self.stand_in(index, &inside);
            return;
        }
        let corner = Point {
            x: bounds.x,
            y: bounds.y,
        };
        for child in inside {
            self.visit(child, corner, Some((&style, bounds)));
        }
    }

    /// Has the collapsed container at `index` stand in for the cells hidden
    /// in it, when lines end on them.
    fn stand_in(&mut self, index: usize, inside: &[usize]) {
        let mut queue = inside.to_vec();
        while let Some(cell) = queue.pop() {
            self.stand_ins.insert(cell, index);
            queue.extend(self.children_of(cell));
        }
    }

    /// Adds the node or nodes for the vertex at `index`.
    fn vertex(&mut self, index: usize, style: &Style, bounds: Rect) {
        let cells = self.cells;
        let cell = &cells[index];
        let mut bounds = bounds;
        let mut shape = shape::shape(style, bounds.width, bounds.height);
        let rotation = style.number("rotation", 0.0).rem_euclid(360.0);
        if rotation != 0.0 {
            let quarter = (rotation - 90.0).abs() < 1e-6 || (rotation - 270.0).abs() < 1e-6;
            let half = (rotation - 180.0).abs() < 1e-6;
            let symmetric = matches!(
                shape,
                Shape::Box { .. }
                    | Shape::Line { .. }
                    | Shape::Yed(ShapeType::Ellipse | ShapeType::Diamond | ShapeType::Octagon)
            );
            if quarter && symmetric {
                // Turned a quarter, they fill their bounds turned instead.
                let center = (
                    bounds.x + bounds.width / 2.0,
                    bounds.y + bounds.height / 2.0,
                );
                (bounds.width, bounds.height) = (bounds.height, bounds.width);
                bounds.x = center.0 - bounds.width / 2.0;
                bounds.y = center.1 - bounds.height / 2.0;
                if let Shape::Line { vertical } = &mut shape {
                    *vertical = !*vertical;
                }
            } else if !(half && symmetric) {
                self.notes.rotated += 1;
            }
        }
        let label = self.label(cell, style, false);
        match shape {
            Shape::Box { radius } => {
                let kind = match radius > 0.0 {
                    true => ShapeType::RoundRectangle,
                    false => ShapeType::Rectangle,
                };
                let mut graphics = self.graphics(NodeKind::Shape(kind), bounds, style, label);
                if style.get("image").is_some() {
                    self.notes.box_images += 1;
                }
                let mut sides = Vec::new();
                if matches!(style.get("shape"), Some("partialRectangle" | "tableRow")) {
                    let drawn = ["top", "right", "bottom", "left"]
                        .map(|side| style.get(side).is_none_or(|value| value == "1"));
                    if !drawn.iter().all(|drawn| *drawn) {
                        sides = side_lines(bounds, drawn, &graphics.border);
                        graphics.border.color = None;
                    }
                }
                let node = self.add_node(index, &graphics, bounds, bounds);
                if radius > 0.0 {
                    self.corner_radii.push((node, radius));
                }
                for side in sides {
                    self.push_node(&side);
                }
            }
            Shape::Yed(kind) => {
                if turned(style, &kind) {
                    self.notes.turned += 1;
                }
                let graphics = self.graphics(NodeKind::Shape(kind), bounds, style, label);
                self.add_node(index, &graphics, bounds, bounds);
            }
            Shape::Generic(configuration) => {
                let kind = NodeKind::Generic(configuration.to_owned());
                let graphics = self.graphics(kind, bounds, style, label);
                self.add_node(index, &graphics, bounds, bounds);
            }
            Shape::Image(image) => self.image(index, &image, bounds, style, label),
            Shape::Line { vertical } => {
                let width = style.number("strokeWidth", 1.0).max(1.0);
                let mut line = bounds;
                if vertical {
                    line.x += (bounds.width - width) / 2.0;
                    line.width = width;
                } else {
                    line.y += (bounds.height - width) / 2.0;
                    line.height = width;
                }
                let kind = NodeKind::Shape(ShapeType::Rectangle);
                let mut graphics = self.graphics(kind, line, style, label);
                graphics.fill.color = style
                    .color("strokeColor", Color::BLACK)
                    .map(|c| opacity(c, style, "strokeOpacity"));
                graphics.fill.gradient = None;
                graphics.border.color = None;
                if style.flag("dashed") {
                    self.notes.dashed_lines += 1;
                }
                self.add_node(index, &graphics, line, bounds);
            }
            Shape::Swimlane { start, horizontal } => {
                let kind = NodeKind::Shape(ShapeType::Rectangle);
                let mut head = bounds;
                if horizontal {
                    head.height = start.min(bounds.height);
                } else {
                    head.width = start.min(bounds.width);
                }
                let titled = head.width > 0.0 && head.height > 0.0;
                let body_label = if titled { None } else { label.clone() };
                let mut body = self.graphics(kind.clone(), bounds, style, body_label);
                body.fill.color = style
                    .color("swimlaneFillColor", Color::WHITE)
                    .map(|c| opacity(c, style, "fillOpacity"));
                body.fill.gradient = None;
                self.add_node(index, &body, bounds, bounds);
                if titled {
                    if !horizontal && label.is_some() {
                        self.notes.vertical_titles += 1;
                    }
                    let graphics = self.graphics(kind, head, style, label);
                    self.push_node(&graphics);
                }
            }
            Shape::Lifeline { head } => {
                let kind = NodeKind::Shape(ShapeType::Rectangle);
                let mut top = bounds;
                top.height = head.max(1.0);
                let graphics = self.graphics(kind.clone(), top, style, label);
                self.push_node(&graphics);
                let width = style.number("strokeWidth", 1.0).max(1.0);
                let line = Rect {
                    x: bounds.x + (bounds.width - width) / 2.0,
                    y: top.y + top.height,
                    width,
                    height: (bounds.height - top.height).max(1.0),
                };
                let mut graphics = self.graphics(kind, line, style, None);
                graphics.fill.color = style.color("strokeColor", Color::BLACK);
                graphics.fill.gradient = None;
                graphics.border.color = None;
                self.add_node(index, &graphics, line, bounds);
            }
            Shape::Group => {
                // Invisible, so needed only for lines to end on.
                if self.terminals.contains(cell.id.as_str()) {
                    let kind = NodeKind::Shape(ShapeType::Rectangle);
                    let graphics = self.graphics(kind, bounds, style, label);
                    self.add_node(index, &graphics, bounds, bounds);
                }
            }
            Shape::Unknown(name) => {
                *self.notes.unknown.entry(name).or_default() += 1;
                let kind = NodeKind::Shape(ShapeType::Rectangle);
                let graphics = self.graphics(kind, bounds, style, label);
                self.add_node(index, &graphics, bounds, bounds);
            }
        }
    }

    /// Adds an image's node: an SVG node if the file holds the SVG, else an
    /// empty dashed box where it was.
    fn image(
        &mut self,
        index: usize,
        image: &str,
        bounds: Rect,
        style: &Style,
        label: Option<Label>,
    ) {
        match svg(image) {
            Some(svg) => {
                let resource = Some(yed::add_resource(&mut self.doc, &svg));
                let mut graphics = NodeGraphics::new(NodeKind::Svg { resource });
                graphics.geometry = node_rect(bounds);
                graphics.labels.extend(label);
                self.add_node(index, &graphics, bounds, bounds);
            }
            None => {
                if image.starts_with("data:") {
                    self.notes.raster_images += 1;
                } else {
                    self.notes.linked_images += 1;
                }
                let kind = NodeKind::Shape(ShapeType::Rectangle);
                let mut graphics = self.graphics(kind, bounds, style, label);
                graphics.fill = Fill::default();
                graphics.border = Stroke {
                    color: Some(Color::rgb(0xC0, 0xC0, 0xC0)),
                    line_type: LineType::Dashed,
                    width: 1.0,
                };
                self.add_node(index, &graphics, bounds, bounds);
            }
        }
    }

    /// Graphics of `kind` in `bounds`, filled and outlined as `style` says.
    fn graphics(
        &self,
        kind: NodeKind,
        bounds: Rect,
        style: &Style,
        label: Option<Label>,
    ) -> NodeGraphics {
        let mut graphics = NodeGraphics::new(kind);
        graphics.geometry = node_rect(bounds);
        let fill = |key| {
            let color = style.color(key, Color::WHITE);
            color.map(|c| opacity(c, style, "fillOpacity"))
        };
        let color = fill("fillColor");
        graphics.fill = Fill {
            gradient: color.and(fill("gradientColor")),
            color,
        };
        graphics.border = stroke(style);
        graphics.labels.extend(label);
        graphics
    }

    /// Adds a node for the vertex at `index`, lines ending on it within
    /// `node_bounds`. Returns its id.
    fn add_node(
        &mut self,
        index: usize,
        graphics: &NodeGraphics,
        node_bounds: Rect,
        bounds: Rect,
    ) -> String {
        let id = self.push_node(graphics);
        let node = self.doc.graphs[0].nodes.last_mut().expect("just added");
        add_properties(&self.doc.keys, node, &self.cells[index]);
        self.placed.insert(
            index,
            Placed {
                node: id.clone(),
                node_bounds: node_rect(node_bounds),
                bounds,
            },
        );
        id
    }

    fn push_node(&mut self, graphics: &NodeGraphics) -> String {
        let id = format!("n{}", self.node_count);
        self.node_count += 1;
        let mut node = Node::new(id.clone());
        self.keys.set_node_graphics(&mut node, graphics);
        self.doc.graphs[0].nodes.push(node);
        id
    }

    /// The label of `cell`, unless it has none.
    fn label(&mut self, cell: &Cell, style: &Style, edge: bool) -> Option<Label> {
        if style.flag("noLabel") {
            return None;
        }
        let mut value = cell.value.clone();
        if cell
            .properties
            .get("placeholders")
            .is_some_and(|p| p == "1")
        {
            for (name, property) in &cell.properties {
                value = value.replace(&format!("%{name}%"), property);
            }
        }
        let text = match style.is("html", "1") {
            true => text::from_html(&value),
            false => text::from_plain(&value),
        };
        if text.text.trim().is_empty() {
            return None;
        }
        if text.lost_formatting {
            self.notes.formatting += 1;
        }
        let mut label = match edge {
            true => Label::new_on_edge(text.text),
            false => Label::new(text.text),
        };
        let bits = style.number("fontStyle", 0.0) as u32;
        let bold = text.bold || bits & 1 != 0;
        let italic = text.italic || bits & 2 != 0;
        label.font = Font {
            family: font_family(style),
            size: style.number("fontSize", 12.0).round().max(1.0) as u32,
            style: match (bold, italic) {
                (true, true) => FontStyle::BoldItalic,
                (true, false) => FontStyle::Bold,
                (false, true) => FontStyle::Italic,
                (false, false) => FontStyle::Plain,
            },
        };
        label.text_color = style
            .color("fontColor", Color::BLACK)
            .map_or(Color::BLACK, |c| opacity(c, style, "textOpacity"));
        label.background = style.color("labelBackgroundColor", Color::WHITE);
        label.border = style.color("labelBorderColor", Color::BLACK);
        label.alignment = match style.get("align") {
            Some("left") => Alignment::Left,
            Some("right") => Alignment::Right,
            _ => Alignment::Center,
        };
        if !edge {
            label.set_position(position(style));
        }
        Some(label)
    }

    /// Adds the line for the edge cell `pending.index`.
    fn edge(&mut self, pending: Pending) {
        let cells = self.cells;
        let cell = &cells[pending.index];
        if !cell.visible {
            self.notes.hidden_lines += 1;
            return;
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
            return;
        };
        let (source, target) = (self.terminal(source), self.terminal(target));
        let hints: Vec<Point> = g.points.iter().copied().map(shift).collect();
        let path = route::route(router(&style), &source.end, &target.end, &hints);
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
        if let Some(label) = self.label(cell, &style, true) {
            let label = place(label, &along, g.x, g.y, g.offset, &style);
            graphics.labels.push(label);
        }
        for child in self.children_of(pending.index) {
            let child = &cells[child];
            if !child.vertex || !child.visible {
                continue;
            }
            let child_style = Style::new(&child.style, false, Some(&style));
            if let Some(label) = self.label(child, &child_style, true) {
                let cg = &child.geometry;
                let label = place(label, &along, cg.x, cg.y, cg.offset, &child_style);
                graphics.labels.push(label);
            }
        }
        let id = format!("e{}", self.edge_count);
        self.edge_count += 1;
        let mut edge = Edge::new(source.node, target.node);
        edge.id = Some(id);
        add_properties(&self.doc.keys, &mut edge, cell);
        self.keys.set_edge_graphics(&mut edge, &graphics);
        self.doc.graphs[0].edges.push(edge);
    }

    /// Where a line ends: on the vertex with id `id`, or loose at `point`.
    fn end(
        &self,
        id: Option<&str>,
        point: Option<Point>,
        style: &Style,
        source: bool,
    ) -> Option<LineEnd> {
        let mut point = point;
        if let Some(&index) = id.and_then(|id| self.index.get(id)) {
            let index = self.stand_ins.get(&index).copied().unwrap_or(index);
            if let Some(placed) = self.placed.get(&index) {
                return Some(LineEnd::On(Terminal {
                    node: placed.node.clone(),
                    node_bounds: placed.node_bounds,
                    end: End {
                        bounds: placed.bounds,
                        point: constraint(style, source, placed.bounds),
                    },
                }));
            }
            if !self.cells[index].edge {
                // A vertex that isn't drawn, such as a hidden one.
                return None;
            }
            point = point.or_else(|| self.middle_of(index));
        }
        point.map(LineEnd::Loose)
    }

    /// The terminal for a line end, adding a small node for a loose one.
    fn terminal(&mut self, end: LineEnd) -> Terminal {
        let point = match end {
            LineEnd::On(terminal) => return terminal,
            LineEnd::Loose(point) => point,
        };
        self.notes.loose_ends += 1;
        let bounds = Rect {
            x: point.x - ANCHOR_SIZE / 2.0,
            y: point.y - ANCHOR_SIZE / 2.0,
            width: ANCHOR_SIZE,
            height: ANCHOR_SIZE,
        };
        let mut graphics = NodeGraphics::new(NodeKind::Shape(ShapeType::Rectangle));
        graphics.geometry = bounds;
        graphics.fill = Fill::default();
        graphics.border.color = None;
        Terminal {
            node: self.push_node(&graphics),
            node_bounds: bounds,
            end: End {
                bounds,
                point: Some(point),
            },
        }
    }

    /// Roughly the middle of the edge cell at `index`, for lines ending on it.
    fn middle_of(&self, index: usize) -> Option<Point> {
        let cell = &self.cells[index];
        let points = &cell.geometry.points;
        if !points.is_empty() {
            return Some(points[points.len() / 2]);
        }
        let center = |id: Option<&str>| {
            let placed = self.placed.get(self.index.get(id?)?)?;
            let b = placed.bounds;
            Some((b.x + b.width / 2.0, b.y + b.height / 2.0))
        };
        let (sx, sy) = center(cell.source.as_deref())?;
        let (tx, ty) = center(cell.target.as_deref())?;
        Some(Point {
            x: (sx + tx) / 2.0,
            y: (sy + ty) / 2.0,
        })
    }
}

/// Lines along the `drawn` sides (top, right, bottom, left) of `bounds`, as
/// nodes filled with the border's color.
fn side_lines(bounds: Rect, drawn: [bool; 4], border: &Stroke) -> Vec<NodeGraphics> {
    let thick = border.width.max(1.0);
    let Rect {
        x,
        y,
        width,
        height,
    } = bounds;
    let half = thick / 2.0;
    let sides = [
        rect(x, y - half, width, thick),
        rect(x + width - half, y, thick, height),
        rect(x, y + height - half, width, thick),
        rect(x - half, y, thick, height),
    ];
    (sides.into_iter().zip(drawn))
        .filter(|(_, drawn)| *drawn)
        .map(|(side, _)| {
            let mut line = NodeGraphics::new(NodeKind::Shape(ShapeType::Rectangle));
            line.geometry = side;
            line.fill = Fill {
                color: border.color,
                gradient: None,
            };
            line.border.color = None;
            line
        })
        .collect()
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

/// Bounds a node can have: drwr and yEd draw nothing narrower than a point.
fn node_rect(bounds: Rect) -> Rect {
    Rect {
        width: bounds.width.max(1.0),
        height: bounds.height.max(1.0),
        ..bounds
    }
}

/// The SVG text of an image given as an SVG data URL.
fn svg(image: &str) -> Option<String> {
    let data = image.strip_prefix("data:image/svg+xml")?;
    let (params, payload) = data.split_once(',')?;
    // draw.io drops ";base64" from images in styles, as ';' ends a value.
    let decoded = match base64::engine::general_purpose::STANDARD.decode(payload.trim()) {
        Ok(bytes) => String::from_utf8(bytes).ok(),
        Err(_) if !params.contains("base64") => percent_decode(payload),
        Err(_) => None,
    };
    decoded.filter(|svg| svg.contains("<svg"))
}

fn percent_decode(text: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(text.len());
    let mut rest = text.as_bytes();
    while let [first, tail @ ..] = rest {
        rest = tail;
        if *first == b'%' && tail.len() >= 2 {
            let hex = std::str::from_utf8(&tail[..2]).ok()?;
            bytes.push(u8::from_str_radix(hex, 16).ok()?);
            rest = &tail[2..];
        } else {
            bytes.push(*first);
        }
    }
    String::from_utf8(bytes).ok()
}

/// Applies draw.io's `opacity`, and the opacity under `key`, to `color`.
fn opacity(color: Color, style: &Style, key: &str) -> Color {
    let fraction = style.number("opacity", 100.0) / 100.0 * style.number(key, 100.0) / 100.0;
    Color {
        a: (f64::from(color.a) * fraction.clamp(0.0, 1.0)).round() as u8,
        ..color
    }
}

fn stroke(style: &Style) -> Stroke {
    Stroke {
        color: style
            .color("strokeColor", Color::BLACK)
            .map(|c| opacity(c, style, "strokeOpacity")),
        line_type: line_type(style),
        width: style.number("strokeWidth", 1.0).max(0.0),
    }
}

/// The line type closest to draw.io's dashes.
fn line_type(style: &Style) -> LineType {
    if !style.flag("dashed") {
        return LineType::Line;
    }
    let pattern: Vec<f64> = style
        .get("dashPattern")
        .unwrap_or_default()
        .split_whitespace()
        .filter_map(|n| n.parse().ok())
        .collect();
    match pattern.as_slice() {
        [dash, ..] if *dash <= 1.0 => LineType::Dotted,
        [_, _, _, _, ..] => LineType::DashedDotted,
        _ => LineType::Dashed,
    }
}

/// The first font family in `fontFamily`, which may be a CSS list.
fn font_family(style: &Style) -> String {
    let families = style.get("fontFamily").unwrap_or("Helvetica");
    let first = families.split(',').next().unwrap_or_default();
    let first = first.trim().trim_matches(['"', '\'']);
    match first.is_empty() {
        true => "Helvetica".to_owned(),
        false => first.to_owned(),
    }
}

/// The yEd position closest to where draw.io puts a vertex's label.
fn position(style: &Style) -> LabelPosition {
    use LabelPosition::*;
    let outside_x = style.get("labelPosition").unwrap_or("center");
    let outside_y = style.get("verticalLabelPosition").unwrap_or("middle");
    match (outside_x, outside_y) {
        ("center", "middle") => {}
        ("center", "top") => return North,
        ("center", "bottom") => return South,
        ("left", "middle") => return West,
        ("right", "middle") => return East,
        ("left", "top") => return NorthWest,
        ("right", "top") => return NorthEast,
        ("left", "bottom") => return SouthWest,
        ("right", "bottom") => return SouthEast,
        _ => {}
    }
    match (
        style.get("align").unwrap_or("center"),
        style.get("verticalAlign").unwrap_or("middle"),
    ) {
        ("left", "top") => TopLeft,
        ("right", "top") => TopRight,
        ("left", "bottom") => BottomLeft,
        ("right", "bottom") => BottomRight,
        ("left", _) => Left,
        ("right", _) => Right,
        (_, "top") => Top,
        (_, "bottom") => Bottom,
        _ => Center,
    }
}

/// Whether draw.io draws a yEd shape turned some way yEd can't.
fn turned(style: &Style, kind: &ShapeType) -> bool {
    let direction = style.get("direction").unwrap_or("east");
    let upright = matches!(direction, "north" | "south");
    match kind {
        // yEd's triangle points up; draw.io's points right unless turned.
        ShapeType::Triangle => direction != "north" || style.flag("flipV"),
        ShapeType::Parallelogram => upright || style.flag("flipH") != (direction == "west"),
        ShapeType::Hexagon | ShapeType::Trapezoid | ShapeType::Trapezoid2 => upright,
        _ => false,
    }
}

/// The point a line's draw.io constraint fixes it to on a node in `bounds`.
fn constraint(style: &Style, source: bool, bounds: Rect) -> Option<Point> {
    let [x, y, dx, dy] = match source {
        true => ["exitX", "exitY", "exitDx", "exitDy"],
        false => ["entryX", "entryY", "entryDx", "entryDy"],
    };
    let fraction = |key: &str| {
        style
            .get(key)?
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
    };
    Some(Point {
        x: bounds.x + fraction(x)? * bounds.width + style.number(dx, 0.0),
        y: bounds.y + fraction(y)? * bounds.height + style.number(dy, 0.0),
    })
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
        "block" | "blockThin" | "doubleBlock" if filled => Arrow::Delta,
        "classic" | "classicThin" | "block" | "blockThin" | "doubleBlock" => Arrow::WhiteDelta,
        "open" | "openThin" | "async" | "openAsync" => Arrow::Plain,
        "oval" | "circle" | "circlePlus" if filled => Arrow::Circle,
        "oval" | "circle" | "circlePlus" | "halfCircle" => Arrow::TransparentCircle,
        "diamond" | "diamondThin" if filled => Arrow::Diamond,
        "diamond" | "diamondThin" => Arrow::WhiteDiamond,
        "dash" | "baseDash" => Arrow::Dash,
        "cross" => Arrow::SkewedDash,
        "ERone" => other("crows_foot_one"),
        "ERmandOne" => other("crows_foot_one_mandatory"),
        "ERmany" => other("crows_foot_many"),
        "ERoneToMany" => other("crows_foot_many_mandatory"),
        "ERzeroToMany" => other("crows_foot_many_optional"),
        "ERzeroToOne" => other("crows_foot_one_optional"),
        _ => Arrow::Standard,
    }
}

/// Places an edge label as draw.io does: `x` from -1 to 1 along the line,
/// `y` away from it, moved by `offset`, then aligned to that point.
fn place(
    mut label: Label,
    route: &yed::Route,
    x: f64,
    y: f64,
    offset: Option<Point>,
    style: &Style,
) -> Label {
    let points = &route.points;
    let lengths: Vec<f64> = points.windows(2).map(|w| distance(w[0], w[1])).collect();
    let total: f64 = lengths.iter().sum();
    let mut center = points[0];
    if total > 0.0 {
        let mut left = ((x / 2.0 + 0.5) * total).clamp(0.0, total);
        for (i, &length) in lengths.iter().enumerate() {
            if left <= length || i == lengths.len() - 1 {
                let (a, b) = (points[i], points[i + 1]);
                let t = if length > 0.0 { left / length } else { 0.0 };
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let (nx, ny) = match length > 0.0 {
                    true => (dy / length, dx / length),
                    false => (0.0, 0.0),
                };
                center = Point {
                    x: a.x + dx * t + nx * y,
                    y: a.y + dy * t - ny * y,
                };
                break;
            }
            left -= length;
        }
    }
    let offset = offset.unwrap_or_default();
    center.x += offset.x;
    center.y += offset.y;
    let (width, height) = estimate_size(&label);
    // The point is the label's center unless it's aligned otherwise.
    match style.get("align") {
        Some("left") => center.x += width / 2.0,
        Some("right") => center.x -= width / 2.0,
        _ => {}
    }
    match style.get("verticalAlign") {
        Some("top") => center.y += height / 2.0,
        Some("bottom") => center.y -= height / 2.0,
        _ => {}
    }
    label.place_on(route, center, width, height);
    label
}

fn distance(a: Point, b: Point) -> f64 {
    (b.x - a.x).hypot(b.y - a.y)
}

/// A label's rough size, before a renderer measures it.
fn estimate_size(label: &Label) -> (f64, f64) {
    let size = f64::from(label.font.size);
    let lines = label.text.lines().count().max(1);
    let widest = label
        .text
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    (
        widest as f64 * size * 0.55 + 4.0,
        lines as f64 * size * 1.2 + 4.0,
    )
}

/// Keeps a draw.io link and tooltip as yEd's `url` and `description` data.
fn add_properties(keys: &[graphml::Key], element: &mut impl HasData, cell: &Cell) {
    let domain = element.domain();
    for (property, name) in [("link", "url"), ("tooltip", "description")] {
        let Some(value) = cell.properties.get(property).filter(|v| !v.is_empty()) else {
            continue;
        };
        let key = keys
            .iter()
            .find(|key| key.domain == domain && key.attr_name.as_deref() == Some(name));
        if let Some(key) = key {
            element
                .data()
                .push(Data::from_text(key.id.clone(), value.clone()));
        }
    }
}

trait HasData {
    fn domain(&self) -> KeyDomain;
    fn data(&mut self) -> &mut Vec<Data>;
}

impl HasData for Node {
    fn domain(&self) -> KeyDomain {
        KeyDomain::Node
    }

    fn data(&mut self) -> &mut Vec<Data> {
        &mut self.data
    }
}

impl HasData for Edge {
    fn domain(&self) -> KeyDomain {
        KeyDomain::Edge
    }

    fn data(&mut self) -> &mut Vec<Data> {
        &mut self.data
    }
}

//! Converts a page's cells to GraphML with yEd graphics.

mod label;
mod line;
mod notes;

use std::collections::{HashMap, HashSet};

use base64::Engine;
use graphml::{
    Data, GraphMl, Key, KeyDomain, Node, drwr,
    yed::{
        self, Color, Fill, Keys, LineType, NodeGraphics, NodeKind, Point, Rect, ShapeType, Stroke,
    },
};

use crate::{
    model::{Cell, Model},
    shape::{self, Shape},
    style::Style,
};
use line::{Frame, Line, Pending};
use notes::Notes;

/// A page converted to GraphML.
#[derive(Clone, Debug)]
pub struct Import {
    /// The converted document, including drwr's corner-radius attributes.
    pub graphml: GraphMl,
    /// What couldn't be carried over, a sentence each.
    pub notes: Vec<String>,
}

pub(crate) fn convert(model: &Model) -> Import {
    let mut converter = Converter::new(model);
    converter.run();
    Import {
        graphml: converter.doc,
        notes: converter.notes.sentences(),
    }
}

struct Converter<'a> {
    cells: &'a [Cell],
    /// Cell indexes by id.
    index: HashMap<&'a str, usize>,
    /// Cell indexes by their parent's index, in document order.
    children: HashMap<usize, Vec<usize>>,
    /// The ids lines end on.
    terminals: HashSet<&'a str>,
    grid_size: f64,
    doc: GraphMl,
    keys: Keys,
    /// The node each drawn vertex became, by cell index.
    placed: HashMap<usize, Placed>,
    /// The collapsed container standing in for each cell hidden in it.
    stand_ins: HashMap<usize, usize>,
    /// Lines queued by the traversal. Converting a line can queue more lines
    /// inside shapes attached to it.
    lines: Vec<Line>,
    /// Which of `lines` each edge cell is, by cell index.
    line_of: HashMap<usize, usize>,
    /// Where lines ending on a converted line, or on a label of one, end, by
    /// cell index.
    anchors: HashMap<usize, Point>,
    notes: Notes,
    node_count: usize,
}

/// Where a vertex went.
#[derive(Clone, Debug)]
struct Placed {
    node: String,
    /// The bounds of the node lines end on.
    node_bounds: Rect,
    /// The vertex's bounds, turned as its node is, which lines are routed by.
    bounds: Rect,
    frame: Frame,
}

/// A vertex's style and unrotated bounds, with the rotation kept on import.
#[derive(Clone, Copy)]
struct Parent<'a> {
    style: &'a Style,
    bounds: Rect,
    rotation: f64,
}

impl<'a> Converter<'a> {
    fn new(model: &'a Model) -> Self {
        let cells = model.cells.as_slice();
        let mut index = HashMap::new();
        for (i, cell) in cells.iter().enumerate() {
            index.entry(cell.id.as_str()).or_insert(i);
        }
        let mut children: HashMap<usize, Vec<usize>> = HashMap::new();
        let mut terminals = HashSet::new();
        for (i, cell) in cells.iter().enumerate() {
            if let Some(&parent) = cell.parent.as_deref().and_then(|id| index.get(id)) {
                children.entry(parent).or_default().push(i);
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
            grid_size: model.grid_size,
            doc,
            keys,
            placed: HashMap::new(),
            stand_ins: HashMap::new(),
            lines: Vec::new(),
            line_of: HashMap::new(),
            anchors: HashMap::new(),
            notes: Notes::default(),
            node_count: 0,
        }
    }

    fn children_of(&self, index: usize) -> Vec<usize> {
        self.children.get(&index).cloned().unwrap_or_default()
    }

    /// The index of the cell the cell at `index` is in, if it's on the page.
    fn parent_of(&self, index: usize) -> Option<usize> {
        let parent = self.cells[index].parent.as_deref()?;
        self.index.get(parent).copied()
    }

    fn run(&mut self) {
        let roots = (0..self.cells.len()).filter(|&i| self.parent_of(i).is_none());
        for root in roots.collect::<Vec<_>>() {
            if self.is_drawn(root) {
                self.visit(root, None);
                continue;
            }
            for layer in self.children_of(root) {
                if self.is_drawn(layer) {
                    self.visit(layer, None);
                } else if !self.cells[layer].visible {
                    self.notes.hidden_layers += 1;
                } else {
                    for child in self.children_of(layer) {
                        self.visit(child, None);
                    }
                }
            }
        }
        let mut line = 0;
        while line < self.lines.len() {
            self.convert_line(line);
            line += 1;
        }
        let edges = std::mem::take(&mut self.lines).into_iter();
        let edges = edges.filter_map(|line| match line {
            Line::Done(edge) => edge,
            _ => None,
        });
        for (n, mut edge) in edges.enumerate() {
            edge.id = Some(format!("e{n}"));
            self.doc.graphs[0].edges.push(*edge);
        }
    }

    fn is_drawn(&self, index: usize) -> bool {
        self.cells[index].vertex || self.cells[index].edge
    }

    /// Converts a cell and its children, using the parent's placement when present.
    fn visit(&mut self, index: usize, parent: Option<Parent<'_>>) {
        let cells = self.cells;
        let cell = &cells[index];
        let origin = parent.map_or_else(Point::default, |parent| Point {
            x: parent.bounds.x,
            y: parent.bounds.y,
        });
        let parent_style = parent.map(|parent| parent.style);
        if cell.edge {
            self.line_of.insert(index, self.lines.len());
            self.lines.push(Line::Waiting(Pending {
                index,
                origin,
                parent_style: parent_style.cloned(),
            }));
            return;
        }
        if !cell.vertex {
            for child in self.children_of(index) {
                self.visit(child, parent);
            }
            return;
        }
        if !cell.visible {
            self.notes.hidden_shapes += 1;
            return;
        }
        let style = Style::new(&cell.style, false, parent_style);
        let g = &cell.geometry;
        let bounds = match parent {
            Some(parent) if g.relative => {
                let offset = g.offset.unwrap_or_default();
                let p = parent.bounds;
                let mut bounds = rect(
                    p.x + g.x * p.width + offset.x,
                    p.y + g.y * p.height + offset.y,
                    g.width,
                    g.height,
                );
                if parent.rotation != 0.0 {
                    let middle = turn(center(bounds), center(p), parent.rotation);
                    bounds.x = middle.x - bounds.width / 2.0;
                    bounds.y = middle.y - bounds.height / 2.0;
                }
                bounds
            }
            _ => rect(origin.x + g.x, origin.y + g.y, g.width, g.height),
        };
        self.visit_vertex(index, &style, bounds);
    }

    /// Converts a vertex whose page bounds have been resolved, then its children.
    fn visit_vertex(&mut self, index: usize, style: &Style, bounds: Rect) {
        let rotation = self.vertex(index, style, bounds);
        let inside = self.children_of(index);
        if self.cells[index].collapsed {
            self.stand_in(index, &inside);
            return;
        }
        let parent = Parent {
            style,
            bounds,
            rotation,
        };
        for child in inside {
            self.visit(child, Some(parent));
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

    /// Adds the nodes for the vertex and returns the rotation they show.
    fn vertex(&mut self, index: usize, style: &Style, bounds: Rect) -> f64 {
        let cells = self.cells;
        let cell = &cells[index];
        let mut shape = shape::shape(style, bounds.width, bounds.height);
        let mut frame = Frame::new(style, bounds);
        let mut bounds = bounds;
        let rotation = style.number("rotation", 0.0).rem_euclid(360.0);
        let quarters = (rotation / 90.0).round();
        let symmetric = matches!(
            shape,
            Shape::Box { .. }
                | Shape::Line { .. }
                | Shape::Yed(ShapeType::Ellipse | ShapeType::Diamond | ShapeType::Octagon)
        );
        if symmetric && (rotation - quarters * 90.0).abs() < 1e-6 {
            frame.rotation = quarters * 90.0;
            if quarters % 2.0 == 1.0 {
                // Turned a quarter, they fill their bounds turned instead.
                let center = center(bounds);
                (bounds.width, bounds.height) = (bounds.height, bounds.width);
                bounds.x = center.x - bounds.width / 2.0;
                bounds.y = center.y - bounds.height / 2.0;
                if let Shape::Line { vertical } = &mut shape {
                    *vertical = !*vertical;
                }
            }
        } else if rotation != 0.0 {
            self.notes.rotated += 1;
        }
        let label = self.label(cell, style).map(|mut label| {
            label.set_position(label::position(style));
            label
        });
        let node = match shape {
            Shape::Box { radius } => {
                let kind = match radius > 0.0 {
                    true => ShapeType::RoundRectangle,
                    false => ShapeType::Rectangle,
                };
                let mut graphics = graphics(NodeKind::Shape(kind), bounds, style, label);
                if style.get("image").is_some() {
                    self.notes.box_images += 1;
                }
                let mut sides = Vec::new();
                if matches!(style.get("shape"), Some("partialRectangle" | "tableRow")) {
                    let drawn = ["top", "right", "bottom", "left"].map(|s| style.flag_or(s, true));
                    if !drawn.iter().all(|drawn| *drawn) {
                        sides = side_lines(bounds, drawn, &graphics.border);
                        graphics.border.color = None;
                    }
                }
                let node = self.add_node(index, &graphics, Some(radius));
                for side in sides {
                    self.push_node(&side);
                }
                Some((node, bounds))
            }
            Shape::Yed(kind) => {
                if turned(style, &kind) {
                    self.notes.turned += 1;
                }
                let graphics = graphics(NodeKind::Shape(kind), bounds, style, label);
                Some((self.add_node(index, &graphics, None), bounds))
            }
            Shape::Generic(configuration) => {
                let kind = NodeKind::Generic(configuration.to_owned());
                let graphics = graphics(kind, bounds, style, label);
                Some((self.add_node(index, &graphics, None), bounds))
            }
            Shape::Image(image) => Some((self.image(index, &image, bounds, style, label), bounds)),
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
                if style.flag("dashed") {
                    self.notes.dashed_lines += 1;
                }
                let graphics = line_graphics(line, style, label);
                Some((self.add_node(index, &graphics, None), line))
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
                let mut body = graphics(kind.clone(), bounds, style, body_label);
                body.fill.color = style
                    .color("swimlaneFillColor", Color::WHITE)
                    .map(|c| opacity(c, style, "fillOpacity"));
                body.fill.gradient = None;
                let node = self.add_node(index, &body, None);
                if titled {
                    self.push_node(&graphics(kind, head, style, label));
                }
                Some((node, bounds))
            }
            Shape::Lifeline { head } => {
                let kind = NodeKind::Shape(ShapeType::Rectangle);
                let mut top = bounds;
                top.height = head.max(1.0);
                self.push_node(&graphics(kind, top, style, label));
                let width = style.number("strokeWidth", 1.0).max(1.0);
                let line = Rect {
                    x: bounds.x + (bounds.width - width) / 2.0,
                    y: top.y + top.height,
                    width,
                    height: (bounds.height - top.height).max(1.0),
                };
                if style.flag_or("lifelineDashed", true) {
                    self.notes.dashed_lines += 1;
                }
                let graphics = line_graphics(line, style, None);
                Some((self.add_node(index, &graphics, None), line))
            }
            Shape::Group if label.is_some() || self.terminals.contains(cell.id.as_str()) => {
                let kind = NodeKind::Shape(ShapeType::Rectangle);
                let graphics = graphics(kind, bounds, style, label);
                Some((self.add_node(index, &graphics, None), bounds))
            }
            Shape::Group => None,
            Shape::Unknown(name) => {
                *self.notes.unknown.entry(name).or_default() += 1;
                let kind = NodeKind::Shape(ShapeType::Rectangle);
                let graphics = graphics(kind, bounds, style, label);
                Some((self.add_node(index, &graphics, None), bounds))
            }
        };
        let rotation = frame.rotation;
        if let Some((node, node_bounds)) = node {
            let node_bounds = node_rect(node_bounds);
            let placed = Placed {
                node,
                node_bounds,
                bounds,
                frame,
            };
            self.placed.insert(index, placed);
        }
        rotation
    }

    /// Adds an image's node, returning its id: an SVG node if the file holds
    /// the SVG, else an empty dashed box where it was.
    fn image(
        &mut self,
        index: usize,
        image: &str,
        bounds: Rect,
        style: &Style,
        label: Option<yed::Label>,
    ) -> String {
        match svg(image) {
            Some(svg) => {
                let resource = Some(yed::add_resource(&mut self.doc, &svg));
                let mut graphics = NodeGraphics::new(NodeKind::Svg { resource });
                graphics.geometry = node_rect(bounds);
                graphics.labels.extend(label);
                self.add_node(index, &graphics, None)
            }
            None => {
                if image.starts_with("data:") {
                    self.notes.raster_images += 1;
                } else {
                    self.notes.linked_images += 1;
                }
                let kind = NodeKind::Shape(ShapeType::Rectangle);
                let mut graphics = graphics(kind, bounds, style, label);
                graphics.fill = Fill::default();
                graphics.border = Stroke {
                    color: Some(Color::rgb(0xC0, 0xC0, 0xC0)),
                    line_type: LineType::Dashed,
                    width: 1.0,
                };
                self.add_node(index, &graphics, None)
            }
        }
    }

    /// Adds a node for the vertex at `index`, keeping its link, tooltip and
    /// round rectangle radius. Returns its id.
    fn add_node(&mut self, index: usize, graphics: &NodeGraphics, radius: Option<f64>) -> String {
        let custom_radius = radius
            .filter(|&radius| radius > 0.0 && radius != drwr::YED_CORNER_RADIUS)
            .map(|radius| (drwr::ensure_corner_radius_key(&mut self.doc), radius));
        let id = self.push_node(graphics);
        let node = self.doc.graphs[0].nodes.last_mut().expect("just added");
        let cell = &self.cells[index];
        add_properties(&self.doc.keys, KeyDomain::Node, &mut node.data, cell);
        if let Some((key, radius)) = custom_radius {
            drwr::store_corner_radius(node, &key, radius);
        }
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
}

/// Graphics of `kind` in `bounds`, filled and outlined as `style` says.
fn graphics(
    kind: NodeKind,
    bounds: Rect,
    style: &Style,
    label: Option<yed::Label>,
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

/// A line drawn as a box `line` filled with the line's color.
fn line_graphics(line: Rect, style: &Style, label: Option<yed::Label>) -> NodeGraphics {
    let kind = NodeKind::Shape(ShapeType::Rectangle);
    let mut graphics = graphics(kind, line, style, label);
    graphics.fill = Fill {
        color: stroke(style).color,
        gradient: None,
    };
    graphics.border.color = None;
    graphics
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

fn center(rect: Rect) -> Point {
    Point {
        x: rect.x + rect.width / 2.0,
        y: rect.y + rect.height / 2.0,
    }
}

/// `point` turned `degrees` clockwise around `center`.
fn turn(point: Point, center: Point, degrees: f64) -> Point {
    let (sin, cos) = match degrees.rem_euclid(360.0) {
        90.0 => (1.0, 0.0),
        180.0 => (0.0, -1.0),
        270.0 => (-1.0, 0.0),
        d => d.to_radians().sin_cos(),
    };
    let (x, y) = (point.x - center.x, point.y - center.y);
    Point {
        x: center.x + x * cos - y * sin,
        y: center.y + x * sin + y * cos,
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
    fade(
        color,
        style.number("opacity", 100.0) * style.number(key, 100.0) / 1e4,
    )
}

/// `color` with its alpha scaled by `fraction`.
fn fade(color: Color, fraction: f64) -> Color {
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

/// Whether draw.io draws a yEd shape turned some way yEd can't.
fn turned(style: &Style, kind: &ShapeType) -> bool {
    let direction = style.get("direction").unwrap_or("east");
    let upright = matches!(direction, "north" | "south");
    match kind {
        // yEd's triangle points up; draw.io's points right unless turned.
        ShapeType::Triangle => direction != "north" || style.flag("flipV"),
        // Turned half round it's the same, while either flip mirrors it.
        ShapeType::Parallelogram => upright || style.flag("flipH") != style.flag("flipV"),
        ShapeType::Hexagon | ShapeType::Trapezoid | ShapeType::Trapezoid2 => upright,
        _ => false,
    }
}

/// Keeps a draw.io link and tooltip as yEd's `url` and `description` data.
fn add_properties(keys: &[Key], domain: KeyDomain, data: &mut Vec<Data>, cell: &Cell) {
    for (property, name) in [("link", "url"), ("tooltip", "description")] {
        let Some(value) = cell.properties.get(property).filter(|v| !v.is_empty()) else {
            continue;
        };
        let key = keys
            .iter()
            .find(|key| key.domain == domain && key.attr_name.as_deref() == Some(name));
        if let Some(key) = key {
            data.push(Data::from_text(key.id.clone(), value.clone()));
        }
    }
}

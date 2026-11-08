use std::collections::HashSet;

use graphml::xml::XmlNode;
use graphml::yed::{
    Arrow, Color, EdgeGraphics, Keys, Label, LabelPosition, NodeGraphics, NodeKind, Point, Rect,
    Route, ShapeType, Stroke,
};
use graphml::{AttrType, Data, Edge, Graph, GraphMl, Key, KeyDomain, Node};

mod clipboard;

pub use clipboard::{copy, paste};

/// The corner radius yEd draws every round rectangle with.
const YED_CORNER_RADIUS: f64 = 4.0;
/// The node attribute other corner radii are kept in. yEd keeps it, but
/// draws its own radius.
const CORNER_RADIUS: &str = "drwr.cornerRadius";

/// A node of the top-level graph, by its id, or an edge, by its index.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Item {
    Node(String),
    Edge(usize),
}

/// A label of a top-level node or edge. The index one past its last label
/// stands for a new label.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LabelRef {
    pub item: Item,
    pub index: usize,
}

/// An end of an edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EdgeEnd {
    Source,
    Target,
}

/// The graphics of a node or an edge.
#[derive(Clone, Debug, PartialEq)]
pub enum Graphics {
    Node(NodeGraphics),
    Edge(EdgeGraphics),
}

impl Graphics {
    pub fn labels(&self) -> &[Label] {
        match self {
            Graphics::Node(graphics) => &graphics.labels,
            Graphics::Edge(graphics) => &graphics.labels,
        }
    }

    pub fn labels_mut(&mut self) -> &mut Vec<Label> {
        match self {
            Graphics::Node(graphics) => &mut graphics.labels,
            Graphics::Edge(graphics) => &mut graphics.labels,
        }
    }

    /// A node's border, or an edge's line.
    pub fn stroke(&self) -> &Stroke {
        match self {
            Graphics::Node(graphics) => &graphics.border,
            Graphics::Edge(graphics) => &graphics.line,
        }
    }

    pub fn stroke_mut(&mut self) -> &mut Stroke {
        match self {
            Graphics::Node(graphics) => &mut graphics.border,
            Graphics::Edge(graphics) => &mut graphics.line,
        }
    }
}

/// A color of a node's or an edge's graphics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorProperty {
    Fill,
    /// The color the fill fades to.
    Gradient,
    Border,
    /// An edge's line.
    Line,
    /// The text color of the label at this index.
    Text(usize),
}

impl ColorProperty {
    /// The colors of the node itself, in the side panel's order.
    pub const NODE: [ColorProperty; 3] = [
        ColorProperty::Fill,
        ColorProperty::Gradient,
        ColorProperty::Border,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ColorProperty::Fill => "Fill",
            ColorProperty::Gradient => "Gradient",
            ColorProperty::Border => "Border",
            ColorProperty::Line => "Line",
            ColorProperty::Text(_) => "Text",
        }
    }

    /// Whether there can be none of this color.
    pub fn optional(self) -> bool {
        !matches!(self, ColorProperty::Text(_))
    }

    /// `None` if the graphics have no such color.
    pub fn get(self, graphics: &Graphics) -> Option<Color> {
        match (self, graphics) {
            (ColorProperty::Fill, Graphics::Node(node)) => node.fill.color,
            (ColorProperty::Gradient, Graphics::Node(node)) => node.fill.gradient,
            (ColorProperty::Border | ColorProperty::Line, _) => graphics.stroke().color,
            (ColorProperty::Text(index), _) => {
                graphics.labels().get(index).map(|label| label.text_color)
            }
            (ColorProperty::Fill | ColorProperty::Gradient, Graphics::Edge(_)) => None,
        }
    }

    /// Sets the color, or removes it if it's `None` and [`Self::optional`].
    pub fn set(self, graphics: &mut Graphics, color: Option<Color>) {
        let property = match (self, &mut *graphics) {
            (ColorProperty::Fill, Graphics::Node(node)) => &mut node.fill.color,
            (ColorProperty::Gradient, Graphics::Node(node)) => &mut node.fill.gradient,
            (ColorProperty::Border | ColorProperty::Line, _) => &mut graphics.stroke_mut().color,
            (ColorProperty::Text(index), _) => {
                if let Some((label, color)) = graphics.labels_mut().get_mut(index).zip(color) {
                    label.text_color = color;
                }
                return;
            }
            (ColorProperty::Fill | ColorProperty::Gradient, Graphics::Edge(_)) => return,
        };
        *property = color;
    }
}

/// Adds a node to the top-level graph, adding the graph and keys if missing.
/// Returns the node's id.
pub fn add_node(doc: &mut GraphMl, graphics: &NodeGraphics) -> String {
    let keys = Keys::ensure(doc);
    let id = unused_node_id(doc);
    let mut node = Node::new(id.clone());
    keys.set_node_graphics(&mut node, graphics);
    top_graph_mut(doc).nodes.push(node);
    id
}

/// The top-level graph, added if missing.
fn top_graph_mut(doc: &mut GraphMl) -> &mut Graph {
    if doc.graphs.is_empty() {
        doc.graphs.push(Graph {
            id: Some("G".to_owned()),
            ..Graph::default()
        });
    }
    &mut doc.graphs[0]
}

/// The readable graphics of the top-level graph's nodes, with their ids and,
/// for boxes, their [`corner_radius`].
pub fn node_graphics(doc: &GraphMl) -> impl Iterator<Item = (&str, NodeGraphics, Option<f64>)> {
    let keys = Keys::find(doc);
    let radius_key = radius_key(doc);
    doc.graphs
        .first()
        .into_iter()
        .flat_map(|graph| &graph.nodes)
        .filter_map(move |node| {
            let graphics = keys.node_graphics(node).ok().flatten()?;
            let radius = box_corner_radius(&graphics, node, radius_key);
            Some((node.id.as_str(), graphics, radius))
        })
}

/// The readable graphics of the top-level graph's node with id `id`.
pub fn graphics_of(doc: &GraphMl, id: &str) -> Option<NodeGraphics> {
    Keys::find(doc).node_graphics(node(doc, id)?).ok().flatten()
}

/// Edits the readable graphics of the top-level graph's node with id `id`.
pub fn edit_graphics(doc: &mut GraphMl, id: &str, edit: impl FnOnce(&mut NodeGraphics)) {
    let keys = Keys::find(doc);
    let Some(node) = node_mut(doc, id) else {
        return;
    };
    if let Ok(Some(mut graphics)) = keys.node_graphics(node) {
        edit(&mut graphics);
        keys.set_node_graphics(node, &graphics);
    }
}

/// The radius the corners of the top-level box with id `id` are rounded to;
/// `None` if there's no such box. Boxes are yEd's rectangles, which have
/// square corners, and its round rectangles.
pub fn corner_radius(doc: &GraphMl, id: &str) -> Option<f64> {
    corner_radius_of(doc, id, &graphics_of(doc, id)?)
}

/// Like [`corner_radius`], with the box's graphics, `graphics`, read already.
pub fn corner_radius_of(doc: &GraphMl, id: &str, graphics: &NodeGraphics) -> Option<f64> {
    box_corner_radius(graphics, node(doc, id)?, radius_key(doc))
}

/// Rounds the corners of the top-level box with id `id` to `radius`, making
/// it yEd's round rectangle, or its rectangle for square corners. A radius
/// other than yEd's is kept in a node attribute.
pub fn set_corner_radius(doc: &mut GraphMl, id: &str, radius: f64) {
    if corner_radius(doc, id).is_none_or(|old| old == radius) {
        return;
    }
    let round = radius > 0.0;
    let shape = if round {
        ShapeType::RoundRectangle
    } else {
        ShapeType::Rectangle
    };
    edit_graphics(doc, id, |graphics| graphics.kind = NodeKind::Shape(shape));
    if round && radius != YED_CORNER_RADIUS {
        keep_corner_radius(doc, id, radius);
    } else {
        drop_corner_radius(doc, id);
    }
}

/// The corner radius of `node`, whose graphics are `graphics`, if it's a box.
/// `key` is the one corner radii are kept under.
fn box_corner_radius(graphics: &NodeGraphics, node: &Node, key: Option<&str>) -> Option<f64> {
    match graphics.kind {
        NodeKind::Shape(ShapeType::Rectangle) => Some(0.0),
        NodeKind::Shape(ShapeType::RoundRectangle) => {
            let kept = (node.data.iter())
                .find(|data| Some(data.key.as_str()) == key)
                .and_then(|data| data.text().trim().parse::<f64>().ok())
                .filter(|radius| radius.is_finite() && *radius >= 0.0);
            Some(kept.unwrap_or(YED_CORNER_RADIUS))
        }
        _ => None,
    }
}

/// Keeps `radius` as the corner radius of the top-level node with id `id`.
fn keep_corner_radius(doc: &mut GraphMl, id: &str, radius: f64) {
    let key = ensure_radius_key(doc);
    let Some(node) = node_mut(doc, id) else {
        return;
    };
    // As yEd writes doubles, e.g. 10.0.
    let text = format!("{radius:?}");
    match node.data.iter_mut().find(|data| data.key == key) {
        Some(data) => data.content = vec![XmlNode::Text(text)],
        None => node.data.push(Data::from_text(key, text)),
    }
}

/// Drops the corner radius kept for the top-level node with id `id`, and the
/// key too once no node keeps one.
fn drop_corner_radius(doc: &mut GraphMl, id: &str) {
    let Some(key) = radius_key(doc).map(str::to_owned) else {
        return;
    };
    if let Some(node) = node_mut(doc, id) {
        node.data.retain(|data| data.key != key);
    }
    let mut used = false;
    for graph in &doc.graphs {
        visit_nodes(graph, &mut |node| {
            used |= node.data.iter().any(|data| data.key == key);
        });
    }
    if !used {
        doc.keys.retain(|declared| declared.id != key);
    }
}

/// The id of the key corner radii are kept under, if the document has it.
fn radius_key(doc: &GraphMl) -> Option<&str> {
    let key = doc.keys.iter().find(|key| {
        key.domain == KeyDomain::Node && key.attr_name.as_deref() == Some(CORNER_RADIUS)
    });
    key.map(|key| key.id.as_str())
}

/// Like [`radius_key`], but declares the key first if it's missing.
fn ensure_radius_key(doc: &mut GraphMl) -> String {
    if let Some(id) = radius_key(doc) {
        return id.to_owned();
    }
    let id = unused_key_id(doc);
    doc.keys.push(Key {
        id: id.clone(),
        domain: KeyDomain::Node,
        attr_name: Some(CORNER_RADIUS.to_owned()),
        attr_type: Some(AttrType::Double),
        ..Key::default()
    });
    id
}

/// The first of yEd's key ids d0, d1, … that no key in the document has.
fn unused_key_id(doc: &GraphMl) -> String {
    (0..)
        .map(|n| format!("d{n}"))
        .find(|id| doc.keys.iter().all(|key| key.id != *id))
        .expect("the ids are endless")
}

/// The top-level graph's node with id `id`.
fn node<'a>(doc: &'a GraphMl, id: &str) -> Option<&'a Node> {
    doc.graphs.first()?.nodes.iter().find(|node| node.id == id)
}

fn node_mut<'a>(doc: &'a mut GraphMl, id: &str) -> Option<&'a mut Node> {
    let nodes = &mut doc.graphs.first_mut()?.nodes;
    nodes.iter_mut().find(|node| node.id == id)
}

/// The readable graphics of `item`.
pub fn graphics(doc: &GraphMl, item: &Item) -> Option<Graphics> {
    match item {
        Item::Node(id) => graphics_of(doc, id).map(Graphics::Node),
        Item::Edge(index) => edge_graphics_of(doc, *index).map(Graphics::Edge),
    }
}

/// Edits the readable graphics of `item`.
pub fn edit(doc: &mut GraphMl, item: &Item, edit: impl FnOnce(&mut Graphics)) {
    match item {
        Item::Node(id) => edit_graphics(doc, id, |node| {
            let mut graphics = Graphics::Node(node.clone());
            edit(&mut graphics);
            if let Graphics::Node(edited) = graphics {
                *node = edited;
            }
        }),
        Item::Edge(index) => edit_edge_graphics(doc, *index, |edge| {
            let mut graphics = Graphics::Edge(edge.clone());
            edit(&mut graphics);
            if let Graphics::Edge(edited) = graphics {
                *edge = edited;
            }
        }),
    }
}

/// A label's text: empty for a new label, `None` if there's no such label.
pub fn label_text(doc: &GraphMl, label: &LabelRef) -> Option<String> {
    let graphics = graphics(doc, &label.item)?;
    let labels = graphics.labels();
    if label.index == labels.len() {
        return Some(String::new());
    }
    labels.get(label.index).map(|label| label.text.clone())
}

/// Sets a label's text. A new label is added with yEd's defaults, unless the
/// text is empty.
pub fn set_label(doc: &mut GraphMl, label: &LabelRef, text: &str) {
    if label_text(doc, label).is_none_or(|old| old == text) {
        return;
    }
    let new = match label.item {
        Item::Node(_) => Label::new(text),
        Item::Edge(_) => Label::new_on_edge(text),
    };
    edit(doc, &label.item, |graphics| {
        let labels = graphics.labels_mut();
        match labels.get_mut(label.index) {
            Some(existing) => existing.text = text.to_owned(),
            None => labels.push(new),
        }
    });
}

/// Edits a label of a top-level node or edge, if it has that label.
pub fn edit_label(doc: &mut GraphMl, label: &LabelRef, edit_label: impl FnOnce(&mut Label)) {
    edit(doc, &label.item, |graphics| {
        if let Some(label) = graphics.labels_mut().get_mut(label.index) {
            edit_label(label);
        }
    });
}

/// Puts a label of a top-level node at `position`, keeping its size.
pub fn set_label_position(doc: &mut GraphMl, label: &LabelRef, position: LabelPosition) {
    let Item::Node(id) = &label.item else {
        return;
    };
    edit_graphics(doc, id, |graphics| {
        let node = graphics.geometry;
        if let Some(label) = graphics.labels.get_mut(label.index) {
            label.set_position(position);
            let Rect { width, height, .. } = label.bounds;
            if let Some(bounds) = label.placed(&node, width, height) {
                label.bounds = bounds;
            }
        }
    });
}

/// Moves a node's bounds to `geometry`. Labels in its middle stay there, and
/// others keep their distance from the side they're nearest to.
pub fn resize(graphics: &mut NodeGraphics, geometry: Rect) {
    let old = std::mem::replace(&mut graphics.geometry, geometry);
    for label in &mut graphics.labels {
        let bounds = &mut label.bounds;
        bounds.x += label_shift(bounds.x + bounds.width / 2.0, old.width, geometry.width);
        bounds.y += label_shift(bounds.y + bounds.height / 2.0, old.height, geometry.height);
    }
}

/// How far a label centered at `center` moves along an axis on which its node
/// changes size from `old` to `new`.
fn label_shift(center: f64, old: f64, new: f64) -> f64 {
    let middle = old / 2.0;
    // yEd centers labels exactly, and places others a pixel or more off center.
    if (center - middle).abs() < 0.1 {
        (new - old) / 2.0
    } else if center < middle {
        0.0
    } else {
        new - old
    }
}

/// The readable graphics of the top-level graph's edges, with their indexes
/// and the ids of the nodes they join.
pub fn edge_graphics(doc: &GraphMl) -> impl Iterator<Item = (usize, &Edge, EdgeGraphics)> {
    let keys = Keys::find(doc);
    let edges = doc
        .graphs
        .first()
        .into_iter()
        .flat_map(|graph| &graph.edges);
    edges.enumerate().filter_map(move |(index, edge)| {
        let graphics = keys.edge_graphics(edge).ok().flatten()?;
        Some((index, edge, graphics))
    })
}

/// The readable graphics of the top-level graph's edge at `index`.
pub fn edge_graphics_of(doc: &GraphMl, index: usize) -> Option<EdgeGraphics> {
    let edge = doc.graphs.first()?.edges.get(index)?;
    Keys::find(doc).edge_graphics(edge).ok().flatten()
}

/// Edits the readable graphics of the top-level graph's edge at `index`.
pub fn edit_edge_graphics(doc: &mut GraphMl, index: usize, edit: impl FnOnce(&mut EdgeGraphics)) {
    let keys = Keys::find(doc);
    let Some(edge) = doc
        .graphs
        .first_mut()
        .and_then(|graph| graph.edges.get_mut(index))
    else {
        return;
    };
    if let Ok(Some(mut graphics)) = keys.edge_graphics(edge) {
        edit(&mut graphics);
        keys.set_edge_graphics(edge, &graphics);
    }
}

/// Adds an edge from the node with id `source` to the one with id `target`
/// to the top-level graph, adding the graph and keys if missing. Returns the
/// edge's index.
pub fn add_edge(doc: &mut GraphMl, source: &str, target: &str, graphics: &EdgeGraphics) -> usize {
    let keys = Keys::ensure(doc);
    let mut edge = Edge::new(source, target);
    edge.id = Some(unused_edge_id(doc));
    keys.set_edge_graphics(&mut edge, graphics);
    let edges = &mut top_graph_mut(doc).edges;
    edges.push(edge);
    edges.len() - 1
}

/// Removes the top-level graph's edge at `index`.
pub fn remove_edge(doc: &mut GraphMl, index: usize) {
    if let Some(graph) = doc.graphs.first_mut()
        && index < graph.edges.len()
    {
        graph.edges.remove(index);
    }
}

/// Removes the top-level nodes and edges `items`, nodes with their edges.
pub fn remove_items(doc: &mut GraphMl, items: &[Item]) {
    let mut edges: Vec<usize> = (items.iter())
        .filter_map(|item| match item {
            Item::Edge(index) => Some(*index),
            Item::Node(_) => None,
        })
        .collect();
    edges.sort_unstable();
    edges.dedup();
    // Last first, as each removal moves the edges after it down.
    for index in edges.into_iter().rev() {
        remove_edge(doc, index);
    }
    for item in items {
        if let Item::Node(id) = item {
            remove_node(doc, id);
        }
    }
}

/// Removes the top-level node with id `id`, with the edges of it and of the
/// nodes nested in it.
pub fn remove_node(doc: &mut GraphMl, id: &str) {
    let Some(graph) = doc.graphs.first_mut() else {
        return;
    };
    let Some(index) = graph.nodes.iter().position(|node| node.id == id) else {
        return;
    };
    let node = graph.nodes.remove(index);
    let mut gone = HashSet::from([node.id.clone()]);
    if let Some(nested) = &node.graph {
        visit_nodes(nested, &mut |node| {
            gone.insert(node.id.clone());
        });
    }
    for graph in &mut doc.graphs {
        remove_edges(graph, &gone);
    }
}

/// Removes the edges of `graph` and of the graphs nested in it that join any
/// of the nodes with ids in `nodes`.
fn remove_edges(graph: &mut Graph, nodes: &HashSet<String>) {
    graph
        .edges
        .retain(|edge| !nodes.contains(&edge.source) && !nodes.contains(&edge.target));
    let nested = (graph.nodes.iter_mut().map(|node| &mut node.graph))
        .chain(graph.edges.iter_mut().map(|edge| &mut edge.graph));
    for graph in nested.flatten() {
        remove_edges(graph, nodes);
    }
}

/// Moves the port at the `end` of the top-level graph's edge at `index` onto
/// the node with id `node`, `port` from its middle.
pub fn move_port(doc: &mut GraphMl, index: usize, end: EdgeEnd, node: &str, port: Point) {
    let Some(edge) = doc
        .graphs
        .first_mut()
        .and_then(|graph| graph.edges.get_mut(index))
    else {
        return;
    };
    let joined = match end {
        EdgeEnd::Source => &mut edge.source,
        EdgeEnd::Target => &mut edge.target,
    };
    if joined != node {
        *joined = node.to_owned();
    }
    edit_edge_graphics(doc, index, |graphics| match end {
        EdgeEnd::Source => graphics.path.source = port,
        EdgeEnd::Target => graphics.path.target = port,
    });
}

/// Sets whether the `end` of the top-level graph's edge at `index` has an
/// arrowhead, keeping the kind it has.
pub fn set_arrow(doc: &mut GraphMl, index: usize, end: EdgeEnd, arrow: bool) {
    edit_edge_graphics(doc, index, |graphics| {
        let current = match end {
            EdgeEnd::Source => &mut graphics.arrows.source,
            EdgeEnd::Target => &mut graphics.arrows.target,
        };
        if arrow != (*current != Arrow::None) {
            *current = if arrow { Arrow::Standard } else { Arrow::None };
        }
    });
}

/// Puts the labels of an edge, which ran along `old`, where they were on its
/// new route `new`.
pub fn keep_labels(graphics: &mut EdgeGraphics, old: &Route, new: &Route) {
    for label in &mut graphics.labels {
        let Rect { width, height, .. } = label.bounds;
        if let Some(placed) = label.placed_on(old, width, height) {
            let center = Point {
                x: placed.x + width / 2.0,
                y: placed.y + height / 2.0,
            };
            label.place_on(new, center, width, height);
        }
    }
}

/// Removes the bend at index `bend` from the top-level graph's edge at
/// `index`, keeping its labels on their segments.
pub fn remove_bend(doc: &mut GraphMl, index: usize, bend: usize) {
    edit_edge_graphics(doc, index, |graphics| {
        let points = &mut graphics.path.points;
        if bend < points.len() {
            let segments = points.len() + 1;
            points.remove(bend);
            for label in &mut graphics.labels {
                label.remove_bend(bend, segments);
            }
        }
    });
}

/// Moves the bend at index `bend` of the top-level graph's edge at `index`,
/// or all of its bends if `bend` is `None`, right and down by these
/// distances.
pub fn move_bends(doc: &mut GraphMl, index: usize, bend: Option<usize>, dx: f64, dy: f64) {
    edit_edge_graphics(doc, index, |graphics| {
        for (i, point) in graphics.path.points.iter_mut().enumerate() {
            if bend.is_none_or(|bend| bend == i) {
                point.x += dx;
                point.y += dy;
            }
        }
    });
}

/// Moves the top-level nodes and edges `items` right and down by these
/// distances: nodes with their labels, edges by their bends. The bends of
/// edges joining two of the nodes move too, so those edges keep their shape.
pub fn move_items(doc: &mut GraphMl, items: &[Item], dx: f64, dy: f64) {
    let edges = bends_moved_with(doc, items);
    for item in items {
        if let Item::Node(id) = item {
            edit_graphics(doc, id, |graphics| {
                graphics.geometry.x += dx;
                graphics.geometry.y += dy;
            });
        }
    }
    for index in edges {
        move_bends(doc, index, None, dx, dy);
    }
}

/// The indexes of the top-level edges whose bends [`move_items`] moves with
/// `items`: those in `items`, and those joining two nodes in them.
pub fn bends_moved_with(doc: &GraphMl, items: &[Item]) -> HashSet<usize> {
    let nodes: HashSet<&str> = (items.iter())
        .filter_map(|item| match item {
            Item::Node(id) => Some(id.as_str()),
            Item::Edge(_) => None,
        })
        .collect();
    let edges = doc
        .graphs
        .first()
        .into_iter()
        .flat_map(|graph| &graph.edges);
    (edges.enumerate())
        .filter(|(index, edge)| {
            items.contains(&Item::Edge(*index))
                || nodes.contains(edge.source.as_str()) && nodes.contains(edge.target.as_str())
        })
        .map(|(index, _)| index)
        .collect()
}

/// The first of yEd's ids e0, e1, … that no edge in the document has.
fn unused_edge_id(doc: &GraphMl) -> String {
    fn visit<'a>(graph: &'a Graph, ids: &mut HashSet<&'a str>) {
        ids.extend(graph.edges.iter().filter_map(|edge| edge.id.as_deref()));
        let nested = (graph.nodes.iter().map(|node| &node.graph))
            .chain(graph.edges.iter().map(|edge| &edge.graph));
        for graph in nested.flatten() {
            visit(graph, ids);
        }
    }
    let mut ids = HashSet::new();
    for graph in &doc.graphs {
        visit(graph, &mut ids);
    }
    (0..)
        .map(|n| format!("e{n}"))
        .find(|id| !ids.contains(id.as_str()))
        .expect("the ids are endless")
}

/// The first of yEd's ids n0, n1, … that no node in the document has.
fn unused_node_id(doc: &GraphMl) -> String {
    let mut ids = HashSet::new();
    for graph in &doc.graphs {
        visit_nodes(graph, &mut |node| {
            ids.insert(node.id.as_str());
        });
    }
    (0..)
        .map(|n| format!("n{n}"))
        .find(|id| !ids.contains(id.as_str()))
        .expect("the ids are endless")
}

/// Calls `visit` with each node of `graph` and of the graphs nested in it.
fn visit_nodes<'a>(graph: &'a Graph, visit: &mut impl FnMut(&'a Node)) {
    for node in &graph.nodes {
        visit(node);
    }
    let nested = graph
        .nodes
        .iter()
        .map(|node| &node.graph)
        .chain(graph.edges.iter().map(|edge| &edge.graph))
        .chain(graph.hyperedges.iter().map(|hyperedge| &hyperedge.graph));
    for graph in nested.flatten() {
        visit_nodes(graph, visit);
    }
}

#[cfg(test)]
mod tests;

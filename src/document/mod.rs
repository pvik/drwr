use std::collections::HashSet;

use graphml::xml::XmlNode;
use graphml::yed::{Color, Keys, Label, LabelPosition, NodeGraphics, NodeKind, Rect, ShapeType};
use graphml::{AttrType, Data, Graph, GraphMl, Key, KeyDomain, Node};

/// The corner radius yEd draws every round rectangle with.
const YED_CORNER_RADIUS: f64 = 4.0;
/// The node attribute other corner radii are kept in. yEd keeps it, but
/// draws its own radius.
const CORNER_RADIUS: &str = "drwr.cornerRadius";

/// A label of a top-level node. The index one past its last label stands for
/// a new label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LabelRef {
    pub node: String,
    pub index: usize,
}

/// A color of a node's graphics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorProperty {
    Fill,
    /// The color the fill fades to.
    Gradient,
    Border,
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
            ColorProperty::Text(_) => "Text",
        }
    }

    /// Whether the node can have none of this color.
    pub fn optional(self) -> bool {
        !matches!(self, ColorProperty::Text(_))
    }

    /// `None` if the node has no such color.
    pub fn get(self, graphics: &NodeGraphics) -> Option<Color> {
        match self {
            ColorProperty::Fill => graphics.fill.color,
            ColorProperty::Gradient => graphics.fill.gradient,
            ColorProperty::Border => graphics.border.color,
            ColorProperty::Text(index) => graphics.labels.get(index).map(|label| label.text_color),
        }
    }

    /// Sets the color, or removes it if it's `None` and [`Self::optional`].
    pub fn set(self, graphics: &mut NodeGraphics, color: Option<Color>) {
        let property = match self {
            ColorProperty::Fill => &mut graphics.fill.color,
            ColorProperty::Gradient => &mut graphics.fill.gradient,
            ColorProperty::Border => &mut graphics.border.color,
            ColorProperty::Text(index) => {
                if let Some((label, color)) = graphics.labels.get_mut(index).zip(color) {
                    label.text_color = color;
                }
                return;
            }
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
    if doc.graphs.is_empty() {
        doc.graphs.push(Graph {
            id: Some("G".to_owned()),
            ..Graph::default()
        });
    }
    doc.graphs[0].nodes.push(node);
    id
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
    let node = node(doc, id)?;
    let graphics = Keys::find(doc).node_graphics(node).ok().flatten()?;
    box_corner_radius(&graphics, node, radius_key(doc))
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
    // yEd numbers its keys d0, d1, …
    let id = (0..)
        .map(|n| format!("d{n}"))
        .find(|id| doc.keys.iter().all(|key| key.id != *id))
        .expect("the ids are endless");
    doc.keys.push(Key {
        id: id.clone(),
        domain: KeyDomain::Node,
        attr_name: Some(CORNER_RADIUS.to_owned()),
        attr_type: Some(AttrType::Double),
        ..Key::default()
    });
    id
}

/// The top-level graph's node with id `id`.
fn node<'a>(doc: &'a GraphMl, id: &str) -> Option<&'a Node> {
    doc.graphs.first()?.nodes.iter().find(|node| node.id == id)
}

fn node_mut<'a>(doc: &'a mut GraphMl, id: &str) -> Option<&'a mut Node> {
    let nodes = &mut doc.graphs.first_mut()?.nodes;
    nodes.iter_mut().find(|node| node.id == id)
}

/// A label's text: empty for a new label, `None` if there's no such label.
pub fn label_text(doc: &GraphMl, label: &LabelRef) -> Option<String> {
    let labels = graphics_of(doc, &label.node)?.labels;
    if label.index == labels.len() {
        return Some(String::new());
    }
    labels.into_iter().nth(label.index).map(|label| label.text)
}

/// Sets a label's text. A new label is added with yEd's defaults, unless the
/// text is empty.
pub fn set_label(doc: &mut GraphMl, label: &LabelRef, text: &str) {
    if label_text(doc, label).is_none_or(|old| old == text) {
        return;
    }
    edit_graphics(doc, &label.node, |graphics| {
        match graphics.labels.get_mut(label.index) {
            Some(existing) => existing.text = text.to_owned(),
            None => graphics.labels.push(Label::new(text)),
        }
    });
}

/// Edits a label of a top-level node, if the node has that label.
pub fn edit_label(doc: &mut GraphMl, label: &LabelRef, edit: impl FnOnce(&mut Label)) {
    edit_graphics(doc, &label.node, |graphics| {
        if let Some(label) = graphics.labels.get_mut(label.index) {
            edit(label);
        }
    });
}

/// Puts a label of a top-level node at `position`, keeping its size.
pub fn set_label_position(doc: &mut GraphMl, label: &LabelRef, position: LabelPosition) {
    edit_graphics(doc, &label.node, |graphics| {
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

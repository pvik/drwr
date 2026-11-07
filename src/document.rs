use std::collections::HashSet;

use graphml::yed::{Color, Keys, Label, NodeGraphics, Rect};
use graphml::{Graph, GraphMl, Node};

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

/// The readable graphics of the top-level graph's nodes, with their ids.
pub fn node_graphics(doc: &GraphMl) -> impl Iterator<Item = (&str, NodeGraphics)> {
    let keys = Keys::find(doc);
    doc.graphs
        .first()
        .into_iter()
        .flat_map(|graph| &graph.nodes)
        .filter_map(move |node| {
            let graphics = keys.node_graphics(node).ok().flatten()?;
            Some((node.id.as_str(), graphics))
        })
}

/// The readable graphics of the top-level graph's node with id `id`.
pub fn graphics_of(doc: &GraphMl, id: &str) -> Option<NodeGraphics> {
    let nodes = &doc.graphs.first()?.nodes;
    let node = nodes.iter().find(|node| node.id == id)?;
    Keys::find(doc).node_graphics(node).ok().flatten()
}

/// Edits the readable graphics of the top-level graph's node with id `id`.
pub fn edit_graphics(doc: &mut GraphMl, id: &str, edit: impl FnOnce(&mut NodeGraphics)) {
    let keys = Keys::find(doc);
    let Some(node) = doc
        .graphs
        .first_mut()
        .and_then(|graph| graph.nodes.iter_mut().find(|node| node.id == id))
    else {
        return;
    };
    if let Ok(Some(mut graphics)) = keys.node_graphics(node) {
        edit(&mut graphics);
        keys.set_node_graphics(node, &graphics);
    }
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
        collect_node_ids(graph, &mut ids);
    }
    (0..)
        .map(|n| format!("n{n}"))
        .find(|id| !ids.contains(id.as_str()))
        .expect("the ids are endless")
}

fn collect_node_ids<'a>(graph: &'a Graph, ids: &mut HashSet<&'a str>) {
    ids.extend(graph.nodes.iter().map(|node| node.id.as_str()));
    let nested = graph
        .nodes
        .iter()
        .map(|node| &node.graph)
        .chain(graph.edges.iter().map(|edge| &edge.graph))
        .chain(graph.hyperedges.iter().map(|hyperedge| &hyperedge.graph));
    for graph in nested.flatten() {
        collect_node_ids(graph, ids);
    }
}

#[cfg(test)]
mod tests;

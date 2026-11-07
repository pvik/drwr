use std::collections::HashSet;

use graphml::yed::{Color, Keys, NodeGraphics};
use graphml::{Graph, GraphMl, Node};

/// A color of a node's graphics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorProperty {
    Fill,
    /// The color the fill fades to.
    Gradient,
    Border,
}

impl ColorProperty {
    pub const ALL: [ColorProperty; 3] = [
        ColorProperty::Fill,
        ColorProperty::Gradient,
        ColorProperty::Border,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ColorProperty::Fill => "Fill",
            ColorProperty::Gradient => "Gradient",
            ColorProperty::Border => "Border",
        }
    }

    /// `None` if the node has no such color.
    pub fn get(self, graphics: &NodeGraphics) -> Option<Color> {
        match self {
            ColorProperty::Fill => graphics.fill.color,
            ColorProperty::Gradient => graphics.fill.gradient,
            ColorProperty::Border => graphics.border.color,
        }
    }

    pub fn set(self, graphics: &mut NodeGraphics, color: Option<Color>) {
        let property = match self {
            ColorProperty::Fill => &mut graphics.fill.color,
            ColorProperty::Gradient => &mut graphics.fill.gradient,
            ColorProperty::Border => &mut graphics.border.color,
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

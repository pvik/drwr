use std::collections::HashSet;

use graphml::yed::{Keys, NodeGraphics};
use graphml::{Graph, GraphMl, Node};

/// Adds a node to the top-level graph, adding the graph and keys if missing.
pub fn add_node(doc: &mut GraphMl, graphics: &NodeGraphics) {
    let keys = Keys::ensure(doc);
    let mut node = Node::new(unused_node_id(doc));
    keys.set_node_graphics(&mut node, graphics);
    if doc.graphs.is_empty() {
        doc.graphs.push(Graph {
            id: Some("G".to_owned()),
            ..Graph::default()
        });
    }
    doc.graphs[0].nodes.push(node);
}

/// The readable graphics of the top-level graph's nodes.
pub fn node_graphics(doc: &GraphMl) -> impl Iterator<Item = NodeGraphics> {
    let keys = Keys::find(doc);
    doc.graphs
        .first()
        .into_iter()
        .flat_map(|graph| &graph.nodes)
        .filter_map(move |node| keys.node_graphics(node).ok().flatten())
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

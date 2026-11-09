//! Copies of top-level nodes and edges as documents of their own, which go on
//! the clipboard as GraphML, and pasting them back into a document.

use std::collections::{HashMap, HashSet};

use graphml::xml::{Element, XmlNode};
use graphml::yed::{self, Keys};
use graphml::{Edge, Graph, GraphMl, Key};

use super::{
    Item, fresh, node_graphics, resource_refs, taken_ids, top_graph_mut, unused_key_id, visit_data,
    visit_elements, visit_graphs_mut, visit_nodes,
};

/// How far right and down a paste goes for each copy already in its place.
const PASTE_STEP: f64 = 10.0;

/// The top-level nodes in `items`, with the edges in `items` that join two of
/// them, as a document of their own; `None` if `items` has no nodes.
pub fn copy(doc: &GraphMl, items: &[Item]) -> Option<GraphMl> {
    let graph = doc.graphs.first()?;
    let nodes: HashSet<&str> = items.iter().filter_map(Item::node_id).collect();
    let edges: HashSet<usize> = items.iter().filter_map(Item::edge_index).collect();
    let mut copied = Graph {
        id: Some("G".to_owned()),
        edge_default: graph.edge_default,
        nodes: (graph.nodes.iter())
            .filter(|node| nodes.contains(node.id.as_str()))
            .cloned()
            .collect(),
        ..Graph::default()
    };
    if copied.nodes.is_empty() {
        return None;
    }
    let ids = node_ids(&copied);
    copied.edges = (graph.edges.iter().enumerate())
        .filter(|(index, edge)| edges.contains(index) && joins(edge, &ids))
        .map(|(_, edge)| edge.clone())
        .collect();
    let used = resource_refs(&mut copied);
    let mut clip = GraphMl {
        keys: doc.keys.clone(),
        graphs: vec![copied],
        attributes: doc.attributes.clone(),
        prefix: doc.prefix.clone(),
        ..GraphMl::default()
    };
    if !used.is_empty()
        && let Some(resources) = yed::resources(doc)
    {
        let mut resources = resources.clone();
        resources.children.retain(|resource| {
            let id = resource
                .as_element()
                .and_then(|resource| resource.attr("id"));
            id.is_some_and(|id| used.contains(id))
        });
        *yed::resources_mut(&mut clip) = resources;
    }
    Some(clip)
}

/// Adds the top-level nodes of `copied`, a document like [`copy`] makes, with
/// the edges joining them, to the top-level graph of `doc` under new ids.
/// They go where they were, moved right and down past any copies of them
/// already there. Returns the items added.
pub fn paste(doc: &mut GraphMl, mut copied: GraphMl) -> Vec<Item> {
    let Some(graph) = copied.graphs.first_mut() else {
        return Vec::new();
    };
    let ids = node_ids(graph);
    graph.edges.retain(|edge| joins(edge, &ids));
    if graph.nodes.is_empty() {
        return Vec::new();
    }
    let offset = f64::from(steps(doc, &copied)) * PASTE_STEP;
    if offset > 0.0 {
        shift(&mut copied, offset);
    }
    let graph = &mut copied.graphs[0];
    let mut pasted = Graph {
        nodes: std::mem::take(&mut graph.nodes),
        edges: std::mem::take(&mut graph.edges),
        ..Graph::default()
    };
    merge_keys(doc, &copied.keys, &mut pasted);
    merge_resources(doc, &copied, &mut pasted);
    rename(doc, &mut pasted);
    // For the prefixes in the copied data.
    for (name, value) in copied.attributes {
        if name == "xmlns" || name.starts_with("xmlns:") {
            doc.attributes.entry(name).or_insert(value);
        }
    }
    let graph = top_graph_mut(doc);
    let nodes = pasted.nodes.iter().map(|node| Item::Node(node.id.clone()));
    let edges = (graph.edges.len()..)
        .take(pasted.edges.len())
        .map(Item::Edge);
    let items = nodes.chain(edges).collect();
    graph.nodes.append(&mut pasted.nodes);
    graph.edges.append(&mut pasted.edges);
    items
}

/// How many [`PASTE_STEP`]s right and down the top-level nodes of `copied` go
/// to pass any copies of them in their place: nodes within half a step.
fn steps(doc: &GraphMl, copied: &GraphMl) -> u32 {
    let (taken, pasted) = (corners(doc), corners(copied));
    let near = |a: f64, b: f64| (a - b).abs() < PASTE_STEP / 2.0;
    (0..)
        .find(|&steps| {
            let offset = f64::from(steps) * PASTE_STEP;
            !pasted.iter().any(|&(x, y)| {
                (taken.iter()).any(|&(tx, ty)| near(x + offset, tx) && near(y + offset, ty))
            })
        })
        .expect("each node is in the way of one step at most")
}

/// The top-left corners of the top-level nodes.
fn corners(doc: &GraphMl) -> Vec<(f64, f64)> {
    let corner = |(_, graphics, _): (_, graphml::yed::NodeGraphics, _)| {
        (graphics.geometry.x, graphics.geometry.y)
    };
    node_graphics(doc).map(corner).collect()
}

/// Moves the nodes and bends of `doc` right and down by `offset`.
fn shift(doc: &mut GraphMl, offset: f64) {
    let keys = Keys::find(doc);
    for graph in &mut doc.graphs {
        visit_graphs_mut(graph, &mut |graph| {
            for node in &mut graph.nodes {
                if let Ok(Some(mut graphics)) = keys.node_graphics(node) {
                    graphics.geometry.x += offset;
                    graphics.geometry.y += offset;
                    keys.set_node_graphics(node, &graphics);
                }
            }
            for edge in &mut graph.edges {
                if let Ok(Some(mut graphics)) = keys.edge_graphics(edge) {
                    for point in &mut graphics.path.points {
                        point.x += offset;
                        point.y += offset;
                    }
                    keys.set_edge_graphics(edge, &graphics);
                }
            }
        });
    }
}

/// Points the data of `graph`, whose keys are `keys`, at the same keys in
/// `doc`, declaring those it lacks.
fn merge_keys(doc: &mut GraphMl, keys: &[Key], graph: &mut Graph) {
    let mut used = HashSet::new();
    visit_data(graph, &mut |data| {
        used.insert(data.key.clone());
    });
    let mut ids = HashMap::new();
    for key in keys.iter().filter(|key| used.contains(&key.id)) {
        let id = match doc.keys.iter().find(|declared| same_key(declared, key)) {
            Some(declared) => declared.id.clone(),
            None => {
                let mut key = key.clone();
                if doc.keys.iter().any(|declared| declared.id == key.id) {
                    key.id = unused_key_id(doc);
                }
                let id = key.id.clone();
                doc.keys.push(key);
                id
            }
        };
        ids.insert(key.id.as_str(), id);
    }
    visit_data(graph, &mut |data| {
        if let Some(id) = ids.get(data.key.as_str()) {
            data.key = id.clone();
        }
    });
}

/// Whether two keys declare the same attribute.
fn same_key(a: &Key, b: &Key) -> bool {
    fn yfiles_type(key: &Key) -> Option<&String> {
        key.attributes.get("yfiles.type")
    }
    a.domain == b.domain
        && a.attr_name == b.attr_name
        && a.attr_type == b.attr_type
        && yfiles_type(a) == yfiles_type(b)
}

/// Adds the resources of `copied` that `graph` refers to to `doc`, unless it
/// has them already, and points `graph` at them.
fn merge_resources(doc: &mut GraphMl, copied: &GraphMl, graph: &mut Graph) {
    let used = resource_refs(graph);
    let Some(resources) = yed::resources(copied) else {
        return;
    };
    let mut ids = HashMap::new();
    for resource in resources.elements() {
        let Some(id) = resource.attr("id").filter(|id| used.contains(*id)) else {
            continue;
        };
        let merged = yed::resources_mut(doc);
        let had = merged.elements().find(|had| same_resource(had, resource));
        let new_id = match had.map(|had| had.attr("id").unwrap_or_default().to_owned()) {
            Some(had) => had,
            None => {
                let mut resource = resource.clone();
                if merged.elements().any(|had| had.attr("id") == Some(id)) {
                    resource.set_attr("id", yed::unused_resource_id(merged));
                }
                let new_id = resource.attr("id").unwrap_or_default().to_owned();
                merged.children.push(XmlNode::Element(resource));
                new_id
            }
        };
        ids.insert(id, new_id);
    }
    visit_elements(graph, &mut |element| {
        if let Some(id) = element.attr("refid").and_then(|id| ids.get(id)).cloned() {
            element.set_attr("refid", id);
        }
    });
}

/// Whether two of yEd's resources are the same, whatever their ids.
fn same_resource(a: &Element, b: &Element) -> bool {
    let attributes = |resource: &Element| {
        let mut attributes = resource.attributes.clone();
        attributes.remove("id");
        attributes
    };
    a.children == b.children && attributes(a) == attributes(b)
}

/// Gives the nodes and edges of `graph` new ids, numbered as yEd numbers
/// them, and points the edges at the renamed nodes.
fn rename(doc: &GraphMl, graph: &mut Graph) {
    let mut taken = taken_ids(doc);
    let mut renamed = HashMap::new();
    rename_nodes(graph, "", &mut taken, &mut renamed);
    rename_edges(graph, "", &mut taken, &renamed);
}

fn rename_nodes(
    graph: &mut Graph,
    prefix: &str,
    taken: &mut HashSet<String>,
    renamed: &mut HashMap<String, String>,
) {
    for node in &mut graph.nodes {
        let id = fresh(taken, &format!("{prefix}n"));
        renamed.insert(std::mem::replace(&mut node.id, id.clone()), id.clone());
        if let Some(nested) = &mut node.graph {
            nested.id = Some(format!("{id}:"));
            rename_nodes(nested, &format!("{id}::"), taken, renamed);
        }
    }
}

fn rename_edges(
    graph: &mut Graph,
    prefix: &str,
    taken: &mut HashSet<String>,
    renamed: &HashMap<String, String>,
) {
    for edge in &mut graph.edges {
        edge.id = Some(fresh(taken, &format!("{prefix}e")));
        for end in [&mut edge.source, &mut edge.target] {
            if let Some(id) = renamed.get(end.as_str()) {
                *end = id.clone();
            }
        }
    }
    for node in &mut graph.nodes {
        if let Some(nested) = &mut node.graph {
            rename_edges(nested, &format!("{}::", node.id), taken, renamed);
        }
    }
}

/// The ids of the nodes in `graph`, at any depth.
fn node_ids(graph: &Graph) -> HashSet<String> {
    let mut ids = HashSet::new();
    visit_nodes(graph, &mut |node| {
        ids.insert(node.id.clone());
    });
    ids
}

/// Whether `edge` joins two of the nodes with ids in `ids`.
fn joins(edge: &Edge, ids: &HashSet<String>) -> bool {
    ids.contains(&edge.source) && ids.contains(&edge.target)
}

#[cfg(test)]
mod tests;

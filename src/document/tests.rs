use graphml::yed::{self, NodeKind, ShapeType};

use super::*;

fn rectangle() -> NodeGraphics {
    NodeGraphics::new(NodeKind::Shape(ShapeType::Rectangle))
}

#[test]
fn adding_to_an_empty_document_adds_a_graph_and_keys() {
    let mut doc = GraphMl::default();
    assert_eq!(add_node(&mut doc, &rectangle()), "n0");

    let [graph] = doc.graphs.as_slice() else {
        panic!("expected one graph: {:?}", doc.graphs);
    };
    assert_eq!(graph.id.as_deref(), Some("G"));
    assert_eq!(graph.nodes.len(), 1);
    assert_eq!(graph.nodes[0].id, "n0");
    let read: Vec<_> = node_graphics(&doc).collect();
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].0, "n0");
    assert_eq!(read[0].1.to_element(), rectangle().to_element());
}

#[test]
fn new_nodes_get_an_id_no_node_has() {
    let mut doc = yed::new_document();
    let mut group = Node::new("n1");
    group.graph = Some(Graph {
        nodes: vec![Node::new("n1::n0"), Node::new("n2")],
        ..Graph::default()
    });
    doc.graphs[0].nodes = vec![Node::new("n0"), group];
    let added = [
        add_node(&mut doc, &rectangle()),
        add_node(&mut doc, &rectangle()),
    ];

    assert_eq!(added, ["n3", "n4"]);
    assert_eq!(doc.graphs.len(), 1);
    let ids: Vec<_> = doc.graphs[0].nodes.iter().map(|node| &node.id).collect();
    assert_eq!(ids, ["n0", "n1", "n3", "n4"]);
}

#[test]
fn node_graphics_skips_nodes_without_graphics() {
    let mut doc = yed::new_document();
    doc.graphs[0].nodes.push(Node::new("plain"));
    add_node(&mut doc, &rectangle());
    assert_eq!(node_graphics(&doc).count(), 1);
}

#[test]
fn graphics_of_looks_up_a_top_level_node() {
    let mut doc = yed::new_document();
    let mut wide = rectangle();
    wide.geometry.width = 100.0;
    add_node(&mut doc, &rectangle());
    let id = add_node(&mut doc, &wide);
    let mut group = Node::new("group");
    group.graph = Some(Graph {
        nodes: vec![Node::new("group::n0")],
        ..Graph::default()
    });
    Keys::find(&doc).set_node_graphics(&mut group.graph.as_mut().unwrap().nodes[0], &wide);
    doc.graphs[0].nodes.extend([Node::new("plain"), group]);

    assert_eq!(
        graphics_of(&doc, &id).map(|g| g.geometry),
        Some(wide.geometry)
    );
    assert_eq!(graphics_of(&doc, "plain"), None);
    assert_eq!(graphics_of(&doc, "group::n0"), None);
    assert_eq!(graphics_of(&doc, "missing"), None);
}

use graphml::yed::{self, NodeGraphics, NodeKind, Point, ShapeType};

use super::*;
use crate::document::{add_node, edge_graphics_of, graphics_of, remove_items};

const FIXTURE: &str = include_str!("../../../crates/graphml/tests/fixtures/yed.graphml");

/// n3 is an SVG node with resource 1, n5 a group holding n5::n0, e0 joins n0
/// to n1 and e1 joins n2 to n3.
fn fixture() -> GraphMl {
    GraphMl::parse(FIXTURE).unwrap()
}

fn node(id: &str) -> Item {
    Item::Node(id.to_owned())
}

fn ids(graph: &Graph) -> (Vec<&str>, Vec<&str>) {
    let nodes = graph.nodes.iter().map(|node| node.id.as_str());
    let edges = graph.edges.iter().filter_map(|edge| edge.id.as_deref());
    (nodes.collect(), edges.collect())
}

fn corner(doc: &GraphMl, id: &str) -> (f64, f64) {
    let geometry = graphics_of(doc, id).unwrap().geometry;
    (geometry.x, geometry.y)
}

fn point(x: f64, y: f64) -> Point {
    Point { x, y }
}

/// The ids of the resources that the top-level node with id `id` refers to.
fn refids(doc: &GraphMl, id: &str) -> Vec<String> {
    let node = doc.graphs[0].nodes.iter().find(|node| node.id == id);
    let mut graph = Graph {
        nodes: vec![node.unwrap().clone()],
        ..Graph::default()
    };
    let mut ids = Vec::new();
    visit_elements(&mut graph, &mut |element| {
        ids.extend(element.attr("refid").map(str::to_owned));
    });
    ids
}

fn resource_ids(doc: &GraphMl) -> Vec<&str> {
    let resources = yed::resources(doc).into_iter().flat_map(Element::elements);
    resources
        .filter_map(|resource| resource.attr("id"))
        .collect()
}

#[test]
fn copies_take_the_selected_edges_between_copied_nodes() {
    let doc = fixture();
    let items = [
        node("n1"),
        Item::Edge(0),
        Item::Edge(1),
        node("n0"),
        node("n2"),
    ];
    let copied = copy(&doc, &items).unwrap();
    // In the document's order. e1 leads to n3, which isn't copied.
    assert_eq!(ids(&copied.graphs[0]), (vec!["n0", "n1", "n2"], vec!["e0"]));
    assert_eq!(copied.keys, doc.keys);
    assert_eq!(GraphMl::parse(&copied.to_xml()).unwrap(), copied);

    // Edges between copied nodes stay behind unless selected.
    let copied = copy(&doc, &[node("n0"), node("n1")]).unwrap();
    assert_eq!(copied.graphs[0].edges, []);
    // Edges can't be pasted without their nodes.
    assert_eq!(copy(&doc, &[Item::Edge(0)]), None);
    assert_eq!(copy(&doc, &[]), None);
}

#[test]
fn copies_keep_the_resources_their_nodes_refer_to() {
    let mut doc = fixture();
    let copied = copy(&doc, &[node("n3")]).unwrap();
    assert_eq!(copied.data, doc.data);

    // And only those.
    yed::add_resource(&mut doc, "<svg/>");
    let copied = copy(&doc, &[node("n3")]).unwrap();
    assert_eq!(resource_ids(&copied), ["1"]);
    let copied = copy(&doc, &[node("n0")]).unwrap();
    assert_eq!(copied.data, []);
}

#[test]
fn pastes_go_beside_the_copies_already_in_place() {
    let mut doc = fixture();
    let copied = copy(&doc, &[node("n0"), node("n1"), Item::Edge(0)]).unwrap();
    let pasted = paste(&mut doc, copied.clone());
    assert_eq!(pasted, [node("n6"), node("n7"), Item::Edge(2)]);
    assert_eq!(corner(&doc, "n6"), (110.0, 50.5));
    assert_eq!(corner(&doc, "n7"), (310.0, 55.5));
    let edge = &doc.graphs[0].edges[2];
    assert_eq!(edge.id.as_deref(), Some("e2"));
    assert_eq!((edge.source.as_str(), edge.target.as_str()), ("n6", "n7"));
    let pasted_edge = edge_graphics_of(&doc, 2).unwrap();
    let original = edge_graphics_of(&doc, 0).unwrap();
    assert_eq!(
        pasted_edge.path.points,
        [point(270.0, 30.0), point(335.0, 30.0)]
    );
    // Everything else comes along, labels included.
    assert_eq!(pasted_edge.labels, original.labels);
    let (nodes, edges) = (&doc.graphs[0].nodes, &doc.graphs[0].edges);
    assert_eq!(nodes[6].data[0], nodes[0].data[0]);
    assert_eq!(
        graphics_of(&doc, "n6").unwrap().labels,
        graphics_of(&doc, "n0").unwrap().labels
    );
    assert_eq!(edges[2].data.len(), edges[0].data.len());

    let pasted = paste(&mut doc, copied);
    assert_eq!(pasted, [node("n8"), node("n9"), Item::Edge(3)]);
    assert_eq!(corner(&doc, "n8"), (120.0, 60.5));
    assert_eq!(GraphMl::parse(&doc.to_xml()).unwrap(), doc);

    // Even if only some of them are in place.
    let mut doc = fixture();
    let copied = copy(&doc, &[node("n0"), node("n1")]).unwrap();
    super::super::move_items(&mut doc, &[node("n0")], 50.0, 0.0);
    paste(&mut doc, copied);
    assert_eq!(corner(&doc, "n6"), (110.0, 50.5));
    assert_eq!(corner(&doc, "n7"), (310.0, 55.5));
}

#[test]
fn pastes_go_back_in_place_once_the_originals_are_gone() {
    let mut doc = fixture();
    let items = [node("n0"), node("n1"), Item::Edge(0)];
    let copied = copy(&doc, &items).unwrap();
    remove_items(&mut doc, &items);
    let pasted = paste(&mut doc, copied);
    assert_eq!(pasted, [node("n0"), node("n1"), Item::Edge(1)]);
    assert_eq!(corner(&doc, "n0"), (100.0, 40.5));
    let bends = edge_graphics_of(&doc, 1).unwrap().path.points;
    assert_eq!(bends, [point(260.0, 20.0), point(325.0, 20.0)]);
}

#[test]
fn nodes_nudged_less_than_half_a_step_count_as_in_place() {
    let mut doc = fixture();
    let copied = copy(&doc, &[node("n2")]).unwrap();
    super::super::move_items(&mut doc, &[node("n2")], 4.0, -4.0);
    paste(&mut doc, copied.clone());
    assert_eq!(corner(&doc, "n6"), (110.0, 210.0));

    let mut doc = fixture();
    super::super::move_items(&mut doc, &[node("n2")], 5.0, 0.0);
    paste(&mut doc, copied);
    assert_eq!(corner(&doc, "n6"), (100.0, 200.0));
}

#[test]
fn pastes_into_another_document_bring_their_keys_and_resources() {
    let copied = copy(&fixture(), &[node("n3"), node("n2"), Item::Edge(1)]).unwrap();
    let mut doc = GraphMl::default();
    let pasted = paste(&mut doc, copied);
    assert_eq!(pasted, [node("n0"), node("n1"), Item::Edge(0)]);
    let kind = graphics_of(&doc, "n1").unwrap().kind;
    let resource = Some("1".to_owned());
    assert_eq!(kind, NodeKind::Svg { resource });
    assert_eq!(refids(&doc, "n1"), ["1"]);
    assert_eq!(resource_ids(&doc), ["1"]);
    assert_eq!(
        edge_graphics_of(&doc, 0).unwrap().path.points,
        [point(240.0, 180.0)]
    );
    // All the namespaces of the copy, for the prefixes in its data.
    let mut namespaces = fixture().attributes;
    namespaces.retain(|name, _| name.starts_with("xmlns"));
    assert_eq!(doc.attributes, namespaces);
    assert_eq!(GraphMl::parse(&doc.to_xml()).unwrap(), doc);
}

#[test]
fn pasted_data_goes_under_the_documents_own_keys() {
    let mut doc = GraphMl::default();
    let id = add_node(
        &mut doc,
        &NodeGraphics::new(NodeKind::Shape(ShapeType::Ellipse)),
    );
    let keys = doc.keys.clone();
    // n0 has a description, under d5, and its graphics under d6.
    let copied = copy(&fixture(), &[node("n0")]).unwrap();
    let pasted = paste(&mut doc, copied);
    assert_eq!(pasted, [node("n1")]);
    let data: Vec<_> = (doc.graphs[0].nodes[1].data.iter())
        .map(|data| data.key.as_str())
        .collect();
    assert_eq!(data, ["d5", "d0"]);
    assert_eq!(doc.keys[..keys.len()], keys);
    let added: Vec<_> = (doc.keys[keys.len()..].iter())
        .map(|key| (key.id.as_str(), key.attr_name.as_deref()))
        .collect();
    assert_eq!(added, [("d5", Some("description"))]);
    assert_eq!(corner(&doc, "n1"), (100.0, 40.5));
    assert!(graphics_of(&doc, &id).is_some());
}

#[test]
fn pasted_keys_get_new_ids_if_theirs_are_taken() {
    let mut doc = yed::new_document();
    // Under d5, the id of n0's description in the copy.
    doc.keys[5].attr_name = Some("notes".to_owned());
    let keys = doc.keys.clone();
    let copied = copy(&fixture(), &[node("n0")]).unwrap();
    assert_eq!(paste(&mut doc, copied), [node("n0")]);
    let data: Vec<_> = (doc.graphs[0].nodes[0].data.iter())
        .map(|data| data.key.as_str())
        .collect();
    assert_eq!(data, ["d11", "d6"]);
    assert_eq!(doc.keys[..keys.len()], keys);
    let added: Vec<_> = (doc.keys[keys.len()..].iter())
        .map(|key| (key.id.as_str(), key.attr_name.as_deref()))
        .collect();
    assert_eq!(added, [("d11", Some("description"))]);
}

#[test]
fn pasted_resources_are_shared_if_the_same_and_renamed_if_not() {
    let mut doc = fixture();
    let copied = copy(&doc, &[node("n3")]).unwrap();
    paste(&mut doc, copied.clone());
    assert_eq!(refids(&doc, "n6"), ["1"]);
    assert_eq!(resource_ids(&doc), ["1"]);

    // Another drawing under the same id.
    let mut doc = fixture();
    let resources = doc.data[0].element_mut().unwrap();
    let resource = resources.elements_mut().next().unwrap();
    resource.children = vec![XmlNode::Text("<svg/>".to_owned())];
    paste(&mut doc, copied.clone());
    assert_eq!(refids(&doc, "n6"), ["2"]);
    assert_eq!(resource_ids(&doc), ["1", "2"]);
    let resources = yed::resources(&doc).unwrap();
    let original = yed::resources(&copied).unwrap();
    let pasted = resources.elements().nth(1).unwrap();
    assert_eq!(
        pasted.children,
        original.elements().next().unwrap().children
    );
    assert_eq!(GraphMl::parse(&doc.to_xml()).unwrap(), doc);

    // The same drawing under another id.
    let text = FIXTURE
        .replace(r#"refid="1""#, r#"refid="5""#)
        .replace(r#"<y:Resource id="1">"#, r#"<y:Resource id="5">"#);
    let mut doc = GraphMl::parse(&text).unwrap();
    paste(&mut doc, copied);
    assert_eq!(refids(&doc, "n6"), ["5"]);
    assert_eq!(resource_ids(&doc), ["5"]);
}

#[test]
fn pasted_groups_take_their_nested_nodes_along() {
    let mut doc = fixture();
    // With an edge between two of them, which yEd keeps in the group.
    let group = doc.graphs[0].nodes[5].graph.as_mut().unwrap();
    let mut second = group.nodes[0].clone();
    second.id = "n5::n1".to_owned();
    group.nodes.push(second);
    let mut edge = Edge::new("n5::n0", "n5::n1");
    edge.id = Some("n5::e0".to_owned());
    group.edges.push(edge);
    let copied = copy(&doc, &[node("n5")]).unwrap();
    assert_eq!(paste(&mut doc, copied), [node("n6")]);
    assert_eq!(corner(&doc, "n6"), (90.0, 330.0));
    let nested = doc.graphs[0].nodes[6].graph.as_ref().unwrap();
    assert_eq!(nested.id.as_deref(), Some("n6:"));
    assert_eq!(ids(nested), (vec!["n6::n0", "n6::n1"], vec!["n6::e0"]));
    let edge = &nested.edges[0];
    assert_eq!(
        (edge.source.as_str(), edge.target.as_str()),
        ("n6::n0", "n6::n1")
    );
    let graphics = Keys::find(&doc).node_graphics(&nested.nodes[0]);
    let geometry = graphics.unwrap().unwrap().geometry;
    assert_eq!((geometry.x, geometry.y), (160.0, 390.0));
}

#[test]
fn pastes_skip_edges_to_nodes_not_pasted() {
    let mut doc = fixture();
    let mut copied = copy(&doc, &[node("n0")]).unwrap();
    // To n1, which isn't in the copy.
    copied.graphs[0].edges.push(doc.graphs[0].edges[0].clone());
    assert_eq!(paste(&mut doc, copied), [node("n6")]);
    assert_eq!(doc.graphs[0].edges.len(), 2);

    let before = doc.clone();
    assert_eq!(paste(&mut doc, GraphMl::default()), []);
    assert_eq!(paste(&mut doc, yed::new_document()), []);
    assert_eq!(doc, before);
}

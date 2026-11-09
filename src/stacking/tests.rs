use graphml::yed::{self, NodeGraphics, NodeKind, ShapeType};

use super::*;
use crate::document::Item;

/// A document with a box per `(x, y)`, 10 wide and high, bottom up: n0, n1…
fn boxes(corners: &[(f64, f64)]) -> GraphMl {
    let mut doc = yed::new_document();
    for &(x, y) in corners {
        let mut graphics = NodeGraphics::new(NodeKind::Shape(ShapeType::Rectangle));
        graphics.geometry = yed::Rect {
            x,
            y,
            width: 10.0,
            height: 10.0,
        };
        document::add_node(&mut doc, &graphics);
    }
    doc
}

type Restacked = fn(&GraphMl, &Scene, &Selection) -> Option<Vec<usize>>;

/// The ids of the nodes of `doc`, bottom up, after `restacked` with the
/// nodes `selected`; `None` if they wouldn't move.
fn restack<'a>(restacked: Restacked, doc: &'a GraphMl, selected: &[&str]) -> Option<Vec<&'a str>> {
    let selected = Selection::of(selected.iter().map(|&id| Item::Node(id.to_owned())));
    let order = restacked(doc, &Scene::read(doc), &selected)?;
    let nodes = document::top_nodes(doc);
    Some(order.into_iter().map(|index| &*nodes[index].id).collect())
}

#[test]
fn raise_passes_the_nearest_overlapping_node() {
    // n1 is out of the way; n2 and n3 overlap n0.
    let doc = boxes(&[(0.0, 0.0), (50.0, 0.0), (5.0, 5.0), (6.0, 6.0)]);
    let order = vec!["n1", "n2", "n0", "n3"];
    assert_eq!(restack(raised, &doc, &["n0"]), Some(order));
}

#[test]
fn lower_passes_the_nearest_overlapping_node() {
    let doc = boxes(&[(6.0, 6.0), (5.0, 5.0), (50.0, 0.0), (0.0, 0.0)]);
    let order = vec!["n0", "n3", "n1", "n2"];
    assert_eq!(restack(lowered, &doc, &["n3"]), Some(order));
}

#[test]
fn nothing_moves_without_an_overlapping_node_to_pass() {
    // n1 to n4 only touch n0, one on each side; n5 is out of the way.
    let doc = boxes(&[
        (0.0, 0.0),
        (10.0, 0.0),
        (-10.0, 0.0),
        (0.0, 10.0),
        (0.0, -10.0),
        (50.0, 0.0),
    ]);
    assert_eq!(restack(raised, &doc, &["n0"]), None);
    assert_eq!(restack(lowered, &doc, &["n5"]), None);

    let doc = boxes(&[(0.0, 0.0), (5.0, 5.0)]);
    assert_eq!(restack(raised, &doc, &["n1"]), None);
    assert_eq!(restack(lowered, &doc, &["n0"]), None);
    assert_eq!(restack(raised, &doc, &[]), None);
    assert_eq!(restack(raised, &doc, &["n0", "n1"]), None);
}

#[test]
fn the_selected_nodes_keep_their_order() {
    // n0 and n1 overlap each other, and n2 overlaps n0 only.
    let doc = boxes(&[(0.0, 0.0), (5.0, 0.0), (-5.0, 0.0)]);
    let order = vec!["n2", "n0", "n1"];
    assert_eq!(restack(raised, &doc, &["n0", "n1"]), Some(order));

    // n2 overlaps n1 only, and n3 n0 only.
    let doc = boxes(&[(0.0, 0.0), (20.0, 0.0), (25.0, 0.0), (-5.0, 0.0)]);
    let order = vec!["n2", "n3", "n0", "n1"];
    assert_eq!(restack(raised, &doc, &["n0", "n1"]), Some(order.clone()));
    assert_eq!(restack(lowered, &doc, &["n2", "n3"]), Some(order));
}

#[test]
fn selected_nodes_without_a_node_to_pass_stay() {
    // n0 and n1 are out of the way, and n3 overlaps n2 only.
    let doc = boxes(&[(50.0, 0.0), (0.0, 0.0), (20.0, 0.0), (25.0, 0.0)]);
    let order = vec!["n0", "n1", "n3", "n2"];
    assert_eq!(restack(raised, &doc, &["n1", "n2"]), Some(order));
}

#[test]
fn nodes_without_graphics_keep_their_place() {
    let mut doc = boxes(&[(0.0, 0.0), (5.0, 5.0)]);
    doc.graphs[0].nodes.insert(1, graphml::Node::new("plain"));
    let order = vec!["plain", "n1", "n0"];
    assert_eq!(restack(raised, &doc, &["n0"]), Some(order));
}

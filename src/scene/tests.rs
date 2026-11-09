//! Tests of the scene and of reading it again where the document changed.

use egui::vec2;
use graphml::yed::{EdgeKind, NodeKind, ShapeType};

use super::*;

const FIXTURE: &str = include_str!("../../crates/graphml/tests/fixtures/yed.graphml");

fn fixture() -> GraphMl {
    GraphMl::parse(FIXTURE).unwrap()
}

fn node(id: &str) -> Item {
    Item::Node(id.to_owned())
}

fn rectangle() -> NodeGraphics {
    NodeGraphics::new(NodeKind::Shape(ShapeType::Rectangle))
}

fn run(at: usize, removed: usize, added: usize) -> Run {
    Run { at, removed, added }
}

#[test]
fn reading_the_runs_that_changed_matches_reading_everything() {
    let mut doc = fixture();
    let mut cache = Cache::read(&doc);
    let read = cache.reads;

    document::move_items(&mut doc, &[node("n1"), node("n2")], 4.0, 2.0);
    let changed = Changed::Runs {
        nodes: vec![run(1, 2, 2)],
        edges: vec![],
    };
    cache.update(&doc, &changed);
    assert_eq!(*cache.scene(), Scene::read(&doc));

    // n2, with the edge of it, e1.
    document::remove_items(&mut doc, &[node("n2")]);
    let changed = Changed::Runs {
        nodes: vec![run(2, 1, 0)],
        edges: vec![run(1, 1, 0)],
    };
    cache.update(&doc, &changed);
    assert_eq!(*cache.scene(), Scene::read(&doc));

    let id = document::add_node(&mut doc, &rectangle());
    let edge = EdgeGraphics::new(EdgeKind::PolyLine);
    document::add_edge(&mut doc, "n0", &id, &edge);
    let changed = Changed::Runs {
        nodes: vec![run(5, 0, 1)],
        edges: vec![run(1, 0, 1)],
    };
    cache.update(&doc, &changed);
    assert_eq!(*cache.scene(), Scene::read(&doc));
    assert_eq!(cache.reads, read + 2 + 2);
}

#[test]
fn edges_are_found_by_their_index_in_the_document() {
    let mut doc = fixture();
    // Its node goes, so the scene skips it.
    let id = document::add_node(&mut doc, &rectangle());
    let edge = EdgeGraphics::new(EdgeKind::PolyLine);
    document::add_edge(&mut doc, &id, "n1", &edge);
    document::add_edge(&mut doc, "n1", "n3", &edge);
    doc.graphs[0].nodes.pop();
    let scene = Scene::read(&doc);

    let indexes: Vec<_> = scene.edges.iter().map(|edge| edge.index).collect();
    assert_eq!(indexes, [0, 1, 3]);
    assert_eq!(scene.edge(3).map(|edge| edge.index), Some(3));
    assert!(scene.edge(2).is_none());
    assert!(scene.edge(4).is_none());
}

#[test]
fn a_shared_id_finds_the_first_node_with_it() {
    let mut doc = fixture();
    let duplicate = doc.graphs[0].nodes[3].clone();
    doc.graphs[0].nodes.push(duplicate);
    let scene = Scene::read(&doc);
    assert_eq!(scene.node("n3"), Some(3));
    assert_eq!(scene.node("missing"), None);
}

#[test]
fn moving_copies_only_what_moves() {
    let doc = fixture();
    let scene = Scene::read(&doc);
    let mut moved = scene.clone();
    let edges = HashSet::from([1]);
    moved.translate(&Selection::node("n2"), &edges, vec2(5.0, 0.0));

    let shared = |a: &Scene, b: &Scene, node: usize| Arc::ptr_eq(&a.nodes[node], &b.nodes[node]);
    assert!(!shared(&scene, &moved, 2));
    assert!(
        (0..scene.nodes.len())
            .filter(|&n| n != 2)
            .all(|n| shared(&scene, &moved, n))
    );
    let edge = |scene: &Scene, index| scene.edge(index).unwrap().graphics.clone();
    assert!(Arc::ptr_eq(&edge(&scene, 0), &edge(&moved, 0)));
    assert!(!Arc::ptr_eq(&edge(&scene, 1), &edge(&moved, 1)));

    let x = |scene: &Scene| scene.nodes[2].graphics.geometry.x;
    assert_eq!(x(&moved), x(&scene) + 5.0);
}

#[test]
fn graphics_are_found_for_items_in_the_scene() {
    let scene = Scene::read(&fixture());
    let Some(GraphicsRef::Node(n1)) = scene.graphics(&node("n1")) else {
        panic!("n1 is a node with graphics");
    };
    assert_eq!(n1.kind, NodeKind::Shape(ShapeType::Ellipse));
    assert!(matches!(
        scene.graphics(&Item::Edge(1)),
        Some(GraphicsRef::Edge(_))
    ));
    assert!(scene.graphics(&Item::Edge(2)).is_none());
    // Nested nodes aren't in the scene.
    assert!(scene.graphics(&node("n5::n0")).is_none());
}

/// The drawing of the node with id `id` in `scene`.
fn image(scene: &Scene, id: &str) -> Option<Arc<SvgImage>> {
    scene.nodes[scene.node(id).unwrap()].image.clone()
}

#[test]
fn svg_nodes_share_their_drawing_parsed_once() {
    let mut doc = fixture();
    let circle = yed::resource_text(&doc, "1").unwrap();
    let geometry = yed::Rect::default();
    let copy = document::add_svg_node(&mut doc, &circle, geometry);
    let mut cache = Cache::read(&doc);
    let n3 = image(cache.scene(), "n3").unwrap();
    assert!(Arc::ptr_eq(&n3, &image(cache.scene(), &copy).unwrap()));
    assert_eq!(n3.bounds().size(), vec2(8.0, 8.0));
    assert_eq!(image(cache.scene(), "n0"), None);

    // Read again, as when the document's resources change.
    document::move_items(&mut doc, &[node("n3")], 1.0, 0.0);
    cache.update(&doc, &Changed::All);
    assert!(Arc::ptr_eq(&n3, &image(cache.scene(), "n3").unwrap()));
}

#[test]
fn svg_nodes_have_no_drawing_if_theirs_cannot_be_read() {
    let mut doc = fixture();
    let broken = document::add_svg_node(&mut doc, "not svg", yed::Rect::default());
    let missing = document::add_svg_node(&mut doc, "", yed::Rect::default());
    document::edit_graphics(&mut doc, &missing, |graphics| {
        let resource = Some("9".to_owned());
        graphics.kind = NodeKind::Svg { resource };
    });
    let scene = Scene::read(&doc);
    assert_eq!(image(&scene, &broken), None);
    assert_eq!(image(&scene, &missing), None);
    assert!(image(&scene, "n3").is_some());
}

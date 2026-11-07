use graphml::yed::{self, NodeKind, ShapeType};

use super::*;

const FIXTURE: &str = include_str!("../../crates/graphml/tests/fixtures/yed.graphml");

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

#[test]
fn color_properties_are_the_fill_gradient_and_border_colors() {
    let mut graphics = rectangle();
    let [fill, gradient, border] = [1, 2, 3].map(|n| Some(Color::rgb(n, n, n)));
    ColorProperty::Fill.set(&mut graphics, fill);
    ColorProperty::Gradient.set(&mut graphics, gradient);
    ColorProperty::Border.set(&mut graphics, border);

    assert_eq!(graphics.fill.color, fill);
    assert_eq!(graphics.fill.gradient, gradient);
    assert_eq!(graphics.border.color, border);
    let read = ColorProperty::NODE.map(|property| property.get(&graphics));
    assert_eq!(read, [fill, gradient, border]);
}

#[test]
fn text_colors_are_those_of_the_labels_and_never_none() {
    let mut graphics = rectangle();
    graphics.labels = vec![Label::new("One"), Label::new("Two")];
    let red = Color::rgb(0xFF, 0x00, 0x00);
    ColorProperty::Text(1).set(&mut graphics, Some(red));
    ColorProperty::Text(0).set(&mut graphics, None);
    ColorProperty::Text(2).set(&mut graphics, Some(red));

    let colors: Vec<_> = graphics
        .labels
        .iter()
        .map(|label| label.text_color)
        .collect();
    assert_eq!(colors, [Color::BLACK, red]);
    let read = [0, 1, 2].map(|index| ColorProperty::Text(index).get(&graphics));
    assert_eq!(read, [Some(Color::BLACK), Some(red), None]);
    assert!(!ColorProperty::Text(0).optional());
    assert!(
        ColorProperty::NODE
            .iter()
            .all(|property| property.optional())
    );
}

#[test]
fn edit_graphics_changes_only_what_was_edited() {
    let mut doc = GraphMl::parse(FIXTURE).unwrap();
    edit_graphics(&mut doc, "n0", |graphics| {
        ColorProperty::Fill.set(graphics, Some(Color::rgb(0xFF, 0x00, 0x00)));
    });

    let text = doc.to_xml();
    assert_eq!(text.lines().count(), FIXTURE.lines().count());
    let changed: Vec<_> = FIXTURE
        .lines()
        .zip(text.lines())
        .filter(|(before, after)| before != after)
        .collect();
    assert_eq!(
        changed,
        [(
            r##"          <y:Fill color="#FFCC00" transparent="false"/>"##,
            r##"          <y:Fill color="#FF0000" transparent="false"/>"##,
        )]
    );
}

#[test]
fn edit_graphics_skips_nodes_without_graphics() {
    let mut doc = yed::new_document();
    doc.graphs[0].nodes.push(Node::new("plain"));
    let before = doc.clone();
    for id in ["plain", "missing"] {
        edit_graphics(&mut doc, id, |graphics| graphics.geometry.x = 10.0);
    }
    assert_eq!(doc, before);
}

fn label(node: &str, index: usize) -> LabelRef {
    LabelRef {
        node: node.to_owned(),
        index,
    }
}

#[test]
fn label_text_reads_a_label_or_a_new_one() {
    let mut doc = yed::new_document();
    let mut graphics = rectangle();
    graphics.labels = vec![Label::new("One"), Label::new("Two\nlines")];
    let id = add_node(&mut doc, &graphics);

    let texts = [0, 1, 2, 3].map(|index| label_text(&doc, &label(&id, index)));
    let expected = [Some("One"), Some("Two\nlines"), Some(""), None];
    assert_eq!(texts, expected.map(|text| text.map(str::to_owned)));
    assert_eq!(label_text(&doc, &label("missing", 0)), None);
}

#[test]
fn set_label_changes_only_the_text() {
    let mut doc = GraphMl::parse(FIXTURE).unwrap();
    set_label(&mut doc, &label("n2", 0), "Topic");

    let text = doc.to_xml();
    assert_eq!(text.lines().count(), FIXTURE.lines().count());
    let changed: Vec<_> = FIXTURE
        .lines()
        .zip(text.lines())
        .filter(|(before, after)| before != after)
        .collect();
    let [(before, after)] = changed.as_slice() else {
        panic!("expected one line to change: {changed:?}");
    };
    assert_eq!(*after, before.replace(">Queue<", ">Topic<"));
}

#[test]
fn set_label_to_the_same_text_changes_nothing() {
    let mut doc = GraphMl::parse(FIXTURE).unwrap();
    set_label(&mut doc, &label("n2", 0), "Queue");
    assert_eq!(doc.to_xml(), FIXTURE);
}

#[test]
fn set_label_adds_a_first_label() {
    let mut doc = yed::new_document();
    let id = add_node(&mut doc, &rectangle());
    set_label(&mut doc, &label(&id, 0), "");
    assert_eq!(graphics_of(&doc, &id).unwrap().labels, []);

    set_label(&mut doc, &label(&id, 0), "Hello\nworld");
    let labels = graphics_of(&doc, &id).unwrap().labels;
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0].text, "Hello\nworld");
    let defaults = Label::new("Hello\nworld");
    assert_eq!(labels[0].font, defaults.font);
    assert_eq!(labels[0].alignment, defaults.alignment);
}

#[test]
fn emptied_labels_are_kept() {
    let mut doc = GraphMl::parse(FIXTURE).unwrap();
    set_label(&mut doc, &label("n2", 0), "");
    let labels = graphics_of(&doc, "n2").unwrap().labels;
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0].text, "");
}

#[test]
fn set_label_skips_labels_that_are_missing() {
    let mut doc = GraphMl::parse(FIXTURE).unwrap();
    for missing in [label("n2", 2), label("missing", 0)] {
        set_label(&mut doc, &missing, "Topic");
    }
    assert_eq!(doc.to_xml(), FIXTURE);
}

#[test]
fn edit_label_changes_only_what_was_edited() {
    let mut doc = GraphMl::parse(FIXTURE).unwrap();
    edit_label(&mut doc, &label("n2", 0), |label| {
        label.font.family = "Noto Sans".to_owned();
        label.font.size = 16;
        label.font.style = yed::FontStyle::BoldItalic;
    });

    let text = doc.to_xml();
    assert_eq!(text.lines().count(), FIXTURE.lines().count());
    let changed: Vec<_> = FIXTURE
        .lines()
        .zip(text.lines())
        .filter(|(before, after)| before != after)
        .collect();
    let [(before, after)] = changed.as_slice() else {
        panic!("expected one line to change: {changed:?}");
    };
    let expected = before.replace(
        r#"fontFamily="Dialog" fontSize="12" fontStyle="plain""#,
        r#"fontFamily="Noto Sans" fontSize="16" fontStyle="bolditalic""#,
    );
    assert_eq!(*after, expected);
}

#[test]
fn edit_label_skips_labels_that_are_missing() {
    let mut doc = GraphMl::parse(FIXTURE).unwrap();
    for missing in [label("n2", 1), label("missing", 0)] {
        edit_label(&mut doc, &missing, |label| label.font.size = 16);
    }
    assert_eq!(doc.to_xml(), FIXTURE);
}

fn bounds(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn resize_keeps_labels_in_the_middle_or_by_the_nearest_side() {
    let placed = |x, y, width, height| {
        let mut label = Label::new("Label");
        label.bounds = bounds(x, y, width, height);
        label
    };
    let mut graphics = rectangle();
    graphics.geometry = bounds(10.0, 20.0, 100.0, 60.0);
    graphics.labels = vec![
        // In the middle.
        placed(30.0, 20.0, 40.0, 20.0),
        // Along the top, in the middle across.
        placed(30.0, 0.0, 40.0, 18.0),
        // Below the node, near its right side.
        placed(76.0, 64.0, 20.0, 10.0),
        // No bounds yet, so the canvas puts it in the middle anyway.
        Label::new("New"),
    ];
    resize(&mut graphics, bounds(0.0, 20.0, 150.0, 100.0));

    assert_eq!(graphics.geometry, bounds(0.0, 20.0, 150.0, 100.0));
    let moved: Vec<_> = graphics.labels.iter().map(|label| label.bounds).collect();
    assert_eq!(
        moved,
        [
            bounds(55.0, 40.0, 40.0, 20.0),
            bounds(55.0, 0.0, 40.0, 18.0),
            bounds(126.0, 104.0, 20.0, 10.0),
            Rect::default(),
        ]
    );
}

#[test]
fn resize_changes_only_the_geometry_and_label_positions() {
    let mut doc = GraphMl::parse(FIXTURE).unwrap();
    let mut geometry = graphics_of(&doc, "n0").unwrap().geometry;
    geometry.width += 40.0;
    edit_graphics(&mut doc, "n0", |graphics| resize(graphics, geometry));

    // n0 is the first node.
    let expected = FIXTURE
        .replacen(r#"width="120.0""#, r#"width="160.0""#, 1)
        .replacen(r#"x="30.9912109375""#, r#"x="50.9912109375""#, 1);
    assert_eq!(doc.to_xml(), expected);
}

#[test]
fn resize_to_the_same_size_changes_only_the_position() {
    let mut doc = GraphMl::parse(FIXTURE).unwrap();
    let mut geometry = graphics_of(&doc, "n0").unwrap().geometry;
    geometry.x += 30.0;
    geometry.y += 40.0;
    edit_graphics(&mut doc, "n0", |graphics| resize(graphics, geometry));

    // n0 is the first node, and its label is in the middle.
    let expected = FIXTURE.replacen(r#"x="100.0" y="40.5""#, r#"x="130.0" y="80.5""#, 1);
    assert_eq!(doc.to_xml(), expected);
}

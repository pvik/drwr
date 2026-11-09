use graphml::yed::{self, EdgeKind, NodeKind, Path, ShapeType};

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
    let mut graphics = Graphics::Node(rectangle());
    let [fill, gradient, border] = [1, 2, 3].map(|n| Some(Color::rgb(n, n, n)));
    ColorProperty::Fill.set(&mut graphics, fill);
    ColorProperty::Gradient.set(&mut graphics, gradient);
    ColorProperty::Border.set(&mut graphics, border);

    let Graphics::Node(node) = &graphics else {
        unreachable!();
    };
    assert_eq!(node.fill.color, fill);
    assert_eq!(node.fill.gradient, gradient);
    assert_eq!(node.border.color, border);
    let read = ColorProperty::NODE.map(|property| property.get(&graphics));
    assert_eq!(read, [fill, gradient, border]);
}

#[test]
fn edges_have_a_line_color_and_no_fill() {
    let mut graphics = Graphics::Edge(EdgeGraphics::new(EdgeKind::PolyLine));
    let red = Some(Color::rgb(0xFF, 0x00, 0x00));
    ColorProperty::Line.set(&mut graphics, red);
    ColorProperty::Fill.set(&mut graphics, red);
    assert_eq!(graphics.stroke().color, red);
    assert_eq!(ColorProperty::Line.get(&graphics), red);
    assert_eq!(ColorProperty::Fill.get(&graphics), None);
    ColorProperty::Line.set(&mut graphics, None);
    assert_eq!(ColorProperty::Line.get(&graphics), None);
}

#[test]
fn text_colors_are_those_of_the_labels_and_never_none() {
    let mut node = rectangle();
    node.labels = vec![Label::new("One"), Label::new("Two")];
    let mut graphics = Graphics::Node(node);
    let red = Color::rgb(0xFF, 0x00, 0x00);
    ColorProperty::Text(1).set(&mut graphics, Some(red));
    ColorProperty::Text(0).set(&mut graphics, None);
    ColorProperty::Text(2).set(&mut graphics, Some(red));

    let colors: Vec<_> = graphics
        .labels()
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
    edit(&mut doc, &Item::Node("n0".into()), |graphics| {
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
        item: Item::Node(node.to_owned()),
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
    set_label(&mut doc, &label("n2", 0), "Topic", &Style::default());

    let text = doc.to_xml();
    let changed = changed_lines(&text);
    let [(before, after)] = changed.as_slice() else {
        panic!("expected one line to change: {changed:?}");
    };
    assert_eq!(*after, before.replace(">Queue<", ">Topic<"));
}

#[test]
fn set_label_to_the_same_text_changes_nothing() {
    let mut doc = GraphMl::parse(FIXTURE).unwrap();
    set_label(&mut doc, &label("n2", 0), "Queue", &Style::default());
    assert_eq!(doc.to_xml(), FIXTURE);
}

#[test]
fn set_label_adds_a_first_label() {
    let mut doc = yed::new_document();
    let id = add_node(&mut doc, &rectangle());
    set_label(&mut doc, &label(&id, 0), "", &Style::default());
    assert_eq!(graphics_of(&doc, &id).unwrap().labels, []);

    set_label(&mut doc, &label(&id, 0), "Hello\nworld", &Style::default());
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
    set_label(&mut doc, &label("n2", 0), "", &Style::default());
    let labels = graphics_of(&doc, "n2").unwrap().labels;
    assert_eq!(labels.len(), 1);
    assert_eq!(labels[0].text, "");
}

#[test]
fn set_label_skips_labels_that_are_missing() {
    let mut doc = GraphMl::parse(FIXTURE).unwrap();
    for missing in [label("n2", 2), label("missing", 0)] {
        set_label(&mut doc, &missing, "Topic", &Style::default());
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
    let changed = changed_lines(&text);
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

#[test]
fn set_label_position_moves_the_label_keeping_its_size() {
    let mut doc = GraphMl::parse(FIXTURE).unwrap();
    set_label_position(&mut doc, &label("n2", 0), LabelPosition::North);

    let labels = graphics_of(&doc, "n2").unwrap().labels;
    assert_eq!(labels[0].position(), Some(LabelPosition::North));
    // 4 above the node, as yEd puts it.
    assert_eq!(labels[0].bounds, bounds(21.0, -21.96875, 38.0, 17.96875));
    let text = doc.to_xml();
    let changed = changed_lines(&text);
    let [(label, moved), (parameter, placed)] = &changed[..] else {
        panic!("expected two lines to change: {changed:?}");
    };
    assert_eq!(
        *moved,
        label.replace(r#"y="11.015625""#, r#"y="-21.96875""#)
    );
    let expected = parameter.replace(
        r#"labelRatioY="0.0" nodeRatioX="0.0" nodeRatioY="0.0" offsetX="0.0" offsetY="0.0""#,
        r#"labelRatioY="0.5" nodeRatioX="0.0" nodeRatioY="-0.5" offsetX="0.0" offsetY="-4.0""#,
    );
    assert_eq!(*placed, expected);
}

#[test]
fn set_label_position_skips_labels_that_are_missing() {
    let mut doc = GraphMl::parse(FIXTURE).unwrap();
    for missing in [label("n2", 1), label("missing", 0)] {
        set_label_position(&mut doc, &missing, LabelPosition::North);
    }
    assert_eq!(doc.to_xml(), FIXTURE);
}

/// The lines of `text` that differ from the fixture's, which it has as many
/// of.
fn changed_lines(text: &str) -> Vec<(&'static str, &str)> {
    assert_eq!(text.lines().count(), FIXTURE.lines().count());
    FIXTURE
        .lines()
        .zip(text.lines())
        .filter(|(before, after)| before != after)
        .collect()
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

fn shape(shape: ShapeType) -> NodeGraphics {
    NodeGraphics::new(NodeKind::Shape(shape))
}

fn kind(doc: &GraphMl, id: &str) -> NodeKind {
    graphics_of(doc, id).unwrap().kind
}

#[test]
fn boxes_have_corner_radii() {
    let mut doc = yed::new_document();
    let square = add_node(&mut doc, &rectangle());
    let round = add_node(&mut doc, &shape(ShapeType::RoundRectangle));
    let ellipse = add_node(&mut doc, &shape(ShapeType::Ellipse));

    assert_eq!(corner_radius(&doc, &square), Some(0.0));
    // yEd's, which it draws every round rectangle with.
    assert_eq!(corner_radius(&doc, &round), Some(4.0));
    assert_eq!(corner_radius(&doc, &ellipse), None);
    assert_eq!(corner_radius(&doc, "missing"), None);
    let radii: Vec<_> = node_graphics(&doc).map(|(_, _, radius)| radius).collect();
    assert_eq!(radii, [Some(0.0), Some(4.0), None]);
}

#[test]
fn rounding_boxes_like_yed_makes_them_its_round_rectangles() {
    let mut doc = yed::new_document();
    let id = add_node(&mut doc, &rectangle());
    let keys = doc.keys.clone();
    set_corner_radius(&mut doc, &id, 4.0);

    assert_eq!(kind(&doc, &id), NodeKind::Shape(ShapeType::RoundRectangle));
    assert_eq!(corner_radius(&doc, &id), Some(4.0));
    // Which yEd draws as they are, so there's nothing else to keep.
    assert_eq!(doc.keys, keys);
    assert_eq!(doc.graphs[0].nodes[0].data.len(), 1);

    set_corner_radius(&mut doc, &id, 0.0);
    assert_eq!(kind(&doc, &id), NodeKind::Shape(ShapeType::Rectangle));
    assert_eq!(corner_radius(&doc, &id), Some(0.0));
}

#[test]
fn other_corner_radii_are_kept_in_a_node_attribute() {
    let mut doc = yed::new_document();
    let id = add_node(&mut doc, &rectangle());
    set_corner_radius(&mut doc, &id, 10.0);

    assert_eq!(kind(&doc, &id), NodeKind::Shape(ShapeType::RoundRectangle));
    assert_eq!(corner_radius(&doc, &id), Some(10.0));
    let text = doc.to_xml();
    let key = r#"<key attr.name="drwr.cornerRadius" attr.type="double" for="node" id="d11"/>"#;
    assert!(text.contains(key), "{text}");
    // As yEd writes doubles.
    assert!(text.contains(r#"<data key="d11">10.0</data>"#), "{text}");
    let read = GraphMl::parse(&text).unwrap();
    assert_eq!(corner_radius(&read, &id), Some(10.0));

    let before = text;
    set_corner_radius(&mut doc, &id, 7.5);
    assert_eq!(doc.to_xml(), before.replace(">10.0<", ">7.5<"));
}

#[test]
fn resetting_the_corner_radius_leaves_the_file_as_it_was() {
    let mut doc = GraphMl::parse(FIXTURE).unwrap();
    assert_eq!(kind(&doc, "n0"), NodeKind::Shape(ShapeType::RoundRectangle));
    set_corner_radius(&mut doc, "n0", 10.0);
    assert_ne!(doc.to_xml(), FIXTURE);
    set_corner_radius(&mut doc, "n0", 4.0);
    assert_eq!(doc.to_xml(), FIXTURE);

    set_corner_radius(&mut doc, "n0", 0.0);
    set_corner_radius(&mut doc, "n0", 12.5);
    set_corner_radius(&mut doc, "n0", 4.0);
    assert_eq!(doc.to_xml(), FIXTURE);
}

#[test]
fn the_key_stays_while_any_node_keeps_a_corner_radius() {
    let mut doc = yed::new_document();
    let first = add_node(&mut doc, &rectangle());
    let second = add_node(&mut doc, &rectangle());
    set_corner_radius(&mut doc, &first, 10.0);
    set_corner_radius(&mut doc, &second, 20.0);
    let key = radius_key(&doc).unwrap().to_owned();
    let declared = doc.keys.iter().filter(|declared| declared.id == key);
    assert_eq!(declared.count(), 1);

    set_corner_radius(&mut doc, &first, 0.0);
    assert_eq!(radius_key(&doc), Some(key.as_str()));
    assert_eq!(corner_radius(&doc, &second), Some(20.0));
    // Even one in a group, which yEd could have moved there.
    let mut nested = Node::new("group::n0");
    nested.data.push(Data::from_text(&key, "3.0"));
    let mut group = Node::new("group");
    group.graph = Some(Graph {
        nodes: vec![nested],
        ..Graph::default()
    });
    doc.graphs[0].nodes.push(group);
    set_corner_radius(&mut doc, &second, 0.0);
    assert_eq!(radius_key(&doc), Some(key.as_str()));

    doc.graphs[0].nodes.pop();
    set_corner_radius(&mut doc, &first, 10.0);
    set_corner_radius(&mut doc, &first, 4.0);
    assert_eq!(radius_key(&doc), None);
}

#[test]
fn kept_corner_radii_round_only_round_rectangles() {
    let mut doc = yed::new_document();
    let id = add_node(&mut doc, &rectangle());
    set_corner_radius(&mut doc, &id, 10.0);
    // As yEd leaves the attribute when it makes the node a rectangle.
    let make = |doc: &mut GraphMl, shape| {
        edit_graphics(doc, &id, |graphics| graphics.kind = NodeKind::Shape(shape));
    };
    make(&mut doc, ShapeType::Rectangle);
    assert_eq!(corner_radius(&doc, &id), Some(0.0));
    make(&mut doc, ShapeType::RoundRectangle);
    assert_eq!(corner_radius(&doc, &id), Some(10.0));
}

#[test]
fn kept_corner_radii_are_read_however_written_else_yeds_is() {
    for (text, radius) in [
        ("10", 10.0),
        (" 2.5\n", 2.5),
        ("1.0E-4", 1e-4),
        ("abc", 4.0),
        ("", 4.0),
        ("-1.0", 4.0),
        ("NaN", 4.0),
        ("Infinity", 4.0),
    ] {
        let mut doc = yed::new_document();
        let id = add_node(&mut doc, &shape(ShapeType::RoundRectangle));
        let key = ensure_radius_key(&mut doc);
        doc.graphs[0].nodes[0].data.push(Data::from_text(key, text));
        assert_eq!(corner_radius(&doc, &id), Some(radius), "{text:?}");
    }
}

#[test]
fn set_corner_radius_skips_other_shapes() {
    let mut doc = yed::new_document();
    let id = add_node(&mut doc, &shape(ShapeType::Ellipse));
    doc.graphs[0].nodes.push(Node::new("plain"));
    let before = doc.clone();
    for id in [id.as_str(), "plain", "missing"] {
        set_corner_radius(&mut doc, id, 10.0);
    }
    assert_eq!(doc, before);
}

fn fixture() -> GraphMl {
    GraphMl::parse(FIXTURE).unwrap()
}

fn edge_ids(doc: &GraphMl) -> Vec<&str> {
    let edges = &doc.graphs[0].edges;
    edges.iter().filter_map(|edge| edge.id.as_deref()).collect()
}

fn point(x: f64, y: f64) -> Point {
    Point { x, y }
}

#[test]
fn new_edges_get_an_id_no_edge_has() {
    let mut doc = GraphMl::default();
    let graphics = EdgeGraphics::new(EdgeKind::PolyLine);
    assert_eq!(add_edge(&mut doc, "n0", "n1", &graphics), 0);
    assert_eq!(doc.graphs[0].id.as_deref(), Some("G"));
    let read = edge_graphics_of(&doc, 0).unwrap();
    assert_eq!(read.to_element(), graphics.to_element());

    let mut doc = fixture();
    doc.graphs[0].edges[0].id = Some("e2".to_owned());
    assert_eq!(add_edge(&mut doc, "n0", "n2", &graphics), 2);
    let edge = &doc.graphs[0].edges[2];
    assert_eq!((edge.source.as_str(), edge.target.as_str()), ("n0", "n2"));
    assert_eq!(edge_ids(&doc), ["e2", "e1", "e0"]);
}

#[test]
fn removing_a_node_removes_the_edges_of_it_and_of_the_nodes_in_it() {
    let mut doc = fixture();
    let graphics = EdgeGraphics::new(EdgeKind::PolyLine);
    // n5 is a group holding n5::n0.
    add_edge(&mut doc, "n5::n0", "n2", &graphics);
    add_edge(&mut doc, "n3", "n4", &graphics);
    remove_node(&mut doc, "n5");
    assert_eq!(edge_ids(&doc), ["e0", "e1", "e3"]);
    remove_node(&mut doc, "n0");
    assert_eq!(edge_ids(&doc), ["e1", "e3"]);
    assert!(doc.graphs[0].nodes.iter().all(|node| node.id != "n0"));
    remove_edge(&mut doc, 0);
    assert_eq!(edge_ids(&doc), ["e3"]);
    // Missing ones are skipped.
    let before = doc.clone();
    remove_node(&mut doc, "missing");
    remove_edge(&mut doc, 5);
    assert_eq!(doc, before);
}

#[test]
fn ports_move_within_their_node_or_onto_another() {
    let mut doc = fixture();
    edit_edge_graphics(&mut doc, 0, |graphics| {
        graphics.path.target = point(5.0, 5.0)
    });
    let before = doc.clone();
    move_port(&mut doc, 0, EdgeEnd::Target, "n1", point(5.0, 5.0));
    assert_eq!(doc, before);

    move_port(&mut doc, 0, EdgeEnd::Target, "n1", point(-10.0, 2.5));
    assert_eq!(doc.graphs[0].edges[0].target, "n1");
    let path = edge_graphics_of(&doc, 0).unwrap().path;
    assert_eq!(path.target, point(-10.0, 2.5));

    move_port(&mut doc, 0, EdgeEnd::Target, "n3", point(0.0, 0.0));
    move_port(&mut doc, 0, EdgeEnd::Source, "n2", point(3.0, -4.0));
    let edge = &doc.graphs[0].edges[0];
    assert_eq!((edge.source.as_str(), edge.target.as_str()), ("n2", "n3"));
    let path = edge_graphics_of(&doc, 0).unwrap().path;
    assert_eq!(
        (path.source, path.target),
        (point(3.0, -4.0), point(0.0, 0.0))
    );
}

#[test]
fn arrows_are_set_on_and_off_keeping_their_kind() {
    let mut doc = fixture();
    edit_edge_graphics(&mut doc, 0, |graphics| {
        graphics.arrows.target = Arrow::Delta
    });
    set_arrow(&mut doc, 0, EdgeEnd::Target, true);
    set_arrow(&mut doc, 0, EdgeEnd::Source, true);
    let arrows = edge_graphics_of(&doc, 0).unwrap().arrows;
    assert_eq!(
        (arrows.source, arrows.target),
        (Arrow::Standard, Arrow::Delta)
    );
    set_arrow(&mut doc, 0, EdgeEnd::Target, false);
    let arrows = edge_graphics_of(&doc, 0).unwrap().arrows;
    assert_eq!(arrows.target, Arrow::None);
}

#[test]
fn bends_move_one_or_all_and_are_removed_with_labels_kept_on_their_segment() {
    let mut doc = fixture();
    move_bends(&mut doc, 0, None, 1.0, 2.0);
    move_bends(&mut doc, 0, Some(1), 10.0, 0.0);
    let points = edge_graphics_of(&doc, 0).unwrap().path.points;
    assert_eq!(points, [point(261.0, 22.0), point(336.0, 22.0)]);

    // The label is on the first segment, which keeps its index.
    let mut doc = fixture();
    let segment = |doc: &GraphMl| {
        let text = doc.to_xml();
        let start = text.find(r#"segment=""#).unwrap() + 9;
        text[start..].split('"').next().unwrap().to_owned()
    };
    let geometry = |id: &str| graphics_of(&doc, id).unwrap().geometry;
    let (n0, n1) = (geometry("n0"), geometry("n1"));
    edit_edge_graphics(&mut doc, 0, |graphics| {
        let route = Route::new(&graphics.path, n0, n1);
        graphics.labels[0].place_on(&route, point(292.5, 0.0), 35.0, 18.0);
    });
    assert_eq!(segment(&doc), "1");
    remove_bend(&mut doc, 0, 0);
    assert_eq!(
        edge_graphics_of(&doc, 0).unwrap().path.points,
        [point(325.0, 20.0)]
    );
    assert_eq!(segment(&doc), "0");
    remove_bend(&mut doc, 0, 5);
    assert_eq!(edge_graphics_of(&doc, 0).unwrap().path.points.len(), 1);
}

#[test]
fn labels_are_kept_where_they_were_on_a_new_route() {
    let (source, target) = (
        Rect {
            x: 0.0,
            y: 0.0,
            width: 40.0,
            height: 20.0,
        },
        Rect {
            x: 200.0,
            y: 0.0,
            width: 40.0,
            height: 20.0,
        },
    );
    let mut graphics = EdgeGraphics::new(EdgeKind::PolyLine);
    let old = Route::new(&graphics.path, source, target);
    let mut label = Label::new_on_edge("Label");
    label.place_on(&old, point(150.0, 10.0), 30.0, 10.0);
    graphics.labels.push(label);
    graphics.path.points.push(point(100.0, 10.0));
    let new = Route::new(&graphics.path, source, target);
    keep_labels(&mut graphics, &old, &new);
    let placed = graphics.labels[0].placed_on(&new, 30.0, 10.0).unwrap();
    assert_eq!((placed.x, placed.y), (135.0, 5.0));
}

#[test]
fn edges_have_labels_like_nodes() {
    let mut doc = fixture();
    let label = LabelRef {
        item: Item::Edge(0),
        index: 1,
    };
    assert_eq!(label_text(&doc, &label), Some(String::new()));
    set_label(&mut doc, &label, "second", &Style::default());
    let labels = edge_graphics_of(&doc, 0).unwrap().labels;
    assert_eq!(labels[1].text, "second");
    // In yEd's Smart model for edge labels.
    let route = Route::new(&Path::default(), Rect::default(), Rect::default());
    assert!(labels[1].placed_on(&route, 10.0, 10.0).is_some());
    edit_label(&mut doc, &label, |label| label.font.size = 20);
    assert_eq!(edge_graphics_of(&doc, 0).unwrap().labels[1].font.size, 20);
    // Positions are for node labels.
    let before = doc.clone();
    set_label_position(&mut doc, &label, LabelPosition::North);
    assert_eq!(doc, before);
}

#[test]
fn moving_items_takes_along_the_bends_of_edges_between_moved_nodes() {
    let mut doc = GraphMl::default();
    for _ in 0..3 {
        add_node(&mut doc, &rectangle());
    }
    let mut graphics = EdgeGraphics::new(EdgeKind::PolyLine);
    graphics.path.points = vec![point(5.0, 5.0)];
    add_edge(&mut doc, "n0", "n1", &graphics);
    add_edge(&mut doc, "n1", "n2", &graphics);
    let nodes = [Item::Node("n0".to_owned()), Item::Node("n1".to_owned())];
    assert_eq!(bends_moved_with(&doc, &nodes), HashSet::from([0]));

    move_items(&mut doc, &nodes, 1.0, 2.0);
    let corner = |doc: &GraphMl, id: &str| {
        let geometry = graphics_of(doc, id).unwrap().geometry;
        (geometry.x, geometry.y)
    };
    let bends = |doc: &GraphMl, index: usize| edge_graphics_of(doc, index).unwrap().path.points;
    assert_eq!(corner(&doc, "n0"), (1.0, 2.0));
    assert_eq!(corner(&doc, "n1"), (1.0, 2.0));
    assert_eq!(corner(&doc, "n2"), (0.0, 0.0));
    assert_eq!(bends(&doc, 0), [point(6.0, 7.0)]);
    assert_eq!(bends(&doc, 1), [point(5.0, 5.0)]);
    // A selected edge's bends move without its nodes.
    move_items(&mut doc, &[Item::Edge(1)], -5.0, 0.0);
    assert_eq!(bends(&doc, 1), [point(0.0, 5.0)]);
    assert_eq!(corner(&doc, "n2"), (0.0, 0.0));
}

#[test]
fn removing_items_removes_edges_by_their_indexes_and_nodes_with_theirs() {
    let mut doc = fixture();
    let graphics = EdgeGraphics::new(EdgeKind::PolyLine);
    add_edge(&mut doc, "n3", "n4", &graphics);
    assert_eq!(edge_ids(&doc), ["e0", "e1", "e2"]);
    // Each edge once, however many times it's listed.
    remove_items(&mut doc, &[Item::Edge(2), Item::Edge(0), Item::Edge(2)]);
    assert_eq!(edge_ids(&doc), ["e1"]);
    // e1 joins n2.
    remove_items(&mut doc, &[Item::Node("n2".to_owned())]);
    assert_eq!(edge_ids(&doc), [""; 0]);
    assert_eq!(graphics_of(&doc, "n2"), None);
}

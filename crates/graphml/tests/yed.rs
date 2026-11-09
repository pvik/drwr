use graphml::{
    Edge, Error, Graph, GraphMl, KeyDomain, Node,
    xml::{Element, XmlNode},
    yed::{
        Alignment, Arrow, Arrows, Color, EdgeGraphics, EdgeKind, Fill, Font, FontStyle, Keys,
        Label, LabelPosition, LineType, NAMESPACE, NodeGraphics, NodeKind, Path, Point, Rect,
        Route, ShapeType, Stroke, add_resource, new_document, resource_text, resources,
        resources_mut,
    },
};

const YED: &str = include_str!("fixtures/yed.graphml");

fn fixture() -> (GraphMl, Keys) {
    let doc = GraphMl::parse(YED).unwrap();
    let keys = Keys::find(&doc);
    (doc, keys)
}

fn find_node<'a>(graph: &'a mut Graph, id: &str) -> Option<&'a mut Node> {
    graph.nodes.iter_mut().find_map(|node| {
        if node.id == id {
            Some(node)
        } else {
            find_node(node.graph.as_mut()?, id)
        }
    })
}

fn node<'a>(doc: &'a mut GraphMl, id: &str) -> &'a mut Node {
    find_node(&mut doc.graphs[0], id).unwrap()
}

fn edge<'a>(doc: &'a mut GraphMl, id: &str) -> &'a mut Edge {
    let edges = &mut doc.graphs[0].edges;
    edges
        .iter_mut()
        .find(|edge| edge.id.as_deref() == Some(id))
        .unwrap()
}

fn node_graphics(doc: &mut GraphMl, keys: &Keys, id: &str) -> NodeGraphics {
    keys.node_graphics(node(doc, id)).unwrap().unwrap()
}

fn edge_graphics(doc: &mut GraphMl, keys: &Keys, id: &str) -> EdgeGraphics {
    keys.edge_graphics(edge(doc, id)).unwrap().unwrap()
}

/// The lines that differ between `before` and `after`, which must have as many.
fn changed_lines(before: &str, after: &str) -> Vec<(String, String)> {
    assert_eq!(before.lines().count(), after.lines().count());
    before
        .lines()
        .zip(after.lines())
        .filter(|(before, after)| before != after)
        .map(|(before, after)| (before.trim().to_owned(), after.trim().to_owned()))
        .collect()
}

fn pairs(lines: &[(&str, &str)]) -> Vec<(String, String)> {
    lines
        .iter()
        .map(|&(a, b)| (a.to_owned(), b.to_owned()))
        .collect()
}

#[test]
fn reads_shape_nodes() {
    let (mut doc, keys) = fixture();
    let n0 = node_graphics(&mut doc, &keys, "n0");
    assert_eq!(n0.kind, NodeKind::Shape(ShapeType::RoundRectangle));
    assert_eq!(
        n0.geometry,
        Rect {
            x: 100.0,
            y: 40.5,
            width: 120.0,
            height: 60.0
        }
    );
    assert_eq!(
        n0.fill,
        Fill {
            color: Some(Color::rgb(0xFF, 0xCC, 0x00)),
            gradient: None
        }
    );
    assert_eq!(n0.border, Stroke::default());
    let [label] = &n0.labels[..] else {
        panic!("expected one label")
    };
    assert_eq!(label.text, "Order\nservice");
    assert_eq!(label.font, Font::default());
    assert_eq!(label.alignment, Alignment::Center);
    assert_eq!(
        label.bounds,
        Rect {
            x: 30.9912109375,
            y: 14.03125,
            width: 58.017578125,
            height: 31.9375
        }
    );

    let n1 = node_graphics(&mut doc, &keys, "n1");
    assert_eq!(n1.kind, NodeKind::Shape(ShapeType::Ellipse));
    assert_eq!(n1.fill.color, None);
    assert_eq!(
        n1.border,
        Stroke {
            color: Some(Color::rgb(0x33, 0x66, 0xFF)),
            line_type: LineType::Dashed,
            width: 2.0
        }
    );
    assert_eq!(n1.labels[0].text, "DB");
    assert_eq!(n1.labels[0].font.style, FontStyle::Bold);
}

#[test]
fn reads_other_node_kinds() {
    let (mut doc, keys) = fixture();
    let n2 = node_graphics(&mut doc, &keys, "n2");
    assert_eq!(
        n2.kind,
        NodeKind::Generic("com.yworks.flowchart.directData".into())
    );
    assert_eq!(n2.fill.gradient, Some(Color::rgb(0xB7, 0xC9, 0xE3)));
    let n3 = node_graphics(&mut doc, &keys, "n3");
    assert_eq!(
        n3.kind,
        NodeKind::Svg {
            resource: Some("1".into())
        }
    );
    assert_eq!(n3.labels[0].text, "");
    let n4 = node_graphics(&mut doc, &keys, "n4");
    assert_eq!(n4.kind, NodeKind::Other("UMLClassNode".into()));
    assert_eq!(n4.labels[0].text, "Order");

    // A group node shows the realizer for its current (open) state.
    let n5 = node_graphics(&mut doc, &keys, "n5");
    assert_eq!(n5.kind, NodeKind::Other("GroupNode".into()));
    assert_eq!(
        n5.geometry,
        Rect {
            x: 80.0,
            y: 320.0,
            width: 200.0,
            height: 120.0
        }
    );
    let label = &n5.labels[0];
    assert_eq!(label.text, "Backend");
    assert_eq!(label.background, Some(Color::rgb(0xEB, 0xEB, 0xEB)));
    assert_eq!(label.border, None);
    assert_eq!(label.alignment, Alignment::Right);

    let worker = node_graphics(&mut doc, &keys, "n5::n0");
    assert_eq!(worker.kind, NodeKind::Shape(ShapeType::Rectangle));
    assert_eq!(worker.labels[0].text, "Worker");
}

#[test]
fn reads_edges() {
    let (mut doc, keys) = fixture();
    let e0 = edge_graphics(&mut doc, &keys, "e0");
    assert_eq!(e0.kind, EdgeKind::PolyLine);
    assert_eq!(
        e0.path.points,
        [Point { x: 260.0, y: 20.0 }, Point { x: 325.0, y: 20.0 }]
    );
    assert_eq!(e0.line, Stroke::default());
    assert_eq!(
        e0.arrows,
        Arrows {
            source: Arrow::None,
            target: Arrow::Standard
        }
    );
    assert_eq!(e0.labels[0].text, "reads");

    let e1 = edge_graphics(&mut doc, &keys, "e1");
    assert_eq!(e1.kind, EdgeKind::Other("ArcEdge".into()));
    assert_eq!(e1.line.line_type, LineType::Dashed);
    assert_eq!(
        e1.arrows,
        Arrows {
            source: Arrow::Standard,
            target: Arrow::Standard
        }
    );
    assert_eq!(e1.path.points, [Point { x: 240.0, y: 180.0 }]);
}

/// [`YED`] with some of n1's and e0's values written otherwise than this
/// library writes them, though they read the same.
fn unusual_yed() -> String {
    let text = YED
        .replace(
            r#"height="50.0" width="50.0" x="300.0""#,
            r#"height="50" width="50.0" x="300.0""#,
        )
        .replace(r##"color="#3366FF""##, r##"color="#3366ff""##)
        .replace(r#"x="13.0""#, r#"x="13""#)
        .replace(r#"<y:Point x="260.0""#, r#"<y:Point x="260""#)
        .replace(r#"ratio="0.5" segment"#, r#"ratio="0.50" segment"#);
    assert_eq!(changed_lines(YED, &text).len(), 5);
    text
}

#[test]
fn unchanged_graphics_write_back_identically() {
    let text = unusual_yed();
    let mut doc = GraphMl::parse(&text).unwrap();
    let keys = Keys::find(&doc);
    for id in ["n0", "n1", "n2", "n3", "n4", "n5", "n5::n0"] {
        let graphics = node_graphics(&mut doc, &keys, id);
        keys.set_node_graphics(node(&mut doc, id), &graphics);
    }
    for id in ["e0", "e1"] {
        let graphics = edge_graphics(&mut doc, &keys, id);
        keys.set_edge_graphics(edge(&mut doc, id), &graphics);
    }
    assert_eq!(doc.to_xml(), text);
}

#[test]
fn moving_a_node_changes_only_its_geometry() {
    let text = unusual_yed();
    let mut doc = GraphMl::parse(&text).unwrap();
    let keys = Keys::find(&doc);
    let mut n1 = node_graphics(&mut doc, &keys, "n1");
    n1.geometry.x += 12.5;
    keys.set_node_graphics(node(&mut doc, "n1"), &n1);
    assert_eq!(
        changed_lines(&text, &doc.to_xml()),
        pairs(&[(
            r#"<y:Geometry height="50" width="50.0" x="300.0" y="45.5"/>"#,
            r#"<y:Geometry height="50" width="50.0" x="312.5" y="45.5"/>"#
        )])
    );
}

#[test]
fn edits_change_only_the_attributes_involved() {
    let (mut doc, keys) = fixture();
    let mut n1 = node_graphics(&mut doc, &keys, "n1");
    n1.kind = NodeKind::Shape(ShapeType::Diamond);
    n1.fill.color = Some(Color::WHITE);
    n1.border.color = None;
    n1.labels[0].text = "Store".into();
    n1.labels[0].font.size = 16;
    keys.set_node_graphics(node(&mut doc, "n1"), &n1);

    let mut e0 = edge_graphics(&mut doc, &keys, "e0");
    e0.path.points.pop();
    e0.arrows.source = Arrow::Diamond;
    keys.set_edge_graphics(edge(&mut doc, "e0"), &e0);

    let text = doc.to_xml();
    let expected_label = r##"<y:NodeLabel alignment="center" autoSizePolicy="content" fontFamily="Dialog" fontSize="16" fontStyle="bold" hasBackgroundColor="false" hasLineColor="false" height="20.296875" horizontalTextPosition="center" iconTextGap="4" modelName="custom" textColor="#000000" verticalTextPosition="bottom" visible="true" width="24.0" x="13.0" y="14.8515625">Store<y:LabelModel>"##;
    assert!(text.contains(r##"<y:Fill color="#FFFFFF" transparent="false"/>"##));
    assert!(
        text.contains(
            r#"<y:BorderStyle hasColor="false" raised="false" type="dashed" width="2.0"/>"#
        )
    );
    assert!(text.contains(expected_label));
    assert!(text.contains(r#"<y:Shape type="diamond"/>"#));
    assert!(text.contains(r#"<y:Arrows source="diamond" target="standard"/>"#));
    assert!(!text.contains(r#"<y:Point x="325.0" y="20.0"/>"#));
    assert_eq!(text.lines().count(), YED.lines().count() - 1);
}

#[test]
fn labels_can_be_added_and_removed() {
    let (mut doc, keys) = fixture();
    let mut n3 = node_graphics(&mut doc, &keys, "n3");
    n3.labels[0].text = " cloud ".into();
    n3.labels.push(Label::new("second"));
    keys.set_node_graphics(node(&mut doc, "n3"), &n3);
    let mut n4 = node_graphics(&mut doc, &keys, "n4");
    n4.labels.clear();
    keys.set_node_graphics(node(&mut doc, "n4"), &n4);

    let mut doc = GraphMl::parse(&doc.to_xml()).unwrap();
    let n3 = node_graphics(&mut doc, &keys, "n3");
    let texts: Vec<_> = n3.labels.iter().map(|label| label.text.as_str()).collect();
    assert_eq!(texts, [" cloud ", "second"]);
    assert!(node_graphics(&mut doc, &keys, "n4").labels.is_empty());

    let text = doc.to_xml();
    // Whitespace around the text is kept, and the empty-label marker removed.
    assert!(text.contains(
        r##"<y:NodeLabel alignment="center" autoSizePolicy="content" fontFamily="Dialog" fontSize="12" fontStyle="plain" hasBackgroundColor="false" hasLineColor="false" height="4.0" horizontalTextPosition="center" iconTextGap="4" modelName="custom" textColor="#000000" verticalTextPosition="bottom" visible="true" width="4.0" x="18.0" xml:space="preserve" y="44.0"> cloud <y:LabelModel>"##
    ));
    assert!(text.contains(concat!(
        r##"<y:NodeLabel alignment="center" autoSizePolicy="content" fontFamily="Dialog" fontSize="12" fontStyle="plain" hasBackgroundColor="false" hasLineColor="false" height="0.0" horizontalTextPosition="center" iconTextGap="4" modelName="internal" modelPosition="c" textColor="#000000" verticalTextPosition="bottom" visible="true" width="0.0" x="0.0" y="0.0">second</y:NodeLabel>"##,
        "\n          <y:SVGNodeProperties"
    )));
    assert!(text.contains(
        "<y:BorderStyle color=\"#000000\" type=\"line\" width=\"1.0\"/>\n          <y:UML "
    ));
}

/// The element of `n1`'s label, which is in the middle of the node.
fn n1_label(doc: &mut GraphMl) -> &mut Element {
    let realizer = node(doc, "n1").data[0].element_mut().unwrap();
    realizer.child_mut("NodeLabel").unwrap()
}

#[test]
fn emptied_labels_are_marked_like_yeds() {
    let (mut doc, keys) = fixture();
    let mut n1 = node_graphics(&mut doc, &keys, "n1");
    n1.labels[0].text.clear();
    keys.set_node_graphics(node(&mut doc, "n1"), &n1);
    let label = n1_label(&mut doc);
    assert_eq!(
        (label.attr("hasText"), label.text()),
        (Some("false"), String::new())
    );

    let mut n1 = node_graphics(&mut doc, &keys, "n1");
    n1.labels[0].text = "DB".into();
    keys.set_node_graphics(node(&mut doc, "n1"), &n1);
    assert_eq!(doc.to_xml(), YED);
}

#[test]
fn label_frames_distances_and_rotations_are_read_and_kept() {
    let (mut doc, keys) = fixture();
    let label = n1_label(&mut doc);
    label.attributes.remove("hasLineColor");
    label.set_attr("lineColor", "#FF0000");
    let model = label.child_mut("LabelModel").unwrap();
    model
        .child_mut("SmartNodeLabelModel")
        .unwrap()
        .set_attr("distance", "8.0");
    let n1 = node_graphics(&mut doc, &keys, "n1");
    let mut label = n1.labels[0].clone();
    assert_eq!(label.border, Some(Color::rgb(0xFF, 0x00, 0x00)));
    // The model's fixed positions keep its distance from the node.
    label.set_position(LabelPosition::North);
    let placed = label.placed(&n1.geometry, 24.0, 20.0).unwrap();
    assert_eq!(placed.y, -28.0);

    let parameter = n1_label(&mut doc)
        .child_mut("ModelParameter")
        .unwrap()
        .child_mut("SmartNodeLabelModelParameter")
        .unwrap();
    parameter.set_attr("upX", "1.0");
    parameter.set_attr("upY", "0.0");
    let before = doc.to_xml();
    let mut n1 = node_graphics(&mut doc, &keys, "n1");
    n1.geometry.x += 12.5;
    keys.set_node_graphics(node(&mut doc, "n1"), &n1);
    assert_eq!(
        changed_lines(&before, &doc.to_xml()),
        pairs(&[(
            r#"<y:Geometry height="50.0" width="50.0" x="300.0" y="45.5"/>"#,
            r#"<y:Geometry height="50.0" width="50.0" x="312.5" y="45.5"/>"#
        )])
    );
}

#[test]
fn reads_where_node_labels_are() {
    let (mut doc, keys) = fixture();
    for (id, position) in [
        ("n0", LabelPosition::Center),
        ("n3", LabelPosition::South),
        ("n5", LabelPosition::Top),
    ] {
        let label = &node_graphics(&mut doc, &keys, id).labels[0];
        assert_eq!(label.position(), Some(position), "{id}");
    }
    assert_eq!(Label::new("New").position(), Some(LabelPosition::Center));
    // Edge labels have no node positions.
    let e0 = edge_graphics(&mut doc, &keys, "e0");
    assert_eq!(e0.labels[0].position(), None);
}

#[test]
fn moving_a_label_within_its_model_changes_only_its_position() {
    let (mut doc, keys) = fixture();
    let mut n1 = node_graphics(&mut doc, &keys, "n1");
    n1.labels[0].set_position(LabelPosition::North);
    keys.set_node_graphics(node(&mut doc, "n1"), &n1);
    let mut n5 = node_graphics(&mut doc, &keys, "n5");
    n5.labels[0].set_position(LabelPosition::BottomRight);
    keys.set_node_graphics(node(&mut doc, "n5"), &n5);

    let text = doc.to_xml();
    let changed = changed_lines(YED, &text);
    let [(before, after), (group_before, group_after)] = &changed[..] else {
        panic!("expected two lines to change: {changed:?}");
    };
    assert_eq!(
        (before.as_str(), after.as_str()),
        (
            r#"<y:SmartNodeLabelModelParameter labelRatioX="0.0" labelRatioY="0.0" nodeRatioX="0.0" nodeRatioY="0.0" offsetX="0.0" offsetY="0.0" upX="0.0" upY="-1.0"/>"#,
            r#"<y:SmartNodeLabelModelParameter labelRatioX="0.0" labelRatioY="0.5" nodeRatioX="0.0" nodeRatioY="-0.5" offsetX="0.0" offsetY="-4.0" upX="0.0" upY="-1.0"/>"#,
        )
    );
    assert_eq!(
        *group_after,
        group_before.replace(r#"modelPosition="t""#, r#"modelPosition="br""#)
    );
    let mut doc = GraphMl::parse(&text).unwrap();
    let position = |doc: &mut GraphMl, id| node_graphics(doc, &keys, id).labels[0].position();
    assert_eq!(position(&mut doc, "n1"), Some(LabelPosition::North));
    assert_eq!(position(&mut doc, "n5"), Some(LabelPosition::BottomRight));
}

#[test]
fn positions_outside_a_labels_model_switch_it_to_smart() {
    let mut label = Label::new("Box");
    label.set_position(LabelPosition::Top);
    let element = |label: &Label| {
        let mut graphics = NodeGraphics::new(NodeKind::Shape(ShapeType::Rectangle));
        graphics.labels.push(label.clone());
        let mut doc = new_document();
        let keys = Keys::ensure(&mut doc);
        let mut node = Node::new("n0");
        keys.set_node_graphics(&mut node, &graphics);
        doc.graphs[0].nodes.push(node);
        let text = doc.to_xml();
        let start = text.find("<y:NodeLabel").unwrap();
        let end = text.find("</y:NodeLabel>").unwrap();
        let element = text[start..end].to_owned();
        let doc = GraphMl::parse(&text).unwrap();
        let graphics = keys
            .node_graphics(&doc.graphs[0].nodes[0])
            .unwrap()
            .unwrap();
        (element, graphics.labels[0].position())
    };
    let (internal, read) = element(&label);
    assert!(internal.contains(r#"modelName="internal" modelPosition="t""#));
    assert_eq!(read, Some(LabelPosition::Top));

    label.set_position(LabelPosition::SouthWest);
    let (smart, read) = element(&label);
    assert_eq!(
        smart,
        r##"<y:NodeLabel alignment="center" autoSizePolicy="content" fontFamily="Dialog" fontSize="12" fontStyle="plain" hasBackgroundColor="false" hasLineColor="false" height="0.0" horizontalTextPosition="center" iconTextGap="4" modelName="custom" textColor="#000000" verticalTextPosition="bottom" visible="true" width="0.0" x="0.0" y="0.0">Box<y:LabelModel>
              <y:SmartNodeLabelModel distance="4.0"/>
            </y:LabelModel>
            <y:ModelParameter>
              <y:SmartNodeLabelModelParameter labelRatioX="0.5" labelRatioY="-0.5" nodeRatioX="-0.5" nodeRatioY="0.5" offsetX="-4.0" offsetY="4.0" upX="0.0" upY="-1.0"/>
            </y:ModelParameter>
          "##
    );
    assert_eq!(read, Some(LabelPosition::SouthWest));
}

#[test]
fn fixed_label_models_have_their_own_positions() {
    use LabelPosition::*;
    let fixed = |model: &str, position: &str| {
        let label = Element::new("y:NodeLabel")
            .with_attr("modelName", model)
            .with_attr("modelPosition", position);
        let node = Element::new("y:ShapeNode").with_child(label);
        NodeGraphics::from_element(&node).unwrap().labels.remove(0)
    };
    // The model a label is written in, and where it reads back.
    let rewritten = |label: &Label| {
        let mut graphics = NodeGraphics::new(NodeKind::Shape(ShapeType::Rectangle));
        graphics.labels.push(label.clone());
        let element = graphics.to_element();
        let model = element.child("NodeLabel").unwrap().attr("modelName");
        let read = NodeGraphics::from_element(&element).unwrap();
        (model.unwrap().to_owned(), read.labels[0].position())
    };
    // Each model, two positions it has, and one it hasn't.
    for (model, (keyword, position), next, (outside_keyword, outside)) in [
        ("internal", ("c", Center), BottomRight, ("n", North)),
        ("sandwich", ("s", South), North, ("nw", NorthWest)),
        ("sides", ("e", East), West, ("se", SouthEast)),
        ("corners", ("nw", NorthWest), SouthEast, ("n", North)),
        ("eight_pos", ("ne", NorthEast), South, ("c", Center)),
    ] {
        let mut label = fixed(model, keyword);
        assert_eq!(label.position(), Some(position), "{model}");
        assert_eq!(fixed(model, outside_keyword).position(), None, "{model}");
        label.set_position(next);
        assert_eq!(rewritten(&label), (model.to_owned(), Some(next)));
        label.set_position(outside);
        assert_eq!(rewritten(&label), ("custom".to_owned(), Some(outside)));
    }
}

#[test]
fn other_placements_are_kept_until_a_position_is_set() {
    let (mut doc, keys) = fixture();
    let label = n1_label(&mut doc);
    label.set_attr("modelName", "free");
    label.set_attr("modelPosition", "anywhere");
    let before = doc.clone();
    let mut n1 = node_graphics(&mut doc, &keys, "n1");
    assert_eq!(n1.labels[0].position(), None);
    assert_eq!(n1.labels[0].placed(&n1.geometry, 24.0, 20.0), None);
    keys.set_node_graphics(node(&mut doc, "n1"), &n1);
    assert_eq!(doc, before);

    n1.labels[0].set_position(LabelPosition::Center);
    keys.set_node_graphics(node(&mut doc, "n1"), &n1);
    let label = n1_label(&mut doc);
    assert_eq!(
        (label.attr("modelName"), label.attr("modelPosition")),
        (Some("custom"), None)
    );
    let n1 = node_graphics(&mut doc, &keys, "n1");
    assert_eq!(n1.labels[0].position(), Some(LabelPosition::Center));
}

#[test]
fn placed_labels_get_the_bounds_yed_gives_them() {
    let (mut doc, keys) = fixture();
    for id in ["n0", "n1", "n3"] {
        let graphics = node_graphics(&mut doc, &keys, id);
        let label = &graphics.labels[0];
        let Rect { width, height, .. } = label.bounds;
        let placed = label.placed(&graphics.geometry, width, height);
        assert_eq!(placed, Some(label.bounds), "{id}");
    }

    let node = Rect {
        x: 50.0,
        y: 50.0,
        width: 100.0,
        height: 60.0,
    };
    let mut label = Label::new("Label");
    for (position, x, y) in [
        (LabelPosition::Center, 30.0, 20.0),
        (LabelPosition::TopLeft, 4.0, 4.0),
        (LabelPosition::Bottom, 30.0, 36.0),
        (LabelPosition::Right, 56.0, 20.0),
        (LabelPosition::North, 30.0, -24.0),
        (LabelPosition::SouthEast, 104.0, 64.0),
        (LabelPosition::West, -44.0, 20.0),
    ] {
        label.set_position(position);
        let placed = label.placed(&node, 40.0, 20.0);
        let expected = Rect {
            x,
            y,
            width: 40.0,
            height: 20.0,
        };
        assert_eq!(placed, Some(expected), "{position:?}");
    }
}

#[test]
fn labels_dragged_elsewhere_are_placed_but_have_no_position() {
    let (mut doc, keys) = fixture();
    let parameter = n1_label(&mut doc)
        .child_mut("ModelParameter")
        .unwrap()
        .child_mut("SmartNodeLabelModelParameter")
        .unwrap();
    parameter.set_attr("nodeRatioX", "0.5");
    parameter.set_attr("offsetX", "10.0");
    let n1 = node_graphics(&mut doc, &keys, "n1");
    let label = &n1.labels[0];
    assert_eq!(label.position(), None);
    // Its middle is 10 right of the node's right side.
    let placed = label.placed(&n1.geometry, 24.0, 20.0).unwrap();
    assert_eq!((placed.x, placed.y), (48.0, 15.0));
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn point(x: f64, y: f64) -> Point {
    Point { x, y }
}

/// A route from the right side of a 40×20 node at the origin to the left
/// side of one at x 200: from (40, 10) to (200, 10).
fn straight_route() -> Route {
    Route::new(
        &Path::default(),
        rect(0.0, 0.0, 40.0, 20.0),
        rect(200.0, 0.0, 40.0, 20.0),
    )
}

/// An edge label 20×10 in yEd's Smart model with `parameter`.
fn smart_label(parameter: &[(&str, &str)]) -> Label {
    let parameter = parameter.iter().fold(
        Element::new("y:SmartEdgeLabelModelParameter"),
        |element, &(name, value)| element.with_attr(name, value),
    );
    let label = Element::new("y:EdgeLabel")
        .with_attr("modelName", "custom")
        .with_attr("width", "20.0")
        .with_attr("height", "10.0")
        .with_child(Element::new("y:LabelModel").with_child(Element::new("y:SmartEdgeLabelModel")))
        .with_child(Element::new("y:ModelParameter").with_child(parameter));
    let edge = Element::new("y:PolyLineEdge").with_child(label);
    EdgeGraphics::from_element(&edge).unwrap().labels.remove(0)
}

#[test]
fn routes_run_between_the_bounds_of_their_nodes() {
    assert_eq!(
        straight_route().points,
        [point(40.0, 10.0), point(200.0, 10.0)]
    );

    let (source, target) = (rect(0.0, 0.0, 40.0, 20.0), rect(200.0, 90.0, 40.0, 20.0));
    let path = Path {
        source: point(10.0, 0.0),
        target: Point::default(),
        // The first bend is inside the source, so the route leaves it later.
        points: vec![point(35.0, 5.0), point(35.0, 100.0)],
    };
    assert_eq!(
        Route::new(&path, source, target).points,
        [point(35.0, 20.0), point(35.0, 100.0), point(200.0, 100.0)]
    );

    // A target inside its source leaves no route in between.
    let inside = rect(10.0, 5.0, 20.0, 10.0);
    assert_eq!(
        Route::new(&Path::default(), source, inside).points,
        [point(20.0, 10.0), point(20.0, 10.0)]
    );
}

#[test]
fn edge_labels_are_placed_by_yeds_smart_model() {
    let route = straight_route();
    for (parameter, x, y) in [
        // Between the nodes, keeping 5 points from each.
        (&[("position", "center"), ("ratio", "0.5")][..], 110.0, 5.0),
        (&[("position", "center"), ("ratio", "0.0")], 45.0, 5.0),
        (&[("position", "center"), ("ratio", "1.0")], 175.0, 5.0),
        // Ratios outside 0..=1 are points before or after those.
        (&[("position", "center"), ("ratio", "-10.0")], 35.0, 5.0),
        (&[("position", "center"), ("ratio", "12.0")], 187.0, 5.0),
        // Labels clear of the nodes needn't keep away from them.
        (
            &[("position", "left"), ("distance", "30.0"), ("ratio", "0.0")],
            30.0,
            -25.0,
        ),
        (
            &[
                ("position", "right"),
                ("distance", "5.0"),
                ("distanceToCenter", "false"),
                ("ratio", "1.0"),
            ],
            175.0,
            15.0,
        ),
    ] {
        let label = smart_label(parameter);
        let placed = label.placed_on(&route, 20.0, 10.0);
        assert_eq!(placed, Some(rect(x, y, 20.0, 10.0)), "{parameter:?}");
    }

    // Negative segments count from the target. Ones past either end, as a
    // bend inside a node leaves, are the end ones.
    let bent = Route::new(
        &Path {
            points: vec![point(20.0, 100.0)],
            ..Path::default()
        },
        rect(0.0, 0.0, 40.0, 20.0),
        rect(200.0, 90.0, 40.0, 20.0),
    );
    for (segment, x, y) in [("-1", 92.5, 95.0), ("2", 92.5, 95.0), ("-5", 10.0, 60.0)] {
        let parameter = [
            ("position", "center"),
            ("ratio", "0.5"),
            ("segment", segment),
        ];
        let label = smart_label(&parameter);
        let placed = label.placed_on(&bent, 20.0, 10.0);
        assert_eq!(placed, Some(rect(x, y, 20.0, 10.0)), "{segment}");
    }
    // Without room to keep clear of both nodes, labels go halfway.
    let short = Route::new(
        &Path::default(),
        rect(0.0, 0.0, 40.0, 20.0),
        rect(60.0, 0.0, 40.0, 20.0),
    );
    for ratio in ["0.0", "1.0"] {
        let label = smart_label(&[("position", "center"), ("ratio", ratio)]);
        let placed = label.placed_on(&short, 20.0, 10.0);
        assert_eq!(placed, Some(rect(40.0, 5.0, 20.0, 10.0)), "{ratio}");
    }
    // Turned labels and other models aren't placed.
    let turned = smart_label(&[("angle", "1.0")]);
    assert_eq!(turned.placed_on(&route, 20.0, 10.0), None);
    let six_pos = Element::new("y:EdgeLabel")
        .with_attr("modelName", "six_pos")
        .with_attr("modelPosition", "tail");
    let polyline = Element::new("y:PolyLineEdge").with_child(six_pos);
    let graphics = EdgeGraphics::from_element(&polyline).unwrap();
    assert_eq!(graphics.labels[0].placed_on(&route, 20.0, 10.0), None);
    // Nor are they moved to the Smart model when written.
    let written = graphics.to_element();
    let label = written.child("EdgeLabel").unwrap();
    assert_eq!(
        (
            label.attr("modelName"),
            label.attr("modelPosition"),
            label.child("LabelModel")
        ),
        (Some("six_pos"), Some("tail"), None)
    );
    // Node labels aren't placed on edges.
    let (mut doc, keys) = fixture();
    let n1 = node_graphics(&mut doc, &keys, "n1");
    assert_eq!(n1.labels[0].placed_on(&route, 20.0, 10.0), None);
}

#[test]
fn edge_labels_go_where_they_are_put() {
    let route = straight_route();
    let mut label = Label::new_on_edge("Label");
    assert_eq!(
        label.placed_on(&route, 20.0, 10.0),
        Some(rect(110.0, 5.0, 20.0, 10.0))
    );
    for center in [
        point(120.0, 10.0),
        point(60.0, -30.0),
        point(150.0, 40.0),
        point(10.0, 10.0),
        point(230.0, 50.0),
    ] {
        label.place_on(&route, center, 20.0, 10.0);
        let placed = label.placed_on(&route, 20.0, 10.0).unwrap();
        let moved = point(placed.x + 10.0, placed.y + 5.0);
        assert!(
            (moved.x - center.x).abs() < 1e-9 && (moved.y - center.y).abs() < 1e-9,
            "{center:?} went to {moved:?}"
        );
        // Relative to where the route starts.
        assert_eq!(
            label.bounds,
            rect(center.x - 50.0, center.y - 15.0, 20.0, 10.0)
        );
    }
    // Labels put nearly on their edge go on it.
    label.place_on(&route, point(80.0, 10.4), 20.0, 10.0);
    let placed = label.placed_on(&route, 20.0, 10.0).unwrap();
    assert_eq!((placed.x, placed.y), (70.0, 5.0));
    // And ones put less than a point past the end go to the end.
    label.place_on(&route, point(185.3, 10.0), 20.0, 10.0);
    let placed = label.placed_on(&route, 20.0, 10.0).unwrap();
    assert_eq!((placed.x, placed.y), (175.0, 5.0));

    // Labels whose distance was to their side, not center, go there too.
    let parameter = [
        ("position", "left"),
        ("distance", "5.0"),
        ("distanceToCenter", "false"),
    ];
    let mut label = smart_label(&parameter);
    label.place_on(&route, point(120.0, -20.0), 20.0, 10.0);
    let placed = label.placed_on(&route, 20.0, 10.0).unwrap();
    assert_eq!((placed.x, placed.y), (110.0, -25.0));
}

#[test]
fn moving_an_edge_label_changes_only_its_bounds_and_parameter() {
    let (mut doc, keys) = fixture();
    let n0 = node_graphics(&mut doc, &keys, "n0").geometry;
    let n1 = node_graphics(&mut doc, &keys, "n1").geometry;
    let mut e0 = edge_graphics(&mut doc, &keys, "e0");
    let route = Route::new(&e0.path, n0, n1);
    let Rect { width, height, .. } = e0.labels[0].bounds;
    // Left of the middle of the segment between the bends, along y 20.
    e0.labels[0].place_on(&route, point(292.5, 0.0), width, height);
    keys.set_edge_graphics(edge(&mut doc, "e0"), &e0);

    let changed = changed_lines(YED, &doc.to_xml());
    let [(before, after)] = &changed[..] else {
        panic!("expected one line to change: {changed:?}");
    };
    let expected = before
        .replace(
            r#"x="22.5""#,
            &format!(r#"x="{:?}""#, 275.0 - route.points[0].x),
        )
        .replace(
            r#"y="-25.0""#,
            &format!(r#"y="{:?}""#, -8.984375 - route.points[0].y),
        )
        .replace(r#"position="right""#, r#"position="left""#)
        .replace(
            r#"distance="30.0" distanceToCenter"#,
            r#"distance="20.0" distanceToCenter"#,
        )
        .replace(r#"segment="0""#, r#"segment="1""#);
    assert_eq!(*after, expected);
}

#[test]
fn removing_a_bend_keeps_labels_on_their_segments() {
    let (source, target) = (rect(0.0, 0.0, 40.0, 20.0), rect(200.0, 90.0, 40.0, 20.0));
    let route = |bends: Vec<Point>| {
        let path = Path {
            points: bends,
            ..Path::default()
        };
        Route::new(&path, source, target)
    };
    let three = route(vec![point(100.0, 10.0), point(100.0, 100.0)]);
    // Without the first bend, and without the second.
    let two = [
        route(vec![point(100.0, 100.0)]),
        route(vec![point(100.0, 10.0)]),
    ];
    // Whether the center of `label` is on the segment at `index` of `route`.
    let on = |route: &Route, index: usize, label: &Label| {
        let placed = label.placed_on(route, 20.0, 10.0).unwrap();
        let center = point(placed.x + 10.0, placed.y + 5.0);
        let (from, to) = (route.points[index], route.points[index + 1]);
        let (dx, dy) = (to.x - from.x, to.y - from.y);
        let across = ((center.x - from.x) * dy - (center.y - from.y) * dx) / dx.hypot(dy);
        let along = ((center.x - from.x) * dx + (center.y - from.y) * dy) / dx.hypot(dy);
        across.abs() < 1e-9 && (0.0..=dx.hypot(dy)).contains(&along)
    };
    for (segment, before, after) in [
        ("0", 0, [0, 0]),
        ("1", 1, [0, 1]),
        ("2", 2, [1, 1]),
        ("-1", 2, [1, 1]),
    ] {
        let parameter = [
            ("segment", segment),
            ("position", "center"),
            ("ratio", "0.5"),
        ];
        let label = smart_label(&parameter);
        assert!(on(&three, before, &label), "{segment}");
        for bend in 0..2 {
            let mut label = label.clone();
            label.remove_bend(bend, 3);
            assert!(on(&two[bend], after[bend], &label), "{segment} {bend}");
        }
    }
}

#[test]
fn group_nodes_edit_the_realizer_in_use() {
    let (mut doc, keys) = fixture();
    let mut n5 = node_graphics(&mut doc, &keys, "n5");
    n5.geometry.width = 240.0;
    keys.set_node_graphics(node(&mut doc, "n5"), &n5);
    assert_eq!(
        changed_lines(YED, &doc.to_xml()),
        pairs(&[(
            r#"<y:Geometry height="120.0" width="200.0" x="80.0" y="320.0"/>"#,
            r#"<y:Geometry height="120.0" width="240.0" x="80.0" y="320.0"/>"#
        )])
    );

    // Closed, it uses its second realizer.
    let closed = YED.replace(r#"<y:Realizers active="0">"#, r#"<y:Realizers active="1">"#);
    let mut doc = GraphMl::parse(&closed).unwrap();
    let mut n5 = node_graphics(&mut doc, &keys, "n5");
    assert_eq!(n5.geometry, rect(80.0, 320.0, 50.0, 50.0));
    n5.geometry.width = 60.0;
    keys.set_node_graphics(node(&mut doc, "n5"), &n5);
    assert_eq!(
        changed_lines(&closed, &doc.to_xml()),
        pairs(&[(
            r#"<y:Geometry height="50.0" width="50.0" x="80.0" y="320.0"/>"#,
            r#"<y:Geometry height="50.0" width="60.0" x="80.0" y="320.0"/>"#
        )])
    );
}

#[test]
fn changing_the_kind_rebuilds_the_element() {
    let (mut doc, keys) = fixture();
    let mut n1 = node_graphics(&mut doc, &keys, "n1");
    n1.kind = NodeKind::Generic("com.yworks.flowchart.process".into());
    keys.set_node_graphics(node(&mut doc, "n1"), &n1);
    let read = node_graphics(&mut doc, &keys, "n1");
    assert_eq!(read.kind, n1.kind);
    assert_eq!(
        (read.geometry, &read.fill, &read.border),
        (n1.geometry, &n1.fill, &n1.border)
    );
    assert_eq!(read.labels[0].text, "DB");
    let data = node(&mut doc, "n1").data[0].element().unwrap();
    assert_eq!(data.name, "y:GenericNode");
    assert!(data.child("Shape").is_none());
}

#[test]
fn generic_edges_keep_their_configuration() {
    let (mut doc, keys) = fixture();
    let mut e0 = edge_graphics(&mut doc, &keys, "e0");
    e0.kind = EdgeKind::Generic("com.yworks.edge.framed".into());
    keys.set_edge_graphics(edge(&mut doc, "e0"), &e0);
    let data = edge(&mut doc, "e0").data[0].element().unwrap();
    assert_eq!(
        (data.name.as_str(), data.attr("configuration")),
        ("y:GenericEdge", Some("com.yworks.edge.framed"))
    );
    assert_eq!(edge_graphics(&mut doc, &keys, "e0").kind, e0.kind);
}

#[test]
fn unknown_keywords_are_kept() {
    let (mut doc, keys) = fixture();
    let realizer = node(&mut doc, "n1").data[0].element_mut().unwrap();
    realizer
        .child_mut("Shape")
        .unwrap()
        .set_attr("type", "star5");
    let n1 = node_graphics(&mut doc, &keys, "n1");
    assert_eq!(n1.kind, NodeKind::Shape(ShapeType::Other("star5".into())));
    let before = doc.clone();
    keys.set_node_graphics(node(&mut doc, "n1"), &n1);
    assert_eq!(doc, before);
}

#[test]
fn malformed_values_are_errors() {
    let (mut doc, keys) = fixture();
    let realizer = node(&mut doc, "n0").data[1].element_mut().unwrap();
    realizer
        .child_mut("Geometry")
        .unwrap()
        .set_attr("width", "wide");
    match keys.node_graphics(node(&mut doc, "n0")) {
        Err(Error::InvalidValue {
            element,
            attribute,
            value,
        }) => {
            assert_eq!(
                (element.as_str(), attribute.as_str(), value.as_str()),
                ("y:Geometry", "width", "wide")
            );
        }
        other => panic!("expected an invalid value error, got {other:?}"),
    }
}

#[test]
fn new_graphics_look_like_yeds() {
    let mut doc = new_document();
    let keys = Keys::ensure(&mut doc);
    let mut node = Node::new("n0");
    let mut graphics = NodeGraphics::new(NodeKind::Shape(ShapeType::Rectangle));
    graphics.labels.push(Label::new("Box"));
    keys.set_node_graphics(&mut node, &graphics);
    let mut edge = Edge::new("n0", "n0");
    edge.id = Some("e0".into());
    keys.set_edge_graphics(&mut edge, &EdgeGraphics::new(EdgeKind::PolyLine));
    doc.graphs[0].nodes.push(node);
    doc.graphs[0].edges.push(edge);

    let header_end = YED.find("  <graph ").unwrap();
    let header = YED[..header_end].replace("  <!--Created by yEd 3.23.2-->\n", "");
    let expected = header
        + r##"  <graph edgedefault="directed" id="G">
    <node id="n0">
      <data key="d6">
        <y:ShapeNode>
          <y:Geometry height="30.0" width="30.0" x="0.0" y="0.0"/>
          <y:Fill color="#FFCC00" transparent="false"/>
          <y:BorderStyle color="#000000" raised="false" type="line" width="1.0"/>
          <y:NodeLabel alignment="center" autoSizePolicy="content" fontFamily="Dialog" fontSize="12" fontStyle="plain" hasBackgroundColor="false" hasLineColor="false" height="0.0" horizontalTextPosition="center" iconTextGap="4" modelName="internal" modelPosition="c" textColor="#000000" verticalTextPosition="bottom" visible="true" width="0.0" x="0.0" y="0.0">Box</y:NodeLabel>
          <y:Shape type="rectangle"/>
        </y:ShapeNode>
      </data>
    </node>
    <edge id="e0" source="n0" target="n0">
      <data key="d10">
        <y:PolyLineEdge>
          <y:Path sx="0.0" sy="0.0" tx="0.0" ty="0.0"/>
          <y:LineStyle color="#000000" type="line" width="1.0"/>
          <y:Arrows source="none" target="standard"/>
          <y:BendStyle smoothed="false"/>
        </y:PolyLineEdge>
      </data>
    </edge>
  </graph>
  <data key="d7">
    <y:Resources/>
  </data>
</graphml>
"##;
    assert_eq!(doc.to_xml(), expected);
}

#[test]
fn new_svg_nodes_look_like_yeds() {
    let mut doc = new_document();
    let keys = Keys::ensure(&mut doc);
    let resource = add_resource(&mut doc, "<svg/>");
    let mut node = Node::new("n0");
    let mut graphics = NodeGraphics::new(NodeKind::Svg {
        resource: Some(resource),
    });
    let mut label = Label::new("Icon");
    label.set_position(LabelPosition::South);
    graphics.labels.push(label);
    keys.set_node_graphics(&mut node, &graphics);
    doc.graphs[0].nodes.push(node);

    let text = doc.to_xml();
    let expected = r##"
        <y:SVGNode>
          <y:Geometry height="30.0" width="30.0" x="0.0" y="0.0"/>
          <y:Fill color="#CCCCFF" transparent="false"/>
          <y:BorderStyle color="#000000" type="line" width="1.0"/>
          <y:NodeLabel alignment="center" autoSizePolicy="content" fontFamily="Dialog" fontSize="12" fontStyle="plain" hasBackgroundColor="false" hasLineColor="false" height="0.0" horizontalTextPosition="center" iconTextGap="4" modelName="custom" textColor="#000000" verticalTextPosition="bottom" visible="true" width="0.0" x="0.0" y="0.0">Icon<y:LabelModel>
              <y:SmartNodeLabelModel distance="4.0"/>
            </y:LabelModel>
            <y:ModelParameter>
              <y:SmartNodeLabelModelParameter labelRatioX="0.0" labelRatioY="-0.5" nodeRatioX="0.0" nodeRatioY="0.5" offsetX="0.0" offsetY="4.0" upX="0.0" upY="-1.0"/>
            </y:ModelParameter>
          </y:NodeLabel>
          <y:SVGNodeProperties usingVisualBounds="true"/>
          <y:SVGModel svgBoundsPolicy="0">
            <y:SVGContent refid="1"/>
          </y:SVGModel>
        </y:SVGNode>
"##;
    assert!(text.contains(expected), "{text}");
    assert!(text.contains(
        r#"
    <y:Resources>
      <y:Resource id="1">&lt;svg/&gt;</y:Resource>
    </y:Resources>"#
    ));
}

#[test]
fn svg_nodes_change_only_the_resource_they_draw() {
    let (mut doc, keys) = fixture();
    let mut n3 = node_graphics(&mut doc, &keys, "n3");
    n3.kind = NodeKind::Svg {
        resource: Some("2".into()),
    };
    keys.set_node_graphics(node(&mut doc, "n3"), &n3);
    assert_eq!(
        changed_lines(YED, &doc.to_xml()),
        pairs(&[(
            r#"<y:SVGContent refid="1"/>"#,
            r#"<y:SVGContent refid="2"/>"#
        )])
    );
}

#[test]
fn resources_are_shared_by_their_text() {
    let (mut doc, _) = fixture();
    let circle = resource_text(&doc, "1").unwrap();
    assert!(circle.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<svg "));
    assert_eq!(add_resource(&mut doc, &circle), "1");
    assert_eq!(doc.to_xml(), YED);

    let square = "<svg>\r\n  <rect/>\r\n</svg>";
    assert_eq!(add_resource(&mut doc, square), "2");
    assert_eq!(add_resource(&mut doc, square), "2");
    assert_eq!(resource_text(&doc, "2").as_deref(), Some(square));
    assert_eq!(resource_text(&doc, "3"), None);
    // Escaped as yEd escapes them, so the line ends stay as they were.
    assert!(doc.to_xml().contains(
        r#"<y:Resource id="2">&lt;svg&gt;&#13;
  &lt;rect/&gt;&#13;
&lt;/svg&gt;</y:Resource>"#
    ));
    let doc = GraphMl::parse(&doc.to_xml()).unwrap();
    assert_eq!(resource_text(&doc, "2").as_deref(), Some(square));
}

#[test]
fn new_edge_labels_look_like_yeds() {
    let mut doc = new_document();
    let keys = Keys::ensure(&mut doc);
    let mut edge = Edge::new("n0", "n1");
    let mut graphics = EdgeGraphics::new(EdgeKind::PolyLine);
    graphics.labels.push(Label::new_on_edge("Label"));
    keys.set_edge_graphics(&mut edge, &graphics);
    doc.graphs[0].edges.push(edge);

    let text = doc.to_xml();
    let start = text.find("          <y:EdgeLabel").unwrap();
    let end = text.find("          <y:BendStyle").unwrap();
    assert_eq!(
        &text[start..end],
        r##"          <y:EdgeLabel alignment="center" configuration="AutoFlippingLabel" distance="2.0" fontFamily="Dialog" fontSize="12" fontStyle="plain" hasBackgroundColor="false" hasLineColor="false" height="0.0" horizontalTextPosition="center" iconTextGap="4" modelName="custom" preferredPlacement="anywhere" ratio="0.5" textColor="#000000" verticalTextPosition="bottom" visible="true" width="0.0" x="0.0" y="0.0">Label<y:LabelModel>
              <y:SmartEdgeLabelModel autoRotationEnabled="false" defaultAngle="0.0" defaultDistance="10.0"/>
            </y:LabelModel>
            <y:ModelParameter>
              <y:SmartEdgeLabelModelParameter angle="0.0" distance="30.0" distanceToCenter="true" position="center" ratio="0.5" segment="0"/>
            </y:ModelParameter>
            <y:PreferredPlacementDescriptor angle="0.0" angleOffsetOnRightSide="0" angleReference="absolute" angleRotationOnRightSide="co" distance="-1.0" frozen="true" placement="anywhere" side="anywhere" sideReference="relative_to_edge_flow"/>
          </y:EdgeLabel>
"##
    );
    let doc = GraphMl::parse(&text).unwrap();
    let read = keys
        .edge_graphics(&doc.graphs[0].edges[0])
        .unwrap()
        .unwrap();
    assert_eq!(
        read.labels[0].placed_on(&straight_route(), 20.0, 10.0),
        Some(rect(110.0, 5.0, 20.0, 10.0))
    );
}

#[test]
fn ensure_declares_missing_keys_and_namespaces() {
    let mut doc =
        GraphMl::parse(r#"<graphml><key id="d0" for="node" attr.name="label"/><graph/></graphml>"#)
            .unwrap();
    assert_eq!(Keys::find(&doc), Keys::default());
    let keys = Keys::ensure(&mut doc);
    assert_eq!(Keys::find(&doc), keys);
    assert_eq!(doc.attributes["xmlns"], graphml::NAMESPACE);
    assert_eq!(doc.attributes["xmlns:y"], NAMESPACE);
    let declared: Vec<_> = doc
        .keys
        .iter()
        .map(|key| {
            (
                key.id.as_str(),
                key.domain,
                key.attributes.get("yfiles.type").map(String::as_str),
            )
        })
        .collect();
    assert_eq!(
        declared,
        [
            ("d0", KeyDomain::Node, None),
            ("d1", KeyDomain::Node, Some("nodegraphics")),
            ("d2", KeyDomain::Edge, Some("edgegraphics")),
        ]
    );

    // A prefix stands for GraphML's namespace already.
    let mut prefixed =
        GraphMl::parse(r#"<g:graphml xmlns:g="http://graphml.graphdrawing.org/xmlns"/>"#).unwrap();
    Keys::ensure(&mut prefixed);
    assert_eq!(prefixed.attributes.get("xmlns"), None);
    // yEd's files have everything already.
    let (mut doc, keys) = fixture();
    assert_eq!(Keys::ensure(&mut doc), keys);
    assert_eq!(doc.to_xml(), YED);
}

#[test]
fn resources_are_found_or_added_like_yeds() {
    let (mut doc, _) = fixture();
    let ids: Vec<_> = (resources(&doc).unwrap().elements())
        .filter_map(|resource| resource.attr("id"))
        .collect();
    assert_eq!(ids, ["1"]);
    resources_mut(&mut doc);
    assert_eq!(doc.to_xml(), YED);
    // yEd writes them even when there are none.
    assert_eq!(resources(&new_document()).unwrap().children, []);

    let mut doc = GraphMl::default();
    assert_eq!(resources(&doc), None);
    let resource = Element::new("y:Resource").with_attr("id", "1");
    resources_mut(&mut doc)
        .children
        .push(XmlNode::Element(resource));
    let expected = r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<graphml xmlns="http://graphml.graphdrawing.org/xmlns" xmlns:y="http://www.yworks.com/xml/graphml">
  <key for="graphml" id="d0" yfiles.type="resources"/>
  <data key="d0">
    <y:Resources>
      <y:Resource id="1"/>
    </y:Resources>
  </data>
</graphml>
"#;
    assert_eq!(doc.to_xml(), expected);
}

#[test]
fn colors_use_yeds_notation() {
    assert_eq!(Color::from_hex("#FFCC00"), Some(Color::rgb(255, 204, 0)));
    assert_eq!(
        Color::from_hex("#ffffffe6"),
        Some(Color {
            r: 255,
            g: 255,
            b: 255,
            a: 0xE6
        })
    );
    for invalid in ["FFCC00", "#FFCC0", "#+FCC00", "#GGGGGG"] {
        assert_eq!(Color::from_hex(invalid), None, "{invalid}");
    }
    assert_eq!(Color::rgb(255, 204, 0).to_string(), "#FFCC00");
    assert_eq!(
        Color {
            r: 1,
            g: 2,
            b: 3,
            a: 4
        }
        .to_string(),
        "#01020304"
    );
}

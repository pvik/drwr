use graphml::{
    Edge, Error, Graph, GraphMl, KeyDomain, Node,
    yed::{
        Alignment, Arrow, Arrows, Color, EdgeGraphics, EdgeKind, Fill, Font, FontStyle, Keys,
        Label, LineType, NAMESPACE, NodeGraphics, NodeKind, Point, Rect, ShapeType, Stroke,
        new_document,
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
    assert_eq!(n3.kind, NodeKind::Other("SVGNode".into()));
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

#[test]
fn unchanged_graphics_write_back_identically() {
    let (mut doc, keys) = fixture();
    for id in ["n0", "n1", "n2", "n3", "n4", "n5", "n5::n0"] {
        let graphics = node_graphics(&mut doc, &keys, id);
        keys.set_node_graphics(node(&mut doc, id), &graphics);
    }
    for id in ["e0", "e1"] {
        let graphics = edge_graphics(&mut doc, &keys, id);
        keys.set_edge_graphics(edge(&mut doc, id), &graphics);
    }
    assert_eq!(doc.to_xml(), YED);
}

#[test]
fn moving_a_node_changes_only_its_geometry() {
    let (mut doc, keys) = fixture();
    let mut n1 = node_graphics(&mut doc, &keys, "n1");
    n1.geometry.x += 12.5;
    keys.set_node_graphics(node(&mut doc, "n1"), &n1);
    assert_eq!(
        changed_lines(YED, &doc.to_xml()),
        pairs(&[(
            r#"<y:Geometry height="50.0" width="50.0" x="300.0" y="45.5"/>"#,
            r#"<y:Geometry height="50.0" width="50.0" x="312.5" y="45.5"/>"#
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
    assert!(
        text.contains(
            r#"width="4.0" x="18.0" xml:space="preserve" y="44.0"> cloud <y:LabelModel>"#
        )
    );
    assert!(text.contains(concat!(
        r##"<y:NodeLabel alignment="center" autoSizePolicy="content" fontFamily="Dialog" fontSize="12" fontStyle="plain" hasBackgroundColor="false" hasLineColor="false" height="0.0" horizontalTextPosition="center" iconTextGap="4" modelName="internal" modelPosition="c" textColor="#000000" verticalTextPosition="bottom" visible="true" width="0.0" x="0.0" y="0.0">second</y:NodeLabel>"##,
        "\n          <y:SVGNodeProperties"
    )));
    assert!(text.contains(
        "<y:BorderStyle color=\"#000000\" type=\"line\" width=\"1.0\"/>\n          <y:UML "
    ));
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
</graphml>
"##;
    assert_eq!(doc.to_xml(), expected);
}

#[test]
fn ensure_declares_missing_keys_and_namespace() {
    let mut doc =
        GraphMl::parse(r#"<graphml><key id="d0" for="node" attr.name="label"/><graph/></graphml>"#)
            .unwrap();
    assert_eq!(Keys::find(&doc), Keys::default());
    let keys = Keys::ensure(&mut doc);
    assert_eq!(Keys::find(&doc), keys);
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

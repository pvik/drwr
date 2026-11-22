use drawio::{Error, File, Import};
use graphml::{
    Edge, GraphMl, drwr,
    yed::{
        self, Alignment, Arrow, Color, EdgeGraphics, FontStyle, Keys, LabelPosition, LineType,
        NodeGraphics, NodeKind, Point, Rect, ShapeType,
    },
};

/// A one-page file holding `cells` in its default layer.
fn page(cells: &str) -> String {
    format!(
        r#"<mxfile><diagram name="P"><mxGraphModel><root><mxCell id="0"/><mxCell id="1" parent="0"/>{cells}</root></mxGraphModel></diagram></mxfile>"#
    )
}

/// A vertex in the default layer.
fn vertex(id: &str, value: &str, style: &str, [x, y, width, height]: [f64; 4]) -> String {
    format!(
        r#"<mxCell id="{id}" value="{value}" style="{style}" vertex="1" parent="1"><mxGeometry x="{x}" y="{y}" width="{width}" height="{height}" as="geometry"/></mxCell>"#
    )
}

/// An edge in the default layer.
fn edge(id: &str, source: &str, target: &str, style: &str, inner: &str) -> String {
    format!(
        r#"<mxCell id="{id}" style="{style}" edge="1" parent="1" source="{source}" target="{target}"><mxGeometry relative="1" as="geometry">{inner}</mxGeometry></mxCell>"#
    )
}

fn convert(text: &str) -> Import {
    File::parse(text).unwrap().pages()[0].to_graphml()
}

fn nodes(import: &Import) -> Vec<NodeGraphics> {
    let keys = Keys::find(&import.graphml);
    let nodes = &import.graphml.graphs[0].nodes;
    nodes
        .iter()
        .map(|node| keys.node_graphics(node).unwrap().unwrap())
        .collect()
}

fn edges(import: &Import) -> Vec<(Edge, EdgeGraphics)> {
    let keys = Keys::find(&import.graphml);
    let edges = &import.graphml.graphs[0].edges;
    edges
        .iter()
        .map(|edge| (edge.clone(), keys.edge_graphics(edge).unwrap().unwrap()))
        .collect()
}

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn at(x: f64, y: f64) -> Point {
    Point { x, y }
}

const ZIPPED: &str = "jZA9DgIhEIVPMz2ChZYGdSsvYEfCZCEBIYjK3l4Co+sWm1iQzPse8wtC+jIkFc0laHQgTiBkCiH3yBeJzgFnVoM4AuesPuDnFXfTXBZVwlv+J4H3hKdyD+zkamNEXRlIAYc9+ZgyltUeDVGDAYPHnKb6pfxOwKaFelmdTSc7QgbtaKjilpi6dz1+q87L1ID2+cj5bs1bnPUN";
/// The same in old files' zlib wrapping.
const ZLIB: &str = "eNqNkD0OAiEQhU8zPYKFlgZ1Ky9gR8JkIQEhiMreXgKj6xabWJDM+x7zC0L6MiQVzSVodCBOIGQKIffIF4nOAWdWgzgC56w+4OcVd9NcFlXCW/4ngfeEp3IP7ORqY0RdGUgBhz35mDKW1R4NUYMBg8ecpvql/E7ApoV6WZ1NJztCBu1oqOKWmLp3PX6rzsvUgPb5yPluzVuc9Q0/g2B+";

#[test]
fn reads_plain_compressed_and_lone_pages() {
    let text = format!(
        r#"<?xml version="1.0"?><mxfile host="x"><diagram name="Plain" id="a"><mxGraphModel><root><mxCell id="0"/></root></mxGraphModel></diagram><diagram id="b">{ZIPPED}</diagram><diagram name="Old">{ZLIB}</diagram><diagram name="Empty"></diagram></mxfile>"#
    );
    let file = File::parse(&text).unwrap();
    let names: Vec<&str> = file.pages().iter().map(|page| page.name()).collect();
    assert_eq!(names, ["Plain", "Page-2", "Old", "Empty"]);
    for page in &file.pages()[1..3] {
        let import = page.to_graphml();
        assert_eq!(nodes(&import)[0].labels[0].text, "Zipped é");
    }
    assert!(
        file.pages()[3].to_graphml().graphml.graphs[0]
            .nodes
            .is_empty()
    );

    let lone = r#"<mxGraphModel><root><mxCell id="0"/></root></mxGraphModel>"#;
    let file = File::parse(lone).unwrap();
    assert_eq!(file.pages()[0].name(), "Page-1");
}

#[test]
fn tells_draw_io_files_from_others() {
    assert!(drawio::is_drawio(
        "\u{feff}<?xml version=\"1.0\"?>\n<!-- x --><mxfile/>"
    ));
    assert!(drawio::is_drawio(
        "<mxGraphModel dx=\"1\"><root/></mxGraphModel>"
    ));
    let with_prolog = r#"<?note comparison="a > b"?><mxGraphModel><root/></mxGraphModel>"#;
    assert!(drawio::is_drawio(with_prolog));
    assert!(File::parse(with_prolog).is_ok());
    assert!(!drawio::is_drawio("<?xml version=\"1.0\"?><graphml/>"));
    assert!(!drawio::is_drawio("<svg/>"));
    assert!(!drawio::is_drawio(""));
}

#[test]
fn errors_say_what_is_wrong() {
    let message = |text: &str| File::parse(text).unwrap_err().to_string();
    assert_eq!(
        message("<graphml/>"),
        "expected <mxfile> or <mxGraphModel>, found <graphml>"
    );
    assert_eq!(message("<mxfile/>"), "the file has no pages");
    assert!(
        message("<mxfile><diagram name=\"A\">!!!</diagram></mxfile>")
            .starts_with("page \"A\" can't be decompressed: ")
    );
    assert!(matches!(File::parse("<mxfile>"), Err(Error::Xml(_))));
}

#[test]
fn malformed_page_bodies_are_errors() {
    // <metadata/> in draw.io's compressed page encoding.
    let compressed = "UzV2zk0tSUxJLElUNXJTNXYFAA==";
    let cases = [
        ("<metadata/>", "expected <mxGraphModel>, found <metadata>"),
        (compressed, "expected <mxGraphModel>, found <metadata>"),
        ("<mxGraphModel/>", "missing <root> in <mxGraphModel>"),
        (
            "<mxGraphModel><root/><root/></mxGraphModel>",
            "more than one <root> in <mxGraphModel>",
        ),
        (
            "<mxGraphModel><root/></mxGraphModel>data",
            "expected a single <mxGraphModel> or compressed text",
        ),
    ];
    for (body, expected) in cases {
        let text = format!(r#"<mxfile><diagram name="Bad">{body}</diagram></mxfile>"#);
        let err = File::parse(&text).unwrap_err();
        assert!(
            matches!(&err, Error::InvalidPage { page, problem } if page == "Bad" && problem == expected),
            "{err}"
        );
    }
}

#[test]
fn negative_dimensions_are_errors_before_conversion() {
    let cases = [
        ("rounded=1;", [0.0, 0.0, -1.0, 40.0], "width"),
        ("shape=umlLifeline;", [0.0, 0.0, 40.0, -1.0], "height"),
    ];
    for (style, bounds, dimension) in cases {
        let cells = vertex("a", "", style, bounds);
        let err = File::parse(&page(&cells)).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!(r#"invalid page "P": cell "a" has negative {dimension}"#)
        );
    }
}

#[test]
fn fixed_ends_on_zero_width_ellipses_are_finite() {
    let cells = [
        vertex("a", "", "ellipse;", [0.0, 0.0, 0.0, 40.0]),
        vertex("b", "", "", [200.0, 0.0, 80.0, 40.0]),
        edge("e", "a", "b", "exitX=0.5;exitY=1;", ""),
    ];
    let import = convert(&page(&cells.concat()));
    // The written node is 1 point wide and the port is on the original axis.
    assert_eq!(edges(&import)[0].1.path.source, at(-0.5, 20.0));
}

#[test]
fn shapes_become_the_closest_yed_shapes() {
    let styles = [
        ("", NodeKind::Shape(ShapeType::Rectangle)),
        ("rounded=1;", NodeKind::Shape(ShapeType::RoundRectangle)),
        (
            "ellipse;whiteSpace=wrap;",
            NodeKind::Shape(ShapeType::Ellipse),
        ),
        ("rhombus;", NodeKind::Shape(ShapeType::Diamond)),
        (
            "triangle;direction=north;",
            NodeKind::Shape(ShapeType::Triangle),
        ),
        ("shape=hexagon;", NodeKind::Shape(ShapeType::Hexagon)),
        (
            "shape=trapezoid;flipV=1;",
            NodeKind::Shape(ShapeType::Trapezoid2),
        ),
        (
            "shape=cylinder3;",
            NodeKind::Generic("com.yworks.flowchart.dataBase".into()),
        ),
        (
            "shape=document;",
            NodeKind::Generic("com.yworks.flowchart.document".into()),
        ),
        (
            "shape=mxgraph.flowchart.terminator;",
            NodeKind::Generic("com.yworks.flowchart.terminator".into()),
        ),
        (
            "shape=mxgraph.aws4.lambda;",
            NodeKind::Shape(ShapeType::Rectangle),
        ),
    ];
    let cells: String = (styles.iter().enumerate())
        .map(|(i, (style, _))| vertex(&format!("v{i}"), "", style, [0.0, 0.0, 80.0, 40.0]))
        .collect();
    let import = convert(&page(&cells));
    let kinds: Vec<NodeKind> = nodes(&import).into_iter().map(|n| n.kind).collect();
    let expected: Vec<NodeKind> = styles.into_iter().map(|(_, kind)| kind).collect();
    assert_eq!(kinds, expected);
    assert_eq!(
        import.notes,
        ["1 shape yEd has nothing like became a box: mxgraph.aws4.lambda (1)."]
    );
}

#[test]
fn shapes_turned_some_other_way_are_noted() {
    let cells = vertex("t", "", "triangle;", [0.0, 0.0, 40.0, 40.0]);
    let import = convert(&page(&cells));
    assert_eq!(
        import.notes,
        ["1 shape points the way yEd draws it, not as draw.io turned it."]
    );
}

#[test]
fn round_boxes_keep_their_radius_in_the_serialized_graphml() {
    let cells = [
        vertex("a", "", "rounded=1;", [0.0, 0.0, 120.0, 60.0]),
        vertex(
            "b",
            "",
            "rounded=1;arcSize=50;absoluteArcSize=1;",
            [0.0, 0.0, 120.0, 60.0],
        ),
        vertex("c", "", "rounded=0;", [0.0, 0.0, 120.0, 60.0]),
        // At most half the shorter side.
        vertex("d", "", "rounded=1;arcSize=80;", [0.0, 0.0, 120.0, 60.0]),
        vertex(
            "e",
            "",
            "rounded=1;arcSize=8;absoluteArcSize=1;",
            [0.0, 0.0, 120.0, 60.0],
        ),
    ];
    let import = convert(&page(&cells.concat()));
    let doc = GraphMl::parse(&import.graphml.to_xml()).unwrap();
    let key = drwr::corner_radius_key(&doc).unwrap();
    let keys = Keys::find(&doc);
    let radii: Vec<_> = doc.graphs[0]
        .nodes
        .iter()
        .map(|node| {
            let graphics = keys.node_graphics(node).unwrap().unwrap();
            drwr::corner_radius(&graphics.kind, node, Some(key)).unwrap()
        })
        .collect();
    assert_eq!(radii, [9.0, 25.0, 0.0, 30.0, 4.0]);
    assert!(
        !doc.graphs[0].nodes[4]
            .data
            .iter()
            .any(|data| data.key == key)
    );
}

#[test]
fn fills_and_borders_follow_the_style() {
    let cells = [
        vertex(
            "a",
            "",
            "fillColor=#dae8fc;strokeColor=#6c8ebf;strokeWidth=2;dashed=1;opacity=50;gradientColor=#7EA6E0;",
            [0.0, 0.0, 80.0, 40.0],
        ),
        vertex("b", "", "", [0.0, 0.0, 80.0, 40.0]),
        vertex("c", "Text", "text;html=1;", [0.0, 0.0, 80.0, 40.0]),
        vertex(
            "d",
            "",
            "fillColor=none;dashed=1;dashPattern=1 2;",
            [0.0, 0.0, 80.0, 40.0],
        ),
    ];
    let nodes = nodes(&convert(&page(&cells.concat())));
    let half = |color: Color| Color { a: 128, ..color };
    assert_eq!(
        nodes[0].fill.color,
        Some(half(Color::rgb(0xDA, 0xE8, 0xFC)))
    );
    assert_eq!(
        nodes[0].fill.gradient,
        Some(half(Color::rgb(0x7E, 0xA6, 0xE0)))
    );
    assert_eq!(
        nodes[0].border.color,
        Some(half(Color::rgb(0x6C, 0x8E, 0xBF)))
    );
    assert_eq!(
        (nodes[0].border.width, &nodes[0].border.line_type),
        (2.0, &LineType::Dashed)
    );
    assert_eq!(nodes[1].fill.color, Some(Color::WHITE));
    assert_eq!(nodes[1].border.color, Some(Color::BLACK));
    assert_eq!((nodes[2].fill.color, nodes[2].border.color), (None, None));
    assert_eq!(nodes[3].fill.color, None);
    assert_eq!(nodes[3].border.line_type, LineType::Dotted);
}

#[test]
fn labels_keep_their_text_font_and_place() {
    let cells = [
        vertex(
            "a",
            "&lt;b&gt;Title&lt;/b&gt;&lt;br&gt;&lt;b&gt;Sub&lt;/b&gt;",
            "html=1;fontSize=14.4;fontFamily=Courier New,monospace;fontColor=#ff0000;align=left;verticalAlign=top;",
            [0.0, 0.0, 80.0, 40.0],
        ),
        vertex(
            "b",
            "Below&#10;two lines",
            "fontStyle=3;verticalLabelPosition=bottom;verticalAlign=top;labelBackgroundColor=#ffffff;",
            [0.0, 0.0, 80.0, 40.0],
        ),
        vertex("c", "  ", "", [0.0, 0.0, 80.0, 40.0]),
    ];
    let nodes = nodes(&convert(&page(&cells.concat())));
    let a = &nodes[0].labels[0];
    assert_eq!(a.text, "Title\nSub");
    assert_eq!((a.font.family.as_str(), a.font.size), ("Courier New", 14));
    assert_eq!(a.font.style, FontStyle::Bold);
    assert_eq!(a.text_color, Color::rgb(0xFF, 0, 0));
    assert_eq!(a.alignment, Alignment::Left);
    assert_eq!(a.position(), Some(LabelPosition::TopLeft));
    let b = &nodes[1].labels[0];
    assert_eq!(b.text, "Below\ntwo lines");
    assert_eq!(b.font.style, FontStyle::BoldItalic);
    assert_eq!(b.font.family, "Helvetica");
    assert_eq!(b.position(), Some(LabelPosition::South));
    assert_eq!(b.background, Some(Color::WHITE));
    assert!(nodes[2].labels.is_empty());
}

#[test]
fn shapes_in_containers_are_placed_on_the_page() {
    let cells = r#"
        <mxCell id="g" style="group" vertex="1" parent="1"><mxGeometry x="100" y="100" width="200" height="100" as="geometry"/></mxCell>
        <mxCell id="a" value="A" vertex="1" parent="g"><mxGeometry x="10" y="20" width="50" height="30" as="geometry"/></mxCell>
        <mxCell id="lane" value="Lane" style="swimlane;startSize=30;" vertex="1" parent="1"><mxGeometry x="0" y="300" width="200" height="200" as="geometry"/></mxCell>
        <mxCell id="b" value="B" vertex="1" parent="lane"><mxGeometry x="20" y="40" width="50" height="30" as="geometry"/></mxCell>
        <mxCell id="port" style="ellipse" vertex="1" parent="b"><mxGeometry x="1" y="0.5" width="10" height="10" relative="1" as="geometry"><mxPoint x="-5" y="-5" as="offset"/></mxGeometry></mxCell>"#;
    let nodes = nodes(&convert(&page(cells)));
    // The invisible group leaves no node.
    let geometry: Vec<Rect> = nodes.iter().map(|n| n.geometry).collect();
    assert_eq!(
        geometry,
        [
            rect(110.0, 120.0, 50.0, 30.0),
            rect(0.0, 300.0, 200.0, 200.0),
            rect(0.0, 300.0, 200.0, 30.0),
            rect(20.0, 340.0, 50.0, 30.0),
            rect(65.0, 350.0, 10.0, 10.0),
        ]
    );
    // The swimlane's title sits in its title bar.
    assert!(nodes[1].labels.is_empty());
    assert_eq!(nodes[2].labels[0].text, "Lane");
}

#[test]
fn collapsed_containers_stand_in_for_what_they_hide() {
    let cells = r#"
        <mxCell id="c" value="Closed" vertex="1" collapsed="1" parent="1"><mxGeometry x="0" y="0" width="80" height="30" as="geometry"/></mxCell>
        <mxCell id="in" value="Inside" vertex="1" parent="c"><mxGeometry x="10" y="10" width="40" height="20" as="geometry"/></mxCell>
        <mxCell id="out" value="Out" vertex="1" parent="1"><mxGeometry x="200" y="0" width="80" height="30" as="geometry"/></mxCell>"#;
    let text = page(&(cells.to_owned() + &edge("e", "out", "in", "", "")));
    let import = convert(&text);
    let labels: Vec<String> = nodes(&import)
        .iter()
        .map(|n| n.labels[0].text.clone())
        .collect();
    assert_eq!(labels, ["Closed", "Out"]);
    let (edge, _) = &edges(&import)[0];
    assert_eq!((edge.source.as_str(), edge.target.as_str()), ("n1", "n0"));
}

#[test]
fn hidden_layers_and_items_are_left_out() {
    let text = r#"<mxGraphModel><root><mxCell id="0"/><mxCell id="1" parent="0"/>
        <mxCell id="2" value="Shown" vertex="1" parent="1"><mxGeometry width="80" height="30" as="geometry"/></mxCell>
        <mxCell id="3" value="Hidden" vertex="1" visible="0" parent="1"><mxGeometry width="80" height="30" as="geometry"/></mxCell>
        <mxCell id="L2" parent="0" visible="0"/>
        <mxCell id="4" value="On a hidden layer" vertex="1" parent="L2"><mxGeometry width="80" height="30" as="geometry"/></mxCell>
        </root></mxGraphModel>"#;
    let import = convert(text);
    assert_eq!(nodes(&import).len(), 1);
    assert_eq!(
        import.notes,
        ["1 hidden layer and 1 hidden item were left out."]
    );
}

#[test]
fn lines_join_their_shapes_with_the_closest_arrowheads() {
    let cells = [
        vertex("a", "", "", [0.0, 0.0, 80.0, 40.0]),
        vertex("b", "", "", [200.0, 0.0, 80.0, 40.0]),
        edge("e1", "a", "b", "", ""),
        edge(
            "e2",
            "a",
            "b",
            "startArrow=diamond;startFill=0;endArrow=open;dashed=1;strokeColor=#ff0000;",
            "",
        ),
        edge("e3", "a", "b", "startArrow=ERmandOne;endArrow=none;", ""),
    ];
    let edges = edges(&convert(&page(&cells.concat())));
    let (edge, graphics) = &edges[0];
    assert_eq!((edge.source.as_str(), edge.target.as_str()), ("n0", "n1"));
    assert_eq!(edge.id.as_deref(), Some("e0"));
    assert_eq!(
        (&graphics.arrows.source, &graphics.arrows.target),
        (&Arrow::None, &Arrow::Standard)
    );
    let (_, graphics) = &edges[1];
    assert_eq!(
        (&graphics.arrows.source, &graphics.arrows.target),
        (&Arrow::WhiteDiamond, &Arrow::Plain)
    );
    assert_eq!(graphics.line.line_type, LineType::Dashed);
    assert_eq!(graphics.line.color, Some(Color::rgb(0xFF, 0, 0)));
    let (_, graphics) = &edges[2];
    assert_eq!(
        (&graphics.arrows.source, &graphics.arrows.target),
        (
            &Arrow::Other("crows_foot_one_mandatory".into()),
            &Arrow::None
        )
    );
}

#[test]
fn orthogonal_lines_get_the_bends_draw_io_draws() {
    let cells = [
        vertex("a", "", "", [0.0, 0.0, 100.0, 50.0]),
        vertex("b", "", "", [300.0, 100.0, 100.0, 50.0]),
        edge("e", "a", "b", "edgeStyle=orthogonalEdgeStyle;", ""),
        edge(
            "f",
            "a",
            "b",
            "edgeStyle=orthogonalEdgeStyle;exitX=1;exitY=0.25;entryX=0;entryY=0.5;",
            "",
        ),
        edge(
            "g",
            "a",
            "b",
            "",
            r#"<Array as="points"><mxPoint x="150" y="60"/></Array>"#,
        ),
    ];
    let edges = edges(&convert(&page(&cells.concat())));
    let path = &edges[0].1.path;
    assert_eq!(path.points, [at(350.0, 25.0)]);
    assert_eq!((path.source, path.target), (at(0.0, 0.0), at(0.0, 0.0)));
    // Fixed points become ports, relative to the node's center.
    let path = &edges[1].1.path;
    assert_eq!(
        (path.source, path.target),
        (at(50.0, -12.5), at(-50.0, 0.0))
    );
    assert_eq!(path.points, [at(200.0, 12.5), at(200.0, 125.0)]);
    // Straight lines keep their points as they are.
    assert_eq!(edges[2].1.path.points, [at(150.0, 60.0)]);
}

#[test]
fn loose_line_ends_get_small_nodes() {
    let cells = [
        vertex("a", "", "", [0.0, 0.0, 80.0, 40.0]),
        edge(
            "e",
            "a",
            "",
            "",
            r#"<mxPoint x="300" y="20" as="targetPoint"/>"#,
        ),
        r#"<mxCell id="f" edge="1" parent="1"><mxGeometry relative="1" as="geometry"><mxPoint x="0" y="100" as="sourcePoint"/><mxPoint x="80" y="100" as="targetPoint"/></mxGeometry></mxCell>"#.to_owned(),
        r#"<mxCell id="g" edge="1" parent="1"><mxGeometry relative="1" as="geometry"/></mxCell>"#.to_owned(),
    ];
    let import = convert(&page(&cells.concat()));
    let nodes = nodes(&import);
    let anchors: Vec<Rect> = nodes[1..].iter().map(|n| n.geometry).collect();
    assert_eq!(
        anchors,
        [
            rect(299.0, 19.0, 2.0, 2.0),
            rect(-1.0, 99.0, 2.0, 2.0),
            rect(79.0, 99.0, 2.0, 2.0),
        ]
    );
    assert!(
        nodes[1..]
            .iter()
            .all(|n| n.fill.color.is_none() && n.border.color.is_none())
    );
    let edges = edges(&import);
    assert_eq!(edges.len(), 2);
    assert_eq!(
        (edges[0].0.source.as_str(), edges[0].0.target.as_str()),
        ("n0", "n1")
    );
    assert_eq!(
        import.notes,
        [
            "3 loose line ends got small invisible nodes to end on.",
            "1 line with an end on nothing shown was left out.",
        ]
    );
}

#[test]
fn edge_labels_sit_where_draw_io_puts_them() {
    let cells = [
        vertex("a", "", "", [0.0, 0.0, 100.0, 40.0]),
        vertex("b", "", "", [300.0, 0.0, 100.0, 40.0]),
        r#"<mxCell id="e" value="mid" edge="1" parent="1" source="a" target="b"><mxGeometry relative="1" as="geometry"/></mxCell>"#.to_owned(),
        r#"<mxCell id="l" value="quarter" style="edgeLabel;html=1;align=center;verticalAlign=middle;" vertex="1" connectable="0" parent="e"><mxGeometry x="-0.5" y="10" relative="1" as="geometry"><mxPoint as="offset"/></mxGeometry></mxCell>"#.to_owned(),
    ];
    let import = convert(&page(&cells.concat()));
    let (_, graphics) = &edges(&import)[0];
    let route = yed::Route::new(
        &graphics.path,
        nodes(&import)[0].geometry,
        nodes(&import)[1].geometry,
    );
    let center = |i: usize| {
        let bounds = graphics.labels[i].bounds;
        let start = route.points[0];
        at(
            start.x + bounds.x + bounds.width / 2.0,
            start.y + bounds.y + bounds.height / 2.0,
        )
    };
    let close = |a: Point, b: Point| (a.x - b.x).abs() < 0.5 && (a.y - b.y).abs() < 0.5;
    // The line runs from x 100 to 300 at y 20; 10 off it is above, as in draw.io.
    assert!(close(center(0), at(200.0, 20.0)), "{:?}", center(0));
    assert!(close(center(1), at(150.0, 10.0)), "{:?}", center(1));
    assert_eq!(graphics.labels[1].font.size, 11);
    assert_eq!(graphics.labels[0].background, Some(Color::WHITE));
}

#[test]
fn edge_labels_aligned_to_the_bottom_sit_above_their_line() {
    let cells = [
        vertex("a", "", "", [0.0, 0.0, 100.0, 40.0]),
        vertex("b", "", "", [300.0, 0.0, 100.0, 40.0]),
        r#"<mxCell id="e" value="above" style="verticalAlign=bottom;" edge="1" parent="1" source="a" target="b"><mxGeometry relative="1" as="geometry"/></mxCell>"#.to_owned(),
    ];
    let import = convert(&page(&cells.concat()));
    let (_, graphics) = &edges(&import)[0];
    let bounds = graphics.labels[0].bounds;
    // Relative to where the line leaves the source, at y 20.
    assert!((bounds.y + bounds.height - 0.0).abs() < 0.5, "{bounds:?}");
}

#[test]
fn images_become_svg_nodes_when_the_file_holds_them() {
    let svg = "PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAxMCAxMCI+PGNpcmNsZSBjeD0iNSIgY3k9IjUiIHI9IjUiLz48L3N2Zz4=";
    let cells = [
        vertex(
            "a",
            "Icon",
            &format!("image;image=data:image/svg+xml,{svg};"),
            [0.0, 0.0, 40.0, 40.0],
        ),
        vertex(
            "b",
            "",
            "shape=image;image=https://example.com/a.png;",
            [0.0, 0.0, 40.0, 40.0],
        ),
        vertex(
            "c",
            "",
            "shape=image;image=data:image/png,iVBORw0KGgo=;",
            [0.0, 0.0, 40.0, 40.0],
        ),
    ];
    let import = convert(&page(&cells.concat()));
    let nodes = nodes(&import);
    let NodeKind::Svg {
        resource: Some(resource),
    } = &nodes[0].kind
    else {
        panic!("not an SVG node: {:?}", nodes[0].kind);
    };
    let text = yed::resource_text(&import.graphml, resource).unwrap();
    assert!(text.contains("<circle"));
    assert_eq!(nodes[0].labels[0].position(), Some(LabelPosition::South));
    assert_eq!(nodes[1].border.line_type, LineType::Dashed);
    assert_eq!(nodes[1].fill.color, None);
    assert_eq!(
        import.notes,
        [
            "1 image the file only links to became empty box.",
            "1 PNG or JPEG image became empty box.",
        ]
    );
}

#[test]
fn links_tooltips_and_placeholders_carry_over() {
    let cells = r#"<UserObject label="%name% box" placeholders="1" name="Fancy" link="https://example.com" tooltip="Tip" id="u"><mxCell style="" vertex="1" parent="1"><mxGeometry width="80" height="30" as="geometry"/></mxCell></UserObject>"#;
    let import = convert(&page(cells));
    assert_eq!(nodes(&import)[0].labels[0].text, "Fancy box");
    let node = &import.graphml.graphs[0].nodes[0];
    let data = |name: &str| {
        let key = import
            .graphml
            .keys
            .iter()
            .find(|k| k.attr_name.as_deref() == Some(name) && k.domain == graphml::KeyDomain::Node)
            .unwrap();
        node.data.iter().find(|d| d.key == key.id).map(|d| d.text())
    };
    assert_eq!(data("url").as_deref(), Some("https://example.com"));
    assert_eq!(data("description").as_deref(), Some("Tip"));
}

#[test]
fn quarter_turned_boxes_turn_their_bounds() {
    let cells = [
        vertex("a", "", "rotation=90;", [0.0, 0.0, 120.0, 60.0]),
        vertex("b", "", "rotation=45;", [0.0, 0.0, 120.0, 60.0]),
    ];
    let import = convert(&page(&cells.concat()));
    assert_eq!(nodes(&import)[0].geometry, rect(30.0, -30.0, 60.0, 120.0));
    assert_eq!(import.notes, ["1 rotated shape is shown unrotated."]);
}

#[test]
fn relative_ports_turn_with_their_parent() {
    let cells = [
        vertex("p", "", "rotation=90;", [0.0, 0.0, 100.0, 40.0]),
        r#"<mxCell id="port" vertex="1" parent="p"><mxGeometry x="1" y="0.5" width="8" height="8" relative="1" as="geometry"><mxPoint x="-4" y="-4" as="offset"/></mxGeometry></mxCell>"#
            .to_owned(),
    ];
    let import = convert(&page(&cells.concat()));
    let nodes = nodes(&import);
    assert_eq!(nodes[0].geometry, rect(30.0, -30.0, 40.0, 100.0));
    assert_eq!(nodes[1].geometry, rect(46.0, 66.0, 8.0, 8.0));
}

#[test]
fn lines_left_out_leave_no_nodes_behind() {
    let cells = [
        r#"<mxCell id="h" vertex="1" visible="0" parent="1"><mxGeometry width="80" height="30" as="geometry"/></mxCell>"#,
        r#"<mxCell id="e" edge="1" parent="1" target="h"><mxGeometry relative="1" as="geometry"><mxPoint x="300" y="20" as="sourcePoint"/></mxGeometry></mxCell>"#,
    ];
    let import = convert(&page(&cells.concat()));
    assert!(import.graphml.graphs[0].nodes.is_empty());
    assert!(import.graphml.graphs[0].edges.is_empty());
}

#[test]
fn tables_get_a_title_bar_and_rows_their_lines() {
    let cells = r#"
        <mxCell id="t" value="Orders" style="shape=table;startSize=30;container=1;fontStyle=1;" vertex="1" parent="1"><mxGeometry x="0" y="0" width="200" height="90" as="geometry"/></mxCell>
        <mxCell id="r" style="shape=partialRectangle;fillColor=none;top=0;left=0;right=0;bottom=1;strokeWidth=2;" vertex="1" parent="t"><mxGeometry y="30" width="200" height="30" as="geometry"/></mxCell>
        <mxCell id="c" value="order_id" style="shape=partialRectangle;fillColor=none;top=0;left=0;bottom=0;right=0;" vertex="1" parent="r"><mxGeometry width="200" height="30" as="geometry"/></mxCell>"#;
    let import = convert(&page(cells));
    let nodes = nodes(&import);
    let shown: Vec<(Rect, Option<&str>)> = nodes
        .iter()
        .map(|n| (n.geometry, n.labels.first().map(|l| l.text.as_str())))
        .collect();
    assert_eq!(
        shown,
        [
            (rect(0.0, 0.0, 200.0, 90.0), None),
            (rect(0.0, 0.0, 200.0, 30.0), Some("Orders")),
            (rect(0.0, 30.0, 200.0, 30.0), None),
            // The row's bottom border, as a line.
            (rect(0.0, 59.0, 200.0, 2.0), None),
            (rect(0.0, 30.0, 200.0, 30.0), Some("order_id")),
        ]
    );
    assert_eq!(nodes[2].border.color, None);
    assert_eq!(nodes[3].fill.color, Some(Color::BLACK));
    assert_eq!(nodes[4].border.color, None);
    assert!(import.notes.is_empty(), "{:?}", import.notes);
}

/// Whether `a` is within 0.01 of `b` both ways.
fn near(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 0.01 && (a.y - b.y).abs() < 0.01
}

#[test]
fn loops_go_round_a_side_whatever_the_line_style() {
    let text = format!(
        r#"<mxGraphModel gridSize="20"><root><mxCell id="0"/><mxCell id="1" parent="0"/>{}{}{}</root></mxGraphModel>"#,
        vertex("a", "", "", [0.0, 0.0, 100.0, 50.0]),
        edge("e", "a", "a", "", ""),
        edge(
            "f",
            "a",
            "a",
            "edgeStyle=orthogonalEdgeStyle;segment=5;",
            ""
        ),
    );
    let edges = edges(&convert(&text));
    // Out of its middle, round the page's grid size, beyond twice that.
    let path = &edges[0].1.path;
    assert_eq!(path.points, [at(140.0, 5.0), at(140.0, 45.0)]);
    assert_eq!((path.source, path.target), (at(0.0, 0.0), at(0.0, 0.0)));
    assert_eq!(edges[1].1.path.points, [at(110.0, 20.0), at(110.0, 30.0)]);
}

#[test]
fn lines_can_end_on_other_lines_and_their_labels() {
    let cells = [
        vertex("a", "", "", [0.0, 0.0, 100.0, 40.0]),
        vertex("b", "", "", [300.0, 0.0, 100.0, 40.0]),
        vertex("c", "", "", [150.0, 200.0, 100.0, 40.0]),
        // Before the line its source is a label of.
        edge("toLabel", "l", "c", "", ""),
        edge("ab", "a", "b", "", ""),
        r#"<mxCell id="l" value="label" style="edgeLabel;" vertex="1" connectable="0" parent="ab"><mxGeometry x="-0.5" y="10" relative="1" as="geometry"/></mxCell>"#.to_owned(),
        edge("toLine", "ab", "c", "", ""),
    ];
    let import = convert(&page(&cells.concat()));
    let edges = edges(&import);
    let ids: Vec<_> = edges.iter().map(|(e, _)| e.id.clone().unwrap()).collect();
    assert_eq!(ids, ["e0", "e1", "e2"]);
    let nodes = nodes(&import);
    let end = |node: &str| {
        let index: usize = node[1..].parse().unwrap();
        let g = nodes[index].geometry;
        at(g.x + g.width / 2.0, g.y + g.height / 2.0)
    };
    // The label's anchor: a quarter along the line from x 100 to 300, 10 above.
    assert_eq!(end(&edges[0].0.source), at(150.0, 10.0));
    assert_eq!(edges[0].0.target, "n2");
    // The middle of the line.
    assert_eq!(end(&edges[2].0.source), at(200.0, 20.0));
    assert_eq!(
        import.notes,
        ["2 loose line ends got small invisible nodes to end on."]
    );
}

#[test]
fn lines_ending_on_a_bent_line_meet_it_halfway_along() {
    let cells = [
        vertex("a", "", "", [0.0, 0.0, 100.0, 40.0]),
        vertex("b", "", "", [300.0, 200.0, 100.0, 40.0]),
        vertex("c", "", "", [0.0, 300.0, 100.0, 40.0]),
        edge(
            "bent",
            "a",
            "b",
            "",
            r#"<Array as="points"><mxPoint x="350" y="20"/></Array>"#,
        ),
        // A shape on the line: its corner where a label's anchor would be.
        r#"<mxCell id="dot" style="ellipse;" vertex="1" parent="bent"><mxGeometry x="0.5" relative="1" width="10" height="10" as="geometry"/></mxCell>"#.to_owned(),
        edge("toLine", "c", "bent", "", ""),
    ];
    let import = convert(&page(&cells.concat()));
    let nodes = nodes(&import);
    // From (100, 20) right 250 and down 180: 215 along is (315, 20).
    let anchor = nodes.last().unwrap().geometry;
    assert_eq!(anchor, rect(314.0, 19.0, 2.0, 2.0));
    // Three quarters along, 322.5 in: (350, 92.5).
    let dot = &nodes[3];
    assert_eq!(dot.kind, NodeKind::Shape(ShapeType::Ellipse));
    assert_eq!(dot.geometry, rect(350.0, 92.5, 10.0, 10.0));
    assert!(edges(&import)[0].1.labels.is_empty());
}

#[test]
fn shapes_on_lines_keep_their_descendants_and_connections() {
    let cells = [
        vertex("a", "", "", [0.0, 0.0, 80.0, 40.0]),
        vertex("b", "", "", [200.0, 0.0, 80.0, 40.0]),
        // Resolve a descendant before the line that positions its parent.
        edge("ref", "nested", "a", "", ""),
        edge("e", "a", "b", "", ""),
        r#"<mxCell id="attached" vertex="1" parent="e"><mxGeometry y="20" width="30" height="16" relative="1" as="geometry"/></mxCell>
            <mxCell id="nested" vertex="1" parent="attached"><mxGeometry x="2" y="2" width="12" height="10" as="geometry"/></mxCell>
            <mxCell id="inner" edge="1" parent="attached" source="nested" target="b"><mxGeometry relative="1" as="geometry"/></mxCell>"#
            .to_owned(),
    ];
    let import = convert(&page(&cells.concat()));
    let nodes = nodes(&import);
    assert_eq!(nodes.len(), 4);
    assert_eq!(nodes[2].geometry, rect(140.0, 0.0, 30.0, 16.0));
    assert_eq!(nodes[3].geometry, rect(142.0, 2.0, 12.0, 10.0));
    let endpoints: Vec<_> = import.graphml.graphs[0]
        .edges
        .iter()
        .map(|edge| (edge.source.as_str(), edge.target.as_str()))
        .collect();
    assert_eq!(endpoints, [("n3", "n0"), ("n0", "n1"), ("n3", "n1")]);
    assert!(import.notes.is_empty(), "{:?}", import.notes);
}

#[test]
fn lines_ending_on_lines_or_shapes_not_shown_are_left_out() {
    let cells = [
        vertex("a", "", "", [0.0, 0.0, 100.0, 40.0]),
        vertex("b", "", "", [300.0, 0.0, 100.0, 40.0]),
        r#"<mxCell id="hidden" edge="1" visible="0" parent="1" source="a" target="b"><mxGeometry relative="1" as="geometry"/></mxCell>"#.to_owned(),
        // Its point doesn't count: draw.io leaves it out too.
        edge(
            "toHidden",
            "hidden",
            "b",
            "",
            r#"<mxPoint x="0" y="200" as="sourcePoint"/>"#,
        ),
    ];
    let import = convert(&page(&cells.concat()));
    assert!(edges(&import).is_empty());
    assert_eq!(nodes(&import).len(), 2);
    assert_eq!(
        import.notes,
        [
            "1 hidden item was left out.",
            "1 line with an end on nothing shown was left out."
        ]
    );
}

#[test]
fn fixed_ends_turn_flip_and_sit_on_the_outline_as_in_draw_io() {
    let cells = [
        vertex("ellipse", "", "ellipse;", [0.0, 0.0, 100.0, 60.0]),
        vertex("rhombus", "", "rhombus;", [200.0, 0.0, 100.0, 60.0]),
        vertex("flipped", "", "flipH=1;", [0.0, 200.0, 120.0, 60.0]),
        vertex("turned", "", "rotation=90;", [200.0, 200.0, 120.0, 60.0]),
        vertex("south", "", "direction=south;", [400.0, 200.0, 120.0, 60.0]),
        edge(
            "e1",
            "ellipse",
            "rhombus",
            "exitX=0;exitY=0;entryX=0;entryY=0;",
            "",
        ),
        edge(
            "e2",
            "flipped",
            "turned",
            "exitX=0;exitY=0.5;entryX=1;entryY=0.5;",
            "",
        ),
        edge(
            "e3",
            "south",
            "ellipse",
            "exitX=1;exitY=0.5;exitPerimeter=0;entryX=0;entryY=0;entryPerimeter=0;",
            "",
        ),
    ];
    let edges = edges(&convert(&page(&cells.concat())));
    let ports = |i: usize| (edges[i].1.path.source, edges[i].1.path.target);
    // Relative to the node's center: on the ellipse at 45°, and on the
    // rhombus a quarter of the way in from its bounds' corner.
    let (source, target) = ports(0);
    assert!(near(source, at(-35.36, -21.21)), "{source:?}");
    assert!(near(target, at(-25.0, -15.0)), "{target:?}");
    // The left side flipped right; the right side turned to the bottom.
    assert_eq!(ports(1), (at(60.0, 0.0), at(0.0, 60.0)));
    // The right side turned to the bottom with the direction, and the
    // ellipse's corner left off its outline.
    assert_eq!(ports(2), (at(0.0, 30.0), at(-50.0, -30.0)));
}

#[test]
fn unknown_arrowheads_draw_none_like_in_draw_io() {
    let ends = [
        ("bogus", "1", Arrow::None),
        ("box", "1", Arrow::Diamond),
        ("box", "0", Arrow::WhiteDiamond),
        ("mermaidExtension", "0", Arrow::WhiteDelta),
        (
            "manyOptional",
            "1",
            Arrow::Other("crows_foot_many_optional".into()),
        ),
        ("sysMLx", "1", Arrow::SkewedDash),
    ];
    let mut cells = vec![
        vertex("a", "", "", [0.0, 0.0, 80.0, 40.0]),
        vertex("b", "", "", [200.0, 0.0, 80.0, 40.0]),
    ];
    for (i, (name, fill, _)) in ends.iter().enumerate() {
        let style = format!("endArrow={name};endFill={fill};");
        cells.push(edge(&format!("e{i}"), "a", "b", &style, ""));
    }
    let edges = edges(&convert(&page(&cells.concat())));
    for ((name, fill, expected), (_, graphics)) in ends.iter().zip(&edges) {
        assert_eq!(&graphics.arrows.target, expected, "{name} {fill}");
    }
}

#[test]
fn colors_naming_another_take_its_value() {
    let cells = r#"
        <mxCell id="p" style="fillColor=#00ff00;strokeColor=#0000ff;" vertex="1" parent="1"><mxGeometry width="200" height="100" as="geometry"/></mxCell>
        <mxCell id="c" style="fillColor=strokeColor;strokeColor=#ff0000;fontColor=parentStrokeColor;" value="x" vertex="1" parent="p"><mxGeometry x="10" y="10" width="50" height="30" as="geometry"/></mxCell>
        <mxCell id="d" style="strokeColor=parentFillColor;" vertex="1" parent="p"><mxGeometry x="100" y="10" width="50" height="30" as="geometry"/></mxCell>
        <mxCell id="e" style="strokeColor=fillColor;" vertex="1" parent="1"><mxGeometry x="300" width="50" height="30" as="geometry"/></mxCell>"#;
    let nodes = nodes(&convert(&page(cells)));
    let red = Some(Color::rgb(0xFF, 0, 0));
    assert_eq!((nodes[1].fill.color, nodes[1].border.color), (red, red));
    assert_eq!(nodes[1].labels[0].text_color, Color::rgb(0, 0, 0xFF));
    assert_eq!(nodes[2].border.color, Some(Color::rgb(0, 0xFF, 0)));
    // The default fill, white.
    assert_eq!(nodes[3].border.color, Some(Color::WHITE));
}

#[test]
fn parallelograms_turned_half_round_look_the_same() {
    let styles = [
        ("shape=parallelogram;direction=west;", false),
        ("shape=parallelogram;flipV=1;", true),
        ("shape=parallelogram;flipH=1;flipV=1;", false),
        ("shape=parallelogram;direction=south;", true),
    ];
    for (style, noted) in styles {
        let import = convert(&page(&vertex("p", "", style, [0.0, 0.0, 80.0, 40.0])));
        assert_eq!(import.notes.len(), usize::from(noted), "{style}");
    }
}

#[test]
fn side_to_side_and_top_to_bottom_lines_are_elbows() {
    let cells = [
        vertex("a", "", "", [0.0, 0.0, 100.0, 50.0]),
        vertex("b", "", "", [300.0, 100.0, 100.0, 50.0]),
        edge("e", "a", "b", "edgeStyle=sideToSideEdgeStyle;", ""),
        edge("f", "a", "b", "edgeStyle=topToBottomEdgeStyle;", ""),
    ];
    let edges = edges(&convert(&page(&cells.concat())));
    assert_eq!(edges[0].1.path.points, [at(200.0, 25.0), at(200.0, 125.0)]);
    assert_eq!(edges[1].1.path.points, [at(50.0, 75.0), at(350.0, 75.0)]);
}

#[test]
fn lifelines_are_noted_as_drawn_solid_unless_they_are() {
    let lifeline = |style: &str| {
        let style = format!("shape=umlLifeline;{style}");
        convert(&page(&vertex("l", "A", &style, [0.0, 0.0, 100.0, 300.0]))).notes
    };
    assert_eq!(lifeline(""), ["1 dashed line shape is drawn solid."]);
    assert!(lifeline("lifelineDashed=0;").is_empty());
}

#[test]
fn any_element_wrapping_a_cell_holds_its_properties() {
    let cells = r#"<Server label="Web" tooltip="Front" id="s"><mxCell vertex="1" parent="1"><mxGeometry width="80" height="30" as="geometry"/></mxCell></Server>"#;
    let import = convert(&page(cells));
    assert_eq!(nodes(&import)[0].labels[0].text, "Web");
    assert_eq!(import.graphml.graphs[0].nodes[0].data.len(), 2);
}

#[test]
fn edge_labels_past_the_ends_extend_the_line() {
    let cells = [
        vertex("a", "", "", [0.0, 0.0, 100.0, 40.0]),
        vertex("b", "", "", [300.0, 0.0, 100.0, 40.0]),
        r#"<mxCell id="e" edge="1" parent="1" source="a" target="b"><mxGeometry relative="1" as="geometry"/></mxCell>"#.to_owned(),
        r#"<mxCell id="l" value="past" style="edgeLabel;" vertex="1" parent="e"><mxGeometry x="1.5" relative="1" as="geometry"/></mxCell>"#.to_owned(),
    ];
    let import = convert(&page(&cells.concat()));
    let (_, graphics) = &edges(&import)[0];
    let bounds = graphics.labels[0].bounds;
    // 250 along a line 200 long, from where it leaves the source. As draw.io's
    // edgeLabel style aligns labels left and top, their corner is there.
    assert_eq!((bounds.x, bounds.y), (250.0, 0.0));
}

#[test]
fn labels_without_a_font_get_draw_ios_and_only_text_opacity_fades_them() {
    let cells = [
        vertex("a", "plain", ";", [0.0, 0.0, 80.0, 40.0]),
        vertex(
            "b",
            "faded",
            "opacity=50;textOpacity=20;fontColor=#ff0000;",
            [0.0, 0.0, 80.0, 40.0],
        ),
    ];
    let nodes = nodes(&convert(&page(&cells.concat())));
    let font = &nodes[0].labels[0].font;
    assert_eq!((font.family.as_str(), font.size), ("Arial", 11));
    assert_eq!(
        nodes[1].labels[0].text_color,
        Color {
            a: 51,
            ..Color::rgb(0xFF, 0, 0)
        }
    );
}

#[test]
fn placeholders_are_replaced_once_as_draw_io_does() {
    let cells = r#"
        <UserObject label="" team="Ops" id="g"><mxCell style="group" vertex="1" parent="1"><mxGeometry width="200" height="100" as="geometry"/></mxCell></UserObject>
        <UserObject label="%name% of %team% (%id%, %width%) %tooltip% %%name% %a% %missing% %name%%team% %date{yyyy}% %name%" placeholders="1" name="Box" a="%name%" tooltip="Tip" id="u"><mxCell vertex="1" parent="g"><mxGeometry width="80" height="30" as="geometry"/></mxCell></UserObject>"#;
    let import = convert(&page(cells));
    assert_eq!(
        nodes(&import)[0].labels[0].text,
        "Box of Ops (u, 80) %tooltip% %name% %name% %missing% BoxOps %date{yyyy}% Box"
    );
}

#[test]
fn underlined_or_upwards_labels_are_noted() {
    let cells = [
        vertex("a", "under", "fontStyle=4;", [0.0, 0.0, 80.0, 40.0]),
        vertex("b", "up", "horizontal=0;", [0.0, 0.0, 80.0, 40.0]),
    ];
    let import = convert(&page(&cells.concat()));
    assert_eq!(
        import.notes,
        [
            "1 label written upwards is shown level.",
            "1 label lost some of its formatting.",
        ]
    );
}

#[test]
fn characters_xml_cant_hold_are_left_out_as_draw_io_does() {
    let text = page(&vertex("a", "a\u{1}b", "", [0.0, 0.0, 80.0, 40.0]));
    let import = convert(&text);
    assert_eq!(nodes(&import)[0].labels[0].text, "ab");
}

/// "Pad" in a box, compressed into Base64 that ends in padding.
const PADDED: &str = "jVBBDgIhDHxN71gu3lH3ZOIXmtAsJCAbRMXfSyi68bCJhybtTKedDGgT65RpcedkOYA+gjY5pSJdrIZDAFTegj4AomoFeNpgd51VC2W+ln8EKIIHhTsLciE7MM6F6+bdDo2jE6fIJb/aytPb4mRjL6+VYz+7odIDo5vM81e5mmzN8PkZ1zw69xPXGw==";

#[test]
fn compressed_pages_decode_as_leniently_as_browsers_do() {
    // Without its padding, and with bits set past its last byte, as atob,
    // which draw.io decodes with, takes them.
    for data in [
        PADDED,
        PADDED.trim_end_matches('='),
        &PADDED.replace("w==", "x=="),
    ] {
        let text = format!(r#"<mxfile><diagram name="A">{data}</diagram></mxfile>"#);
        let import = convert(&text);
        assert_eq!(nodes(&import)[0].labels[0].text, "Pad", "{data}");
    }
}

#[test]
fn html_labels_break_at_newlines_unless_nl2br_is_off() {
    let cells = [
        vertex("a", "one&#10;two", "html=1;", [0.0, 0.0, 80.0, 40.0]),
        vertex(
            "b",
            "one&#10;two",
            "html=1;nl2Br=0;",
            [0.0, 0.0, 80.0, 40.0],
        ),
    ];
    let nodes = nodes(&convert(&page(&cells.concat())));
    assert_eq!(nodes[0].labels[0].text, "one\ntwo");
    assert_eq!(nodes[1].labels[0].text, "one two");
}

#[test]
fn edge_labels_turn_and_move_with_the_line() {
    let cells = [
        vertex("a", "", "", [0.0, 0.0, 100.0, 40.0]),
        vertex("b", "", "", [250.0, 220.0, 100.0, 40.0]),
        edge(
            "e",
            "a",
            "b",
            "",
            r#"<Array as="points"><mxPoint x="300" y="20"/></Array>"#,
        ),
        // At the bend, so off the segment after it: 10 to the right of the
        // downward run. Left- and top-aligned, as edgeLabel is.
        r#"<mxCell id="l1" value="bend" style="edgeLabel;" vertex="1" parent="e"><mxGeometry y="10" relative="1" as="geometry"/></mxCell>"#.to_owned(),
        // At the start, moved by an offset, with its bottom right corner there.
        r#"<mxCell id="l2" value="ab" style="edgeLabel;align=right;verticalAlign=bottom;" vertex="1" parent="e"><mxGeometry x="-1" relative="1" as="geometry"><mxPoint x="5" y="7" as="offset"/></mxGeometry></mxCell>"#.to_owned(),
    ];
    let import = convert(&page(&cells.concat()));
    let (_, graphics) = &edges(&import)[0];
    // Relative to where the line leaves the source, at (100, 20).
    let bend = graphics.labels[0].bounds;
    assert!(near(at(bend.x, bend.y), at(210.0, 0.0)), "{bend:?}");
    let start = graphics.labels[1].bounds;
    // Its size estimated from its 2 characters of 11 pt.
    assert!(
        near(at(start.width, start.height), at(16.1, 17.2)),
        "{start:?}"
    );
    assert!(near(at(start.x, start.y), at(-11.1, -10.2)), "{start:?}");
    // As yEd edge labels.
    let xml = import.graphml.to_xml();
    assert_eq!(xml.matches("<y:EdgeLabel").count(), 2);
}

#[test]
fn labels_of_lines_between_overlapping_shapes_still_get_a_place() {
    let cells = [
        vertex("a", "", "", [0.0, 0.0, 100.0, 40.0]),
        vertex("b", "", "", [0.0, 0.0, 100.0, 40.0]),
        r#"<mxCell id="e" value="x" edge="1" parent="1" source="a" target="b"><mxGeometry relative="1" as="geometry"/></mxCell>"#.to_owned(),
    ];
    let import = convert(&page(&cells.concat()));
    let bounds = edges(&import)[0].1.labels[0].bounds;
    assert!(bounds.x.is_finite() && bounds.y.is_finite(), "{bounds:?}");
}

#[test]
fn lines_in_containers_are_placed_on_the_page() {
    let cells = r#"
        <mxCell id="g" style="group" vertex="1" parent="1"><mxGeometry x="100" y="100" width="300" height="200" as="geometry"/></mxCell>
        <mxCell id="in" vertex="1" parent="g"><mxGeometry width="40" height="40" as="geometry"/></mxCell>
        <mxCell id="e" edge="1" parent="g" source="in"><mxGeometry relative="1" as="geometry"><mxPoint x="200" y="50" as="targetPoint"/><Array as="points"><mxPoint x="10" y="50"/></Array></mxGeometry></mxCell>"#;
    let import = convert(&page(cells));
    let geometry: Vec<Rect> = nodes(&import).iter().map(|n| n.geometry).collect();
    assert_eq!(
        geometry,
        [rect(100.0, 100.0, 40.0, 40.0), rect(299.0, 149.0, 2.0, 2.0)]
    );
    assert_eq!(edges(&import)[0].1.path.points, [at(110.0, 150.0)]);
}

#[test]
fn edge_labels_are_vertices_with_no_size_shown() {
    let cells = [
        vertex("a", "", "", [0.0, 0.0, 100.0, 40.0]),
        vertex("b", "", "", [300.0, 0.0, 100.0, 40.0]),
        edge("e", "a", "b", "", ""),
        r#"<mxCell id="hidden" value="hidden" style="edgeLabel;" vertex="1" visible="0" parent="e"><mxGeometry relative="1" as="geometry"/></mxCell>"#.to_owned(),
        // A plain text style: a vertex's font size and no background.
        r#"<mxCell id="wide" value="wide" style="text;" vertex="1" parent="e"><mxGeometry width="50" relative="1" as="geometry"/></mxCell>"#.to_owned(),
        r#"<mxCell id="tall" value="tall" style="edgeLabel;" vertex="1" parent="e"><mxGeometry height="20" relative="1" as="geometry"/></mxCell>"#.to_owned(),
    ];
    let import = convert(&page(&cells.concat()));
    assert_eq!(nodes(&import).len(), 2);
    let labels = &edges(&import)[0].1.labels;
    let texts: Vec<&str> = labels.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(texts, ["wide", "tall"]);
    assert_eq!((labels[0].font.size, labels[0].background), (12, None));
}

#[test]
fn loops_need_fewer_than_two_points_and_free_ends_if_orthogonal() {
    let cells = [
        vertex("a", "", "", [0.0, 0.0, 100.0, 50.0]),
        // A fixed end still loops, unless the loop is orthogonal.
        edge("fixed", "a", "a", "exitX=1;exitY=0.2;", ""),
        edge(
            "orthogonal",
            "a",
            "a",
            "orthogonalLoop=1;exitX=1;exitY=0.2;",
            "",
        ),
        edge(
            "points",
            "a",
            "a",
            "",
            r#"<Array as="points"><mxPoint x="150" y="10"/><mxPoint x="160" y="40"/></Array>"#,
        ),
    ];
    let edges = edges(&convert(&page(&cells.concat())));
    let path = &edges[0].1.path;
    assert_eq!(path.points, [at(120.0, 15.0), at(120.0, 35.0)]);
    assert_eq!(path.source, at(50.0, -15.0));
    assert!(edges[1].1.path.points.is_empty());
    assert_eq!(edges[2].1.path.points, [at(150.0, 10.0), at(160.0, 40.0)]);
}

#[test]
fn fixed_ends_can_be_offset_flipped_and_turned() {
    let cells = [
        vertex("box", "", "", [0.0, 0.0, 120.0, 60.0]),
        vertex(
            "flipped",
            "",
            "flipH=1;flipV=1;",
            [50.0, 200.0, 120.0, 60.0],
        ),
        vertex("turned", "", "rotation=90;", [300.0, 200.0, 120.0, 60.0]),
        edge(
            "e1",
            "box",
            "flipped",
            "exitX=1;exitY=0.5;exitDx=5;exitDy=-3;exitPerimeter=0;entryX=0;entryY=0.25;",
            "",
        ),
        edge("e2", "box", "turned", "entryX=1;entryY=0;", ""),
    ];
    let edges = edges(&convert(&page(&cells.concat())));
    let (e1, e2) = (&edges[0].1.path, &edges[1].1.path);
    assert_eq!(e1.source, at(65.0, -3.0));
    // The left side, a quarter down, flipped to the right, three down.
    assert_eq!(e1.target, at(60.0, 15.0));
    // The top right corner turned to the bottom right.
    assert_eq!(e2.target, at(30.0, 60.0));
}

#[test]
fn fixed_ends_move_onto_ellipses_and_rhombi_as_in_draw_io() {
    let cells = [
        vertex("ellipse", "", "ellipse;", [0.0, 0.0, 100.0, 60.0]),
        vertex("rhombus", "", "rhombus;", [200.0, 0.0, 100.0, 60.0]),
        vertex("box", "", "", [400.0, 0.0, 100.0, 60.0]),
    ];
    let mut cells = cells.concat();
    let ends = [
        ("ellipse", [0.25, 0.5], "rhombus", [0.5, 0.25]),
        ("ellipse", [0.5, 0.25], "rhombus", [0.5, 0.75]),
        ("ellipse", [0.5, 0.5], "rhombus", [0.25, 0.5]),
        ("box", [0.5, 0.5], "rhombus", [0.75, 0.5]),
        ("box", [0.25, 0.5], "rhombus", [0.25, 0.25]),
        ("box", [0.5, 0.0], "rhombus", [0.4, 0.4]),
        ("box", [0.5, 0.0], "rhombus", [0.5, 0.5]),
    ];
    for (i, (source, [ex, ey], target, [nx, ny])) in ends.iter().enumerate() {
        let style = format!("exitX={ex};exitY={ey};entryX={nx};entryY={ny};");
        cells += &edge(&format!("e{i}"), source, target, &style, "");
    }
    let edges = edges(&convert(&page(&cells)));
    let ports: Vec<(Point, Point)> = edges
        .iter()
        .map(|(_, g)| (g.path.source, g.path.target))
        .collect();
    assert_eq!(
        ports,
        [
            // Straight out from the center to the outline.
            (at(-50.0, 0.0), at(0.0, -30.0)),
            (at(0.0, -30.0), at(0.0, 30.0)),
            // The center stays, but not on a box or a rhombus.
            (at(0.0, 0.0), at(-50.0, 0.0)),
            (at(50.0, 0.0), at(50.0, 0.0)),
            // Inside a box is out to its side; on a rhombus stays.
            (at(-50.0, 0.0), at(-25.0, -15.0)),
            // Inside a rhombus is its center.
            (at(0.0, -30.0), at(0.0, 0.0)),
            (at(0.0, -30.0), at(0.0, 30.0)),
        ]
    );
}

#[test]
fn shapes_outside_any_layer_are_drawn() {
    let text = r#"<mxGraphModel><root><mxCell id="a" value="A" vertex="1"><mxGeometry width="80" height="30" as="geometry"/></mxCell></root></mxGraphModel>"#;
    assert_eq!(nodes(&convert(text))[0].labels[0].text, "A");
}

#[test]
fn partial_rectangles_draw_the_sides_they_leave_on() {
    let style = "shape=partialRectangle;bottom=0;strokeWidth=2;";
    let import = convert(&page(&vertex("p", "", style, [0.0, 0.0, 100.0, 50.0])));
    let geometry: Vec<Rect> = nodes(&import).iter().map(|n| n.geometry).collect();
    assert_eq!(
        geometry,
        [
            rect(0.0, 0.0, 100.0, 50.0),
            rect(0.0, -1.0, 100.0, 2.0),
            rect(99.0, 0.0, 2.0, 50.0),
            rect(-1.0, 0.0, 2.0, 50.0),
        ]
    );
}

#[test]
fn line_shapes_become_boxes_of_their_stroke() {
    let cells = [
        vertex(
            "h",
            "",
            "line;strokeColor=#ff0000;",
            [0.0, 0.0, 100.0, 40.0],
        ),
        vertex("v", "", "line;direction=north;", [200.0, 0.0, 40.0, 100.0]),
    ];
    let nodes = nodes(&convert(&page(&cells.concat())));
    assert_eq!(nodes[0].geometry, rect(0.0, 18.0, 100.0, 4.0));
    assert_eq!(nodes[1].geometry, rect(218.0, 0.0, 4.0, 100.0));
    assert_eq!(nodes[0].fill.color, Some(Color::rgb(0xFF, 0, 0)));
    assert_eq!(nodes[0].border.color, None);
}

#[test]
fn swimlanes_without_a_title_bar_keep_their_label_in_the_body() {
    for style in [
        "swimlane;startSize=0;",
        "swimlane;startSize=0;horizontal=0;",
    ] {
        let import = convert(&page(&vertex("s", "Lane", style, [0.0, 0.0, 200.0, 100.0])));
        let nodes = nodes(&import);
        assert_eq!(nodes.len(), 1, "{style}");
        assert_eq!(nodes[0].labels[0].text, "Lane");
    }
}

#[test]
fn lifelines_are_a_head_over_a_line() {
    let style = "shape=umlLifeline;size=50;";
    let import = convert(&page(&vertex("l", "A", style, [0.0, 0.0, 100.0, 300.0])));
    let nodes = nodes(&import);
    let geometry: Vec<Rect> = nodes.iter().map(|n| n.geometry).collect();
    assert_eq!(
        geometry,
        [rect(0.0, 0.0, 100.0, 50.0), rect(49.5, 50.0, 1.0, 250.0)]
    );
    assert_eq!(nodes[0].labels[0].text, "A");
}

#[test]
fn percent_encoded_svg_images_become_svg_nodes() {
    let svg = "%3Csvg xmlns=%22http://www.w3.org/2000/svg%22%3E%3Ccircle r=%225%22/%3E%3C/svg%3E";
    let style = format!("shape=image;image=data:image/svg+xml,{svg};");
    let import = convert(&page(&vertex("i", "", &style, [0.0, 0.0, 40.0, 40.0])));
    let NodeKind::Svg {
        resource: Some(resource),
    } = &nodes(&import)[0].kind
    else {
        panic!("not an SVG node");
    };
    let text = yed::resource_text(&import.graphml, resource).unwrap();
    assert_eq!(
        text,
        r#"<svg xmlns="http://www.w3.org/2000/svg"><circle r="5"/></svg>"#
    );
}

#[test]
fn a_page_grid_of_zero_is_draw_ios_default_for_loops() {
    let text = format!(
        r#"<mxGraphModel gridSize="0"><root><mxCell id="0"/><mxCell id="1" parent="0"/>{}{}</root></mxGraphModel>"#,
        vertex("a", "", "", [0.0, 0.0, 100.0, 50.0]),
        edge("e", "a", "a", "", ""),
    );
    let edges = edges(&convert(&text));
    assert_eq!(edges[0].1.path.points, [at(120.0, 15.0), at(120.0, 35.0)]);
}

#[test]
fn icons_show_their_image_and_label_shapes_their_box() {
    let svg = "PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciLz4=";
    let cells = [
        vertex(
            "i",
            "",
            &format!("icon;image=data:image/svg+xml,{svg};"),
            [0.0, 0.0, 40.0, 40.0],
        ),
        vertex(
            "l",
            "",
            &format!("label;image=data:image/svg+xml,{svg};"),
            [0.0, 0.0, 40.0, 40.0],
        ),
    ];
    let import = convert(&page(&cells.concat()));
    let nodes = nodes(&import);
    assert!(matches!(nodes[0].kind, NodeKind::Svg { .. }));
    assert_eq!(nodes[1].kind, NodeKind::Shape(ShapeType::RoundRectangle));
    assert_eq!(import.notes, ["1 image inside boxes was left out."]);
}

#[test]
fn groups_with_a_fill_or_border_are_drawn() {
    let cells = [
        vertex("g", "", "group;fillColor=#ff0000;", [0.0, 0.0, 100.0, 50.0]),
        vertex("h", "", "group;", [0.0, 100.0, 100.0, 50.0]),
    ];
    let nodes = nodes(&convert(&page(&cells.concat())));
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].fill.color, Some(Color::rgb(0xFF, 0, 0)));
}

#[test]
fn transparent_groups_keep_their_captions() {
    let cells = vertex(
        "g",
        "&amp;alpha; &amp;eacute; &amp;check;",
        "group;html=1;",
        [0.0, 0.0, 100.0, 50.0],
    );
    let import = convert(&page(&cells));
    let nodes = nodes(&import);
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].labels[0].text, "α é ✓");
    assert_eq!(nodes[0].fill.color, None);
    assert_eq!(nodes[0].border.color, None);
}

#[test]
fn colors_can_be_css_functions() {
    let cells = [
        vertex(
            "a",
            "",
            "fillColor=rgba(255,0,0,0.5);",
            [0.0, 0.0, 80.0, 40.0],
        ),
        vertex(
            "b",
            "",
            "fillColor=light-dark(rgb(1,2,3),#000000);",
            [0.0, 0.0, 80.0, 40.0],
        ),
    ];
    let nodes = nodes(&convert(&page(&cells.concat())));
    assert_eq!(
        nodes[0].fill.color,
        Some(Color {
            a: 128,
            ..Color::rgb(0xFF, 0, 0)
        })
    );
    assert_eq!(nodes[1].fill.color, Some(Color::rgb(1, 2, 3)));
}

#[test]
fn orthogonal_lines_run_straight_at_loose_ends_in_line() {
    let cells = [
        vertex("a", "", "", [0.0, 0.0, 100.0, 50.0]),
        edge(
            "e",
            "a",
            "",
            "edgeStyle=orthogonalEdgeStyle;",
            r#"<mxPoint x="50" y="200" as="targetPoint"/>"#,
        ),
    ];
    let edges = edges(&convert(&page(&cells.concat())));
    assert!(edges[0].1.path.points.is_empty(), "{:?}", edges[0].1.path);
}

#[test]
fn orthogonal_lines_keep_a_turnaround_past_aligned_ports() {
    let cells = [
        vertex("a", "", "", [0.0, 0.0, 80.0, 40.0]),
        vertex("b", "", "", [200.0, 0.0, 80.0, 40.0]),
        edge(
            "e",
            "a",
            "b",
            "edgeStyle=orthogonalEdgeStyle;exitX=1;exitY=0.5;entryX=1;entryY=0.5;",
            "",
        ),
    ];
    let import = convert(&page(&cells.concat()));
    let edges = edges(&import);
    let path = &edges[0].1.path;
    assert_eq!(path.source, at(40.0, 0.0));
    assert_eq!(path.target, at(40.0, 0.0));
    assert_eq!(path.points, [at(300.0, 20.0)]);
}

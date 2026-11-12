use drawio::{Error, File, Import};
use graphml::{
    Edge,
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
fn round_boxes_report_their_radius() {
    let cells = [
        vertex("a", "", "rounded=1;", [0.0, 0.0, 120.0, 60.0]),
        vertex(
            "b",
            "",
            "rounded=1;arcSize=50;absoluteArcSize=1;",
            [0.0, 0.0, 120.0, 60.0],
        ),
        vertex("c", "", "rounded=0;", [0.0, 0.0, 120.0, 60.0]),
    ];
    let import = convert(&page(&cells.concat()));
    assert_eq!(
        import.corner_radii,
        [("n0".to_owned(), 9.0), ("n1".to_owned(), 25.0)]
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

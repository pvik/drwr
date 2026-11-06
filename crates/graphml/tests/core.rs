use graphml::{
    AttrType, Direction, EdgeDefault, Error, GraphMl, KeyDomain, Position, xml::XmlNode,
};

const YED: &str = include_str!("fixtures/yed.graphml");

const HAND_WRITTEN: &str = r#"<?xml version='1.0' encoding='utf-8'?>
<!-- written by hand -->
<graphml xmlns="http://graphml.graphdrawing.org/xmlns">
  <desc>A test graph</desc>
  <key id="weight" for="edge" attr.name="weight" attr.type="double"><default>1.0</default></key>
  <key id="color" for="node" attr.name="color" attr.type="string"/>
  <graph id="G" edgedefault="undirected" parse.nodes="3">
    <node id="a"><data key="color">red</data><port name="north"><port name="inner"/></port></node>
    <edge source="a" target="b" directed="true" sourceport="north"><data key="weight">2.5</data></edge>
    <node id="b"/>
    <node id="c"><graph edgedefault="directed"><node id="c.1"/></graph></node>
    <hyperedge id="h"><endpoint node="a" port="north" type="in"/><endpoint node="b"/></hyperedge>
  </graph>
  <graph edgedefault="directed">
    <locator xmlns:xlink="http://www.w3.org/1999/xlink" xlink:href="other.graphml"/>
  </graph>
</graphml>
"#;

/// `HAND_WRITTEN` the way yEd would format it.
const HAND_WRITTEN_FORMATTED: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<graphml xmlns="http://graphml.graphdrawing.org/xmlns">
  <!-- written by hand -->
  <desc>A test graph</desc>
  <key attr.name="weight" attr.type="double" for="edge" id="weight">
    <default>1.0</default>
  </key>
  <key attr.name="color" attr.type="string" for="node" id="color"/>
  <graph edgedefault="undirected" id="G" parse.nodes="3">
    <node id="a">
      <data key="color">red</data>
      <port name="north">
        <port name="inner"/>
      </port>
    </node>
    <node id="b"/>
    <node id="c">
      <graph edgedefault="directed">
        <node id="c.1"/>
      </graph>
    </node>
    <edge directed="true" source="a" sourceport="north" target="b">
      <data key="weight">2.5</data>
    </edge>
    <hyperedge id="h">
      <endpoint node="a" port="north" type="in"/>
      <endpoint node="b"/>
    </hyperedge>
  </graph>
  <graph edgedefault="directed">
    <locator xlink:href="other.graphml" xmlns:xlink="http://www.w3.org/1999/xlink"/>
  </graph>
</graphml>
"#;

#[test]
fn yed_files_round_trip_byte_for_byte() {
    assert_eq!(GraphMl::parse(YED).unwrap().to_xml(), YED);
}

#[test]
fn parses_the_graphml_structure() {
    let doc = GraphMl::parse(HAND_WRITTEN).unwrap();
    assert_eq!(doc.comments, [" written by hand "]);
    assert_eq!(doc.desc.as_deref(), Some("A test graph"));

    let weight = &doc.keys[0];
    assert_eq!(weight.id, "weight");
    assert_eq!(weight.domain, KeyDomain::Edge);
    assert_eq!(weight.attr_name.as_deref(), Some("weight"));
    assert_eq!(weight.attr_type, Some(AttrType::Double));
    assert_eq!(weight.default.as_ref().unwrap().text(), "1.0");

    let graph = &doc.graphs[0];
    assert_eq!(graph.id.as_deref(), Some("G"));
    assert_eq!(graph.edge_default, EdgeDefault::Undirected);
    assert_eq!(graph.attributes["parse.nodes"], "3");
    let ids: Vec<_> = graph.nodes.iter().map(|node| node.id.as_str()).collect();
    assert_eq!(ids, ["a", "b", "c"]);

    let a = &graph.nodes[0];
    assert_eq!(a.data[0].key, "color");
    assert_eq!(a.data[0].text(), "red");
    assert_eq!(a.ports[0].name, "north");
    assert_eq!(a.ports[0].ports[0].name, "inner");
    assert_eq!(graph.nodes[2].graph.as_ref().unwrap().nodes[0].id, "c.1");

    let edge = &graph.edges[0];
    assert_eq!((edge.source.as_str(), edge.target.as_str()), ("a", "b"));
    assert_eq!(edge.id, None);
    assert_eq!(edge.directed, Some(true));
    assert_eq!(edge.source_port.as_deref(), Some("north"));
    assert_eq!(edge.data[0].text(), "2.5");

    let endpoints = &graph.hyperedges[0].endpoints;
    assert_eq!(endpoints[0].node, "a");
    assert_eq!(endpoints[0].port.as_deref(), Some("north"));
    assert_eq!(endpoints[0].direction, Some(Direction::In));
    assert_eq!(endpoints[1].direction, None);

    let locator = doc.graphs[1].locator.as_ref().unwrap();
    assert_eq!(locator.href(), Some("other.graphml"));
}

#[test]
fn writes_documents_formatted_like_yed() {
    let doc = GraphMl::parse(HAND_WRITTEN).unwrap();
    let text = doc.to_xml();
    assert_eq!(text, HAND_WRITTEN_FORMATTED);
    assert_eq!(GraphMl::parse(&text).unwrap(), doc);
}

#[test]
fn text_is_decoded_and_escaped_again() {
    let doc = GraphMl::parse(concat!(
        "<graphml>\r\n<graph><data key=\"d\">a &lt; b &amp;&#x26; \"q\"&#13;\r\n",
        "<![CDATA[<raw>]]></data></graph></graphml>"
    ))
    .unwrap();
    let content = &doc.graphs[0].data[0].content;
    assert_eq!(
        content,
        &[
            XmlNode::Text("a < b && \"q\"\r\n".into()),
            XmlNode::CData("<raw>".into())
        ]
    );
    let expected = "<data key=\"d\">a &lt; b &amp;&amp; \"q\"&#13;\n<![CDATA[<raw>]]></data>";
    assert!(doc.to_xml().contains(expected));
}

#[test]
fn attribute_values_are_normalized_and_escaped_again() {
    let doc = GraphMl::parse(
        "<graphml><graph><node id=\"a&#10;b\t&quot;c&quot; &lt;&gt;\"/></graph></graphml>",
    )
    .unwrap();
    assert_eq!(doc.graphs[0].nodes[0].id, "a\nb \"c\" <>");
    assert!(
        doc.to_xml()
            .contains(r#"<node id="a&#10;b &quot;c&quot; &lt;&gt;"/>"#)
    );
}

#[test]
fn whitespace_is_formatting_unless_preserved() {
    let doc = GraphMl::parse(
        r#"<graphml><graph>
          <data key="a">   </data>
          <data key="b" xml:space="preserve">   </data>
          <data key="c"> <x>  </x> <y xml:space="preserve"> <z/> </y></data>
          <data key="d">text<x/><y/>tail</data>
        </graph></graphml>"#,
    )
    .unwrap();
    let data = &doc.graphs[0].data;
    assert!(data[0].content.is_empty());
    assert_eq!(data[1].text(), "   ");

    let expected = r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<graphml>
  <graph edgedefault="directed">
    <data key="a"/>
    <data key="b" xml:space="preserve">   </data>
    <data key="c">
      <x/>
      <y xml:space="preserve"> <z/> </y>
    </data>
    <data key="d">text<x/>
      <y/>tail</data>
  </graph>
</graphml>
"#;
    assert_eq!(doc.to_xml(), expected);
}

#[test]
fn xml_space_is_inherited() {
    let doc = GraphMl::parse(concat!(
        r#"<graphml><key id="k"><default xml:space="preserve"> </default></key>"#,
        r#"<graph xml:space="preserve"><node id="a"><data key="k">  </data></node>"#,
        r#"<node id="b" xml:space="default"><data key="k">  </data></node></graph></graphml>"#,
    ))
    .unwrap();
    assert_eq!(doc.keys[0].default.as_ref().unwrap().text(), " ");
    let nodes = &doc.graphs[0].nodes;
    assert_eq!(nodes[0].data[0].text(), "  ");
    assert!(nodes[1].data[0].content.is_empty());
    assert_eq!(GraphMl::parse(&doc.to_xml()).unwrap(), doc);
}

#[test]
fn prefixed_documents_keep_their_prefix() {
    let text = concat!(
        r#"<g:graphml xmlns:g="http://graphml.graphdrawing.org/xmlns">"#,
        r#"<g:graph edgedefault="directed"><g:node id="a"/></g:graph></g:graphml>"#,
    );
    let expected = r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
<g:graphml xmlns:g="http://graphml.graphdrawing.org/xmlns">
  <g:graph edgedefault="directed">
    <g:node id="a"/>
  </g:graph>
</g:graphml>
"#;
    let doc = GraphMl::parse(text).unwrap();
    assert_eq!(doc.prefix.as_deref(), Some("g"));
    assert_eq!(doc.to_xml(), expected);
}

fn parse_error(text: &str) -> (Position, String) {
    match GraphMl::parse(text) {
        Err(Error::Parse { position, message }) => (position, message),
        other => panic!("expected a parse error, got {other:?}"),
    }
}

fn at(line: usize, column: usize, message: &str) -> (Position, String) {
    (Position { line, column }, message.to_owned())
}

#[test]
fn errors_say_what_is_wrong_and_where() {
    let cases = [
        (
            "<graphml>\n  <graph>\n    <node/>\n  </graph>\n</graphml>",
            at(3, 5, "<node> is missing the `id` attribute"),
        ),
        (
            "<graphml>\n<graph edgedefault=\"sideways\"/></graphml>",
            at(
                2,
                1,
                "invalid value \"sideways\" for `edgedefault` on <graph>",
            ),
        ),
        (
            r#"<graphml><graph><edge source="a" target="b"><node id="x"/></edge></graph></graphml>"#,
            at(1, 45, "unexpected <node> in <edge>"),
        ),
        (
            "<graphml><desc/><desc/></graphml>",
            at(1, 17, "unexpected <desc> in <graphml>"),
        ),
        (
            "<graphml><graph>text</graph></graphml>",
            at(1, 17, "unexpected text in <graph>"),
        ),
        ("<root/>", at(1, 1, "expected <graphml>, found <root>")),
        (
            r#"<g:graphml xmlns:g="ns"><graph/></g:graphml>"#,
            at(1, 25, "unexpected <graph> in <g:graphml>"),
        ),
        (
            r#"<graphml xmlns:y="y"><y:graph/></graphml>"#,
            at(1, 22, "unexpected <y:graph> in <graphml>"),
        ),
        (
            r#"<graphml><data key="k">&nbsp;</data></graphml>"#,
            at(1, 24, "unknown entity &nbsp;"),
        ),
        (
            r#"<graphml><key id="&nbsp;"/></graphml>"#,
            at(1, 10, "invalid `id` on <key>: unknown entity &nbsp;"),
        ),
        (
            r#"<graphml a="1" a="2"/>"#,
            at(1, 16, "duplicate attribute"),
        ),
        (
            "<graphml a=1/>",
            at(1, 12, "attribute values must be quoted"),
        ),
        ("<graphml>\n  <graph>", at(2, 3, "<graph> is never closed")),
        (
            "<graphml>\n  <graph><data key=\"k\">",
            at(2, 10, "<data> is never closed"),
        ),
        (
            "<graphml>\n  <graph>\n</graphml>",
            at(
                3,
                1,
                "ill-formed document: expected `</graph>`, but `</graphml>` was found",
            ),
        ),
    ];
    for (text, expected) in cases {
        assert_eq!(parse_error(text), expected, "{text:?}");
    }
}

#[test]
fn error_messages_include_the_position() {
    let err = GraphMl::parse("<graphml>\n  <node/>").unwrap_err();
    assert_eq!(
        err.to_string(),
        "line 2, column 3: unexpected <node> in <graphml>"
    );
}

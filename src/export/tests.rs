use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use egui::{Pos2, RawInput, pos2};
use graphml::GraphMl;
use graphml::yed::{self, EdgeGraphics, EdgeKind, LineType, NodeGraphics, NodeKind, ShapeType};
use resvg::tiny_skia::Pixmap;

use super::*;
use crate::document::{self, Item};
use crate::scene::Scene;
use crate::selection::Selection;

const RED: Color = Color::rgb(0xFF, 0x00, 0x00);
const BLUE: Color = Color::rgb(0x00, 0x00, 0xFF);

/// What the canvas draws of `graphml`, or of the items `only` has, with the
/// labels in egui's own fonts.
fn drawing(graphml: &GraphMl, only: Option<&Selection>) -> Drawing {
    let ctx = egui::Context::default();
    ctx.add_font_provider(Arc::new(Fonts::default()));
    let scene = Scene::read(graphml);
    let mut drawing = None;
    ctx.run_ui(RawInput::default(), |ui| {
        drawing = Some(Drawing::of(ui.ctx(), &scene, only));
    })
    .drop_without_applying_deltas();
    drawing.expect("the frame ran")
}

fn shape(shape: ShapeType, x: f64, y: f64, width: f64, height: f64) -> NodeGraphics {
    let mut graphics = NodeGraphics::new(NodeKind::Shape(shape));
    graphics.geometry = yed::Rect {
        x,
        y,
        width,
        height,
    };
    graphics
}

/// A red box labelled "A" at (100, 50), 150×80, then a circle 80 across at
/// (300, 50) with a blue dashed border 2 wide and no fill, then an arrow from
/// the box to the circle. Returns the document and the nodes' ids.
fn two_nodes() -> (GraphMl, String, String) {
    let mut graphml = yed::new_document();
    let mut a = shape(ShapeType::Rectangle, 100.0, 50.0, 150.0, 80.0);
    a.fill.color = Some(RED);
    a.labels = vec![yed::Label::new("A")];
    let mut b = shape(ShapeType::Ellipse, 300.0, 50.0, 80.0, 80.0);
    b.fill.color = None;
    b.border = yed::Stroke {
        color: Some(BLUE),
        line_type: LineType::Dashed,
        width: 2.0,
    };
    let a = document::add_node(&mut graphml, &a);
    let b = document::add_node(&mut graphml, &b);
    let mut arrow = EdgeGraphics::new(EdgeKind::PolyLine);
    arrow.arrows.target = yed::Arrow::Standard;
    document::add_edge(&mut graphml, &a, &b, &arrow);
    (graphml, a, b)
}

fn svg(drawing: &Drawing, settings: Settings) -> String {
    let settings = Settings {
        format: Format::Svg,
        ..settings
    };
    let bytes = export(drawing, &settings, &Fonts::default()).unwrap();
    String::from_utf8(bytes).unwrap()
}

fn png(drawing: &Drawing, settings: Settings) -> Pixmap {
    let settings = Settings {
        format: Format::Png,
        ..settings
    };
    let bytes = export(drawing, &settings, &Fonts::default()).unwrap();
    Pixmap::decode_png(&bytes).unwrap()
}

/// The elements of `svg` named `name`, e.g. "rect", one per line.
fn elements<'a>(svg: &'a str, name: &str) -> Vec<&'a str> {
    let start = format!("<{name} ");
    svg.lines()
        .filter(|line| line.starts_with(&start))
        .collect()
}

/// The color of the pixel at `at`, unmultiplied.
fn pixel(pixmap: &Pixmap, at: Pos2) -> [u8; 4] {
    let color = pixmap.pixel(at.x as u32, at.y as u32).unwrap().demultiply();
    [color.red(), color.green(), color.blue(), color.alpha()]
}

#[test]
fn svg_draws_what_the_canvas_draws() {
    let (graphml, ..) = two_nodes();
    let svg = svg(&drawing(&graphml, None), Settings::default());
    assert_eq!(
        elements(&svg, "rect")[1],
        r##"<rect x="100" y="50" width="150" height="80" fill="#FF0000" stroke="#000000" stroke-width="1"/>"##
    );
    assert_eq!(
        elements(&svg, "ellipse"),
        [
            r##"<ellipse cx="340" cy="90" rx="40" ry="40" fill="none" stroke="#0000FF" stroke-width="2" stroke-dasharray="8 4"/>"##
        ]
    );
    // From the box's side to the circle's, cut back to the arrowhead's notch.
    let paths = elements(&svg, "path");
    assert_eq!(
        paths[0],
        r##"<path d="M250 90L293 90" fill="none" stroke="#000000" stroke-width="1"/>"##
    );
    assert_eq!(
        paths[1],
        r##"<path d="M300 90L290 94L293 90L290 86Z" fill="#000000"/>"##
    );
    let [text] = elements(&svg, "text").try_into().unwrap();
    assert!(text.contains(r#"font-size="12""#), "{text}");
    assert!(text.contains(r#"text-anchor="middle""#), "{text}");
    assert!(text.contains(r#"<tspan x="175" "#), "{text}");
    assert!(text.ends_with(">A</tspan></text>"), "{text}");
    usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("the SVG is valid");
}

#[test]
fn the_page_is_what_the_drawing_covers_and_the_margin() {
    let (graphml, ..) = two_nodes();
    let drawing = drawing(&graphml, None);
    // The box's border reaches half its width out, the circle's 1 point.
    assert_eq!(
        drawing.bounds,
        Rect::from_min_max(pos2(99.5, 49.0), pos2(381.0, 131.0))
    );
    let svg = svg(&drawing, Settings::default());
    assert!(
        svg.contains(r#"width="301.5" height="102" viewBox="89.5 39 301.5 102""#),
        "{svg}"
    );
    let svg = svg_with_margin(&drawing, 0);
    assert!(svg.contains(r#"viewBox="99.5 49 281.5 82""#), "{svg}");
}

fn svg_with_margin(drawing: &Drawing, margin: u32) -> String {
    svg(
        drawing,
        Settings {
            margin,
            ..Settings::default()
        },
    )
}

#[test]
fn labels_and_arrowheads_outside_nodes_count_toward_the_page() {
    let mut graphml = yed::new_document();
    let mut node = shape(ShapeType::Rectangle, 0.0, 0.0, 10.0, 10.0);
    node.labels = vec![yed::Label::new("A label wider than its node")];
    document::add_node(&mut graphml, &node);
    let bounds = drawing(&graphml, None).bounds;
    assert!(bounds.width() > 50.0, "{bounds:?}");

    let (mut graphml, a, b) = two_nodes();
    // Sideways, so the arrowhead reaches past the circle's top.
    let mut arrow = EdgeGraphics::new(EdgeKind::PolyLine);
    arrow.arrows.source = yed::Arrow::Standard;
    arrow.line.width = 3.0;
    arrow.path.points = vec![yed::Point { x: 340.0, y: 0.0 }];
    document::add_edge(&mut graphml, &b, &a, &arrow);
    assert!(drawing(&graphml, None).bounds.min.y < 0.0);
}

#[test]
fn only_the_selected_items_are_exported() {
    let (graphml, a, b) = two_nodes();
    let only = |items: &[Item]| {
        svg(
            &drawing(&graphml, Some(&Selection::of(items.to_vec()))),
            Settings::default(),
        )
    };
    let svg = only(&[Item::Node(a.clone())]);
    assert_eq!(elements(&svg, "rect").len(), 2, "{svg}");
    assert!(elements(&svg, "ellipse").is_empty(), "{svg}");
    assert!(elements(&svg, "path").is_empty(), "{svg}");
    assert!(svg.contains(r#"viewBox="89.5 39.5 171 101""#), "{svg}");

    // Like Copy, a line between selected nodes only if it's selected too.
    let svg = only(&[Item::Node(a.clone()), Item::Node(b.clone())]);
    assert!(elements(&svg, "path").is_empty(), "{svg}");
    let svg = only(&[Item::Node(a), Item::Node(b), Item::Edge(0)]);
    assert_eq!(elements(&svg, "path").len(), 2, "{svg}");
    let svg = only(&[Item::Edge(0)]);
    assert_eq!(
        elements(&svg, "rect").len(),
        1,
        "only the background: {svg}"
    );
    assert_eq!(elements(&svg, "path").len(), 2, "{svg}");
}

#[test]
fn rounded_boxes_flat_shapes_and_line_types() {
    let mut graphml = yed::new_document();
    let id = document::add_node(
        &mut graphml,
        &shape(ShapeType::Rectangle, 0.0, 0.0, 40.0, 20.0),
    );
    document::set_corner_radius(&mut graphml, &id, 6.0);
    let mut flat = shape(ShapeType::Rectangle, 0.0, 30.0, 40.0, 0.0);
    flat.border.line_type = LineType::DashedDotted;
    flat.border.width = 3.0;
    document::add_node(&mut graphml, &flat);
    let mut dotted = shape(ShapeType::Ellipse, 50.0, 0.0, 20.0, 10.0);
    // Thinner than a point, its dots are still a point.
    dotted.border.line_type = LineType::Dotted;
    dotted.border.width = 0.5;
    document::add_node(&mut graphml, &dotted);

    let svg = svg(&drawing(&graphml, None), Settings::default());
    assert!(
        elements(&svg, "rect")[1].contains(r#"width="40" height="20" rx="6" ry="6""#),
        "{svg}"
    );
    assert_eq!(
        elements(&svg, "path"),
        [
            r##"<path d="M0 30H40V30H0Z" fill="none" stroke="#000000" stroke-width="3" stroke-dasharray="12 6 3 6"/>"##
        ]
    );
    assert!(
        elements(&svg, "ellipse")[0].ends_with(r#"stroke-width="0.5" stroke-dasharray="1 1"/>"#),
        "{svg}"
    );
}

#[test]
fn colors_keep_their_alpha() {
    let mut graphml = yed::new_document();
    let mut node = shape(ShapeType::Rectangle, 0.0, 0.0, 10.0, 10.0);
    node.fill.color = Some(Color { a: 0x80, ..RED });
    node.border.color = None;
    document::add_node(&mut graphml, &node);
    let svg = svg(&drawing(&graphml, None), Settings::default());
    assert!(
        elements(&svg, "rect")[1].ends_with(r##"fill="#FF0000" fill-opacity="0.5"/>"##),
        "{svg}"
    );
}

#[test]
fn transparent_exports_have_no_background() {
    let (graphml, ..) = two_nodes();
    let drawing = drawing(&graphml, None);
    let transparent = Settings {
        transparent: true,
        ..Settings::default()
    };
    assert_eq!(
        elements(&svg(&drawing, transparent.clone()), "rect").len(),
        1
    );
    assert_eq!(pixel(&png(&drawing, transparent), pos2(1.0, 1.0))[3], 0);
    assert_eq!(
        pixel(&png(&drawing, Settings::default()), pos2(1.0, 1.0)),
        [0xFF, 0xFF, 0xFF, 0xFF]
    );
}

#[test]
fn pngs_are_the_page_at_the_scale() {
    let (graphml, ..) = two_nodes();
    let drawing = drawing(&graphml, None);
    // 301.5×102 points, the last pixel partly past the page.
    let pixmap = png(&drawing, Settings::default());
    assert_eq!((pixmap.width(), pixmap.height()), (302, 102));
    let doubled = Settings {
        scale: 200,
        ..Settings::default()
    };
    let pixmap = png(&drawing, doubled);
    assert_eq!((pixmap.width(), pixmap.height()), (603, 204));
    // The box's fill, from the page's corner at (89.5, 39).
    let to_pixels = |x: f32, y: f32| pos2(x - 89.5, y - 39.0) * 2.0;
    assert_eq!(pixel(&pixmap, to_pixels(120.0, 60.0)), [0xFF, 0, 0, 0xFF]);
    // The circle's border, on its left side.
    let [r, g, b, _] = pixel(&pixmap, to_pixels(300.0, 90.0));
    assert!(b > 0xC0 && r < 0x40 && g < 0x40, "{:?}", [r, g, b]);
    // Inside the circle, which isn't filled.
    assert_eq!(pixel(&pixmap, to_pixels(340.0, 70.0)), [0xFF; 4]);
}

#[test]
fn png_text_is_drawn() {
    let (graphml, ..) = two_nodes();
    let pixmap = png(&drawing(&graphml, None), Settings::default());
    // Around the "A" in the middle of the box, from the page's corner.
    let middle = pos2(175.0 - 89.5, 90.0 - 39.0);
    let dark = (-6..6)
        .flat_map(|dx| (-6..6).map(move |dy| middle + egui::vec2(dx as f32, dy as f32)))
        .filter(|&at| pixel(&pixmap, at)[0] < 0x80)
        .count();
    assert!(dark > 5, "{dark} dark pixels");
}

#[test]
fn svg_text_can_be_outlines() {
    let (graphml, ..) = two_nodes();
    let drawing = drawing(&graphml, None);
    let outlined = Settings {
        outlines: true,
        ..Settings::default()
    };
    let svg = svg(&drawing, outlined);
    assert!(elements(&svg, "text").is_empty(), "{svg}");
    let glyphs = elements(&svg, "path").into_iter().last().unwrap();
    assert!(glyphs.starts_with(r#"<path d="M"#), "{glyphs}");
    assert!(glyphs.ends_with(r##"Z" fill="#000000"/>"##), "{glyphs}");
    usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("the SVG is valid");
}

#[test]
fn font_families_name_the_drawn_family_then_a_generic_one() {
    let family = |family: &str, style: yed::FontStyle| {
        let mut graphml = yed::new_document();
        let mut node = shape(ShapeType::Rectangle, 0.0, 0.0, 10.0, 10.0);
        let mut label = yed::Label::new("x");
        label.font.family = family.to_owned();
        label.font.style = style;
        node.labels = vec![label];
        document::add_node(&mut graphml, &node);
        let svg = svg(&drawing(&graphml, None), Settings::default());
        let [text] = elements(&svg, "text").try_into().unwrap();
        text.split_once(" font-size").unwrap().0.to_owned()
    };
    // Without the system's fonts, egui's own.
    assert_eq!(
        family("Dialog", yed::FontStyle::Plain),
        r#"<text font-family="'Ubuntu Light', sans-serif""#
    );
    assert_eq!(
        family("Monospaced", yed::FontStyle::Plain),
        r#"<text font-family="'Hack', monospace""#
    );
    // One that's missing, first for where it isn't.
    assert_eq!(
        family("DejaVu Serif", yed::FontStyle::Plain),
        r#"<text font-family="'DejaVu Serif', 'Ubuntu Light', serif""#
    );
    let mut graphml = yed::new_document();
    let mut node = shape(ShapeType::Rectangle, 0.0, 0.0, 10.0, 10.0);
    let mut label = yed::Label::new("x");
    label.font.style = yed::FontStyle::BoldItalic;
    node.labels = vec![label];
    document::add_node(&mut graphml, &node);
    let svg = svg(&drawing(&graphml, None), Settings::default());
    assert!(
        svg.contains(r#" font-weight="bold" font-style="italic""#),
        "{svg}"
    );
}

#[test]
fn glyphs_a_font_lacks_are_outlined_in_another() {
    // Ubuntu Light has no box drawing, which Hack has.
    let outline = |family: &str| {
        let mut graphml = yed::new_document();
        let mut label = yed::Label::new("─");
        label.font.family = family.to_owned();
        let mut node = shape(ShapeType::Rectangle, 0.0, 0.0, 10.0, 10.0);
        node.labels = vec![label];
        document::add_node(&mut graphml, &node);
        let outlined = Settings {
            outlines: true,
            ..Settings::default()
        };
        let svg = svg(&drawing(&graphml, None), outlined);
        elements(&svg, "path")
            .into_iter()
            .last()
            .unwrap()
            .to_owned()
    };
    assert!(
        !Fonts::default()
            .face("Dialog", &yed::FontStyle::Plain)
            .has('─')
    );
    assert_eq!(outline("Dialog"), outline("Monospaced"));
}

#[test]
fn text_is_escaped_for_xml() {
    assert_eq!(
        svg::escape("<a> & \"b\"\u{1}\u{FFFF}\tc"),
        "&lt;a&gt; &amp; &quot;b&quot;\tc"
    );
    let mut graphml = yed::new_document();
    let mut node = shape(ShapeType::Rectangle, 0.0, 0.0, 10.0, 10.0);
    node.labels = vec![yed::Label::new("x < y & z")];
    document::add_node(&mut graphml, &node);
    let svg = svg(&drawing(&graphml, None), Settings::default());
    assert!(svg.contains(">x &lt; y &amp; z</tspan>"), "{svg}");
    usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("the SVG is valid");
}

#[test]
fn numbers_have_at_most_two_decimals() {
    assert_eq!(svg::num(1.0), "1");
    assert_eq!(svg::num(2.5), "2.5");
    assert_eq!(svg::num(10.125), "10.13");
    assert_eq!(svg::num(-0.001), "0");
    assert_eq!(svg::num(-3.333), "-3.33");
}

#[test]
fn an_empty_drawing_has_nothing_to_export() {
    let drawing = drawing(&yed::new_document(), None);
    assert!(matches!(
        export(&drawing, &Settings::default(), &Fonts::default()),
        Err(Error::Empty)
    ));
    assert_eq!(page(drawing.bounds, 10), None);
}

#[test]
fn pngs_over_the_limit_are_refused() {
    let page = Rect::from_min_size(Pos2::ZERO, egui::vec2(20_000.0, 10_000.0));
    assert_eq!(pixels(page, 50), [10_000, 5_000]);
    assert!(!too_large(pixels(page, 50)));
    assert!(too_large(pixels(page, 100)));
    let mut graphml = yed::new_document();
    document::add_node(
        &mut graphml,
        &shape(ShapeType::Rectangle, 0.0, 0.0, 20_000.0, 10_000.0),
    );
    let drawing = drawing(&graphml, None);
    assert!(matches!(
        export(&drawing, &Settings::default(), &Fonts::default()),
        Err(Error::TooLarge)
    ));
    let svg = Settings {
        format: Format::Svg,
        ..Settings::default()
    };
    assert!(export(&drawing, &svg, &Fonts::default()).is_ok());
}

/// A red disc 20 across on a 40 by 30 canvas, which an SVG node at (100, 50)
/// stretches to 80×40. Returns the document.
fn red_disc() -> GraphMl {
    let mut graphml = yed::new_document();
    let disc = r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="30"><circle cx="15" cy="12" r="10" fill="red"/></svg>"#;
    let geometry = yed::Rect {
        x: 100.0,
        y: 50.0,
        width: 80.0,
        height: 40.0,
    };
    let id = document::add_svg_node(&mut graphml, disc, geometry);
    document::set_label(
        &mut graphml,
        &document::LabelRef {
            item: Item::Node(id),
            index: 0,
        },
        "Disc",
        &crate::style::Style::default(),
    );
    graphml
}

#[test]
fn svg_nodes_embed_their_drawing_showing_what_it_draws() {
    let graphml = red_disc();
    let svg = svg(&drawing(&graphml, None), Settings::default());
    let viewport = r#"<svg x="100" y="50" width="80" height="40" viewBox="5 2 20 20" preserveAspectRatio="none">"#;
    // After the page's own.
    assert_eq!(elements(&svg, "svg")[1..], [viewport]);
    let [image] = elements(&svg, "image").try_into().unwrap();
    let data = image
        .strip_prefix(r#"<image width="40" height="30" href="data:image/svg+xml;base64,"#)
        .and_then(|image| image.strip_suffix(r#""/>"#))
        .unwrap();
    let embedded = STANDARD.decode(data).unwrap();
    let text = yed::resource_text(&graphml, "1").unwrap();
    assert_eq!(String::from_utf8(embedded).unwrap(), text);
    // Without its fill or border, but with its label.
    assert_eq!(elements(&svg, "rect").len(), 1);
    assert_eq!(elements(&svg, "text").len(), 1);
    usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("the SVG is valid");
}

#[test]
fn pngs_draw_svg_nodes_stretched_as_on_the_canvas() {
    let graphml = red_disc();
    let transparent = Settings {
        transparent: true,
        margin: 0,
        ..Settings::default()
    };
    let pixmap = png(&drawing(&graphml, None), transparent);
    // The page starts at the node's corner, and the label is below it.
    let at = |x: f32, y: f32| pos2(x - 100.0, y - 50.0);
    assert_eq!(pixel(&pixmap, at(140.0, 70.0)), [0xFF, 0, 0, 0xFF]);
    // The disc is an ellipse touching the node's sides.
    assert_eq!(pixel(&pixmap, at(102.0, 70.0)), [0xFF, 0, 0, 0xFF]);
    assert_eq!(pixel(&pixmap, at(140.0, 51.0)), [0xFF, 0, 0, 0xFF]);
    assert_eq!(pixel(&pixmap, at(103.0, 53.0))[3], 0);
    assert_eq!(pixel(&pixmap, at(177.0, 87.0))[3], 0);
}

use graphml::yed::{EdgeKind, Fill, Font, NodeKind, ShapeType};

use super::*;
use crate::document::Item;

const RED: Color = Color::rgb(0xFF, 0x00, 0x00);

/// A new node, edge and label in `style`.
fn new_items(style: &Style) -> (NodeGraphics, EdgeGraphics, Label) {
    let mut node = NodeGraphics::new(NodeKind::Shape(ShapeType::Rectangle));
    style.apply_to_node(&mut node);
    let mut edge = EdgeGraphics::new(EdgeKind::PolyLine);
    style.apply_to_edge(&mut edge);
    let mut label = Label::new("New");
    style.apply_to_label(&mut label);
    (node, edge, label)
}

#[test]
fn what_was_never_set_keeps_its_default() {
    let (node, edge, label) = new_items(&Style::default());
    assert_eq!(
        node,
        NodeGraphics::new(NodeKind::Shape(ShapeType::Rectangle))
    );
    assert_eq!(edge, EdgeGraphics::new(EdgeKind::PolyLine));
    assert_eq!(label, Label::new("New"));
    assert_eq!(Style::default().corner_radius(), None);
}

#[test]
fn borders_and_lines_are_remembered_apart() {
    let mut style = Style::default();
    style.set_line_width(&Selection::node("n0"), 2.0);
    style.set_line_type(&Selection::edge(0), &LineType::Dotted);
    style.set_color(&Selection::edge(0), ColorProperty::Line, Some(RED));
    let (node, edge, _) = new_items(&style);
    let border = Stroke {
        width: 2.0,
        ..Stroke::default()
    };
    assert_eq!(node.border, border);
    let line = Stroke {
        color: Some(RED),
        line_type: LineType::Dotted,
        width: 1.0,
    };
    assert_eq!(edge.line, line);
}

#[test]
fn an_edit_to_nodes_and_edges_together_is_remembered_for_both() {
    let mut style = Style::default();
    let both = Selection::of([Item::Node("n0".to_owned()), Item::Edge(0)]);
    style.set_color(&both, ColorProperty::Line, None);
    style.set_line_width(&both, 3.0);
    style.set_line_type(&both, &LineType::Dashed);
    let (node, edge, _) = new_items(&style);
    let stroke = Stroke {
        color: None,
        line_type: LineType::Dashed,
        width: 3.0,
    };
    assert_eq!(node.border, stroke);
    assert_eq!(edge.line, stroke);
}

#[test]
fn fills_and_corner_radii_are_remembered_only_from_nodes() {
    let mut style = Style::default();
    for property in [ColorProperty::Fill, ColorProperty::Gradient] {
        style.set_color(&Selection::edge(0), property, Some(RED));
    }
    style.set_corner_radius(&Selection::edge(0), 10.0);
    assert_eq!(style, Style::default());

    let node = Selection::node("n0");
    style.set_color(&node, ColorProperty::Fill, None);
    style.set_color(&node, ColorProperty::Gradient, Some(RED));
    style.set_corner_radius(&node, 10.0);
    let (graphics, _, _) = new_items(&style);
    let fill = Fill {
        color: None,
        gradient: Some(RED),
    };
    assert_eq!(graphics.fill, fill);
    assert_eq!(style.corner_radius(), Some(10.0));
}

#[test]
fn labels_get_each_font_part_and_the_text_color_last_set() {
    let mut style = Style::default();
    style.set_font(&FontPart::Family("Monospaced".to_owned()));
    style.set_font(&FontPart::Style(FontStyle::Bold));
    style.set_font(&FontPart::Size(20));
    style.set_color(&Selection::node("n0"), ColorProperty::Text(0), Some(RED));
    // Text can't have no color.
    style.set_color(&Selection::node("n0"), ColorProperty::Text(0), None);
    let (_, _, label) = new_items(&style);
    let font = Font {
        family: "Monospaced".to_owned(),
        size: 20,
        style: FontStyle::Bold,
    };
    assert_eq!(label.font, font);
    assert_eq!(label.text_color, RED);
}

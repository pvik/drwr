use super::*;
use crate::style::Style;

/// The light blue new nodes get.
const LIGHT_BLUE: yed::Color = yed::Color::rgb(0xAD, 0xD8, 0xE6);

#[test]
fn new_nodes_get_the_fill_and_border_last_set() {
    let mut harness = Harness::new();
    click_border_width(&mut harness);
    harness.type_text("3");
    harness.click_text("#FFCC00");
    harness.run(vec![]);
    harness.run(vec![Event::Text("#FF0000".to_owned())]);
    harness.press(Modifiers::NONE, Key::Enter);
    for action in [
        Action::SetColor(ColorProperty::Gradient, Some(yed::Color::WHITE)),
        Action::SetColor(ColorProperty::Border, None),
        Action::SetLineType(yed::LineType::Dashed),
    ] {
        harness.perform(action);
    }

    let fill = yed::Fill {
        color: Some(RED),
        gradient: Some(yed::Color::WHITE),
    };
    let border = yed::Stroke {
        color: None,
        line_type: yed::LineType::Dashed,
        width: 3.0,
    };
    for (tool, x) in [
        (Tool::Box, 20.0),
        (Tool::Circle, 200.0),
        (Tool::Ellipse, 380.0),
    ] {
        let id = harness.draw_node(tool, vec2(x, 200.0), vec2(x + 120.0, 260.0));
        let graphics = harness.graphics(&id);
        assert_eq!(graphics.fill, fill, "{tool:?}");
        assert_eq!(graphics.border, border, "{tool:?}");
    }
}

#[test]
fn new_lines_get_the_line_last_set_and_new_nodes_keep_their_look() {
    let mut harness = Harness::new();
    let graphml = harness.app.doc.graphml_mut();
    let left = document::add_node(graphml, &new_box());
    let right = document::add_node(graphml, &box_at(300.0, 0.0));
    document::add_edge(graphml, &left, &right, &edge_through(&[]));
    harness.app.selected = Selection::edge(0);
    for action in [
        Action::SetColor(ColorProperty::Line, Some(RED)),
        Action::SetLineWidth(2.5),
        Action::SetLineType(yed::LineType::Dotted),
        // Arrowheads are up to the tool.
        Action::SetArrow(document::EdgeEnd::Source, true),
    ] {
        harness.perform(action);
    }

    let below = harness.draw_node(Tool::Box, vec2(100.0, 250.0), vec2(250.0, 330.0));
    let graphics = harness.graphics(&below);
    assert_eq!(graphics.fill.color, Some(LIGHT_BLUE));
    assert_eq!(graphics.border, yed::Stroke::default());
    let line = yed::Stroke {
        color: Some(RED),
        line_type: yed::LineType::Dotted,
        width: 2.5,
    };
    for (tool, target) in [
        (Tool::Line, yed::Arrow::None),
        (Tool::Arrow, yed::Arrow::Standard),
    ] {
        let index = harness.draw_line(tool, vec2(175.0, 290.0), vec2(175.0, 90.0));
        let edge = harness.edge(index);
        assert_eq!(edge.line, line, "{tool:?}");
        let arrows = yed::Arrows {
            source: yed::Arrow::None,
            target,
        };
        assert_eq!(edge.arrows, arrows, "{tool:?}");
    }
}

#[test]
fn line_rows_set_on_nodes_and_lines_together_restyle_new_ones_of_both() {
    let mut harness = Harness::new();
    let graphml = harness.app.doc.graphml_mut();
    let left = document::add_node(graphml, &new_box());
    let right = document::add_node(graphml, &box_at(300.0, 0.0));
    document::add_edge(graphml, &left, &right, &edge_through(&[]));
    harness.app.selected = Selection::of([node(&left), Item::Edge(0)]);
    harness.click_text("#000000");
    assert!(harness.painted("Line color"));
    harness.run(vec![Event::Text("#FF0000".to_owned())]);
    harness.press(Modifiers::NONE, Key::Enter);
    harness.click_number_field(0);
    harness.type_text("4");
    harness.press(Modifiers::NONE, Key::Enter);
    harness.click_text("Solid");
    harness.click_text("Dashed");

    let stroke = yed::Stroke {
        color: Some(RED),
        line_type: yed::LineType::Dashed,
        width: 4.0,
    };
    let below = harness.draw_node(Tool::Box, vec2(100.0, 250.0), vec2(250.0, 330.0));
    assert_eq!(harness.graphics(&below).border, stroke);
    let index = harness.draw_line(Tool::Line, vec2(175.0, 290.0), vec2(475.0, 90.0));
    assert_eq!(harness.edge(index).line, stroke);
}

#[test]
fn new_items_keep_the_defaults_of_what_was_never_set() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.app.selected = Selection::node(id);
    harness.perform(Action::SetLineWidth(2.0));

    let below = harness.draw_node(Tool::Box, vec2(100.0, 250.0), vec2(250.0, 330.0));
    let graphics = harness.graphics(&below);
    let fill = yed::Fill {
        color: Some(LIGHT_BLUE),
        gradient: None,
    };
    assert_eq!(graphics.fill, fill);
    let border = yed::Stroke {
        width: 2.0,
        ..yed::Stroke::default()
    };
    assert_eq!(graphics.border, border);
    // With square corners.
    assert_eq!(graphics.kind, NodeKind::Shape(ShapeType::Rectangle));
    let index = harness.draw_line(Tool::Line, vec2(175.0, 290.0), vec2(175.0, 90.0));
    assert_eq!(harness.edge(index).line, yed::Stroke::default());
    harness.type_label("New");
    let label = &harness.edge(index).labels[0];
    assert_eq!(label.font, yed::Font::default());
    assert_eq!(label.text_color, yed::Color::BLACK);
}

#[test]
fn new_labels_get_the_font_and_text_color_last_set() {
    let mut harness = Harness::new();
    let graphml = harness.app.doc.graphml_mut();
    let styled = document::add_node(graphml, &labelled_box("Styled"));
    let mut kept = labelled_box("Kept");
    kept.geometry.x = 400.0;
    let kept = document::add_node(graphml, &kept);
    harness.app.selected = Selection::node(styled.clone());
    let label = LabelRef {
        item: node(&styled),
        index: 0,
    };
    for action in [
        Action::SetFont(label.clone(), FontPart::Family("Monospaced".to_owned())),
        Action::SetFont(label.clone(), FontPart::Style(yed::FontStyle::Bold)),
        Action::SetFont(label, FontPart::Size(20)),
        Action::SetColor(ColorProperty::Text(0), Some(RED)),
    ] {
        harness.perform(action);
    }
    let font = yed::Font {
        family: "Monospaced".to_owned(),
        size: 20,
        style: yed::FontStyle::Bold,
    };

    // Typed in that font, too.
    let below = harness.draw_node(Tool::Box, vec2(100.0, 250.0), vec2(250.0, 330.0));
    harness.press(CTRL, Key::Enter);
    harness.run(vec![]);
    harness.type_text("New");
    let family = fonts::family("Monospaced", &yed::FontStyle::Bold);
    let typed = harness.canvas_text("New").font_id;
    assert_eq!(typed, egui::FontId::new(20.0, family));
    harness.press(Modifiers::NONE, Key::Enter);
    let new = &harness.graphics(&below).labels[0];
    assert_eq!(
        (new.text.as_str(), &new.font, new.text_color),
        ("New", &font, RED)
    );

    let index = harness.draw_line(Tool::Line, vec2(175.0, 290.0), vec2(175.0, 90.0));
    harness.type_label("Line");
    let new = &harness.edge(index).labels[0];
    assert_eq!(
        (new.text.as_str(), &new.font, new.text_color),
        ("Line", &font, RED)
    );

    // Labels there already keep theirs.
    harness.app.selected = Selection::node(kept.clone());
    harness.type_label("Retyped");
    let retyped = &harness.graphics(&kept).labels[0];
    assert_eq!(retyped.text, "Retyped");
    assert_eq!(retyped.font, yed::Font::default());
    assert_eq!(retyped.text_color, yed::Color::BLACK);
}

#[test]
fn labels_added_in_the_side_panel_or_its_dialog_get_the_font_last_set() {
    let mut harness = Harness::new();
    let graphml = harness.app.doc.graphml_mut();
    let styled = document::add_node(graphml, &labelled_box("Styled"));
    let in_field = document::add_node(graphml, &box_at(300.0, 0.0));
    let in_dialog = document::add_node(graphml, &box_at(600.0, 0.0));
    let label = LabelRef {
        item: node(&styled),
        index: 0,
    };
    harness.perform(Action::SetFont(label, FontPart::Size(20)));

    harness.app.selected = Selection::node(in_field.clone());
    let field = harness.empty_field();
    harness.click_at(field);
    harness.type_text("Field");
    harness.press(Modifiers::NONE, Key::Enter);
    harness.app.selected = Selection::node(in_dialog.clone());
    harness.click_text("...");
    harness.run(vec![]);
    harness.type_text("Dialog");
    harness.click_text("OK");

    for (id, text) in [(&in_field, "Field"), (&in_dialog, "Dialog")] {
        let label = &harness.graphics(id).labels[0];
        assert_eq!((label.text.as_str(), label.font.size), (text, 20));
    }
}

#[test]
fn only_new_boxes_get_the_corner_radius_last_set() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.app.selected = Selection::node(id.clone());
    harness.perform(Action::SetCornerRadius(10.0));
    for (tool, x) in [(Tool::Circle, 20.0), (Tool::Ellipse, 200.0)] {
        let drawn = harness.draw_node(tool, vec2(x, 250.0), vec2(x + 120.0, 310.0));
        assert_eq!(
            harness.graphics(&drawn).kind,
            NodeKind::Shape(ShapeType::Ellipse)
        );
        // Only its graphics, with no radius kept.
        let nodes = &harness.app.doc.graphml().graphs[0].nodes;
        let node = nodes.iter().find(|node| node.id == drawn).unwrap();
        assert_eq!(node.data.len(), 1, "{tool:?}");
    }

    for (radius, shape, x) in [
        (10.0, ShapeType::RoundRectangle, 20.0),
        (4.0, ShapeType::RoundRectangle, 200.0),
        (0.0, ShapeType::Rectangle, 380.0),
    ] {
        harness.app.selected = Selection::node(id.clone());
        harness.perform(Action::SetCornerRadius(radius));
        let drawn = harness.draw_node(Tool::Box, vec2(x, 400.0), vec2(x + 120.0, 460.0));
        assert_eq!(harness.graphics(&drawn).kind, NodeKind::Shape(shape));
        let drawn_radius = document::corner_radius(harness.app.doc.graphml(), &drawn);
        assert_eq!(drawn_radius, Some(radius));
    }
}

#[test]
fn pasting_reproduces_what_was_copied_whatever_was_set_since() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::of([node("n0"), node("n1"), Item::Edge(0)]);
    let nodes = [harness.graphics("n0"), harness.graphics("n1")];
    let edge = harness.edge(0);
    // Cut, so that the paste lands where they were.
    let (copied, _) = harness.clipboard(vec![Event::Cut]);

    // All there is to remember, set on what's left.
    harness.app.selected = Selection::of([node("n2"), node("n3"), Item::Edge(0)]);
    let label = LabelRef {
        item: node("n2"),
        index: 0,
    };
    for action in [
        Action::SetColor(ColorProperty::Fill, Some(RED)),
        Action::SetColor(ColorProperty::Gradient, Some(RED)),
        Action::SetColor(ColorProperty::Line, Some(RED)),
        Action::SetLineWidth(5.0),
        Action::SetLineType(yed::LineType::DashedDotted),
        Action::SetCornerRadius(10.0),
        Action::SetFont(label.clone(), FontPart::Family("Monospaced".to_owned())),
        Action::SetFont(label.clone(), FontPart::Style(yed::FontStyle::Italic)),
        Action::SetFont(label, FontPart::Size(30)),
        Action::SetColor(ColorProperty::Text(0), Some(RED)),
    ] {
        harness.perform(action);
    }
    assert_ne!(harness.app.settings.style, Style::default());

    harness.run(vec![Event::Paste(copied[0].clone())]);
    let pasted = harness.app.selected.clone();
    assert!(pasted.contains_node("n0") && pasted.contains_node("n1"));
    let index = pasted.edges().next().expect("the line was pasted");
    assert_eq!([harness.graphics("n0"), harness.graphics("n1")], nodes);
    assert_eq!(harness.edge(index), edge);
}

#[test]
fn open_neither_restyles_the_file_nor_forgets_what_was_set() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.app.selected = Selection::node(id);
    harness.perform(Action::SetColor(ColorProperty::Fill, Some(RED)));
    harness.perform(Action::SetLineWidth(3.0));
    harness.open_fixture();
    assert_eq!(
        *harness.app.doc.graphml(),
        GraphMl::read_file(FIXTURE).unwrap()
    );

    // Below the file's nodes.
    let drawn = harness.draw_node(Tool::Box, vec2(20.0, 500.0), vec2(140.0, 560.0));
    let graphics = harness.graphics(&drawn);
    assert_eq!(graphics.fill.color, Some(RED));
    assert_eq!(graphics.border.width, 3.0);
}

#[test]
fn new_keeps_what_was_set_for_new_items() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.app.selected = Selection::node(id);
    harness.perform(Action::SetColor(ColorProperty::Fill, Some(RED)));
    harness.perform(Action::New);
    harness.answer_prompt("Don't Save");
    assert_eq!(*harness.app.doc.graphml(), yed::new_document());

    let drawn = harness.draw_node(Tool::Box, vec2(20.0, 20.0), vec2(140.0, 80.0));
    assert_eq!(harness.graphics(&drawn).fill.color, Some(RED));
}

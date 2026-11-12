use super::*;
use crate::style::Style;

/// The solid border painted around the node filled with `fill` on the canvas.
fn border(shapes: &[Shape], fill: Color32) -> Stroke {
    let canvas = canvas(shapes);
    let rects = || {
        shapes.iter().filter_map(|shape| match shape {
            Shape::Rect(rect) if canvas.contains_rect(rect.rect) => Some(rect),
            _ => None,
        })
    };
    let nodes: Vec<_> = rects().filter(|rect| rect.fill == fill).collect();
    let [node] = nodes[..] else {
        panic!("expected one node filled with {fill:?}: {nodes:?}");
    };
    let borders: Vec<_> = rects()
        .filter(|rect| rect.rect == node.rect && !rect.stroke.is_empty())
        .map(|rect| rect.stroke)
        .collect();
    let [border] = borders[..] else {
        panic!("expected one border around {node:?}: {borders:?}");
    };
    border
}

/// How many dashes were painted on the canvas.
fn dashes(shapes: &[Shape]) -> usize {
    let canvas = canvas(shapes);
    shapes
        .iter()
        .map(|shape| match shape {
            Shape::Mesh(mesh) if canvas.contains_rect(mesh.calc_bounds()) => {
                mesh.vertices.len() / 4
            }
            _ => 0,
        })
        .sum()
}

#[test]
fn the_side_panel_shows_the_clicked_node() {
    let mut harness = Harness::new();
    let mut graphics = NodeGraphics::new(NodeKind::Shape(ShapeType::Rectangle));
    graphics.geometry = yed::Rect {
        x: 100.0,
        y: 50.5,
        width: 150.0,
        height: 80.0 / 3.0,
    };
    graphics.fill.gradient = Some(yed::Color::WHITE);
    graphics.border.color = None;
    graphics.labels = vec![yed::Label::new("Start"), yed::Label::new("Two\nlines")];
    document::add_node(harness.app.doc.graphml_mut(), &graphics);
    assert_eq!(harness.side_panel(), ["Nothing selected"]);

    harness.click(vec2(150.0, 60.0));
    // The side panel is shown before the canvas changes the selection, so it
    // catches up a frame later.
    #[rustfmt::skip]
    assert_eq!(harness.side_panel(), [
        "Id",            "n0",
        "Shape",         "rectangle",
        "X",             "100",
        "Y",             "50.5",
        "Width",         "150",
        "Height",        "26.67",
        "Fill",          "#FFCC00",
        "Gradient",      "#FFFFFF",
        "Border",        "None",
        "Border width",  "1",
        "Border style",  "Solid",
        "Corner radius", "0",
        "Label",         "...", "Start",
        "Font",          "Dialog",
        "Font style",    "Plain",
        "Font size",     "12",
        "Text color",    "#000000",
        "Position",      "Center",
        "Label",         "...", "Two\nlines",
        "Font",          "Dialog",
        "Font style",    "Plain",
        "Font size",     "12",
        "Text color",    "#000000",
        "Position",      "Center",
    ]);

    harness.click(vec2(400.0, 300.0));
    assert_eq!(harness.side_panel(), ["Nothing selected"]);
}

#[test]
fn the_fixtures_nodes_of_kinds_without_a_shape_show_and_select_as_boxes() {
    let mut harness = fixture_harness();
    // A flowchart GenericNode, a UMLClassNode and a group node.
    for (text, id) in [("Queue", "n2"), ("Order", "n4"), ("Backend", "n5")] {
        harness.canvas_text(text);
        harness.click_text(text);
        assert_eq!(harness.app.selected_node(), Some(id), "{text}");
    }
}

#[test]
fn dragging_a_side_resizes_the_selected_node() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.click(vec2(150.0, 60.0));
    harness.drag(vec2(250.0, 90.0), vec2(300.0, 90.0));
    assert_eq!(harness.graphics(&id).geometry.width, 200.0);
    let side_panel = harness.side_panel();
    assert!(
        side_panel.windows(2).any(|row| row == ["Width", "200"]),
        "{side_panel:?}"
    );
}

#[test]
fn dragging_the_selected_node_moves_it() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.click(vec2(150.0, 60.0));
    harness.drag(vec2(175.0, 90.0), vec2(205.0, 130.0));
    let geometry = harness.graphics(&id).geometry;
    assert_eq!((geometry.x, geometry.y), (130.0, 90.0));
    let side_panel = harness.side_panel();
    assert!(
        side_panel
            .windows(4)
            .any(|rows| rows == ["X", "130", "Y", "90"]),
        "{side_panel:?}"
    );
}

#[test]
fn cmd_arrows_move_the_selected_node_on_macs() {
    let mut harness = Harness::mac();
    let id = document::add_node(harness.app.doc.graphml_mut(), &labelled_box("Start"));
    harness.click(vec2(150.0, 60.0));
    harness.press(CMD, Key::ArrowRight);
    assert_eq!(harness.graphics(&id).geometry.x, 101.0);
}

#[test]
fn ctrl_arrows_move_the_selected_node() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &labelled_box("Start"));
    let unmoved = harness.app.doc.graphml().clone();
    harness.press(CTRL, Key::ArrowRight);
    assert_eq!(*harness.app.doc.graphml(), unmoved);

    harness.click(vec2(150.0, 60.0));
    let before = harness.graphics(&id);
    for (key, x, y) in [
        (Key::ArrowRight, 101.0, 50.0),
        (Key::ArrowDown, 101.0, 51.0),
        (Key::ArrowLeft, 100.0, 51.0),
        (Key::ArrowUp, 100.0, 50.0),
        (Key::ArrowUp, 100.0, 49.0),
    ] {
        harness.press(CTRL, key);
        let geometry = harness.graphics(&id).geometry;
        assert_eq!((geometry.x, geometry.y), (x, y), "{key:?}");
    }
    // Whatever the tool.
    harness.press(Modifiers::NONE, Key::B);
    harness.press(CTRL, Key::ArrowLeft);

    let after = harness.graphics(&id);
    let geometry = yed::Rect {
        x: 99.0,
        y: 49.0,
        ..before.geometry
    };
    assert_eq!(after.geometry, geometry);
    // Placed relative to the node, they move with it unchanged.
    assert_eq!(after.labels, before.labels);
    let side_panel = harness.side_panel();
    assert!(
        side_panel
            .windows(4)
            .any(|rows| rows == ["X", "99", "Y", "49"]),
        "{side_panel:?}"
    );
}

#[test]
fn clicking_a_color_in_the_side_panel_picks_a_new_one() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.click(vec2(150.0, 60.0));

    harness.click_text("#FFCC00");
    assert!(harness.painted("Fill color"));
    // The hex field has the focus, with the fill's color selected.
    harness.run(vec![Event::Text("#FF0000".to_owned())]);
    harness.press(Modifiers::NONE, Key::Enter);

    assert!(!harness.app.color_picker.is_open());
    assert_eq!(harness.graphics(&id).fill.color, Some(RED));
    #[rustfmt::skip]
    assert_eq!(harness.side_panel(), [
        "Id",           "n0",
        "Shape",        "rectangle",
        "X",            "100",
        "Y",            "50",
        "Width",        "150",
        "Height",       "80",
        "Fill",          "#FF0000",
        "Gradient",      "None",
        "Border",        "#000000",
        "Border width",  "1",
        "Border style",  "Solid",
        "Corner radius", "0",
        "Label",         "...", "",
    ]);
}

#[test]
fn no_color_removes_the_color_clicked() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.click(vec2(150.0, 60.0));

    harness.click_text("#000000");
    assert!(harness.painted("Border color"));
    harness.click_text("No color");
    harness.click_text("OK");

    let graphics = harness.graphics(&id);
    assert_eq!(graphics.border.color, None);
    assert_eq!(graphics.fill, new_box().fill);
}

#[test]
fn typing_a_color_selects_no_tools() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.click(vec2(150.0, 60.0));
    harness.click_text("#FFCC00");
    harness.run(vec![]);

    // Each key is a tool's shortcut.
    for (key, text) in [
        (Key::A, "a"),
        (Key::B, "b"),
        (Key::C, "c"),
        (Key::E, "e"),
        (Key::A, "a"),
        (Key::B, "b"),
    ] {
        let press = testing::key(Modifiers::NONE, key);
        harness.run(vec![press, Event::Text(text.to_owned())]);
    }
    harness.press(Modifiers::NONE, Key::Enter);

    assert_eq!(harness.app.tool, Tool::Cursor);
    let typed = yed::Color::rgb(0xAB, 0xCE, 0xAB);
    assert_eq!(harness.graphics(&id).fill.color, Some(typed));
}

/// yEd's default node fill, which [`new_box`] has.
const YED_FILL: Color32 = Color32::from_rgb(0xFF, 0xCC, 0x00);

#[test]
fn typing_a_border_width_in_the_side_panel_sets_it() {
    let mut harness = Harness::new();
    let id = click_border_width(&mut harness);
    // The width is selected, so typing replaces it.
    harness.type_text("4.5");
    assert_eq!(harness.graphics(&id).border.width, 1.0);
    harness.press(Modifiers::NONE, Key::Enter);

    assert_eq!(harness.graphics(&id).border.width, 4.5);
    assert!(!harness.ctx.text_edit_focused());
    let shapes = harness.run(vec![]);
    assert_eq!(border(&shapes, YED_FILL), Stroke::new(4.5, Color32::BLACK));
    let side_panel = harness.side_panel();
    assert!(
        side_panel
            .windows(2)
            .any(|row| row == ["Border width", "4.5"]),
        "{side_panel:?}"
    );
}

#[test]
fn dragging_the_border_width_changes_it_as_it_goes() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.click(vec2(150.0, 60.0));
    let field = harness.text_at("1");
    // A tenth per point, rightwards.
    let shapes = harness.drag_and_hold(field, field + vec2(20.0, 0.0));
    assert_eq!(harness.graphics(&id).border.width, 3.0);
    assert_eq!(border(&shapes, YED_FILL).width, 3.0);
    harness.run(vec![button(field + vec2(20.0, 0.0), false)]);
    assert_eq!(harness.graphics(&id).border.width, 3.0);
}

#[test]
fn escape_in_the_border_width_field_keeps_the_width_and_the_tool() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.click(vec2(150.0, 60.0));
    harness.press(Modifiers::NONE, Key::B);
    harness.click_text("1");
    harness.type_text("4");
    harness.press(Modifiers::NONE, Key::Escape);
    harness.run(vec![]);

    assert_eq!(harness.graphics(&id).border.width, 1.0);
    assert_eq!(harness.app.tool, Tool::Box);
    assert!(!harness.ctx.text_edit_focused());
    // Once the field has let go of the keys.
    harness.press(Modifiers::NONE, Key::Escape);
    assert_eq!(harness.app.tool, Tool::Cursor);
}

#[test]
fn clicking_a_color_applies_the_border_width_typed_first() {
    let mut harness = Harness::new();
    let id = click_border_width(&mut harness);
    harness.type_text("4");
    harness.click_text("#FFCC00");
    assert_eq!(harness.graphics(&id).border.width, 4.0);
    assert!(harness.painted("Fill color"));
}

#[test]
fn the_dots_button_applies_the_border_width_typed_first() {
    let mut harness = Harness::new();
    let id = click_border_width(&mut harness);
    harness.type_text("4");
    harness.click_text("...");
    assert_eq!(harness.graphics(&id).border.width, 4.0);
    assert!(harness.app.label_dialog.is_open());
}

#[test]
fn entering_the_border_width_unchanged_keeps_it_exactly() {
    let mut harness = Harness::new();
    let mut graphics = new_box();
    graphics.border.width = 0.125;
    let id = document::add_node(harness.app.doc.graphml_mut(), &graphics);
    harness.click(vec2(150.0, 60.0));
    harness.click_text("0.125");
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(harness.graphics(&id).border.width, 0.125);
}

#[test]
fn typed_border_widths_are_kept_in_range() {
    for (typed, width) in [("-2", 0.0), ("1000", 100.0), ("inf", 100.0)] {
        let mut harness = Harness::new();
        let id = click_border_width(&mut harness);
        harness.type_text(typed);
        harness.press(Modifiers::NONE, Key::Enter);
        assert_eq!(harness.graphics(&id).border.width, width, "{typed}");
    }
}

#[test]
fn showing_a_border_width_out_of_range_keeps_it() {
    let mut harness = Harness::new();
    let mut graphics = new_box();
    graphics.border.width = 250.0;
    document::add_node(harness.app.doc.graphml_mut(), &graphics);
    let before = harness.app.doc.graphml().clone();
    harness.click(vec2(150.0, 60.0));
    let side_panel = harness.side_panel();
    assert!(
        side_panel
            .windows(2)
            .any(|row| row == ["Border width", "250"]),
        "{side_panel:?}"
    );
    assert_eq!(*harness.app.doc.graphml(), before);
}

#[test]
fn typing_a_corner_radius_in_the_side_panel_rounds_the_box() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.click(vec2(150.0, 60.0));
    assert_eq!(rows(&harness.side_panel(), "Corner radius"), ["0"]);
    harness.click_text("0");
    harness.type_text("10");
    harness.press(Modifiers::NONE, Key::Enter);

    let graphml = harness.app.doc.graphml();
    assert_eq!(document::corner_radius(graphml, &id), Some(10.0));
    let side_panel = harness.side_panel();
    assert_eq!(rows(&side_panel, "Corner radius"), ["10"]);
    assert_eq!(rows(&side_panel, "Shape"), ["roundrectangle"]);
    // A polygon now, not a rect.
    let shapes = harness.run(vec![]);
    let canvas = canvas(&shapes);
    let on_canvas = |shape: &&Shape| match shape {
        Shape::Rect(rect) => rect.fill == YED_FILL && canvas.contains_rect(rect.rect),
        Shape::Path(path) => path.fill == YED_FILL && canvas.contains(path.points[0]),
        _ => false,
    };
    let filled: Vec<_> = shapes.iter().filter(on_canvas).collect();
    assert!(matches!(filled[..], [Shape::Path(_)]), "{filled:?}");
}

#[test]
fn round_rectangles_read_with_yeds_corner_radius() {
    let mut harness = Harness::new();
    let mut graphics = new_box();
    graphics.kind = NodeKind::Shape(ShapeType::RoundRectangle);
    document::add_node(harness.app.doc.graphml_mut(), &graphics);
    harness.click(vec2(150.0, 60.0));
    assert_eq!(rows(&harness.side_panel(), "Corner radius"), ["4"]);
}

#[test]
fn only_boxes_have_a_corner_radius() {
    let mut harness = Harness::new();
    let mut graphics = new_box();
    graphics.kind = NodeKind::Shape(ShapeType::Ellipse);
    document::add_node(harness.app.doc.graphml_mut(), &graphics);
    harness.click(vec2(175.0, 90.0));
    let side_panel = harness.side_panel();
    assert_eq!(side_panel[..4], ["Id", "n0", "Shape", "ellipse"]);
    assert_eq!(rows(&side_panel, "Corner radius"), [""; 0]);
}

#[test]
fn typed_corner_radii_are_kept_in_range() {
    for (typed, radius) in [("-2", 0.0), ("5000", 1000.0), ("inf", 1000.0)] {
        let mut harness = Harness::new();
        let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
        harness.click(vec2(150.0, 60.0));
        harness.click_text("0");
        harness.type_text(typed);
        harness.press(Modifiers::NONE, Key::Enter);
        let read = document::corner_radius(harness.app.doc.graphml(), &id);
        assert_eq!(read, Some(radius), "{typed}");
    }
}

#[test]
fn picking_a_border_style_in_the_side_panel_sets_it() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.click(vec2(150.0, 60.0));
    let mut shown = "Solid";
    for (name, line_type) in [
        ("Dashed", yed::LineType::Dashed),
        ("Dotted", yed::LineType::Dotted),
        ("Dash-dotted", yed::LineType::DashedDotted),
        ("Solid", yed::LineType::Line),
    ] {
        harness.click_text(shown);
        harness.click_text(name);
        assert_eq!(harness.graphics(&id).border.line_type, line_type);
        assert!(!Popup::is_any_open(&harness.ctx));
        let side_panel = harness.side_panel();
        assert!(
            side_panel
                .windows(2)
                .any(|row| row == ["Border style", name]),
            "{side_panel:?}"
        );
        let shapes = harness.run(vec![]);
        assert_eq!(dashes(&shapes) > 0, name != "Solid", "{name}");
        shown = name;
    }
    let shapes = harness.run(vec![]);
    assert_eq!(border(&shapes, YED_FILL), Stroke::new(1.0, Color32::BLACK));
}

#[test]
fn picking_the_border_style_a_node_has_changes_nothing() {
    let mut harness = Harness::new();
    document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.click(vec2(150.0, 60.0));
    let before = harness.app.doc.graphml().clone();
    harness.click_text("Solid");
    // The list's, painted over the dropdown's own.
    let solids: Vec<_> = harness
        .run(vec![])
        .iter()
        .filter_map(|shape| match shape {
            Shape::Text(text) if text.galley.text() == "Solid" => {
                Some(text.pos + text.galley.rect.center().to_vec2())
            }
            _ => None,
        })
        .collect();
    let [_, listed] = solids[..] else {
        panic!("expected the dropdown and its list to show Solid: {solids:?}");
    };
    harness.click_at(listed);
    assert!(!Popup::is_any_open(&harness.ctx));
    assert_eq!(*harness.app.doc.graphml(), before);
    // Nor is it remembered for new nodes, as setting it again would be.
    assert_eq!(harness.app.settings.style, Style::default());
}

#[test]
fn escape_closes_the_border_style_dropdown_before_it_selects_the_cursor() {
    let mut harness = Harness::new();
    document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.click(vec2(150.0, 60.0));
    harness.press(Modifiers::NONE, Key::B);
    harness.click_text("Solid");
    assert!(Popup::is_any_open(&harness.ctx));

    harness.press(Modifiers::NONE, Key::Escape);
    assert!(!Popup::is_any_open(&harness.ctx));
    assert_eq!(harness.app.tool, Tool::Box);

    harness.press(Modifiers::NONE, Key::Escape);
    assert_eq!(harness.app.tool, Tool::Cursor);
}

#[test]
fn clicking_the_border_style_applies_the_border_width_typed_first() {
    let mut harness = Harness::new();
    let id = click_border_width(&mut harness);
    harness.type_text("4");
    harness.click_text("Solid");
    assert_eq!(harness.graphics(&id).border.width, 4.0);
    assert!(Popup::is_any_open(&harness.ctx));
}

#[test]
fn unknown_border_styles_read_as_written_and_are_drawn_solid() {
    let mut harness = Harness::new();
    let mut graphics = new_box();
    graphics.border.line_type = yed::LineType::Other("wavy".to_owned());
    document::add_node(harness.app.doc.graphml_mut(), &graphics);
    let before = harness.app.doc.graphml().clone();
    harness.click(vec2(150.0, 60.0));
    let side_panel = harness.side_panel();
    assert!(
        side_panel
            .windows(2)
            .any(|row| row == ["Border style", "wavy"]),
        "{side_panel:?}"
    );
    let shapes = harness.run(vec![]);
    assert_eq!(border(&shapes, YED_FILL), Stroke::new(1.0, Color32::BLACK));
    assert_eq!(*harness.app.doc.graphml(), before);
}

#[test]
fn circles_and_ellipses_are_drawn_with_their_tools() {
    let mut harness = Harness::new();
    harness.press(Modifiers::NONE, Key::C);
    harness.drag(vec2(20.0, 20.0), vec2(140.0, 80.0));
    harness.press(Modifiers::NONE, Key::E);
    harness.drag(vec2(20.0, 150.0), vec2(140.0, 210.0));

    let ellipse = NodeKind::Shape(ShapeType::Ellipse);
    let circle = harness.graphics("n0");
    assert_eq!(circle.kind, ellipse);
    assert_eq!(
        circle.geometry,
        yed::Rect {
            x: 20.0,
            y: 20.0,
            width: 120.0,
            height: 120.0,
        }
    );
    let oval = harness.graphics("n1");
    assert_eq!(oval.kind, ellipse);
    assert_eq!(
        oval.geometry,
        yed::Rect {
            x: 20.0,
            y: 150.0,
            width: 120.0,
            height: 60.0,
        }
    );
    assert_eq!(harness.app.selected_node(), Some("n1"));
    assert_eq!(harness.side_panel()[..4], ["Id", "n1", "Shape", "ellipse"]);
}

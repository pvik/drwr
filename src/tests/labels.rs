use super::*;

#[test]
fn enter_in_the_side_panel_applies_the_label() {
    let mut harness = Harness::new();
    let id = type_in_label_field(&mut harness, "Start");
    harness.type_text("Stop");
    assert_eq!(harness.label(&id), "Start");
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(harness.label(&id), "Stop");
    assert!(!harness.ctx.text_edit_focused());
}

#[test]
fn shift_enter_in_the_side_panel_adds_a_line() {
    let mut harness = Harness::new();
    let id = type_in_label_field(&mut harness, "Start");
    harness.type_text("One");
    harness.press(Modifiers::SHIFT, Key::Enter);
    harness.type_text("Two");
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(harness.label(&id), "One\nTwo");
}

#[test]
fn escape_in_the_side_panel_keeps_the_label() {
    let mut harness = Harness::new();
    let id = type_in_label_field(&mut harness, "Start");
    harness.type_text("Stop");
    harness.press(Modifiers::NONE, Key::Escape);
    assert_eq!(harness.label(&id), "Start");
    let side_panel = harness.side_panel();
    assert!(side_panel.contains(&"Start".to_owned()), "{side_panel:?}");
    assert!(!side_panel.contains(&"Stop".to_owned()), "{side_panel:?}");
}

#[test]
fn clicking_away_from_the_side_panel_applies_the_label() {
    let mut harness = Harness::new();
    let id = type_in_label_field(&mut harness, "Start");
    harness.type_text("Stop");
    // The same click selects nothing, but the label still goes to the box.
    harness.click(vec2(400.0, 300.0));
    assert_eq!(harness.label(&id), "Stop");
    assert!(harness.app.selected.is_empty());
}

#[test]
fn a_node_without_labels_gets_one_from_the_side_panel() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.click(vec2(150.0, 60.0));
    let field = harness.empty_field();
    harness.click_at(field);
    harness.type_text("Hello");
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(harness.graphics(&id).labels.len(), 1);
    assert_eq!(harness.label(&id), "Hello");
}

#[test]
fn typing_a_label_selects_no_tools() {
    let mut harness = Harness::new();
    let id = type_in_label_field(&mut harness, "Start");
    harness.app.tool = Tool::Box;
    // Each key is a tool's shortcut.
    for (key, text) in [(Key::C, "c"), (Key::A, "a"), (Key::B, "b")] {
        let press = testing::key(Modifiers::NONE, key);
        harness.run(vec![press, Event::Text(text.to_owned())]);
    }
    assert_eq!(harness.app.tool, Tool::Box);
    harness.press(Modifiers::NONE, Key::Escape);
    assert_eq!(harness.app.tool, Tool::Box);
    assert_eq!(harness.label(&id), "Start");
}

#[test]
fn ctrl_arrows_in_a_label_field_move_the_text_cursor() {
    let mut harness = Harness::new();
    let id = type_in_label_field(&mut harness, "Start");
    harness.type_text("One two");
    // Back a word.
    harness.press(CTRL, Key::ArrowLeft);
    harness.type_text("big ");
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(harness.label(&id), "One big two");
    assert_eq!(harness.graphics(&id).geometry, new_box().geometry);
}

#[test]
fn the_dots_button_edits_the_label_in_a_dialog() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &labelled_box("Start"));
    harness.click(vec2(150.0, 60.0));
    harness.click_text("...");
    assert!(harness.app.label_dialog.is_open());
    harness.run(vec![]);
    // The label's text is selected, so typing replaces it.
    harness.type_text("Stop");
    harness.press(Modifiers::NONE, Key::Enter);
    harness.type_text("Go");
    harness.click_text("OK");
    assert!(!harness.app.label_dialog.is_open());
    assert_eq!(harness.label(&id), "Stop\nGo");
}

#[test]
fn the_dots_button_applies_the_label_being_typed_first() {
    let mut harness = Harness::new();
    let id = type_in_label_field(&mut harness, "Start");
    harness.type_text("Stop");
    harness.click_text("...");
    assert_eq!(harness.label(&id), "Stop");
    // The dialog opened on the text applied, so OK keeps it.
    harness.run(vec![]);
    harness.click_text("OK");
    assert_eq!(harness.label(&id), "Stop");
}

#[test]
fn double_clicking_a_node_edits_its_label_on_the_canvas() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &labelled_box("Start"));
    harness.double_click(vec2(150.0, 60.0));
    harness.run(vec![]);
    assert!(harness.ctx.text_edit_focused());
    // The label's text is selected, so typing replaces it.
    harness.type_text("Stop");
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(harness.label(&id), "Stop");
    assert!(!harness.ctx.text_edit_focused());
}

#[test]
fn ctrl_enter_edits_the_selected_nodes_label_on_the_canvas() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.press(CTRL, Key::Enter);
    assert!(!harness.ctx.text_edit_focused());

    harness.click(vec2(150.0, 60.0));
    harness.press(CTRL, Key::Enter);
    harness.run(vec![]);
    assert!(harness.ctx.text_edit_focused());
    harness.type_text("Hello");
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(harness.label(&id), "Hello");
}

#[test]
fn ctrl_enter_edits_the_label_whatever_the_tool() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.click(vec2(150.0, 60.0));
    for (key, tool) in [
        (Key::B, Tool::Box),
        (Key::C, Tool::Circle),
        (Key::E, Tool::Ellipse),
        (Key::L, Tool::Line),
        (Key::A, Tool::Arrow),
        (Key::T, Tool::TextBox),
        (Key::Escape, Tool::Cursor),
    ] {
        harness.press(Modifiers::NONE, key);
        harness.press(CTRL, Key::Enter);
        harness.run(vec![]);
        assert!(harness.ctx.text_edit_focused(), "{tool:?}");
        let text = format!("{tool:?}");
        harness.type_text(&text);
        harness.press(Modifiers::NONE, Key::Enter);
        assert_eq!(harness.label(&id), text);
        assert_eq!(harness.app.tool, tool);
    }
}

#[test]
fn boxes_can_be_drawn_and_labelled_without_changing_tools() {
    let mut harness = Harness::new();
    harness.press(Modifiers::NONE, Key::B);
    for (corner, text) in [(vec2(20.0, 20.0), "One"), (vec2(20.0, 150.0), "Two")] {
        harness.drag(corner, corner + vec2(120.0, 60.0));
        harness.press(CTRL, Key::Enter);
        harness.run(vec![]);
        harness.type_text(text);
        harness.press(Modifiers::NONE, Key::Enter);
    }
    assert_eq!(harness.label("n0"), "One");
    assert_eq!(harness.label("n1"), "Two");
    assert_eq!(harness.app.tool, Tool::Box);
}

#[test]
fn drawing_the_next_box_applies_the_label_typed() {
    let mut harness = Harness::new();
    harness.press(Modifiers::NONE, Key::B);
    harness.drag(vec2(20.0, 20.0), vec2(140.0, 80.0));
    harness.press(CTRL, Key::Enter);
    harness.run(vec![]);
    harness.type_text("One");
    harness.drag(vec2(20.0, 150.0), vec2(140.0, 210.0));
    assert_eq!(harness.label("n0"), "One");
    assert!(!harness.ctx.text_edit_focused());
    assert_eq!(harness.app.selected_node(), Some("n1"));
    assert_eq!(harness.label("n1"), "");
}

#[test]
fn drawing_the_next_box_applies_the_label_typed_in_the_side_panel() {
    let mut harness = Harness::new();
    let id = type_in_label_field(&mut harness, "Start");
    harness.type_text("Stop");
    harness.app.tool = Tool::Box;
    harness.drag(vec2(20.0, 150.0), vec2(140.0, 210.0));
    harness.run(vec![]);
    assert_eq!(harness.label(&id), "Stop");
    assert!(!harness.ctx.text_edit_focused());
}

#[test]
fn escape_on_the_canvas_keeps_the_label_and_the_tool() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &labelled_box("Start"));
    harness.click(vec2(150.0, 60.0));
    harness.press(Modifiers::NONE, Key::B);
    harness.press(CTRL, Key::Enter);
    harness.run(vec![]);
    harness.type_text("Stop");
    harness.press(Modifiers::NONE, Key::Escape);
    assert_eq!(harness.label(&id), "Start");
    assert_eq!(harness.app.tool, Tool::Box);
    assert!(!harness.ctx.text_edit_focused());
}

/// Adds a box labelled "Start" and selects it. Its border has no color, so its
/// label's is the only black in the side panel. Returns the box's id.
fn select_labelled_box(harness: &mut Harness) -> String {
    let mut graphics = labelled_box("Start");
    graphics.border.color = None;
    let id = document::add_node(harness.app.doc.graphml_mut(), &graphics);
    harness.click(vec2(150.0, 60.0));
    id
}

#[test]
fn picking_a_font_in_the_side_panel_sets_it() {
    let mut harness = Harness::new();
    let id = select_labelled_box(&mut harness);
    harness.click_text("Dialog");
    harness.run(vec![]);
    // The search field has the focus.
    harness.type_text("mono");
    harness.press(Modifiers::NONE, Key::Enter);

    assert_eq!(harness.graphics(&id).labels[0].font.family, "Monospaced");
    assert!(!Popup::is_any_open(&harness.ctx));
    let family = fonts::family("Monospaced", &yed::FontStyle::Plain);
    assert_eq!(harness.canvas_text("Start").font_id.family, family);
    let side_panel = harness.side_panel();
    assert!(
        side_panel
            .windows(2)
            .any(|row| row == ["Font", "Monospaced"]),
        "{side_panel:?}"
    );
}

#[test]
fn searching_fonts_selects_no_tools() {
    let mut harness = Harness::new();
    let id = select_labelled_box(&mut harness);
    harness.press(Modifiers::NONE, Key::B);
    harness.click_text("Dialog");
    harness.run(vec![]);
    // Each key is a tool's shortcut.
    for (key, text) in [(Key::C, "c"), (Key::E, "e"), (Key::L, "l")] {
        let press = testing::key(Modifiers::NONE, key);
        harness.run(vec![press, Event::Text(text.to_owned())]);
    }
    assert_eq!(harness.app.tool, Tool::Box);

    harness.press(Modifiers::NONE, Key::Escape);
    assert!(!Popup::is_any_open(&harness.ctx));
    assert_eq!(harness.app.tool, Tool::Box);
    assert_eq!(harness.graphics(&id).labels[0].font.family, "Dialog");
    harness.press(Modifiers::NONE, Key::Escape);
    assert_eq!(harness.app.tool, Tool::Cursor);
}

#[test]
fn picking_a_font_style_in_the_side_panel_sets_it() {
    use yed::FontStyle::{Bold, BoldItalic, Italic, Plain};
    let mut harness = Harness::new();
    let id = select_labelled_box(&mut harness);
    let mut shown = "Plain";
    for (name, style) in [
        ("Bold", Bold),
        ("Italic", Italic),
        ("Bold italic", BoldItalic),
        ("Plain", Plain),
    ] {
        harness.click_text(shown);
        harness.click_text(name);
        assert_eq!(harness.graphics(&id).labels[0].font.style, style);
        assert!(!Popup::is_any_open(&harness.ctx));
        let family = fonts::family("Dialog", &style);
        assert_eq!(
            harness.canvas_text("Start").font_id.family,
            family,
            "{name}"
        );
        shown = name;
    }
}

#[test]
fn typing_a_font_size_in_the_side_panel_sets_it() {
    let mut harness = Harness::new();
    let id = select_labelled_box(&mut harness);
    harness.click_text("12");
    // The size is selected, so typing replaces it.
    harness.type_text("20");
    assert_eq!(harness.graphics(&id).labels[0].font.size, 12);
    harness.press(Modifiers::NONE, Key::Enter);

    assert_eq!(harness.graphics(&id).labels[0].font.size, 20);
    assert_eq!(harness.canvas_text("Start").font_id.size, 20.0);
    let side_panel = harness.side_panel();
    assert!(
        side_panel.windows(2).any(|row| row == ["Font size", "20"]),
        "{side_panel:?}"
    );
}

#[test]
fn escape_in_the_font_size_field_keeps_the_size_and_the_tool() {
    let mut harness = Harness::new();
    let id = select_labelled_box(&mut harness);
    harness.press(Modifiers::NONE, Key::B);
    harness.click_text("12");
    harness.type_text("20");
    harness.press(Modifiers::NONE, Key::Escape);
    harness.run(vec![]);

    assert_eq!(harness.graphics(&id).labels[0].font.size, 12);
    assert_eq!(harness.app.tool, Tool::Box);
    harness.press(Modifiers::NONE, Key::Escape);
    assert_eq!(harness.app.tool, Tool::Cursor);
}

#[test]
fn typed_font_sizes_are_kept_in_range() {
    for (typed, size) in [("0", 1), ("-3", 1), ("1000", 500), ("inf", 500)] {
        let mut harness = Harness::new();
        let id = select_labelled_box(&mut harness);
        harness.click_text("12");
        harness.type_text(typed);
        harness.press(Modifiers::NONE, Key::Enter);
        assert_eq!(harness.graphics(&id).labels[0].font.size, size, "{typed}");
    }
}

#[test]
fn font_rows_edit_their_own_label() {
    let mut harness = Harness::new();
    let mut graphics = new_box();
    graphics.labels = vec![yed::Label::new("One"), yed::Label::new("Two")];
    let id = document::add_node(harness.app.doc.graphml_mut(), &graphics);
    harness.click(vec2(150.0, 60.0));
    let sizes = harness.texts_at("12");
    let [_, second] = sizes[..] else {
        panic!("expected two font sizes: {sizes:?}");
    };
    harness.click_at(second);
    harness.type_text("20");
    harness.press(Modifiers::NONE, Key::Enter);

    let labels = harness.graphics(&id).labels;
    let sizes: Vec<_> = labels.iter().map(|label| label.font.size).collect();
    assert_eq!(sizes, [12, 20]);
}

/// Runs a frame, returning where the text `text` was painted on the canvas.
fn canvas_text_rect(harness: &mut Harness, text: &str) -> Rect {
    let shapes = harness.run(vec![]);
    let canvas = canvas(&shapes);
    let found = shapes.iter().find_map(|shape| match shape {
        Shape::Text(shape) if shape.galley.text() == text && canvas.contains(shape.pos) => {
            Some(shape.galley.rect.translate(shape.pos.to_vec2()))
        }
        _ => None,
    });
    found.unwrap_or_else(|| panic!("{text:?} wasn't painted on the canvas"))
}

#[test]
fn picking_a_label_position_in_the_side_panel_moves_the_label() {
    let mut harness = Harness::new();
    let id = select_labelled_box(&mut harness);
    let node = Rect::from_min_size(canvas(&harness.run(vec![])).min, vec2(150.0, 80.0))
        .translate(vec2(100.0, 50.0));
    let mut shown = "Center";
    for (name, position) in [
        ("North", yed::LabelPosition::North),
        ("Bottom right", yed::LabelPosition::BottomRight),
        ("Center", yed::LabelPosition::Center),
    ] {
        harness.click_text(shown);
        harness.click_text(name);
        assert_eq!(harness.graphics(&id).labels[0].position(), Some(position));
        assert!(!Popup::is_any_open(&harness.ctx));
        let side_panel = harness.side_panel();
        assert!(
            side_panel.windows(2).any(|row| row == ["Position", name]),
            "{side_panel:?}"
        );
        // yEd hasn't sized the new label, so it goes 4 from the node, and its
        // text 2 more, as yEd would put it.
        let text = canvas_text_rect(&mut harness, "Start");
        let (at, expected) = match position {
            yed::LabelPosition::North => (text.center_bottom(), node.center_top() - vec2(0.0, 6.0)),
            yed::LabelPosition::BottomRight => {
                (text.right_bottom(), node.right_bottom() - vec2(6.0, 6.0))
            }
            _ => (text.center(), node.center()),
        };
        assert!(
            (at - expected).length() < 0.01,
            "{name}: {at:?} != {expected:?}"
        );
        shown = name;
    }
}

#[test]
fn the_position_list_shows_every_position() {
    let mut harness = Harness::new();
    select_labelled_box(&mut harness);
    harness.click_text("Center");
    let listed: Vec<_> = harness
        .side_panel()
        .into_iter()
        .skip_while(|text| text != "Top left")
        .collect();
    #[rustfmt::skip]
    assert_eq!(listed, [
        "Top left", "Top", "Top right",
        "Left", "Center", "Right",
        "Bottom left", "Bottom", "Bottom right",
        "Northwest", "North", "Northeast",
        "West", "East",
        "Southwest", "South", "Southeast",
    ]);
    // Clicks reach the last one, so the list doesn't scroll.
    harness.click_text("Southeast");
    assert!(!Popup::is_any_open(&harness.ctx));
}

#[test]
fn labels_placed_elsewhere_read_as_custom() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &labelled_box("Start"));
    let realizer = harness.app.doc.graphml_mut().graphs[0].nodes[0].data[0]
        .element_mut()
        .unwrap();
    let label = realizer.child_mut("NodeLabel").unwrap();
    label.set_attr("modelName", "free");
    label.set_attr("modelPosition", "anywhere");
    harness.click(vec2(150.0, 60.0));
    let side_panel = harness.side_panel();
    assert!(
        side_panel
            .windows(2)
            .any(|row| row == ["Position", "Custom"]),
        "{side_panel:?}"
    );

    harness.click_text("Custom");
    harness.click_text("Center");
    let label = &harness.graphics(&id).labels[0];
    assert_eq!(label.position(), Some(yed::LabelPosition::Center));
}

#[test]
fn clicking_the_text_color_picks_a_new_one() {
    let mut harness = Harness::new();
    let id = select_labelled_box(&mut harness);
    harness.click_text("#000000");
    assert!(harness.app.color_picker.is_open());
    // Labels always have a text color.
    assert!(!harness.painted("No color"));
    // The hex field has the focus, with the text color selected.
    harness.type_text("#FF0000");
    harness.press(Modifiers::NONE, Key::Enter);

    assert_eq!(harness.graphics(&id).labels[0].text_color, RED);
    let painted = harness.canvas_text("Start").color;
    assert_eq!(painted, Color32::from_rgb(0xFF, 0x00, 0x00));
}

use super::*;

#[test]
fn the_side_panel_shows_what_the_selected_nodes_share() {
    let mut harness = Harness::new();
    let mut left = labelled_box("Same");
    left.border.width = 2.0;
    let mut right = labelled_box("Same");
    right.geometry.x = 400.0;
    right.geometry.width = 100.0;
    right.fill.color = Some(RED);
    right.labels.push(yed::Label::new("Second"));
    let graphml = &mut harness.app.graphml;
    document::add_node(graphml, &left);
    document::add_node(graphml, &right);
    harness.click(vec2(150.0, 60.0));
    harness.with_ctrl(|harness| harness.click(vec2(450.0, 60.0)));
    // Blank where they differ: X, Width, Fill and Border width.
    #[rustfmt::skip]
    assert_eq!(harness.side_panel(), [
        "Selected",      "2 nodes",
        "Shape",         "rectangle",
        "X",             "",
        "Y",             "50",
        "Width",         "",
        "Height",        "80",
        // A blank swatch.
        "Fill",
        "Gradient",      "None",
        "Border",        "#000000",
        "Border width",  "",
        "Border style",  "Solid",
        "Corner radius", "0",
        // Only the label both have.
        "Label",         "...", "Same",
        "Font",          "Dialog",
        "Font style",    "Plain",
        "Font size",     "12",
        "Text color",    "#000000",
        "Position",      "Center",
    ]);
}

#[test]
fn nodes_and_lines_selected_together_share_line_rows() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let left = document::add_node(graphml, &labelled_box("Same"));
    let right = document::add_node(graphml, &box_at(300.0, 0.0));
    let mut line = edge_through(&[]);
    line.line.width = 2.0;
    line.labels = vec![yed::Label::new_on_edge("Same")];
    document::add_edge(graphml, &left, &right, &line);
    harness.app.selected = Selection::of([Item::Node(left), Item::Edge(0)]);
    // No fill or label position, which only nodes have.
    #[rustfmt::skip]
    assert_eq!(harness.side_panel(), [
        "Selected",   "1 node, 1 line",
        "Line",       "#000000",
        "Line width", "",
        "Line style", "Solid",
        "Label",      "...", "Same",
        "Font",       "Dialog",
        "Font style", "Plain",
        "Font size",  "12",
        "Text color", "#000000",
    ]);
}

#[test]
fn the_side_panel_shows_what_the_selected_lines_share() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let left = document::add_node(graphml, &new_box());
    let right = document::add_node(graphml, &box_at(300.0, 0.0));
    let below = document::add_node(graphml, &box_at(0.0, 300.0));
    let mut line = edge_through(&[]);
    line.arrows.target = yed::Arrow::None;
    document::add_edge(graphml, &left, &right, &edge_through(&[]));
    document::add_edge(graphml, &left, &below, &line);
    harness.app.selected = Selection::of([Item::Edge(0), Item::Edge(1)]);
    #[rustfmt::skip]
    assert_eq!(harness.side_panel(), [
        "Selected",    "2 lines",
        "Source",      "n0",
        "Target",      "",
        "Line",        "#000000",
        "Line width",  "1",
        "Line style",  "Solid",
        // The check boxes' empty texts.
        "Start arrow", "",
        "End arrow",   "",
        "Label",       "...", "",
    ]);
    // Neither checked nor not where they differ, and then set on both.
    let [start, end] = harness.checkboxes().try_into().unwrap();
    assert_eq!(start.toggled(), Some(egui::accesskit::Toggled::False));
    assert_eq!(end.toggled(), Some(egui::accesskit::Toggled::Mixed));
    harness.click_at(testing::center(&end));
    for index in [0, 1] {
        assert_eq!(harness.edge(index).arrows.target, yed::Arrow::Standard);
    }
}

#[test]
fn values_set_in_the_side_panel_go_to_every_selected_item() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let mut thick = new_box();
    thick.border.width = 2.0;
    let thick = document::add_node(graphml, &thick);
    let right = document::add_node(graphml, &box_at(300.0, 0.0));
    let line = document::add_edge(graphml, &thick, &right, &edge_through(&[]));
    let items = [
        Item::Node(thick.clone()),
        Item::Node(right.clone()),
        Item::Edge(line),
    ];
    harness.app.selected = Selection::of(items);
    // Blank, as the widths differ. The first's width still goes to the rest.
    harness.click_number_field(0);
    harness.type_text("2");
    harness.press(Modifiers::NONE, Key::Enter);
    let widths = [
        harness.graphics(&thick).border.width,
        harness.graphics(&right).border.width,
        harness.edge(line).line.width,
    ];
    assert_eq!(widths, [2.0; 3]);
    assert_eq!(rows(&harness.side_panel(), "Line width"), ["2"]);

    harness.click_text("Solid");
    harness.click_text("Dotted");
    let dotted = yed::LineType::Dotted;
    assert_eq!(harness.graphics(&thick).border.line_type, dotted);
    assert_eq!(harness.graphics(&right).border.line_type, dotted);
    assert_eq!(harness.edge(line).line.line_type, dotted);
}

#[test]
fn colors_picked_in_the_side_panel_go_to_every_selected_node() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let left = document::add_node(graphml, &new_box());
    let mut right = box_at(300.0, 0.0);
    right.fill.color = Some(yed::Color::rgb(0x00, 0x00, 0xFF));
    let right = document::add_node(graphml, &right);
    harness.app.selected = Selection::of([Item::Node(left.clone()), Item::Node(right.clone())]);
    // The blank swatch beside "Fill".
    let fill = harness.text_at("Fill");
    let buttons = harness.widgets_of(egui::accesskit::Role::Button);
    let swatch = (buttons.iter().map(testing::center))
        .find(|at| at.x > fill.x && (at.y - fill.y).abs() < 8.0)
        .expect("a button is beside Fill");
    harness.click_at(swatch);
    assert!(harness.painted("Fill color"));
    harness.run(vec![Event::Text("#FF0000".to_owned())]);
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(harness.graphics(&left).fill.color, Some(RED));
    assert_eq!(harness.graphics(&right).fill.color, Some(RED));
}

#[test]
fn a_blank_label_field_sets_every_label_only_once_typed_in() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let left = document::add_node(graphml, &labelled_box("Left"));
    let mut right = labelled_box("Right");
    right.geometry.x = 400.0;
    let right = document::add_node(graphml, &right);
    harness.app.selected = Selection::of([Item::Node(left.clone()), Item::Node(right.clone())]);
    let labels = |harness: &Harness| [harness.label(&left), harness.label(&right)];

    let field = harness.empty_field();
    harness.click_at(field);
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(labels(&harness), ["Left", "Right"]);
    harness.click_at(field);
    harness.type_text("Both");
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(labels(&harness), ["Both", "Both"]);
}

#[test]
fn the_label_dialog_sets_every_selected_label_once_changed() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let left = document::add_node(graphml, &labelled_box("Left"));
    // Without a label, which it gets.
    let right = document::add_node(graphml, &box_at(300.0, 0.0));
    harness.app.selected = Selection::of([Item::Node(left.clone()), Item::Node(right.clone())]);
    let labels = |harness: &Harness| [harness.label(&left), harness.label(&right)];

    harness.click_text("...");
    harness.run(vec![]);
    harness.click_text("OK");
    assert!(!harness.app.label_dialog.is_open());
    assert_eq!(labels(&harness), ["Left", ""]);
    harness.click_text("...");
    harness.run(vec![]);
    harness.type_text("Both");
    harness.click_text("OK");
    assert_eq!(labels(&harness), ["Both", "Both"]);
}

#[test]
fn font_changes_go_to_every_selected_label_keeping_the_rest_of_each_font() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let left = document::add_node(graphml, &labelled_box("Left"));
    let mut right = labelled_box("Right");
    right.geometry.x = 400.0;
    right.labels[0].font.family = "Monospaced".to_owned();
    right.labels[0].font.size = 20;
    let right = document::add_node(graphml, &right);
    harness.app.selected = Selection::of([Item::Node(left.clone()), Item::Node(right.clone())]);
    let side_panel = harness.side_panel();
    assert_eq!(rows(&side_panel, "Font"), [""]);
    assert_eq!(rows(&side_panel, "Font size"), [""]);

    // After the border width and corner radius.
    harness.click_number_field(2);
    harness.type_text("16");
    harness.press(Modifiers::NONE, Key::Enter);
    let fonts = [&left, &right].map(|id| harness.graphics(id).labels[0].font.clone());
    let fonts = fonts.map(|font| (font.family, font.size));
    assert_eq!(
        fonts,
        [("Dialog".to_owned(), 16), ("Monospaced".to_owned(), 16)]
    );
}

#[test]
fn only_boxes_together_have_a_corner_radius() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let mut ellipse = box_at(300.0, 0.0);
    ellipse.kind = NodeKind::Shape(ShapeType::Ellipse);
    let items = [new_box(), box_at(0.0, 200.0), ellipse]
        .map(|graphics| Item::Node(document::add_node(graphml, &graphics)));
    harness.app.selected = Selection::of(items[..2].iter().cloned());
    assert_eq!(rows(&harness.side_panel(), "Corner radius"), ["0"]);
    harness.app.selected = Selection::of(items);
    assert_eq!(rows(&harness.side_panel(), "Corner radius"), [""; 0]);
}

#[test]
fn ctrl_arrows_move_everything_selected() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let left = document::add_node(graphml, &new_box());
    let right = document::add_node(graphml, &box_at(300.0, 0.0));
    let below = document::add_node(graphml, &box_at(0.0, 300.0));
    // Between the two moved nodes, and to the one left behind.
    document::add_edge(graphml, &left, &right, &edge_through(&[(325.0, 20.0)]));
    document::add_edge(graphml, &right, &below, &edge_through(&[(600.0, 200.0)]));
    harness.app.selected = Selection::of([Item::Node(left.clone()), Item::Node(right.clone())]);
    harness.press(CTRL, Key::ArrowRight);

    let x = |id: &str| harness.graphics(id).geometry.x;
    assert_eq!([x(&left), x(&right), x(&below)], [101.0, 401.0, 100.0]);
    assert_eq!(harness.edge(0).path.points, [bend(326.0, 20.0)]);
    assert_eq!(harness.edge(1).path.points, [bend(600.0, 200.0)]);
}

#[test]
fn delete_removes_everything_selected() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let left = document::add_node(graphml, &new_box());
    let right = document::add_node(graphml, &box_at(300.0, 0.0));
    let below = document::add_node(graphml, &box_at(0.0, 300.0));
    for (source, target) in [(&left, &right), (&right, &below), (&left, &below)] {
        document::add_edge(graphml, source, target, &edge_through(&[]));
    }
    harness.app.selected = Selection::of([Item::Edge(0), Item::Edge(2)]);
    harness.press(Modifiers::NONE, Key::Delete);
    assert_eq!(harness.edge_ids(), ["e1"]);
    assert!(harness.app.selected.is_empty());

    harness.app.selected = Selection::of([Item::Edge(0), Item::Node(left.clone())]);
    harness.click_widget("Delete");
    assert_eq!(harness.edge_ids(), [""; 0]);
    assert_eq!(document::graphics_of(&harness.app.graphml, &left), None);
    assert!(document::graphics_of(&harness.app.graphml, &right).is_some());
}

/// Drags a box round the fixture's n0 and n1 and the bends of e0 between
/// them, and round n2 and n3, which the canvas doesn't draw.
fn drag_box_round_e0(harness: &mut Harness) {
    harness.drag(vec2(50.0, 5.0), vec2(400.0, 300.0));
}

#[test]
fn boxed_edges_are_counted_and_nudged_with_their_nodes() {
    let mut harness = fixture_harness();
    drag_box_round_e0(&mut harness);
    let boxed = Selection::of([node("n0"), node("n1"), Item::Edge(0)]);
    assert_eq!(harness.app.selected, boxed);
    assert_eq!(rows(&harness.side_panel(), "Selected"), ["2 nodes, 1 line"]);

    harness.press(CTRL, Key::ArrowRight);
    let x = |id: &str| harness.graphics(id).geometry.x;
    assert_eq!([x("n0"), x("n1")], [101.0, 301.0]);
    // Once, though e0 is selected and joins two moved nodes.
    assert_eq!(
        harness.edge(0).path.points,
        [bend(261.0, 20.0), bend(326.0, 20.0)]
    );
}

#[test]
fn delete_removes_the_boxed_nodes_and_edges() {
    let mut harness = fixture_harness();
    drag_box_round_e0(&mut harness);
    harness.press(Modifiers::NONE, Key::Delete);
    for id in ["n0", "n1"] {
        assert_eq!(document::graphics_of(&harness.app.graphml, id), None);
    }
    assert_eq!(harness.edge_ids(), ["e1"]);
    assert!(harness.app.selected.is_empty());
}

#[test]
fn boxed_edges_are_copied_and_pasted_with_their_nodes() {
    let mut harness = fixture_harness();
    drag_box_round_e0(&mut harness);
    let (copied, _) = harness.clipboard(vec![Event::Copy]);
    assert_eq!(parse_copied(copied.clone()).graphs[0].edges.len(), 1);
    harness.run(vec![Event::Paste(copied[0].clone())]);
    let pasted = Selection::of([node("n6"), node("n7"), Item::Edge(2)]);
    assert_eq!(harness.app.selected, pasted);
    assert_eq!(harness.edge_ids(), ["e0", "e1", "e2"]);
}

#[test]
fn font_rows_show_for_the_labels_that_some_selected_items_have() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let mut labelled = labelled_box("Box");
    labelled.labels[0].font.size = 20;
    labelled.labels[0].text_color = RED;
    let left = document::add_node(graphml, &labelled);
    let right = document::add_node(graphml, &box_at(300.0, 0.0));
    document::add_edge(graphml, &left, &right, &edge_through(&[]));
    // The line, which has no label, first.
    let items = [
        Item::Edge(0),
        Item::Node(left.clone()),
        Item::Node(right.clone()),
    ];
    harness.app.selected = Selection::of(items);
    #[rustfmt::skip]
    assert_eq!(harness.side_panel(), [
        "Selected",   "2 nodes, 1 line",
        "Line",       "#000000",
        "Line width", "1",
        "Line style", "Solid",
        // Blank, as only the box has a label, whose font shows below.
        "Label",      "...", "",
        "Font",       "Dialog",
        "Font style", "Plain",
        "Font size",  "20",
        "Text color", "#FF0000",
    ]);

    // After the line width. The others get no labels.
    harness.click_number_field(1);
    harness.type_text("16");
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(harness.graphics(&left).labels[0].font.size, 16);
    assert_eq!(harness.graphics(&right).labels, []);
    assert_eq!(harness.edge(0).labels, []);
    // The picker starts on the label's color, so OK keeps it.
    harness.click_text("#FF0000");
    assert!(harness.painted("Text color"));
    harness.run(vec![]);
    harness.press(Modifiers::NONE, Key::Enter);
    assert!(!harness.app.color_picker.is_open());
    assert_eq!(harness.graphics(&left).labels[0].text_color, RED);
}

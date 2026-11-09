use super::*;

#[test]
fn delete_removes_the_selected_bend_then_its_edge() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::bend_of(0, 1);
    harness.press(Modifiers::NONE, Key::Delete);
    assert_eq!(harness.edge(0).path.points, [bend(260.0, 20.0)]);
    assert_eq!(harness.app.selected, Selection::edge(0));

    harness.press(Modifiers::NONE, Key::Delete);
    assert_eq!(harness.edge_ids(), ["e1"]);
    assert!(harness.app.selected.is_empty());
}

#[test]
fn delete_removes_the_selected_node_with_its_edges() {
    let mut harness = fixture_harness();
    let unchanged = harness.app.doc.graphml().clone();
    harness.press(Modifiers::NONE, Key::Delete);
    assert_eq!(*harness.app.doc.graphml(), unchanged);

    harness.app.selected = Selection::node("n1");
    harness.press(Modifiers::NONE, Key::Delete);
    assert_eq!(document::graphics_of(harness.app.doc.graphml(), "n1"), None);
    assert_eq!(harness.edge_ids(), ["e1"]);
    assert!(harness.app.selected.is_empty());
}

#[test]
fn the_side_panels_delete_button_deletes_the_selected_node() {
    let mut harness = fixture_harness();
    assert!(harness.widget("Delete").is_disabled());
    harness.app.selected = Selection::node("n1");
    harness.click_widget("Delete");
    assert_eq!(document::graphics_of(harness.app.doc.graphml(), "n1"), None);
    assert_eq!(harness.edge_ids(), ["e1"]);
    assert!(harness.app.selected.is_empty());
    assert!(harness.widget("Delete").is_disabled());
}

#[test]
fn the_delete_button_keeps_the_label_typed_off_the_next_edge() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::edge(0);
    // The side panel is painted first, so this is its field, not the canvas.
    harness.click_text("reads");
    harness.press(CTRL, Key::A);
    harness.type_text("writes");
    harness.click_widget("Delete");
    assert_eq!(harness.edge_ids(), ["e1"]);
    assert_eq!(harness.edge(0).labels, []);
}

#[test]
fn the_delete_button_keeps_the_label_edited_on_the_canvas_off_the_next_edge() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::edge(0);
    harness.press(CTRL, Key::Enter);
    harness.run(vec![]);
    harness.type_text("writes");
    harness.click_widget("Delete");
    assert_eq!(harness.edge_ids(), ["e1"]);
    assert_eq!(harness.edge(0).labels, []);
}

#[test]
fn the_side_panel_shows_the_selected_edge() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::edge(0);
    let side_panel = harness.side_panel();
    for (name, value) in [
        ("Id", "e0"),
        ("Source", "n0"),
        ("Target", "n1"),
        ("Line", "#000000"),
        ("Line width", "1"),
        ("Line style", "Solid"),
    ] {
        assert_eq!(rows(&side_panel, name), [value], "{side_panel:?}");
    }
    let label = ["Label", "...", "reads"].map(str::to_owned);
    assert!(
        side_panel.windows(3).any(|row| row == label),
        "{side_panel:?}"
    );
    // Edge labels are placed by dragging them.
    assert!(!side_panel.contains(&"Position".to_owned()));
    let checked: Vec<_> = (harness.checkboxes().iter())
        .map(|checkbox| checkbox.toggled() == Some(egui::accesskit::Toggled::True))
        .collect();
    assert_eq!(checked, [false, true]);
}

#[test]
fn the_arrow_check_boxes_set_the_arrowheads() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::edge(0);
    let [start, end] = harness.checkboxes().try_into().unwrap();
    harness.click_at(testing::center(&start));
    harness.click_at(testing::center(&end));
    let arrows = harness.edge(0).arrows;
    assert_eq!(
        (arrows.source, arrows.target),
        (yed::Arrow::Standard, yed::Arrow::None)
    );
}

#[test]
fn line_properties_apply_to_the_selected_edge() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::edge(0);
    let red = Some(RED);
    for action in [
        Action::SetLineWidth(2.5),
        Action::SetLineType(yed::LineType::Dotted),
        Action::SetColor(ColorProperty::Line, red),
    ] {
        harness.perform(action);
    }
    let line = harness.edge(0).line;
    assert_eq!(
        (line.width, line.line_type, line.color),
        (2.5, yed::LineType::Dotted, red)
    );
}

#[test]
fn ctrl_arrows_move_the_selected_bend_or_else_all_bends() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::edge(0);
    harness.press(CTRL, Key::ArrowRight);
    assert_eq!(
        harness.edge(0).path.points,
        [bend(261.0, 20.0), bend(326.0, 20.0)]
    );
    harness.app.selected = Selection::bend_of(0, 1);
    harness.press(CTRL, Key::ArrowDown);
    assert_eq!(
        harness.edge(0).path.points,
        [bend(261.0, 20.0), bend(326.0, 21.0)]
    );
}

#[test]
fn ctrl_enter_edits_the_selected_edges_label_on_the_canvas() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::edge(0);
    harness.press(CTRL, Key::Enter);
    harness.run(vec![]);
    // The label's text is selected, so typing replaces it.
    harness.type_text("writes");
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(harness.edge(0).labels[0].text, "writes");
}

#[test]
fn the_line_tool_joins_nodes_and_its_edges_get_labels_beside_them() {
    let mut harness = Harness::new();
    harness.press(Modifiers::NONE, Key::L);
    let graphml = harness.app.doc.graphml_mut();
    let left = document::add_node(graphml, &new_box());
    let mut right = new_box();
    right.geometry.x += 300.0;
    let right = document::add_node(graphml, &right);
    harness.drag(vec2(150.0, 60.0), vec2(450.0, 60.0));

    let edge = &harness.app.doc.graphml().graphs[0].edges[0];
    assert_eq!((&edge.source, &edge.target), (&left, &right));
    assert_eq!(harness.app.selected, Selection::edge(0));
    harness.press(CTRL, Key::Enter);
    harness.run(vec![]);
    harness.type_text("joins");
    harness.press(Modifiers::NONE, Key::Enter);
    let label = &harness.edge(0).labels[0];
    assert_eq!(label.text, "joins");
    assert!(
        label
            .placed_on(&route_of(&harness, 0), 40.0, 20.0)
            .is_some()
    );
}

/// The route the labels of the edge at `index` are placed along.
fn route_of(harness: &Harness, index: usize) -> yed::Route {
    let graphml = harness.app.doc.graphml();
    let edge = &graphml.graphs[0].edges[index];
    let bounds = |id: &str| document::graphics_of(graphml, id).unwrap().geometry;
    yed::Route::new(
        &harness.edge(index).path,
        bounds(&edge.source),
        bounds(&edge.target),
    )
}

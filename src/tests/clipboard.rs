use super::*;

#[test]
fn cut_copy_and_paste_sit_between_export_and_undo() {
    let mut harness = Harness::new();
    let labels = ["Export", "Cut", "Copy", "Paste", "Undo"];
    let [export, cut, copy, paste, undo] =
        labels.map(|label| testing::center(&harness.widget(label)).x);
    assert!(export < cut && cut < copy && copy < paste && paste < undo);
    // Separators either side.
    assert!(cut - export > copy - cut + 4.0);
    assert!(undo - paste > copy - cut + 4.0);
}

#[test]
fn cut_and_copy_need_a_selected_node() {
    let mut harness = fixture_harness();
    for (selected, enabled) in [
        (Selection::default(), false),
        (Selection::edge(0), false),
        (Selection::of([Item::Edge(0), node("n0")]), true),
    ] {
        harness.app.selected = selected;
        for label in ["Cut", "Copy"] {
            assert_eq!(!harness.widget(label).is_disabled(), enabled, "{label}");
        }
        assert!(!harness.widget("Paste").is_disabled());
    }
}

#[test]
fn ctrl_c_then_ctrl_v_pastes_beside_the_originals() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::of([node("n0"), node("n1"), Item::Edge(0), Item::Edge(1)]);
    let (copied, _) = harness.clipboard(vec![Event::Copy]);
    let copy = parse_copied(copied.clone());
    let nodes: Vec<_> = copy.graphs[0].nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(nodes, ["n0", "n1"]);
    assert_eq!(copy.graphs[0].edges.len(), 1);

    harness.run(vec![Event::Paste(copied[0].clone())]);
    let pasted = Selection::of([node("n6"), node("n7"), Item::Edge(2)]);
    assert_eq!(harness.app.selected, pasted);
    let geometry = harness.graphics("n6").geometry;
    assert_eq!((geometry.x, geometry.y), (110.0, 50.5));
    assert_eq!(harness.edge_ids(), ["e0", "e1", "e2"]);
}

#[test]
fn ctrl_x_then_ctrl_v_puts_the_nodes_back() {
    let mut harness = fixture_harness();
    let before = harness.graphics("n4");
    harness.app.selected = Selection::node("n4");
    let (copied, _) = harness.clipboard(vec![Event::Cut]);
    assert_eq!(copied.len(), 1);
    assert!(harness.app.selected.is_empty());
    assert_eq!(document::graphics_of(harness.app.doc.graphml(), "n4"), None);

    harness.run(vec![Event::Paste(copied[0].clone())]);
    assert_eq!(harness.app.selected, Selection::node("n4"));
    assert_eq!(harness.graphics("n4"), before);
}

#[test]
fn cut_and_copy_with_only_lines_selected_do_nothing() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::edge(0);
    let before = harness.app.doc.graphml().clone();
    let (copied, _) = harness.clipboard(vec![Event::Copy, Event::Cut]);
    assert_eq!(copied, [""; 0]);
    assert_eq!(*harness.app.doc.graphml(), before);
    assert_eq!(harness.app.selected, Selection::edge(0));
}

#[test]
fn pasting_text_without_nodes_does_nothing() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::node("n0");
    let before = harness.app.doc.graphml().clone();
    // Text other than GraphML, and GraphML without nodes.
    for text in ["Hello".to_owned(), yed::new_document().to_xml()] {
        harness.run(vec![Event::Paste(text)]);
        assert_eq!(*harness.app.doc.graphml(), before);
        assert_eq!(harness.app.selected, Selection::node("n0"));
    }
}

#[test]
fn clipboard_keys_in_a_label_field_edit_its_text() {
    let mut harness = Harness::new();
    let id = type_in_label_field(&mut harness, "Start");
    // The selected text, not the box.
    let (copied, _) = harness.clipboard(vec![Event::Copy]);
    assert_eq!(copied, ["Start"]);
    harness.run(vec![Event::Paste("Pasted".to_owned())]);
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(harness.label(&id), "Pasted");
}

#[test]
fn the_toolbar_buttons_cut_copy_and_paste() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::node("n2");
    let at = |harness: &mut Harness, label| testing::center(&harness.widget(label));
    let copy = at(&mut harness, "Copy");
    let (copied, paste) = harness.click_for_clipboard(copy);
    assert_eq!(parse_copied(copied.clone()).graphs[0].nodes[0].id, "n2");
    assert!(!paste);
    assert_eq!(harness.app.selected, Selection::node("n2"));

    // eframe answers with a paste event in the next frame.
    let paste_at = at(&mut harness, "Paste");
    let (_, paste) = harness.click_for_clipboard(paste_at);
    assert!(paste);
    harness.run(vec![Event::Paste(copied[0].clone())]);
    assert_eq!(harness.app.selected, Selection::node("n6"));

    let cut = at(&mut harness, "Cut");
    let (copied, _) = harness.click_for_clipboard(cut);
    assert_eq!(parse_copied(copied).graphs[0].nodes[0].id, "n6");
    assert_eq!(document::graphics_of(harness.app.doc.graphml(), "n6"), None);
    assert!(harness.app.selected.is_empty());
}

#[test]
fn the_edit_menu_cuts_copies_and_pastes() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::node("n0");
    harness.click_widget("Edit");
    // Items read with their shortcuts.
    let items = [
        ("Cut", Action::Cut),
        ("Copy", Action::Copy),
        ("Paste", Action::Paste),
    ]
    .map(|(name, action)| {
        let shortcut = shortcuts::text(&harness.ctx, &action).unwrap();
        testing::center(&harness.widget(&format!("{name} {shortcut}")))
    });
    let [cut, copy, paste] = items;
    assert!(cut.y < copy.y && copy.y < paste.y);
    let (copied, _) = harness.click_for_clipboard(copy);
    assert_eq!(parse_copied(copied.clone()).graphs[0].nodes[0].id, "n0");
    harness.run(vec![]);
    assert!(!Popup::is_any_open(&harness.ctx));

    // eframe answers with a paste event in the next frame.
    harness.click_widget("Edit");
    let (_, asked) = harness.click_for_clipboard(paste);
    assert!(asked);
    harness.run(vec![Event::Paste(copied[0].clone())]);
    assert_eq!(harness.app.selected, Selection::node("n6"));

    harness.click_widget("Edit");
    let (copied, _) = harness.click_for_clipboard(cut);
    assert_eq!(parse_copied(copied).graphs[0].nodes[0].id, "n6");
    assert_eq!(document::graphics_of(harness.app.doc.graphml(), "n6"), None);
}

#[test]
fn the_cut_button_applies_the_label_typed_first() {
    let mut harness = Harness::new();
    let graphml = harness.app.doc.graphml_mut();
    let left = document::add_node(graphml, &labelled_box("Left"));
    let middle = document::add_node(graphml, &box_at(300.0, 0.0));
    let right = document::add_node(graphml, &box_at(600.0, 0.0));
    document::add_edge(graphml, &left, &middle, &edge_through(&[]));
    document::add_edge(graphml, &middle, &right, &edge_through(&[]));
    harness.app.selected = Selection::of([node(&left), Item::Edge(0)]);
    let field = harness.empty_field();
    harness.click_at(field);
    harness.type_text("Typed");
    let cut = testing::center(&harness.widget("Cut"));
    let (copied, _) = harness.click_for_clipboard(cut);
    // Not on the edge that took the cut edge's index.
    assert_eq!(harness.edge_ids(), ["e1"]);
    assert_eq!(harness.edge(0).labels, []);
    let copy = parse_copied(copied);
    let label = LabelRef {
        item: node(&left),
        index: 0,
    };
    assert_eq!(
        document::label_text(&copy, &label).as_deref(),
        Some("Typed")
    );
}

#[test]
fn the_copy_button_copies_the_label_edited_on_the_canvas() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::node("n2");
    harness.press(CTRL, Key::Enter);
    harness.run(vec![]);
    harness.type_text("Renamed");
    let copy = testing::center(&harness.widget("Copy"));
    let (copied, _) = harness.click_for_clipboard(copy);
    let label = LabelRef {
        item: node("n2"),
        index: 0,
    };
    let copy = parse_copied(copied);
    assert_eq!(
        document::label_text(&copy, &label).as_deref(),
        Some("Renamed")
    );
    assert_eq!(harness.label("n2"), "Renamed");
}

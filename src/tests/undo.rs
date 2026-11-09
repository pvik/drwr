use super::*;

/// The fixture as opened.
fn fixture() -> GraphMl {
    GraphMl::read_file(FIXTURE).unwrap()
}

/// The left side of the node with id `id`.
fn left(harness: &Harness, id: &str) -> f64 {
    harness.graphics(id).geometry.x
}

#[test]
fn ctrl_z_undoes_a_drawn_box_and_ctrl_y_redoes_it() {
    let mut harness = Harness::new();
    harness.draw_box();
    let drawn = harness.app.doc.graphml().clone();
    let id = harness.app.selected_node().unwrap().to_owned();

    harness.press(CTRL, Key::Z);
    assert_eq!(*harness.app.doc.graphml(), yed::new_document());
    assert!(harness.app.selected.is_empty());
    assert!(!harness.app.modified);
    harness.press(CTRL, Key::Y);
    assert_eq!(*harness.app.doc.graphml(), drawn);
    assert_eq!(harness.app.selected, Selection::node(id));
    assert!(harness.app.modified);
}

#[test]
fn ctrl_shift_z_redoes_too() {
    let mut harness = Harness::new();
    harness.draw_box();
    let drawn = harness.app.doc.graphml().clone();
    harness.press(CTRL, Key::Z);
    harness.press(CTRL | Modifiers::SHIFT, Key::Z);
    assert_eq!(*harness.app.doc.graphml(), drawn);
}

#[test]
fn undo_brings_back_the_deleted_items_selected() {
    let mut harness = fixture_harness();
    let deleted = Selection::of([node("n0"), node("n1"), Item::Edge(1)]);
    harness.app.selected = deleted.clone();
    harness.press(Modifiers::NONE, Key::Delete);
    assert!(harness.app.selected.is_empty());

    harness.press(CTRL, Key::Z);
    assert_eq!(*harness.app.doc.graphml(), fixture());
    assert_eq!(harness.app.selected, deleted);
    // Shown again, with the labels of n0 and e0.
    assert!(harness.painted("Order\nservice"));
    assert!(harness.painted("reads"));
}

#[test]
fn a_drag_is_one_step() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.click(vec2(150.0, 60.0));
    harness.drag(vec2(175.0, 90.0), vec2(205.0, 130.0));
    assert_eq!(left(&harness, &id), 130.0);
    harness.press(CTRL, Key::Z);
    assert_eq!(harness.graphics(&id).geometry, new_box().geometry);
}

#[test]
fn a_run_of_nudges_is_undone_at_once() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.click(vec2(150.0, 60.0));
    for key in [Key::ArrowRight, Key::ArrowRight, Key::ArrowDown] {
        harness.press(CTRL, key);
    }
    // Key repeats while held.
    harness.run(vec![testing::key(CTRL, Key::ArrowRight); 3]);
    assert_eq!(left(&harness, &id), 105.0);

    harness.press(CTRL, Key::Z);
    assert_eq!(harness.graphics(&id).geometry, new_box().geometry);
    harness.press(CTRL, Key::Z);
    assert_eq!(document::graphics_of(harness.app.doc.graphml(), &id), None);
}

#[test]
fn a_new_selection_ends_a_run_of_nudges() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.click(vec2(150.0, 60.0));
    harness.press(CTRL, Key::ArrowRight);
    harness.click(vec2(600.0, 400.0));
    harness.click(vec2(150.0, 60.0));
    harness.press(CTRL, Key::ArrowRight);
    harness.press(CTRL, Key::Z);
    assert_eq!(left(&harness, &id), 101.0);
}

#[test]
fn another_edit_ends_a_run_of_nudges() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.click(vec2(150.0, 60.0));
    harness.press(CTRL, Key::ArrowRight);
    // In a frame of its own, as when set in the side panel.
    harness.perform(Action::SetLineWidth(3.0));
    harness.run(vec![]);
    harness.press(CTRL, Key::ArrowRight);
    harness.press(CTRL, Key::Z);
    assert_eq!(left(&harness, &id), 101.0);
    assert_eq!(harness.graphics(&id).border.width, 3.0);
    harness.press(CTRL, Key::Z);
    assert_eq!(harness.graphics(&id).border.width, 1.0);
}

#[test]
fn dragging_a_number_field_is_one_step() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.click(vec2(150.0, 60.0));
    let field = harness.text_at("1");
    harness.drag_and_hold(field, field + vec2(10.0, 0.0));
    let to = field + vec2(20.0, 0.0);
    harness.run(vec![Event::PointerMoved(to)]);
    harness.run(vec![button(to, false)]);
    assert_eq!(harness.graphics(&id).border.width, 3.0);

    harness.press(CTRL, Key::Z);
    assert_eq!(harness.graphics(&id).border.width, 1.0);
    harness.press(CTRL, Key::Y);
    assert_eq!(harness.graphics(&id).border.width, 3.0);
}

#[test]
fn undo_and_redo_sit_between_paste_and_the_tools() {
    let mut harness = Harness::new();
    let labels = ["Paste", "Undo", "Redo", "Cursor"];
    let [paste, undo, redo, cursor] = labels.map(|label| testing::center(&harness.widget(label)).x);
    assert!(paste < undo && undo < redo && redo < cursor);
    // Separators either side.
    assert!(undo - paste > redo - undo + 4.0);
    assert!(cursor - redo > redo - undo + 4.0);
}

#[test]
fn the_toolbar_buttons_undo_and_redo_when_there_is_something_to() {
    let mut harness = Harness::new();
    assert!(harness.widget("Undo").is_disabled());
    assert!(harness.widget("Redo").is_disabled());
    harness.draw_box();
    assert!(!harness.widget("Undo").is_disabled());
    assert!(harness.widget("Redo").is_disabled());

    harness.click_widget("Undo");
    assert_eq!(*harness.app.doc.graphml(), yed::new_document());
    assert!(harness.widget("Undo").is_disabled());
    assert!(!harness.widget("Redo").is_disabled());
    harness.click_widget("Redo");
    assert!(harness.app.selected_node().is_some());
    assert!(harness.widget("Redo").is_disabled());
}

#[test]
fn the_edit_menu_undoes_and_redoes() {
    let mut harness = Harness::new();
    harness.draw_box();
    let drawn = harness.app.doc.graphml().clone();
    let item = |harness: &Harness, name: &str, action: Action| {
        let shortcut = shortcuts::text(&harness.ctx, &action).unwrap();
        format!("{name} {shortcut}")
    };
    let (undo, redo) = (
        item(&harness, "Undo", Action::Undo),
        item(&harness, "Redo", Action::Redo),
    );
    assert_eq!(undo, "Undo Ctrl+Z");
    assert_eq!(redo, "Redo Ctrl+Y");

    harness.click_widget("Edit");
    assert!(harness.widget(&redo).is_disabled());
    let cut = format!(
        "Cut {}",
        shortcuts::text(&harness.ctx, &Action::Cut).unwrap()
    );
    assert!(testing::center(&harness.widget(&redo)).y < testing::center(&harness.widget(&cut)).y);
    harness.click_widget(&undo);
    assert_eq!(*harness.app.doc.graphml(), yed::new_document());
    harness.click_widget("Edit");
    harness.click_widget(&redo);
    assert_eq!(*harness.app.doc.graphml(), drawn);
}

#[test]
fn undoing_back_to_the_saved_file_clears_the_modified_marker() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::node("n0");
    harness.press(CTRL, Key::ArrowRight);
    assert!(harness.app.modified);
    assert!(harness.app.title.contains('*'));

    harness.press(CTRL, Key::Z);
    assert!(!harness.app.modified);
    assert!(!harness.app.title.contains('*'));
    assert_eq!(*harness.app.doc.graphml(), harness.app.saved);
    harness.press(CTRL, Key::Y);
    assert!(harness.app.modified);
}

#[test]
fn a_new_drawing_forgets_what_was_done() {
    let mut harness = Harness::new();
    harness.draw_box();
    harness.press(CTRL, Key::N);
    harness.answer_prompt("Don't Save");
    assert!(harness.widget("Undo").is_disabled());
    harness.press(CTRL, Key::Z);
    assert_eq!(*harness.app.doc.graphml(), yed::new_document());
}

#[test]
fn ctrl_z_in_a_label_field_is_left_to_the_field() {
    let mut harness = Harness::new();
    let id = type_in_label_field(&mut harness, "Start");
    harness.type_text("Typed");
    harness.press(CTRL, Key::Z);
    assert!(harness.ctx.text_edit_focused());
    assert_eq!(harness.label(&id), "Start");
    assert!(document::graphics_of(harness.app.doc.graphml(), &id).is_some());
}

#[test]
fn the_undo_button_takes_back_the_label_being_typed() {
    let mut harness = Harness::new();
    let id = type_in_label_field(&mut harness, "Start");
    harness.type_text("Typed");
    harness.click_widget("Undo");
    assert_eq!(harness.label(&id), "Start");
    // Selected, as when the label was typed.
    assert_eq!(harness.app.selected, Selection::node(id.clone()));
    harness.click_widget("Redo");
    assert_eq!(harness.label(&id), "Typed");
    assert_eq!(harness.app.selected, Selection::node(id.clone()));
}

#[test]
fn the_redo_button_applies_the_label_being_typed_first() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &labelled_box("Start"));
    harness.click(vec2(150.0, 60.0));
    harness.press(CTRL, Key::ArrowRight);
    harness.press(CTRL, Key::Z);
    // The side panel is painted first, so this is its field, not the canvas.
    harness.click_text("Start");
    harness.press(CTRL, Key::A);
    harness.type_text("Typed");
    harness.click_widget("Redo");
    assert_eq!(harness.label(&id), "Typed");
    // A new change, which leaves the nudge undone.
    assert_eq!(harness.graphics(&id).geometry, new_box().geometry);
}

#[test]
fn undo_drops_a_drag_in_progress() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.click(vec2(150.0, 60.0));
    harness.press(CTRL, Key::ArrowRight);
    let origin = canvas(&harness.run(vec![])).min;
    let to = origin + vec2(205.0, 130.0);
    harness.drag_and_hold(origin + vec2(175.0, 90.0), to);
    harness.press(CTRL, Key::Z);
    harness.run(vec![button(to, false)]);
    assert_eq!(harness.graphics(&id).geometry, new_box().geometry);
}

#[test]
fn the_undo_button_takes_back_the_label_typed_on_the_canvas() {
    let mut harness = fixture_harness();
    let before = harness.label("n0");
    harness.app.selected = Selection::node("n0");
    harness.press(CTRL, Key::ArrowRight);
    let nudged = harness.app.doc.graphml().clone();
    harness.press(CTRL, Key::Enter);
    harness.run(vec![]);
    harness.type_text("Renamed");
    harness.click_widget("Undo");
    assert_eq!(*harness.app.doc.graphml(), nudged);
    assert_eq!(harness.label("n0"), before);
    assert!(!harness.ctx.text_edit_focused());
    assert_eq!(harness.app.selected, Selection::node("n0"));
}

use egui::accesskit::Toggled;

use super::*;

/// Adds the box from [`new_box`], and one 300 right of it, returning the
/// first's id.
fn two_boxes(harness: &mut Harness) -> String {
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    document::add_node(harness.app.doc.graphml_mut(), &box_at(300.0, 0.0));
    id
}

/// Drags the first of [`two_boxes`] by `by`, holding `modifiers`.
fn drag_box(harness: &mut Harness, by: Vec2, modifiers: Modifiers) {
    harness.run(vec![Event::ModifiersChanged(modifiers)]);
    harness.drag(vec2(175.0, 90.0), vec2(175.0, 90.0) + by);
    harness.run(vec![Event::ModifiersChanged(Modifiers::NONE)]);
}

#[test]
fn snap_lines_start_on_and_toggle_from_the_view_menu() {
    let mut harness = Harness::new();
    harness.click_widget("View");
    assert_eq!(harness.widget("Snap Lines").toggled(), Some(Toggled::True));
    harness.click_widget("Snap Lines");
    assert!(!harness.app.settings.view.snap_lines);
    harness.click_widget("View");
    assert_eq!(harness.widget("Snap Lines").toggled(), Some(Toggled::False));
}

#[test]
fn dragged_nodes_snap_into_line_unless_alt_is_held() {
    for (modifiers, x) in [(Modifiers::NONE, 400.0), (Modifiers::ALT, 397.0)] {
        let mut harness = Harness::new();
        let id = two_boxes(&mut harness);
        // Under the other box, 3 left of it.
        drag_box(&mut harness, vec2(297.0, 200.0), modifiers);
        assert_eq!(harness.graphics(&id).geometry.x, x, "{modifiers:?}");
    }
}

#[test]
fn nothing_snaps_with_snap_lines_off() {
    let mut harness = Harness::new();
    let id = two_boxes(&mut harness);
    harness.perform(Action::ToggleSnapLines);
    drag_box(&mut harness, vec2(297.0, 200.0), Modifiers::NONE);
    assert_eq!(harness.graphics(&id).geometry.x, 397.0);
}

#[test]
fn a_snapped_move_undoes_in_one_step() {
    let mut harness = Harness::new();
    let id = two_boxes(&mut harness);
    harness.run(vec![]);
    drag_box(&mut harness, vec2(297.0, 200.0), Modifiers::NONE);
    harness.press(CTRL, Key::Z);
    harness.run(vec![]);
    assert_eq!(harness.graphics(&id).geometry, new_box().geometry);
}

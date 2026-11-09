use super::*;

/// A harness with three boxes, bottom up: n0, n1 out of its way, and n2 over
/// its lower right corner.
fn stacked_harness() -> Harness {
    let mut harness = Harness::new();
    for (dx, dy) in [(0.0, 0.0), (300.0, 0.0), (60.0, 40.0)] {
        document::add_node(harness.app.doc.graphml_mut(), &box_at(dx, dy));
    }
    harness.run(vec![]);
    harness
}

/// Where n0 and n2 overlap, relative to the canvas.
const OVERLAP: Vec2 = vec2(200.0, 110.0);

/// The ids of the nodes, bottom up.
fn node_ids(harness: &Harness) -> Vec<&str> {
    let nodes = document::top_nodes(harness.app.doc.graphml()).iter();
    nodes.map(|node| node.id.as_str()).collect()
}

#[test]
fn the_raise_and_lower_buttons_restack_the_selected_node() {
    let mut harness = stacked_harness();
    assert!(harness.widget("Raise").is_disabled());
    assert!(harness.widget("Lower").is_disabled());

    harness.app.selected = Selection::node("n0");
    assert!(harness.widget("Lower").is_disabled());
    harness.click_widget("Raise");
    assert_eq!(node_ids(&harness), ["n1", "n2", "n0"]);
    assert!(harness.widget("Raise").is_disabled());

    harness.click_widget("Lower");
    assert_eq!(node_ids(&harness), ["n1", "n0", "n2"]);
    assert_eq!(harness.app.selected, Selection::node("n0"));
}

#[test]
fn raise_and_lower_need_a_selected_node() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::edge(0);
    assert!(harness.widget("Raise").is_disabled());
    assert!(harness.widget("Lower").is_disabled());
}

#[test]
fn page_up_and_page_down_restack_the_selected_node() {
    let mut harness = stacked_harness();
    harness.app.selected = Selection::node("n0");
    harness.press(Modifiers::NONE, Key::PageUp);
    assert_eq!(node_ids(&harness), ["n1", "n2", "n0"]);
    harness.press(Modifiers::NONE, Key::PageUp);
    assert_eq!(node_ids(&harness), ["n1", "n2", "n0"]);

    harness.press(Modifiers::NONE, Key::PageDown);
    assert_eq!(node_ids(&harness), ["n1", "n0", "n2"]);
}

#[test]
fn a_click_picks_the_raised_node() {
    let mut harness = stacked_harness();
    harness.click(OVERLAP);
    assert_eq!(harness.app.selected_node(), Some("n2"));

    harness.app.selected = Selection::node("n0");
    harness.press(Modifiers::NONE, Key::PageUp);
    harness.click(vec2(20.0, 300.0));
    assert!(harness.app.selected.is_empty());
    harness.click(OVERLAP);
    assert_eq!(harness.app.selected_node(), Some("n0"));
}

#[test]
fn undo_puts_the_raised_node_back() {
    let mut harness = stacked_harness();
    harness.app.selected = Selection::node("n0");
    harness.click_widget("Raise");
    harness.click_widget("Lower");

    harness.press(CTRL, Key::Z);
    assert_eq!(node_ids(&harness), ["n1", "n2", "n0"]);
    harness.press(CTRL, Key::Z);
    assert_eq!(node_ids(&harness), ["n0", "n1", "n2"]);
    assert_eq!(harness.app.selected, Selection::node("n0"));

    harness.press(CTRL, Key::Y);
    assert_eq!(node_ids(&harness), ["n1", "n2", "n0"]);
}

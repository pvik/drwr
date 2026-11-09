use super::*;

#[test]
fn the_zoom_buttons_step_the_zoom_which_the_dropdown_shows() {
    let mut harness = Harness::new();
    assert!(harness.painted("100%"));
    harness.click_widget("Zoom In");
    assert_eq!(harness.app.canvas.zoom(), 1.25);
    assert!(harness.painted("125%"));
    for _ in 0..2 {
        harness.click_widget("Zoom Out");
    }
    assert_eq!(harness.app.canvas.zoom(), 0.75);
    assert!(harness.painted("75%"));
}

#[test]
fn the_zoom_buttons_are_disabled_at_the_limits() {
    let mut harness = Harness::new();
    harness.app.canvas.set_zoom(zoom::MAX);
    assert!(harness.widget("Zoom In").is_disabled());
    assert!(!harness.widget("Zoom Out").is_disabled());
    harness.app.canvas.set_zoom(zoom::MIN);
    assert!(!harness.widget("Zoom In").is_disabled());
    assert!(harness.widget("Zoom Out").is_disabled());
}

#[test]
fn picking_a_zoom_in_the_dropdown_sets_it() {
    let mut harness = Harness::new();
    harness.click_text("100%");
    harness.click_text("150%");
    assert_eq!(harness.app.canvas.zoom(), 1.5);
    assert!(harness.painted("150%"));
    assert!(!Popup::is_any_open(&harness.ctx));
}

#[test]
fn the_dropdown_shows_the_zoom_the_wheel_sets() {
    let mut harness = Harness::new();
    let at = canvas(&harness.run(vec![])).center();
    harness.wheel(at, 1.0);
    assert!(harness.painted("122%"));
    harness.wheel(at, -2.0);
    assert!(harness.painted("82%"));
}

#[test]
fn the_wheel_over_the_side_panel_does_not_zoom() {
    let mut harness = Harness::new();
    let at = canvas(&harness.run(vec![])).right_center() + vec2(50.0, 0.0);
    harness.wheel(at, 1.0);
    assert_eq!(harness.app.canvas.zoom(), 1.0);
}

#[test]
fn ctrl_plus_minus_and_0_zoom_the_canvas_not_the_ui() {
    let mut harness = Harness::new();
    for (key, zoom) in [
        (Key::Plus, 1.25),
        (Key::Equals, 1.5),
        (Key::Minus, 1.25),
        (Key::Num0, 1.0),
    ] {
        harness.press(CTRL, key);
        harness.run(vec![]);
        assert_eq!(harness.app.canvas.zoom(), zoom, "{key:?}");
        assert_eq!(harness.ctx.zoom_factor(), 1.0, "{key:?}");
    }
}

#[test]
fn ctrl_plus_in_a_label_field_zooms_nothing() {
    let mut harness = Harness::new();
    type_in_label_field(&mut harness, "Start");
    harness.press(CTRL, Key::Plus);
    harness.run(vec![]);
    assert_eq!(harness.app.canvas.zoom(), 1.0);
    assert_eq!(harness.ctx.zoom_factor(), 1.0);
}

#[test]
fn middle_dragging_the_canvas_pans_it() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    let origin = canvas(&harness.run(vec![])).min;
    let (from, to) = (origin + vec2(300.0, 300.0), origin + vec2(250.0, 280.0));
    harness.run(vec![Event::PointerMoved(from)]);
    harness.run(vec![testing::middle_button(from, true)]);
    harness.run(vec![Event::PointerMoved(to)]);
    harness.run(vec![testing::middle_button(to, false)]);

    // The box at (100, 50) shows 50 left and 20 up from there, unmoved.
    harness.click_at(origin + vec2(60.0, 40.0));
    assert_eq!(harness.app.selected, Selection::node(id.clone()));
    assert_eq!(harness.graphics(&id).geometry, new_box().geometry);
}

#[test]
fn arrows_pan_the_canvas() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    for key in [Key::ArrowRight, Key::ArrowRight, Key::ArrowDown] {
        harness.press(Modifiers::NONE, key);
    }

    // The box at (100, 50) shows 80 left and 40 up from there, unmoved.
    harness.click(vec2(60.0, 40.0));
    assert_eq!(harness.app.selected, Selection::node(id.clone()));
    assert_eq!(harness.graphics(&id).geometry, new_box().geometry);
}

#[test]
fn arrows_pan_while_ctrl_arrows_move_the_selected_node() {
    let mut harness = Harness::new();
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.click(vec2(150.0, 60.0));
    harness.press(Modifiers::NONE, Key::ArrowLeft);
    harness.press(CTRL, Key::ArrowLeft);
    assert_eq!(harness.graphics(&id).geometry.x, 99.0);

    // The box spans 99 to 249 across, and shows 40 right of there.
    harness.click(vec2(270.0, 60.0));
    assert_eq!(harness.app.selected, Selection::node(id));
}

#[test]
fn arrows_in_a_label_field_move_the_text_cursor() {
    let mut harness = Harness::new();
    let id = type_in_label_field(&mut harness, "Start");
    harness.type_text("One two");
    for _ in 0..3 {
        harness.press(Modifiers::NONE, Key::ArrowLeft);
    }
    harness.type_text("big ");
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(harness.label(&id), "One big two");

    // Unpanned, the box at (100, 50) is still there.
    harness.click(vec2(110.0, 60.0));
    assert_eq!(harness.app.selected, Selection::node(id));
}

#[test]
fn frames_that_change_nothing_read_nothing() {
    let mut harness = fixture_harness();
    let reads = harness.app.doc.reads();
    let origin = canvas(&harness.run(vec![])).min;
    for x in [60.0, 120.0, 180.0, 240.0] {
        harness.run(vec![Event::PointerMoved(origin + vec2(x, 40.0))]);
    }
    harness.app.selected = Selection::of([node("n0"), node("n1"), node("n2"), Item::Edge(0)]);
    harness.run(vec![]);
    harness.drag_and_hold(origin + vec2(500.0, 500.0), origin + vec2(520.0, 530.0));
    assert_eq!(harness.app.doc.reads(), reads);

    // Only what changes is read again.
    harness.app.selected = Selection::node("n3");
    harness.press(CTRL, Key::ArrowRight);
    assert_eq!(harness.app.doc.reads(), reads + 1);
}

const GRID_LINES: &str = "Canvas Grid Lines Ctrl+'";

/// Whether the canvas painted grid lines: quads of gray.
fn grid_painted(shapes: &[Shape]) -> bool {
    shapes.iter().any(|shape| match shape {
        Shape::Mesh(mesh) => mesh.vertices.iter().any(|vertex| {
            let [r, g, b, _] = vertex.color.to_array();
            r == g && g == b && 0xC0 < r && r < 0xFF
        }),
        _ => false,
    })
}

#[test]
fn the_view_menu_has_the_zoom_commands_then_grid_and_snap_lines() {
    let mut harness = Harness::new();
    harness.click_widget("View");
    let labels = [
        "Zoom In Ctrl+Plus",
        "Zoom Out Ctrl+Minus",
        "Zoom to 100% Ctrl+0",
        GRID_LINES,
        "Snap Lines",
    ];
    let ys: Vec<_> = (labels.iter())
        .map(|label| testing::center(&harness.widget(label)).y)
        .collect();
    assert!(ys.is_sorted(), "{ys:?}");
}

#[test]
fn the_view_menu_zooms_the_canvas() {
    let mut harness = Harness::new();
    for (label, zoom) in [
        ("Zoom In Ctrl+Plus", 1.25),
        ("Zoom In Ctrl+Plus", 1.5),
        ("Zoom Out Ctrl+Minus", 1.25),
        ("Zoom to 100% Ctrl+0", 1.0),
    ] {
        harness.click_widget("View");
        harness.click_widget(label);
        assert_eq!(harness.app.canvas.zoom(), zoom, "{label}");
        assert!(!Popup::is_any_open(&harness.ctx), "{label}");
    }
}

#[test]
fn the_view_menus_zoom_items_are_disabled_at_the_limits() {
    let mut harness = Harness::new();
    harness.app.canvas.set_zoom(zoom::MAX);
    harness.click_widget("View");
    assert!(harness.widget("Zoom In Ctrl+Plus").is_disabled());
    assert!(!harness.widget("Zoom Out Ctrl+Minus").is_disabled());
    harness.press(Modifiers::NONE, Key::Escape);
    harness.app.canvas.set_zoom(zoom::MIN);
    harness.click_widget("View");
    assert!(!harness.widget("Zoom In Ctrl+Plus").is_disabled());
    assert!(harness.widget("Zoom Out Ctrl+Minus").is_disabled());
}

#[test]
fn canvas_grid_lines_toggle_from_the_view_menu_and_with_ctrl_quote() {
    use egui::accesskit::{Role, Toggled};
    let mut harness = Harness::new();
    assert!(!grid_painted(&harness.run(vec![])));
    harness.click_widget("View");
    let item = harness.widget(GRID_LINES);
    assert_eq!(item.role(), Role::CheckBox);
    assert_eq!(item.toggled(), Some(Toggled::False));
    harness.click_widget(GRID_LINES);
    assert!(harness.app.settings.view.grid_lines);
    assert!(!Popup::is_any_open(&harness.ctx));
    assert!(grid_painted(&harness.run(vec![])));

    harness.click_widget("View");
    assert_eq!(harness.widget(GRID_LINES).toggled(), Some(Toggled::True));
    harness.press(Modifiers::NONE, Key::Escape);
    harness.press(CTRL, Key::Quote);
    assert!(!harness.app.settings.view.grid_lines);
    assert!(!grid_painted(&harness.run(vec![])));
    harness.press(CTRL, Key::Quote);
    assert!(harness.app.settings.view.grid_lines);
}

#[test]
fn grid_and_snap_lines_stay_as_set_for_new_and_opened_graphs() {
    let mut harness = Harness::new();
    harness.press(CTRL, Key::Quote);
    harness.perform(Action::ToggleSnapLines);
    harness.press(CTRL, Key::N);
    harness.run(vec![]);
    let options = canvas::Options {
        grid_lines: true,
        snap_lines: false,
    };
    assert_eq!(harness.app.settings.view, options);
    harness.app.open(Path::new(FIXTURE)).unwrap();
    assert!(grid_painted(&harness.run(vec![])));
    assert_eq!(harness.app.settings.view, options);
}

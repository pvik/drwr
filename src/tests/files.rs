use super::*;
use crate::testing::ScratchDir;

#[test]
fn starts_with_an_empty_unsaved_graph() {
    let app = DrwrApp::new(Fonts::default());
    assert_eq!(*app.doc.graphml(), yed::new_document());
    assert_eq!(app.file, None);
    assert_eq!(app.saved, *app.doc.graphml());
    assert!(!app.modified);
}

#[test]
fn open_replaces_the_graph() {
    let mut app = DrwrApp::default();
    app.open(Path::new(FIXTURE)).unwrap();
    assert_eq!(*app.doc.graphml(), GraphMl::read_file(FIXTURE).unwrap());
    assert_ne!(*app.doc.graphml(), GraphMl::default());
}

#[test]
fn open_records_the_absolute_path() {
    let mut app = DrwrApp::default();
    // Cargo runs tests from the package root.
    let relative = Path::new("crates/graphml/tests/fixtures/yed.graphml");
    app.open(relative).unwrap();
    let cwd = std::env::current_dir().unwrap();
    assert_eq!(app.file, Some(cwd.join(relative)));
}

#[test]
fn failed_open_keeps_the_graph() {
    let mut app = DrwrApp::default();
    app.open(Path::new(FIXTURE)).unwrap();
    let before = app.doc.graphml().clone();

    app.selected = Selection::node("n0".to_owned());

    let not_graphml = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/box.svg");
    let err = app.open(Path::new(not_graphml)).unwrap_err();
    assert!(matches!(err, graphml::Error::Parse { .. }), "{err:?}");

    let missing = Path::new("/nonexistent/drwr.graphml");
    let err = app.open(missing).unwrap_err();
    assert!(matches!(err, graphml::Error::Io(_)), "{err:?}");

    assert_eq!(*app.doc.graphml(), before);
    assert_eq!(app.file.as_deref(), Some(Path::new(FIXTURE)));
    assert_eq!(app.selected_node(), Some("n0"));
}

#[test]
fn open_clears_the_selection() {
    let mut app = DrwrApp {
        selected: Selection::node("n0"),
        ..DrwrApp::default()
    };
    app.open(Path::new(FIXTURE)).unwrap();
    assert!(app.selected.is_empty());
}

#[test]
fn ctrl_n_replaces_the_graph_with_an_empty_unsaved_one() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::node("n0");
    harness.app.tool = Tool::Box;
    harness.app.canvas.set_zoom(2.0);
    let fonts = harness.app.fonts.clone();
    harness.press(CTRL, Key::N);
    assert_eq!(*harness.app.doc.graphml(), yed::new_document());
    assert_eq!(harness.app.file, None);
    assert!(harness.app.selected.is_empty());
    assert_eq!(harness.app.canvas.zoom(), 1.0);
    assert_eq!(harness.app.tool, Tool::Box);
    assert!(Arc::ptr_eq(&harness.app.fonts, &fonts));
}

#[test]
fn new_closes_the_dialogs_and_drops_the_labels_being_edited() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::node("n0");
    harness.click_text("Order\nservice");
    harness.type_text("Stop");
    harness
        .app
        .color_picker
        .open(ColorProperty::Fill, "Fill", None, true);
    let edit = LabelEdit {
        labels: vec![],
        text: String::new(),
    };
    harness.app.label_dialog.open(edit, "Label", "");
    harness.perform(Action::New);
    assert!(!harness.app.color_picker.is_open());
    assert!(!harness.app.label_dialog.is_open());
    harness.run(vec![]);
    harness.run(vec![]);
    assert_eq!(*harness.app.doc.graphml(), yed::new_document());
}

#[test]
fn open_closes_the_color_picker() {
    let mut app = DrwrApp::default();
    app.color_picker
        .open(ColorProperty::Fill, "Fill color", None, true);
    app.open(Path::new(FIXTURE)).unwrap();
    assert!(!app.color_picker.is_open());
}

#[test]
fn open_drops_the_labels_being_edited() {
    let mut harness = Harness::new();
    // The fixture has a node with the same id, n0.
    type_in_label_field(&mut harness, "Start");
    harness.type_text("Stop");
    harness.app.open(Path::new(FIXTURE)).unwrap();
    harness.run(vec![]);
    harness.run(vec![]);
    assert_eq!(harness.label("n0"), "Order\nservice");

    harness.app.selected = Selection::node("n0".to_owned());
    harness.press(CTRL, Key::Enter);
    harness.run(vec![]);
    harness.type_text("Stop");
    harness.app.open(Path::new(FIXTURE)).unwrap();
    harness.run(vec![]);
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(harness.label("n0"), "Order\nservice");
}

#[test]
fn open_closes_the_label_dialog() {
    let mut app = DrwrApp::default();
    let label = LabelRef {
        item: Item::Node("n0".to_owned()),
        index: 0,
    };
    let edit = LabelEdit {
        labels: vec![label],
        text: "Start".to_owned(),
    };
    app.label_dialog.open(edit, "Label", "Start");
    app.open(Path::new(FIXTURE)).unwrap();
    assert!(!app.label_dialog.is_open());
}

#[test]
fn open_resets_the_zoom() {
    let mut app = DrwrApp::default();
    app.canvas.set_zoom(2.0);
    app.open(Path::new(FIXTURE)).unwrap();
    assert_eq!(app.canvas.zoom(), 1.0);
}

impl ScratchDir {
    /// Copies the yEd fixture in, returning the copy's path.
    fn fixture(&self) -> PathBuf {
        let copy = self.path("yed.graphml");
        std::fs::copy(FIXTURE, &copy).unwrap();
        copy
    }
}

/// The GraphML file at `path`, read.
fn read(path: &Path) -> GraphMl {
    GraphMl::read_file(path).unwrap()
}

#[test]
fn the_file_menu_has_save_as_after_save() {
    let mut harness = Harness::new();
    harness.click_widget("File");
    let actions = [
        Action::New,
        Action::Open,
        Action::Save,
        Action::SaveAs,
        Action::Quit,
    ];
    let names = ["New", "Open…", "Save", "Save As…", "Quit"];
    let items: Vec<_> = (actions.iter().zip(names))
        .map(|(action, name)| {
            let shortcut = shortcuts::text(&harness.ctx, action).unwrap();
            testing::center(&harness.widget(&format!("{name} {shortcut}"))).y
        })
        .collect();
    assert!(items.is_sorted(), "{items:?}");
    assert_eq!(
        shortcuts::text(&harness.ctx, &Action::SaveAs).as_deref(),
        Some("Ctrl+Shift+S")
    );
}

#[test]
fn ctrl_s_saves_to_the_file() {
    let dir = ScratchDir::new("ctrl-s");
    let file = dir.fixture();
    let mut harness = Harness::open(&file);
    harness.app.selected = Selection::node("n0");
    harness.press(CTRL, Key::ArrowRight);
    harness.press(CTRL, Key::S);
    assert!(!harness.app.file_dialog.is_open());
    assert_eq!(harness.app.file.as_deref(), Some(file.as_path()));
    assert_eq!(read(&file), *harness.app.doc.graphml());
    assert_ne!(read(&file), read(Path::new(FIXTURE)));
}

#[test]
fn saving_an_unsaved_graph_asks_where_and_adds_the_extension() {
    let dir = ScratchDir::new("save-unsaved");
    let mut harness = Harness::new();
    harness.draw_box();
    harness.press(CTRL, Key::S);
    let request = Request::Save {
        name: "untitled.graphml".to_owned(),
        folder: None,
    };
    assert_eq!(harness.app.file_dialog.request(), Some(&request));
    assert_eq!(harness.app.file, None);

    harness.answer(Some(&dir.path("drawn")));
    let file = dir.path("drawn.graphml");
    assert_eq!(harness.app.file.as_deref(), Some(file.as_path()));
    assert_eq!(read(&file), *harness.app.doc.graphml());
    assert_eq!(harness.app.doc.graphml().graphs[0].nodes.len(), 1);
}

#[test]
fn a_box_drawn_and_saved_is_in_a_yed_document() {
    let dir = ScratchDir::new("save-new-document");
    let mut harness = Harness::new();
    harness.draw_box();
    harness.press(CTRL, Key::S);
    let file = dir.path("drawn.graphml");
    harness.answer(Some(&file));

    let saved = read(&file);
    let xmlns = saved.attributes.get("xmlns").map(String::as_str);
    assert_eq!(xmlns, Some("http://graphml.graphdrawing.org/xmlns"));
    let new = yed::new_document();
    assert_eq!(saved.attributes, new.attributes);
    assert_eq!(saved.keys, new.keys);
    assert_eq!(saved.graphs[0].nodes.len(), 1);
}

#[test]
fn only_names_without_an_extension_get_graphml() {
    let dir = ScratchDir::new("save-extensions");
    std::fs::create_dir(dir.path("v1.2")).unwrap();
    for (picked, saved) in [
        ("plan.xml", "plan.xml"),
        ("plan.graphml", "plan.graphml"),
        ("v1.2/plan", "v1.2/plan.graphml"),
    ] {
        let mut harness = Harness::new();
        harness.draw_box();
        harness.press(CTRL, Key::S);
        harness.answer(Some(&dir.path(picked)));
        assert_eq!(harness.app.file, Some(dir.path(saved)), "{picked}");
    }
}

#[test]
fn a_name_without_an_extension_asks_before_replacing_that_file() {
    let dir = ScratchDir::new("save-replace");
    let file = dir.path("plan.graphml");
    std::fs::copy(FIXTURE, &file).unwrap();
    let question = "plan.graphml already exists. Replace it?";
    let mut harness = Harness::new();
    harness.draw_box();

    harness.press(CTRL, Key::S);
    harness.answer(Some(&dir.path("plan")));
    assert!(harness.painted(question));
    harness.answer_prompt("Cancel");
    assert!(!harness.painted(question));
    assert_eq!(read(&file), read(Path::new(FIXTURE)));
    assert_eq!(harness.app.file, None);
    assert!(harness.app.modified);

    harness.press(CTRL, Key::S);
    harness.answer(Some(&dir.path("plan")));
    harness.answer_prompt("Replace");
    assert_eq!(read(&file), *harness.app.doc.graphml());
    assert_eq!(harness.app.file, Some(file));
    assert!(!harness.app.modified);
}

#[test]
fn replacing_goes_on_to_close_the_graph_and_cancelling_drops_that() {
    let dir = ScratchDir::new("save-replace-new");
    let file = dir.path("plan.graphml");
    std::fs::copy(FIXTURE, &file).unwrap();
    let mut harness = Harness::new();
    harness.draw_box();
    let drawn = harness.app.doc.graphml().clone();
    let new = yed::new_document();
    for (answer, graphml) in [("Cancel", &drawn), ("Replace", &new)] {
        harness.press(CTRL, Key::N);
        harness.answer_prompt("Save");
        harness.answer(Some(&dir.path("plan")));
        harness.answer_prompt(answer);
        assert_eq!(harness.app.doc.graphml(), graphml, "{answer}");
        assert!(!harness.asks_to_save(), "{answer}");
    }
    assert_eq!(read(&file), drawn);
}

#[test]
fn prompts_are_opaque_on_their_first_visible_frame() {
    let mut harness = Harness::new();
    let message = "Couldn't save";
    harness.app.error = Some(message.to_owned());
    let painted = |shape: &Shape| match shape {
        Shape::Text(text) if text.galley.text() == message => Some(text.fallback_color),
        _ => None,
    };
    // A dialog shown for the first time is only sized in its first frame.
    let shapes = (0..3)
        .map(|_| harness.run(vec![]))
        .find(|shapes| shapes.iter().any(|shape| painted(shape).is_some()))
        .expect("the error was shown");
    let color = shapes.iter().find_map(painted).unwrap();
    assert_eq!(color.a(), 255);
    let backdrop = Color32::from_black_alpha(100);
    let dimmed = |shape: &Shape| matches!(shape, Shape::Rect(rect) if rect.fill == backdrop);
    assert!(shapes.iter().any(dimmed));
}

#[test]
fn save_as_writes_the_fixture_byte_for_byte_and_switches_to_the_copy() {
    let dir = ScratchDir::new("save-as");
    let mut harness = Harness::open(Path::new(FIXTURE));
    harness.press(CTRL.plus(Modifiers::SHIFT), Key::S);
    let request = Request::Save {
        name: "yed.graphml".to_owned(),
        folder: Path::new(FIXTURE).parent().map(Path::to_path_buf),
    };
    assert_eq!(harness.app.file_dialog.request(), Some(&request));

    let copy = dir.path("copy.graphml");
    harness.answer(Some(&copy));
    assert_eq!(harness.app.file.as_deref(), Some(copy.as_path()));
    assert_eq!(
        std::fs::read(&copy).unwrap(),
        std::fs::read(FIXTURE).unwrap()
    );
}

#[test]
fn cancelling_save_as_saves_nothing() {
    let dir = ScratchDir::new("save-as-cancel");
    let file = dir.fixture();
    let mut harness = Harness::open(&file);
    harness.app.selected = Selection::node("n0");
    harness.press(CTRL, Key::ArrowRight);
    harness.press(CTRL.plus(Modifiers::SHIFT), Key::S);
    harness.answer(None);
    assert!(!harness.app.file_dialog.is_open());
    assert_eq!(harness.app.file.as_deref(), Some(file.as_path()));
    assert_eq!(read(&file), read(Path::new(FIXTURE)));
}

#[test]
fn a_failed_save_keeps_the_file_and_shows_why() {
    let mut harness = Harness::open(Path::new(FIXTURE));
    harness.press(CTRL.plus(Modifiers::SHIFT), Key::S);
    harness.answer(Some(Path::new("/nonexistent/drwr.graphml")));
    assert_eq!(harness.app.file.as_deref(), Some(Path::new(FIXTURE)));
    let error = harness.app.error.clone().unwrap_or_default();
    assert!(
        error.starts_with("Couldn't save /nonexistent/drwr.graphml:\n"),
        "{error:?}"
    );

    harness.press(Modifiers::NONE, Key::Escape);
    // The dialog keeps the keys in the frame Escape closes it in.
    harness.run(vec![]);
    harness.app.file = Some(PathBuf::from("/nonexistent/yed.graphml"));
    harness.press(CTRL, Key::S);
    assert!(harness.app.error.is_some());
    assert_eq!(
        harness.app.file.as_deref(),
        Some(Path::new("/nonexistent/yed.graphml"))
    );
}

#[test]
fn file_actions_wait_while_a_file_dialog_is_open() {
    let mut harness = fixture_harness();
    harness.press(CTRL.plus(Modifiers::SHIFT), Key::S);
    for label in ["New", "Open", "Save"] {
        assert!(harness.widget(label).is_disabled(), "{label}");
    }
    let before = harness.app.doc.graphml().clone();
    for key in [Key::N, Key::O] {
        harness.press(CTRL, key);
    }
    assert_eq!(*harness.app.doc.graphml(), before);
    let request = harness.app.file_dialog.request();
    assert!(matches!(request, Some(Request::Save { .. })), "{request:?}");

    harness.answer(None);
    assert!(!harness.widget("Open").is_disabled());
}

#[test]
fn the_save_button_saves_the_label_typed_first() {
    let dir = ScratchDir::new("save-button");
    let file = dir.path("typed.graphml");
    let mut harness = Harness::new();
    let id = type_in_label_field(&mut harness, "Start");
    harness.app.file = Some(file.clone());
    harness.type_text("Typed");
    harness.click_widget("Save");
    let label = LabelRef {
        item: node(&id),
        index: 0,
    };
    let saved = read(&file);
    assert_eq!(
        document::label_text(&saved, &label).as_deref(),
        Some("Typed")
    );
}

#[test]
fn the_save_button_saves_the_label_edited_on_the_canvas() {
    let dir = ScratchDir::new("save-canvas-label");
    let file = dir.fixture();
    let mut harness = Harness::open(&file);
    harness.app.selected = Selection::node("n2");
    harness.press(CTRL, Key::Enter);
    harness.run(vec![]);
    harness.type_text("Renamed");
    harness.click_widget("Save");
    let label = LabelRef {
        item: node("n2"),
        index: 0,
    };
    let saved = read(&file);
    assert_eq!(
        document::label_text(&saved, &label).as_deref(),
        Some("Renamed")
    );
}

#[test]
fn ctrl_s_in_a_label_field_saves_the_label_typed() {
    let dir = ScratchDir::new("ctrl-s-label-field");
    let file = dir.fixture();
    let mut harness = Harness::open(&file);
    retype_queue(&mut harness);
    harness.press(CTRL, Key::S);
    assert_eq!(harness.label("n2"), "Stack");
    assert_eq!(read(&file), *harness.app.doc.graphml());
    assert!(!harness.app.modified);
    assert!(!harness.ctx.text_edit_focused());
}

#[test]
fn ctrl_s_saves_the_label_edited_on_the_canvas() {
    let dir = ScratchDir::new("ctrl-s-canvas-label");
    let file = dir.fixture();
    let mut harness = Harness::open(&file);
    harness.app.selected = Selection::node("n2");
    harness.press(CTRL, Key::Enter);
    harness.run(vec![]);
    harness.type_text("Renamed");
    harness.press(CTRL, Key::S);
    assert_eq!(harness.label("n2"), "Renamed");
    assert_eq!(read(&file), *harness.app.doc.graphml());
}

#[test]
fn ctrl_s_in_the_border_width_field_saves_the_width_typed() {
    let dir = ScratchDir::new("ctrl-s-border-width");
    let file = dir.path("box.graphml");
    let mut harness = Harness::new();
    let id = click_border_width(&mut harness);
    harness.app.file = Some(file.clone());
    harness.type_text("4.5");
    harness.press(CTRL, Key::S);
    assert_eq!(harness.graphics(&id).border.width, 4.5);
    assert_eq!(read(&file), *harness.app.doc.graphml());
}

/// Selects the fixture's n2 and types "Stack" over its label, "Queue", in
/// the side panel, without applying it.
fn retype_queue(harness: &mut Harness) {
    harness.app.selected = Selection::node("n2");
    harness.click_text("Queue");
    harness.press(CTRL, Key::A);
    harness.type_text("Stack");
}

/// A copy of the yEd fixture in `dir`, opened, with n0 moved 1 pt right.
fn modified_fixture(dir: &ScratchDir) -> (Harness, PathBuf) {
    let file = dir.fixture();
    let mut harness = Harness::open(&file);
    harness.app.selected = Selection::node("n0");
    harness.press(CTRL, Key::ArrowRight);
    assert!(harness.app.modified);
    (harness, file)
}

#[test]
fn a_marker_shows_unsaved_changes_until_saved() {
    let dir = ScratchDir::new("marker");
    let mut harness = Harness::new();
    assert!(!harness.painted("*"));
    assert_eq!(harness.app.title, "<Not Saved> — drwr");

    harness.draw_box();
    assert!(harness.app.modified);
    assert!(harness.painted("<Not Saved>"));
    assert!(harness.painted("*"));
    assert_eq!(harness.app.title, "<Not Saved>* — drwr");

    harness.press(CTRL, Key::S);
    harness.answer(Some(&dir.path("drawn.graphml")));
    assert!(!harness.app.modified);
    assert!(!harness.painted("*"));
    assert_eq!(harness.app.title, "drawn.graphml — drwr");
}

#[test]
fn the_title_is_set_only_when_it_changes() {
    let mut harness = fixture_harness();
    let title = egui::ViewportCommand::Title("yed.graphml — drwr".to_owned());
    harness.app.title.clear();
    assert!(harness.commands(RawInput::default()).contains(&title));
    let commands = harness.commands(RawInput::default());
    let titles = (commands.iter()).filter(|c| matches!(c, egui::ViewportCommand::Title(_)));
    assert_eq!(titles.count(), 0, "{commands:?}");

    harness.app.selected = Selection::node("n0");
    let modified = egui::ViewportCommand::Title("yed.graphml* — drwr".to_owned());
    let commands = harness.commands(RawInput {
        events: vec![testing::key(CTRL, Key::ArrowRight)],
        ..RawInput::default()
    });
    assert!(commands.contains(&modified), "{commands:?}");
}

#[test]
fn moving_a_node_on_the_canvas_marks_the_graph_modified() {
    let mut harness = fixture_harness();
    assert!(!harness.app.modified);
    // n0 spans (100, 40.5) to (220, 100.5).
    harness.drag(vec2(110.0, 90.0), vec2(150.0, 130.0));
    assert_eq!(harness.graphics("n0").geometry.x, 140.0);
    assert!(harness.app.modified);
    assert!(harness.painted("*"));
}

#[test]
fn side_panel_edits_mark_the_graph_modified() {
    let mut harness = fixture_harness();
    retype_queue(&mut harness);
    assert!(!harness.app.modified);
    harness.press(Modifiers::NONE, Key::Enter);
    assert!(harness.app.modified);
}

#[test]
fn undoing_an_edit_by_hand_clears_the_marker() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::node("n0");
    harness.press(CTRL, Key::ArrowRight);
    assert!(harness.app.modified);
    harness.press(CTRL, Key::ArrowLeft);
    assert!(!harness.app.modified);
}

#[test]
fn a_truncated_path_keeps_the_marker() {
    let mut harness = Harness::new();
    harness.draw_box();
    let long = format!("/{}/drawn.graphml", "folder/".repeat(40));
    harness.app.file = Some(PathBuf::from(long));
    let narrow = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(500.0, 400.0))),
        ..RawInput::default()
    };
    for _ in 0..2 {
        let mut output = harness.run_input(narrow.clone());
        let shapes = std::mem::take(&mut output.shapes);
        output.drop_without_applying_deltas();
        let marker = shapes.iter().find_map(|clipped| match &clipped.shape {
            Shape::Text(text) if text.galley.text() == "*" => Some(text.pos.x),
            _ => None,
        });
        assert!(marker.is_some_and(|x| x < 500.0), "{marker:?}");
    }
}

#[test]
fn quit_closes_a_graph_without_changes() {
    let mut harness = fixture_harness();
    assert!(harness.quit());
    assert!(!harness.asks_to_save());
}

#[test]
fn quit_asks_to_save_changes_first() {
    let dir = ScratchDir::new("quit");
    let (mut harness, file) = modified_fixture(&dir);
    assert!(!harness.quit());
    assert!(harness.painted("Save changes to yed.graphml?"));

    let commands = harness.answer_prompt("Cancel");
    assert!(!commands.contains(&egui::ViewportCommand::Close));
    assert!(!harness.asks_to_save());
    assert!(harness.app.modified);

    harness.quit();
    let commands = harness.answer_prompt("Save");
    assert!(commands.contains(&egui::ViewportCommand::Close));
    assert_eq!(read(&file), *harness.app.doc.graphml());
    assert!(!harness.close_window());
}

#[test]
fn quit_without_saving_leaves_the_file_and_lets_the_window_close() {
    let dir = ScratchDir::new("quit-dont-save");
    let (mut harness, file) = modified_fixture(&dir);
    harness.quit();
    let commands = harness.answer_prompt("Don't Save");
    assert!(commands.contains(&egui::ViewportCommand::Close));
    assert!(harness.app.modified);
    assert!(!harness.close_window());
    assert_eq!(read(&file), read(Path::new(FIXTURE)));
}

#[test]
fn ctrl_q_closes_a_graph_without_changes() {
    let mut harness = fixture_harness();
    assert!(harness.press_ctrl_q());
    assert!(!harness.asks_to_save());
}

#[test]
fn ctrl_q_asks_to_save_changes_first() {
    let dir = ScratchDir::new("ctrl-q");
    let (mut harness, file) = modified_fixture(&dir);
    assert!(!harness.press_ctrl_q());
    assert!(harness.painted("Save changes to yed.graphml?"));

    let commands = harness.answer_prompt("Save");
    assert!(commands.contains(&egui::ViewportCommand::Close));
    assert_eq!(read(&file), *harness.app.doc.graphml());
    assert!(!harness.close_window());
}

#[test]
fn ctrl_q_in_a_label_field_applies_the_label_typed_then_asks() {
    let mut harness = fixture_harness();
    retype_queue(&mut harness);
    assert!(!harness.app.modified);
    assert!(!harness.press_ctrl_q());
    assert_eq!(harness.label("n2"), "Stack");
    assert!(harness.asks_to_save());
}

#[test]
fn closing_the_window_asks_to_save_changes_first() {
    let mut harness = fixture_harness();
    assert!(!harness.close_window());

    harness.app.selected = Selection::node("n0");
    harness.press(CTRL, Key::ArrowRight);
    assert!(harness.close_window());
    assert!(harness.asks_to_save());
    // Escape cancels.
    harness.press(Modifiers::NONE, Key::Escape);
    assert!(!harness.asks_to_save());
    assert!(harness.close_window());
}

#[test]
fn closing_the_window_applies_the_label_being_typed_then_asks() {
    let mut harness = fixture_harness();
    retype_queue(&mut harness);
    assert!(!harness.app.modified);
    assert!(harness.close_window());
    assert_eq!(harness.label("n2"), "Stack");
    assert!(harness.asks_to_save());
}

#[test]
fn open_asks_to_save_changes_before_showing_the_dialog() {
    let dir = ScratchDir::new("open");
    let (mut harness, _) = modified_fixture(&dir);
    harness.press(CTRL, Key::O);
    assert!(!harness.app.file_dialog.is_open());
    assert!(harness.asks_to_save());
    harness.answer_prompt("Cancel");
    assert!(!harness.app.file_dialog.is_open());

    harness.press(CTRL, Key::O);
    harness.answer_prompt("Don't Save");
    assert_eq!(harness.app.file_dialog.request(), Some(&Request::Open));
    // Cancelling the dialog keeps the changes.
    harness.answer(None);
    assert!(harness.app.modified);
}

#[test]
fn saving_from_the_prompt_goes_on_to_open() {
    let dir = ScratchDir::new("open-save");
    let (mut harness, file) = modified_fixture(&dir);
    let moved = harness.app.doc.graphml().clone();
    harness.press(CTRL, Key::O);
    harness.answer_prompt("Save");
    assert_eq!(read(&file), moved);
    assert_eq!(harness.app.file_dialog.request(), Some(&Request::Open));

    harness.answer(Some(Path::new(FIXTURE)));
    assert_eq!(*harness.app.doc.graphml(), read(Path::new(FIXTURE)));
    assert!(!harness.app.modified);
}

#[test]
fn new_asks_to_save_an_unsaved_graph_where_first() {
    let dir = ScratchDir::new("new-save-as");
    let mut harness = Harness::new();
    harness.draw_box();
    harness.press(CTRL, Key::N);
    assert!(harness.painted("Save changes to the new diagram?"));
    harness.answer_prompt("Save");
    let request = harness.app.file_dialog.request().cloned();
    assert!(matches!(request, Some(Request::Save { .. })), "{request:?}");

    let file = dir.path("drawn.graphml");
    let drawn = harness.app.doc.graphml().clone();
    harness.answer(Some(&file));
    assert_eq!(read(&file), drawn);
    assert_eq!(*harness.app.doc.graphml(), yed::new_document());
    assert_eq!(harness.app.file, None);
    assert!(!harness.app.modified);
}

#[test]
fn cancelling_save_as_from_the_prompt_keeps_the_graph() {
    let mut harness = Harness::new();
    harness.draw_box();
    let drawn = harness.app.doc.graphml().clone();
    harness.press(CTRL, Key::N);
    harness.answer_prompt("Save");
    harness.answer(None);
    assert_eq!(*harness.app.doc.graphml(), drawn);
    assert!(harness.app.modified);
    assert!(!harness.asks_to_save());
}

#[test]
fn a_failed_save_from_the_prompt_keeps_the_graph() {
    let mut harness = fixture_harness();
    harness.app.file = Some(PathBuf::from("/nonexistent/yed.graphml"));
    harness.app.selected = Selection::node("n0");
    harness.press(CTRL, Key::ArrowRight);
    let moved = harness.app.doc.graphml().clone();
    harness.press(CTRL, Key::N);
    harness.answer_prompt("Save");
    assert!(harness.app.error.is_some());
    assert_eq!(*harness.app.doc.graphml(), moved);
    assert!(harness.app.modified);
}

#[test]
fn new_without_saving_drops_the_changes() {
    let mut harness = Harness::new();
    harness.draw_box();
    harness.press(CTRL, Key::N);
    harness.answer_prompt("Don't Save");
    assert_eq!(*harness.app.doc.graphml(), yed::new_document());
    assert!(!harness.app.modified);
}

#[test]
fn enter_saves_from_the_prompt() {
    let dir = ScratchDir::new("prompt-enter");
    let (mut harness, file) = modified_fixture(&dir);
    harness.press(CTRL, Key::N);
    harness.run(vec![]);
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(*harness.app.doc.graphml(), yed::new_document());
    assert_ne!(read(&file), read(Path::new(FIXTURE)));
}

#[test]
fn the_prompt_keeps_the_keys() {
    let mut harness = Harness::new();
    harness.draw_box();
    harness.press(CTRL, Key::N);
    harness.press(Modifiers::NONE, Key::B);
    harness.press(CTRL, Key::S);
    assert_eq!(harness.app.tool, Tool::Cursor);
    assert!(!harness.app.file_dialog.is_open());
    assert!(harness.asks_to_save());
}

#[test]
fn the_new_button_applies_the_label_typed_first_then_asks() {
    let mut harness = fixture_harness();
    retype_queue(&mut harness);
    harness.click_widget("New");
    assert_eq!(harness.label("n2"), "Stack");
    assert!(harness.asks_to_save());
}

#[test]
fn the_save_button_leaves_nothing_unsaved() {
    let dir = ScratchDir::new("save-button-clean");
    let file = dir.fixture();
    let mut harness = Harness::open(&file);
    retype_queue(&mut harness);
    harness.click_widget("Save");
    assert_eq!(harness.label("n2"), "Stack");
    assert!(!harness.app.modified);
    assert!(!harness.painted("*"));
}

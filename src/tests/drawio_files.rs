use super::*;
use crate::export::Format;
use crate::testing::ScratchDir;

/// A page of a draw.io file named `name`, with `cells` on its layer.
fn page(name: &str, cells: &str) -> String {
    format!(
        r#"<diagram name="{name}"><mxGraphModel><root><mxCell id="0"/><mxCell id="1" parent="0"/>{cells}</root></mxGraphModel></diagram>"#
    )
}

/// A 120×60 shape labelled `label` at (`x`, 10), styled by `style`.
fn shape(id: &str, label: &str, style: &str, x: u32) -> String {
    format!(
        r#"<mxCell id="{id}" value="{label}" style="{style}" vertex="1" parent="1"><mxGeometry x="{x}" y="10" width="120" height="60" as="geometry"/></mxCell>"#
    )
}

/// A page with a box labelled `label`.
fn box_page(name: &str, label: &str) -> String {
    page(name, &shape("2", label, "", 10))
}

/// A line between two points on no shape, which converting gives small nodes
/// to end on, with a note saying so.
const LOOSE_LINE: &str = r#"<mxCell id="9" edge="1" parent="1"><mxGeometry relative="1" as="geometry"><mxPoint x="20" y="200" as="sourcePoint"/><mxPoint x="220" y="200" as="targetPoint"/></mxGeometry></mxCell>"#;

const LOOSE_LINE_NOTE: &str = "2 loose line ends got small invisible nodes to end on.";

impl ScratchDir {
    /// Writes a draw.io file of `pages` as `name`, returning its path.
    fn drawio(&self, name: &str, pages: &[String]) -> PathBuf {
        let path = self.path(name);
        std::fs::write(&path, format!("<mxfile>{}</mxfile>", pages.concat())).unwrap();
        path
    }
}

impl Harness {
    /// Opens the file at `path` from File > Open.
    fn open_file(&mut self, path: &Path) {
        self.press(CTRL, Key::O);
        self.answer(Some(path));
    }

    /// The ids of the top-level nodes.
    fn node_ids(&self) -> Vec<String> {
        let nodes = &self.app.doc.graphml().graphs[0].nodes;
        nodes.iter().map(|node| node.id.clone()).collect()
    }

    /// The labels of the top-level nodes.
    fn labels(&self) -> Vec<String> {
        self.node_ids().iter().map(|id| self.label(id)).collect()
    }
}

#[test]
fn a_drawio_file_of_one_page_opens_converted_to_graphml() {
    let dir = ScratchDir::new("drawio-one-page");
    let file = dir.drawio("plan.drawio", &[box_page("A", "Box")]);
    let mut harness = Harness::new();
    harness.open_file(&file);
    assert!(!harness.app.page_dialog.is_open());
    assert_eq!(harness.labels(), ["Box"]);
    let id = &harness.node_ids()[0];
    let bounds = yed::Rect {
        x: 10.0,
        y: 10.0,
        width: 120.0,
        height: 60.0,
    };
    assert_eq!(harness.graphics(id).geometry, bounds);
    harness.canvas_text("Box");
    assert_eq!(harness.app.file, None);
    assert_eq!(harness.app.imported.as_deref(), Some(file.as_path()));
}

#[test]
fn an_imported_graph_is_titled_as_such_and_unmodified() {
    let dir = ScratchDir::new("drawio-title");
    let file = dir.drawio("plan.drawio", &[box_page("A", "Box")]);
    let mut harness = Harness::open(&file);
    assert!(!harness.app.modified);
    assert_eq!(harness.app.title, "plan.drawio (imported) — drwr");
    assert!(harness.painted(&format!("{} (imported)", file.display())));
    assert!(!harness.painted("*"));

    harness.app.selected = Selection::node(harness.node_ids()[0].clone());
    harness.press(CTRL, Key::ArrowRight);
    assert_eq!(harness.app.title, "plan.drawio (imported)* — drwr");
    assert!(harness.painted("*"));
}

#[test]
fn closing_an_unedited_import_asks_nothing() {
    let dir = ScratchDir::new("drawio-close");
    let file = dir.drawio("plan.drawio", &[box_page("A", "Box")]);
    let mut harness = Harness::open(&file);
    assert!(!harness.close_window());
    assert!(harness.press_ctrl_q());
    assert!(!harness.asks_to_save());
}

#[test]
fn saving_asks_where_suggesting_a_graphml_file_beside_the_drawio_file() {
    let dir = ScratchDir::new("drawio-save");
    let file = dir.drawio("plan.drawio", &[box_page("A", "Box")]);
    let drawio = std::fs::read(&file).unwrap();
    let mut harness = Harness::open(&file);
    let request = Request::Save {
        name: "plan.graphml".to_owned(),
        folder: file.parent().map(Path::to_path_buf),
    };
    for keys in [CTRL, CTRL.plus(Modifiers::SHIFT)] {
        harness.press(keys, Key::S);
        assert_eq!(harness.app.file_dialog.request(), Some(&request));
        harness.answer(None);
    }
    assert_eq!(harness.app.imported.as_deref(), Some(file.as_path()));

    harness.press(CTRL, Key::S);
    let saved = dir.path("plan.graphml");
    harness.answer(Some(&saved));
    assert_eq!(harness.app.file.as_deref(), Some(saved.as_path()));
    assert_eq!(harness.app.imported, None);
    assert_eq!(harness.app.title, "plan.graphml — drwr");
    assert!(harness.painted(&saved.display().to_string()));
    assert_eq!(
        GraphMl::read_file(&saved).unwrap(),
        *harness.app.doc.graphml()
    );
    assert_eq!(std::fs::read(&file).unwrap(), drawio);
}

#[test]
fn exporting_an_import_suggests_the_drawio_files_name_and_folder() {
    let dir = ScratchDir::new("drawio-export");
    let file = dir.drawio("plan.drawio", &[box_page("A", "Box")]);
    let mut harness = Harness::open(&file);
    harness.press(CTRL, Key::E);
    harness.click_widget("Export…");
    let request = Request::Export {
        format: Format::Png,
        name: "plan.png".to_owned(),
        folder: file.parent().map(Path::to_path_buf),
    };
    assert_eq!(harness.app.file_dialog.request(), Some(&request));
}

#[test]
fn quitting_an_edited_import_asks_to_save_it_as_graphml() {
    let dir = ScratchDir::new("drawio-quit");
    let file = dir.drawio("plan.drawio", &[box_page("A", "Box")]);
    let mut harness = Harness::open(&file);
    harness.app.selected = Selection::node(harness.node_ids()[0].clone());
    harness.press(CTRL, Key::ArrowRight);
    assert!(!harness.quit());
    assert!(harness.painted("Save changes to the diagram imported from plan.drawio?"));
    harness.answer_prompt("Save");
    let request = harness.app.file_dialog.request().cloned();
    assert!(
        matches!(&request, Some(Request::Save { name, .. }) if name == "plan.graphml"),
        "{request:?}"
    );
    let saved = dir.path("plan.graphml");
    harness.answer(Some(&saved));
    assert_eq!(
        GraphMl::read_file(&saved).unwrap(),
        *harness.app.doc.graphml()
    );
    assert!(!harness.close_window());
}

#[test]
fn new_or_opening_another_file_drops_the_import() {
    let dir = ScratchDir::new("drawio-replaced");
    let file = dir.drawio("plan.drawio", &[box_page("A", "Box")]);
    let mut harness = Harness::open(&file);
    harness.press(CTRL, Key::N);
    assert_eq!(harness.app.imported, None);
    assert_eq!(harness.app.title, "<Not Saved> — drwr");

    harness.open_file(&file);
    assert!(harness.app.imported.is_some());
    harness.open_fixture();
    harness.run(vec![]);
    assert_eq!(harness.app.imported, None);
    assert_eq!(harness.app.title, "yed.graphml — drwr");
}

#[test]
fn a_drawio_file_of_several_pages_asks_which_to_open() {
    let dir = ScratchDir::new("drawio-pages");
    let pages = [
        box_page("First", "One"),
        box_page("Second", "Two"),
        box_page("Third", "Three"),
    ];
    let file = dir.drawio("plan.drawio", &pages);
    let mut harness = fixture_harness();
    let fixture = harness.app.file.clone();
    harness.open_file(&file);
    assert!(harness.painted("plan.drawio has 3 pages. Pick the one to open:"));
    for name in ["First", "Second", "Third"] {
        assert!(harness.painted(name), "{name}");
    }
    // Opened only once picked.
    assert_eq!(harness.app.file, fixture);

    let view = harness.app.canvas.view_center();
    harness.press(Modifiers::NONE, Key::ArrowDown);
    // Not panned by the arrow either.
    assert_eq!(harness.app.canvas.view_center(), view);
    harness.press(Modifiers::NONE, Key::Enter);
    assert!(!harness.app.page_dialog.is_open());
    assert_eq!(harness.labels(), ["Two"]);
    assert_eq!(harness.app.imported.as_deref(), Some(file.as_path()));
    assert_eq!(harness.app.file, None);
    // The dialog keeps the keys in the frame it closes in.
    harness.run(vec![]);

    harness.open_file(&file);
    harness.click_widget("Third");
    harness.answer_prompt("Open");
    assert_eq!(harness.labels(), ["Three"]);
}

#[test]
fn the_keyboard_alone_opens_a_page_from_recent_files() {
    let dir = ScratchDir::new("drawio-pages-keys");
    let pages = [box_page("First", "One"), box_page("Second", "Two")];
    let file = dir.drawio("plan.drawio", &pages);
    let mut harness = Harness::new();
    harness.app.config.add_recent_file(&file);
    harness.press(Modifiers::ALT, Key::F);
    // New, Open…, Recent Files, into it, then its first file. Each moves the
    // focus the frame after.
    let keys = [
        Key::ArrowDown,
        Key::ArrowDown,
        Key::ArrowDown,
        Key::ArrowRight,
    ];
    for key in keys.into_iter().chain([Key::Enter]) {
        harness.run(vec![]);
        harness.press(Modifiers::NONE, key);
    }
    assert!(harness.app.page_dialog.is_open());
    // As egui repaints right after input.
    harness.run(vec![]);
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(harness.labels(), ["One"]);
}

#[test]
fn enter_opens_a_page_right_after_dont_save_was_picked_with_the_keyboard() {
    let dir = ScratchDir::new("drawio-pages-dont-save");
    let pages = [box_page("First", "One"), box_page("Second", "Two")];
    let file = dir.drawio("plan.drawio", &pages);
    let mut harness = fixture_harness();
    harness.app.selected = Selection::node("n0");
    harness.press(CTRL, Key::ArrowRight);
    harness.perform(Action::OpenRecent(file));
    // Save takes the focus the frame after.
    harness.run(vec![]);
    harness.press(Modifiers::NONE, Key::Tab);
    harness.press(Modifiers::NONE, Key::Enter);
    harness.run(vec![]);
    // Don't Save still had the focus as the page dialog came up.
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(harness.labels(), ["One"]);
}

#[test]
fn cancelling_the_page_dialog_keeps_the_graph() {
    let dir = ScratchDir::new("drawio-pages-cancel");
    let pages = [box_page("First", "One"), box_page("Second", "Two")];
    let file = dir.drawio("plan.drawio", &pages);
    let mut harness = fixture_harness();
    harness.app.selected = Selection::node("n0");
    harness.press(CTRL, Key::ArrowRight);
    let (before, fixture) = (harness.app.doc.graphml().clone(), harness.app.file.clone());
    // Escape, then Cancel.
    for key in [Some(Key::Escape), None] {
        harness.press(CTRL, Key::O);
        harness.answer_prompt("Don't Save");
        harness.answer(Some(&file));
        harness.press(Modifiers::NONE, Key::ArrowDown);
        match key {
            Some(key) => harness.press(Modifiers::NONE, key),
            None => {
                harness.answer_prompt("Cancel");
            }
        }
        harness.run(vec![]);
        assert!(!harness.app.page_dialog.is_open(), "{key:?}");
        assert_eq!(*harness.app.doc.graphml(), before, "{key:?}");
        assert_eq!(harness.app.file, fixture, "{key:?}");
        assert_eq!(harness.app.imported, None, "{key:?}");
        assert!(harness.app.modified, "{key:?}");
    }
    let file = dunce::canonicalize(&file).unwrap();
    assert!(!harness.app.config.recent_files().contains(&file));
}

#[test]
fn what_converting_couldnt_carry_over_shows_once_opened() {
    let dir = ScratchDir::new("drawio-notes");
    let file = dir.drawio("plan.drawio", &[page("A", LOOSE_LINE)]);
    for key in [Key::Enter, Key::Escape] {
        let mut harness = Harness::new();
        harness.press(Modifiers::NONE, Key::B);
        harness.open_file(&file);
        assert!(harness.painted("Opened with changes"));
        assert!(harness.painted(
            "plan.drawio was converted to GraphML. Some things couldn't be carried over:"
        ));
        assert!(harness.painted(LOOSE_LINE_NOTE));
        assert!(harness.app.imported.is_some());

        harness.press(Modifiers::NONE, key);
        assert!(!harness.painted("Opened with changes"), "{key:?}");
        // Escape didn't also pick the Cursor tool.
        assert_eq!(harness.app.tool, Tool::Box);
    }
}

#[test]
fn long_notes_wrap_beside_their_bullets() {
    let dir = ScratchDir::new("drawio-long-notes");
    let cells = [
        shape("2", "", "shape=mxgraph.aws4.lambda_function;", 10),
        shape("3", "", "shape=mxgraph.azure.storage_blob_container;", 200),
        shape("4", "", "shape=mxgraph.gcp2.cloud_run_service;", 400),
    ];
    let file = dir.drawio("plan.drawio", &[page("A", &cells.concat())]);
    let mut harness = Harness::new();
    harness.open_file(&file);
    let shapes = harness.run(vec![]);
    let galley = |text: &str| {
        let found = shapes.iter().find_map(|shape| match shape {
            Shape::Text(shape) if shape.galley.text().starts_with(text) => {
                Some((shape.pos, shape.galley.clone()))
            }
            _ => None,
        });
        found.unwrap_or_else(|| panic!("{text:?} wasn't painted"))
    };
    let (at, note) = galley("3 shapes yEd has nothing like became boxes: ");
    assert!(note.rows.len() > 1, "{:?}", note.text());
    assert!(note.size().x <= 400.0, "{:?}", note.size());
    let (bullet, _) = galley("•");
    assert!(
        bullet.x < at.x && (bullet.y - at.y).abs() < 1.0,
        "{bullet:?} {at:?}"
    );
}

#[test]
fn nothing_shows_when_everything_was_carried_over() {
    let dir = ScratchDir::new("drawio-no-notes");
    let file = dir.drawio("plan.drawio", &[box_page("A", "Box")]);
    let mut harness = Harness::new();
    harness.open_file(&file);
    assert!(!harness.painted("Opened with changes"));
    assert!(harness.app.import_notes.is_none());
}

#[test]
fn notes_on_the_page_picked_show_and_close_with_enter() {
    let dir = ScratchDir::new("drawio-page-notes");
    let pages = [box_page("First", "One"), page("Second", LOOSE_LINE)];
    let file = dir.drawio("plan.drawio", &pages);
    let mut harness = Harness::new();
    harness.open_file(&file);
    harness.press(Modifiers::NONE, Key::ArrowDown);
    harness.press(Modifiers::NONE, Key::Enter);
    assert!(harness.painted(LOOSE_LINE_NOTE));
    harness.press(Modifiers::NONE, Key::Enter);
    assert!(!harness.painted(LOOSE_LINE_NOTE));
}

#[test]
fn the_page_and_notes_dialogs_are_opaque_on_their_first_visible_frame() {
    let dir = ScratchDir::new("drawio-opaque");
    let pages = [box_page("First", "One"), page("Second", LOOSE_LINE)];
    let file = dir.drawio("plan.drawio", &pages);
    let mut harness = Harness::new();
    harness.app.open(&file).unwrap();
    assert_eq!(first_alpha(&mut harness, vec![], "Open page"), 255);
    harness.press(Modifiers::NONE, Key::ArrowDown);
    let enter = testing::key(Modifiers::NONE, Key::Enter);
    assert_eq!(
        first_alpha(&mut harness, vec![enter], "Opened with changes"),
        255
    );
}

/// The alpha of `text` in the first of three frames to paint it, the first
/// with `events`, as a dialog shown for the first time is only sized then.
fn first_alpha(harness: &mut Harness, events: Vec<Event>, text: &str) -> u8 {
    let mut events = Some(events);
    for _ in 0..3 {
        let shapes = harness.run(events.take().unwrap_or_default());
        let color = shapes.iter().find_map(|shape| match shape {
            Shape::Text(shape) if shape.galley.text() == text => Some(shape.fallback_color),
            _ => None,
        });
        if let Some(color) = color {
            return color.a();
        }
    }
    panic!("{text:?} wasn't painted");
}

#[test]
fn imported_drawio_files_join_recent_files_and_reopen_from_there() {
    let dir = ScratchDir::new("drawio-recent");
    let file = dir.drawio("plan.drawio", &[box_page("A", "Box")]);
    let file = dunce::canonicalize(file).unwrap();
    let mut harness = Harness::open(&file);
    let listed = std::slice::from_ref(&file);
    assert_eq!(harness.app.config.recent_files(), listed);

    harness.press(CTRL, Key::N);
    harness.perform(Action::OpenRecent(file.clone()));
    assert_eq!(harness.app.imported.as_deref(), Some(file.as_path()));
    assert_eq!(harness.labels(), ["Box"]);
}

#[test]
fn round_boxes_keep_their_drawio_corner_radius() {
    let dir = ScratchDir::new("drawio-radius");
    let file = dir.drawio(
        "plan.drawio",
        &[page("A", &shape("2", "", "rounded=1;", 10))],
    );
    let mut harness = Harness::open(&file);
    let id = harness.node_ids()[0].clone();
    assert_eq!(
        document::corner_radius(harness.app.doc.graphml(), &id),
        Some(9.0)
    );
    assert!(!harness.app.modified);
    harness.app.selected = Selection::node(id);
    assert_eq!(rows(&harness.side_panel(), "Corner radius"), ["9"]);
}

#[test]
fn shapes_drwr_has_none_for_yet_show_as_boxes() {
    let dir = ScratchDir::new("drawio-boxes");
    let cells = [
        shape("2", "Choice", "rhombus;fillColor=#FF0000;", 10),
        shape("3", "Store", "shape=cylinder3;fillColor=#00FF00;", 200),
    ];
    let file = dir.drawio("plan.drawio", &[page("A", &cells.concat())]);
    let mut harness = Harness::open(&file);
    let shapes = harness.run(vec![]);
    let canvas = canvas(&shapes);
    for (fill, x) in [(Color32::RED, 10.0), (Color32::GREEN, 200.0)] {
        let filled: Vec<_> = (shapes.iter())
            .filter_map(|shape| match shape {
                Shape::Rect(rect) if rect.fill == fill => Some(rect.rect),
                _ => None,
            })
            .collect();
        let bounds = Rect::from_min_size(canvas.min + vec2(x, 10.0), vec2(120.0, 60.0));
        assert_eq!(filled, [bounds], "{fill:?}");
    }
    for label in ["Choice", "Store"] {
        harness.canvas_text(label);
    }
}

#[test]
fn a_drawio_file_that_cant_be_read_shows_why_and_keeps_the_graph() {
    let dir = ScratchDir::new("drawio-broken");
    let broken = dir.path("broken.drawio");
    let compressed = r#"<mxfile><diagram name="A">not base64!</diagram></mxfile>"#;
    std::fs::write(&broken, compressed).unwrap();
    let mut harness = fixture_harness();
    let before = harness.app.doc.graphml().clone();
    harness.open_file(&broken);
    let error = harness.app.error.clone().unwrap_or_default();
    let message = format!(
        "Couldn't open {}:\npage \"A\" can't be decompressed",
        broken.display()
    );
    assert!(error.starts_with(&message), "{error:?}");
    assert_eq!(*harness.app.doc.graphml(), before);
    assert_eq!(harness.app.imported, None);
}

#[test]
fn dash_open_imports_a_drawio_file_asking_for_the_page_from_the_first_frame() {
    let dir = ScratchDir::new("drawio-dash-open");
    let pages = [box_page("First", "One"), box_page("Second", "Two")];
    let file = dir.drawio("plan.drawio", &pages);
    // As `main` does for `-open`, before the window opens.
    let mut app = DrwrApp::new(Fonts::default(), Config::default());
    app.open(&file).unwrap();
    let mut harness = Harness::with_app(app);
    assert!(harness.painted("plan.drawio has 2 pages. Pick the one to open:"));
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(harness.labels(), ["One"]);
    assert_eq!(harness.app.title, "plan.drawio (imported) — drwr");

    let mut app = DrwrApp::new(Fonts::default(), Config::default());
    let broken = dir.path("broken.drawio");
    std::fs::write(&broken, "<mxfile/>").unwrap();
    let err = app.open(&broken).unwrap_err();
    assert_eq!(err.to_string(), "the file has no pages");
}

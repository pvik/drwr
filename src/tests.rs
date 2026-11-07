use eframe::App as _;
use egui::{
    Color32, Event, Key, Modifiers, PointerButton, Popup, Pos2, RawInput, Rect, Shape, Vec2, vec2,
};
use graphml::yed::{self, NodeGraphics, NodeKind, ShapeType};

use super::*;

/// Ctrl as egui-winit reports it outside macOS.
const CTRL: Modifiers = Modifiers::CTRL.plus(Modifiers::COMMAND);

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/crates/graphml/tests/fixtures/yed.graphml"
);

/// Drives the whole app, one frame at a time. Never press Ctrl+O or click
/// Open in it: that opens the real file dialog on the desktop.
struct Harness {
    ctx: egui::Context,
    frame: eframe::Frame,
    app: DrwrApp,
}

impl Harness {
    fn new() -> Self {
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        let mut harness = Harness {
            ctx,
            frame: eframe::Frame::_new_kittest(),
            app: DrwrApp::default(),
        };
        harness.run(vec![]);
        harness
    }

    /// Runs a frame with `events`, returning what it painted.
    fn run(&mut self, events: Vec<Event>) -> Vec<Shape> {
        let input = RawInput {
            events,
            ..RawInput::default()
        };
        let mut output = self
            .ctx
            .run_ui(input, |ui| self.app.ui(ui, &mut self.frame));
        let shapes = std::mem::take(&mut output.shapes);
        output.drop_without_applying_deltas();
        shapes.into_iter().map(|clipped| clipped.shape).collect()
    }

    /// Clicks a point relative to the canvas's top-left corner.
    fn click(&mut self, at: Vec2) {
        let at = canvas(&self.run(vec![])).min + at;
        self.click_at(at);
    }

    /// Clicks the middle of the first text painted as `text`.
    fn click_text(&mut self, text: &str) {
        let at = self
            .run(vec![])
            .iter()
            .find_map(|shape| match shape {
                Shape::Text(shape) if shape.galley.text() == text => {
                    Some(shape.pos + shape.galley.rect.center().to_vec2())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("{text:?} wasn't painted"));
        self.click_at(at);
    }

    /// Double-clicks a point relative to the canvas's top-left corner.
    fn double_click(&mut self, at: Vec2) {
        let at = canvas(&self.run(vec![])).min + at;
        self.run(vec![Event::PointerMoved(at)]);
        for pressed in [true, false, true, false] {
            self.run(vec![Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            }]);
        }
    }

    fn click_at(&mut self, at: Pos2) {
        self.run(vec![Event::PointerMoved(at)]);
        for pressed in [true, false] {
            self.run(vec![Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            }]);
        }
    }

    /// Runs a frame, returning the texts it painted right of the canvas.
    fn side_panel(&mut self) -> Vec<String> {
        let shapes = self.run(vec![]);
        let canvas = canvas(&shapes);
        shapes
            .iter()
            .filter_map(|shape| match shape {
                Shape::Text(text) if text.pos.x > canvas.max.x => {
                    Some(text.galley.text().to_owned())
                }
                _ => None,
            })
            .collect()
    }

    /// Runs a frame, returning whether it painted `text`.
    fn painted(&mut self, text: &str) -> bool {
        self.run(vec![])
            .iter()
            .any(|shape| matches!(shape, Shape::Text(shape) if shape.galley.text() == text))
    }

    /// The graphics of the node with id `id`.
    fn graphics(&self, id: &str) -> NodeGraphics {
        document::graphics_of(&self.app.graphml, id).expect("the node has graphics")
    }

    /// The text of the first label of the node with id `id`.
    fn label(&self, id: &str) -> String {
        let label = LabelRef {
            node: id.to_owned(),
            index: 0,
        };
        document::label_text(&self.app.graphml, &label).expect("the node exists")
    }

    fn type_text(&mut self, text: &str) {
        self.run(vec![Event::Text(text.to_owned())]);
    }

    fn press(&mut self, modifiers: Modifiers, key: Key) {
        self.run(vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }]);
    }
}

/// Where the canvas is: the largest white rect painted, since the side
/// panel's color swatches can be white too.
fn canvas(shapes: &[Shape]) -> Rect {
    shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Rect(rect) if rect.fill == Color32::WHITE => Some(rect.rect),
            _ => None,
        })
        .max_by(|a, b| a.area().total_cmp(&b.area()))
        .expect("the canvas was painted")
}

#[test]
fn tool_shortcuts_select_the_tool() {
    let mut harness = Harness::new();
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
        assert_eq!(harness.app.tool, tool, "{key:?}");
    }
}

#[test]
fn alt_f_opens_the_file_menu() {
    let mut harness = Harness::new();
    harness.press(Modifiers::ALT, Key::F);
    // A popup that isn't shown is closed again by the end of the frame.
    harness.run(vec![]);
    assert!(Popup::is_any_open(&harness.ctx));
}

#[test]
fn escape_closes_the_menu_before_it_selects_the_cursor() {
    let mut harness = Harness::new();
    harness.press(Modifiers::NONE, Key::B);
    harness.press(Modifiers::ALT, Key::F);
    harness.run(vec![]);

    harness.press(Modifiers::NONE, Key::Escape);
    assert!(!Popup::is_any_open(&harness.ctx));
    assert_eq!(harness.app.tool, Tool::Box);

    harness.press(Modifiers::NONE, Key::Escape);
    assert_eq!(harness.app.tool, Tool::Cursor);
}

#[test]
fn the_error_dialog_keeps_the_keys_until_dismissed() {
    let mut harness = Harness::new();
    harness.press(Modifiers::NONE, Key::B);
    harness.app.error = Some("Couldn't open".to_owned());
    harness.run(vec![]);

    harness.press(Modifiers::NONE, Key::C);
    assert_eq!(harness.app.tool, Tool::Box);

    harness.press(Modifiers::NONE, Key::Escape);
    assert_eq!(harness.app.error, None);
    assert_eq!(harness.app.tool, Tool::Box);

    // The dialog was still up during the frame Escape closed it in.
    harness.run(vec![]);
    harness.press(Modifiers::NONE, Key::C);
    assert_eq!(harness.app.tool, Tool::Circle);
}

#[test]
fn starts_with_an_empty_unsaved_graph() {
    let app = DrwrApp::default();
    assert_eq!(app.graphml, GraphMl::default());
    assert_eq!(app.file, None);
}

#[test]
fn open_replaces_the_graph() {
    let mut app = DrwrApp::default();
    app.open(Path::new(FIXTURE)).unwrap();
    assert_eq!(app.graphml, GraphMl::read_file(FIXTURE).unwrap());
    assert_ne!(app.graphml, GraphMl::default());
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
    let before = app.graphml.clone();

    app.selected = Some("n0".to_owned());

    let not_graphml = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/box.svg");
    let err = app.open(Path::new(not_graphml)).unwrap_err();
    assert!(matches!(err, graphml::Error::Parse { .. }), "{err:?}");

    let missing = Path::new("/nonexistent/drwr.graphml");
    let err = app.open(missing).unwrap_err();
    assert!(matches!(err, graphml::Error::Io(_)), "{err:?}");

    assert_eq!(app.graphml, before);
    assert_eq!(app.file.as_deref(), Some(Path::new(FIXTURE)));
    assert_eq!(app.selected.as_deref(), Some("n0"));
}

#[test]
fn open_clears_the_selection() {
    let mut app = DrwrApp {
        selected: Some("n0".to_owned()),
        ..DrwrApp::default()
    };
    app.open(Path::new(FIXTURE)).unwrap();
    assert_eq!(app.selected, None);
}

#[test]
fn the_side_panel_shows_the_clicked_node() {
    let mut harness = Harness::new();
    let mut graphics = NodeGraphics::new(NodeKind::Shape(ShapeType::Rectangle));
    graphics.geometry = yed::Rect {
        x: 100.0,
        y: 50.5,
        width: 150.0,
        height: 80.0 / 3.0,
    };
    graphics.fill.gradient = Some(yed::Color::WHITE);
    graphics.border.color = None;
    graphics.labels = vec![yed::Label::new("Start"), yed::Label::new("Two\nlines")];
    document::add_node(&mut harness.app.graphml, &graphics);
    assert_eq!(harness.side_panel(), ["Nothing selected"]);

    harness.click(vec2(150.0, 60.0));
    // The side panel is shown before the canvas changes the selection, so it
    // catches up a frame later.
    #[rustfmt::skip]
    assert_eq!(harness.side_panel(), [
        "Id",           "n0",
        "Shape",        "rectangle",
        "X",            "100",
        "Y",            "50.5",
        "Width",        "150",
        "Height",       "26.67",
        "Fill",         "#FFCC00",
        "Gradient",     "#FFFFFF",
        "Border",       "None",
        "Border width", "1",
        "Border style", "line",
        "Label",        "...", "Start",
        "Label",        "...", "Two\nlines",
    ]);

    harness.click(vec2(400.0, 300.0));
    assert_eq!(harness.side_panel(), ["Nothing selected"]);
}

/// A 150×80 box at (100, 50) with yEd's default look.
fn new_box() -> NodeGraphics {
    let mut graphics = NodeGraphics::new(NodeKind::Shape(ShapeType::Rectangle));
    graphics.geometry = yed::Rect {
        x: 100.0,
        y: 50.0,
        width: 150.0,
        height: 80.0,
    };
    graphics
}

#[test]
fn clicking_a_color_in_the_side_panel_picks_a_new_one() {
    let mut harness = Harness::new();
    let id = document::add_node(&mut harness.app.graphml, &new_box());
    harness.click(vec2(150.0, 60.0));

    harness.click_text("#FFCC00");
    assert!(harness.painted("Fill color"));
    // The hex field has the focus, with the fill's color selected.
    harness.run(vec![Event::Text("#FF0000".to_owned())]);
    harness.press(Modifiers::NONE, Key::Enter);

    assert!(!harness.app.color_picker.is_open());
    let red = yed::Color::rgb(0xFF, 0x00, 0x00);
    assert_eq!(harness.graphics(&id).fill.color, Some(red));
    #[rustfmt::skip]
    assert_eq!(harness.side_panel(), [
        "Id",           "n0",
        "Shape",        "rectangle",
        "X",            "100",
        "Y",            "50",
        "Width",        "150",
        "Height",       "80",
        "Fill",         "#FF0000",
        "Gradient",     "None",
        "Border",       "#000000",
        "Border width", "1",
        "Border style", "line",
        "Label",        "...", "",
    ]);
}

#[test]
fn no_color_removes_the_color_clicked() {
    let mut harness = Harness::new();
    let id = document::add_node(&mut harness.app.graphml, &new_box());
    harness.click(vec2(150.0, 60.0));

    harness.click_text("#000000");
    assert!(harness.painted("Border color"));
    harness.click_text("No color");
    harness.click_text("OK");

    let graphics = harness.graphics(&id);
    assert_eq!(graphics.border.color, None);
    assert_eq!(graphics.fill, new_box().fill);
}

#[test]
fn typing_a_color_selects_no_tools() {
    let mut harness = Harness::new();
    let id = document::add_node(&mut harness.app.graphml, &new_box());
    harness.click(vec2(150.0, 60.0));
    harness.click_text("#FFCC00");
    harness.run(vec![]);

    // Each key is a tool's shortcut.
    for (key, text) in [
        (Key::A, "a"),
        (Key::B, "b"),
        (Key::C, "c"),
        (Key::E, "e"),
        (Key::A, "a"),
        (Key::B, "b"),
    ] {
        let press = Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        harness.run(vec![press, Event::Text(text.to_owned())]);
    }
    harness.press(Modifiers::NONE, Key::Enter);

    assert_eq!(harness.app.tool, Tool::Cursor);
    let typed = yed::Color::rgb(0xAB, 0xCE, 0xAB);
    assert_eq!(harness.graphics(&id).fill.color, Some(typed));
}

#[test]
fn open_closes_the_color_picker() {
    let mut app = DrwrApp::default();
    app.color_picker
        .open(ColorProperty::Fill, "Fill color", None);
    app.open(Path::new(FIXTURE)).unwrap();
    assert!(!app.color_picker.is_open());
}

/// The box from [`new_box`], labelled `text`.
fn labelled_box(text: &str) -> NodeGraphics {
    let mut graphics = new_box();
    graphics.labels = vec![yed::Label::new(text)];
    graphics
}

/// Adds a box labelled `text`, then selects it and clicks its label's field in
/// the side panel. Returns the box's id.
fn type_in_label_field(harness: &mut Harness, text: &str) -> String {
    let id = document::add_node(&mut harness.app.graphml, &labelled_box(text));
    harness.click(vec2(150.0, 60.0));
    // The side panel is painted first, so this is its field, not the canvas.
    harness.click_text(text);
    harness.press(CTRL, Key::A);
    id
}

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
    assert_eq!(harness.app.selected, None);
}

#[test]
fn a_node_without_labels_gets_one_from_the_side_panel() {
    let mut harness = Harness::new();
    let id = document::add_node(&mut harness.app.graphml, &new_box());
    harness.click(vec2(150.0, 60.0));
    let shapes = harness.run(vec![]);
    let side_panel = canvas(&shapes).max.x;
    let field = shapes
        .iter()
        .find_map(|shape| match shape {
            Shape::Text(text) if text.pos.x > side_panel && text.galley.text().is_empty() => {
                Some(text.pos + text.galley.rect.center().to_vec2())
            }
            _ => None,
        })
        .expect("the side panel has an empty label field");
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
        let press = Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        harness.run(vec![press, Event::Text(text.to_owned())]);
    }
    assert_eq!(harness.app.tool, Tool::Box);
    harness.press(Modifiers::NONE, Key::Escape);
    assert_eq!(harness.app.tool, Tool::Box);
    assert_eq!(harness.label(&id), "Start");
}

#[test]
fn the_dots_button_edits_the_label_in_a_dialog() {
    let mut harness = Harness::new();
    let id = document::add_node(&mut harness.app.graphml, &labelled_box("Start"));
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
    let id = document::add_node(&mut harness.app.graphml, &labelled_box("Start"));
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
    let id = document::add_node(&mut harness.app.graphml, &new_box());
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
fn escape_on_the_canvas_keeps_the_label_and_the_tool() {
    let mut harness = Harness::new();
    let id = document::add_node(&mut harness.app.graphml, &labelled_box("Start"));
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

    harness.app.selected = Some("n0".to_owned());
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
        node: "n0".to_owned(),
        index: 0,
    };
    app.label_dialog.open(label, "Label", "Start");
    app.open(Path::new(FIXTURE)).unwrap();
    assert!(!app.label_dialog.is_open());
}

use eframe::App as _;
use egui::{
    Color32, Event, Key, Modifiers, PointerButton, Popup, RawInput, Rect, Shape, Vec2, vec2,
};
use graphml::yed::{self, NodeGraphics, NodeKind, ShapeType};

use super::*;

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

/// Where the canvas is: the first white rect painted.
fn canvas(shapes: &[Shape]) -> Rect {
    shapes
        .iter()
        .find_map(|shape| match shape {
            Shape::Rect(rect) if rect.fill == Color32::WHITE => Some(rect.rect),
            _ => None,
        })
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
        "Label",        "Start",
        "Label",        "Two\nlines",
    ]);

    harness.click(vec2(400.0, 300.0));
    assert_eq!(harness.side_panel(), ["Nothing selected"]);
}

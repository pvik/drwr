use eframe::App as _;
use egui::{
    Color32, Event, Key, Modifiers, Popup, Pos2, RawInput, Rect, Shape, Stroke, Vec2, vec2,
};
use graphml::yed::{self, NodeGraphics, NodeKind, ShapeType};

use super::*;
use crate::testing::{self, CTRL, button};

impl DrwrApp {
    /// The id of the selected node.
    fn selected_node(&self) -> Option<&str> {
        match self.selected.single()? {
            Item::Node(id) => Some(id),
            Item::Edge(_) => None,
        }
    }
}

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
        let app = DrwrApp::default();
        app.init(&ctx);
        let mut harness = Harness {
            ctx,
            frame: eframe::Frame::_new_kittest(),
            app,
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
        let at = self.text_at(text);
        self.click_at(at);
    }

    /// Runs a frame, returning the middle of the first text it painted as
    /// `text`.
    fn text_at(&mut self, text: &str) -> Pos2 {
        let found = self.texts_at(text).first().copied();
        found.unwrap_or_else(|| panic!("{text:?} wasn't painted"))
    }

    /// Runs a frame, returning the middles of the texts it painted as `text`.
    fn texts_at(&mut self, text: &str) -> Vec<Pos2> {
        self.run(vec![])
            .iter()
            .filter_map(|shape| match shape {
                Shape::Text(shape) if shape.galley.text() == text => {
                    Some(shape.pos + shape.galley.rect.center().to_vec2())
                }
                _ => None,
            })
            .collect()
    }

    /// Runs a frame, returning the font and color of `text` on the canvas,
    /// which must be visible.
    fn canvas_text(&mut self, text: &str) -> egui::TextFormat {
        let shapes = self.run(vec![]);
        let canvas = canvas(&shapes);
        let galley = shapes.iter().find_map(|shape| match shape {
            Shape::Text(shape) if shape.galley.text() == text && canvas.contains(shape.pos) => {
                Some(&shape.galley)
            }
            _ => None,
        });
        let galley = galley.unwrap_or_else(|| panic!("{text:?} wasn't painted on the canvas"));
        assert!(galley.size().x > 0.0, "{text:?} has no glyphs");
        galley.job.sections[0].format.clone()
    }

    /// Double-clicks a point relative to the canvas's top-left corner.
    fn double_click(&mut self, at: Vec2) {
        let at = canvas(&self.run(vec![])).min + at;
        self.run(vec![Event::PointerMoved(at)]);
        for pressed in [true, false, true, false] {
            self.run(vec![button(at, pressed)]);
        }
    }

    /// Drags between two points relative to the canvas's top-left corner.
    fn drag(&mut self, from: Vec2, to: Vec2) {
        let origin = canvas(&self.run(vec![])).min;
        self.drag_and_hold(origin + from, origin + to);
        self.run(vec![button(origin + to, false)]);
    }

    /// Drags between two points without letting go, returning what the last
    /// frame painted.
    fn drag_and_hold(&mut self, from: Pos2, to: Pos2) -> Vec<Shape> {
        self.run(vec![Event::PointerMoved(from)]);
        self.run(vec![button(from, true)]);
        self.run(vec![Event::PointerMoved(to)])
    }

    fn click_at(&mut self, at: Pos2) {
        self.run(vec![Event::PointerMoved(at)]);
        for pressed in [true, false] {
            self.run(vec![button(at, pressed)]);
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
            item: Item::Node(id.to_owned()),
            index: 0,
        };
        document::label_text(&self.app.graphml, &label).expect("the node exists")
    }

    fn type_text(&mut self, text: &str) {
        self.run(vec![Event::Text(text.to_owned())]);
    }

    fn press(&mut self, modifiers: Modifiers, key: Key) {
        self.run(vec![testing::key(modifiers, key)]);
    }

    /// Does `act` with Ctrl held.
    fn with_ctrl(&mut self, act: impl FnOnce(&mut Self)) {
        self.run(vec![Event::ModifiersChanged(CTRL)]);
        act(self);
        self.run(vec![Event::ModifiersChanged(Modifiers::NONE)]);
    }

    /// Runs a frame, returning the widget labelled `label`, e.g. a toolbar
    /// button.
    fn widget(&mut self, label: &str) -> egui::accesskit::Node {
        self.ctx.enable_accesskit();
        let mut output = self
            .ctx
            .run_ui(RawInput::default(), |ui| self.app.ui(ui, &mut self.frame));
        let widgets = testing::widgets(&mut output);
        output.drop_without_applying_deltas();
        let widget = widgets.into_iter().find(|w| w.label() == Some(label));
        widget.unwrap_or_else(|| panic!("no widget is labelled {label:?}"))
    }

    fn click_widget(&mut self, label: &str) {
        let at = testing::center(&self.widget(label));
        self.click_at(at);
    }

    /// Runs a frame with `events`, returning the texts it put on the
    /// clipboard and whether it asked for the clipboard's text.
    fn clipboard(&mut self, events: Vec<Event>) -> (Vec<String>, bool) {
        let input = RawInput {
            events,
            ..RawInput::default()
        };
        let output = self
            .ctx
            .run_ui(input, |ui| self.app.ui(ui, &mut self.frame));
        let copied = (output.platform_output.commands.iter())
            .filter_map(|command| match command {
                egui::OutputCommand::CopyText(text) => Some(text.clone()),
                _ => None,
            })
            .collect();
        let paste = (output.viewport_output.values())
            .flat_map(|viewport| &viewport.commands)
            .any(|command| matches!(command, egui::ViewportCommand::RequestPaste));
        output.drop_without_applying_deltas();
        (copied, paste)
    }

    /// Clicks `at`, returning what [`Self::clipboard`] does for the release.
    fn click_for_clipboard(&mut self, at: Pos2) -> (Vec<String>, bool) {
        self.run(vec![Event::PointerMoved(at)]);
        self.run(vec![button(at, true)]);
        self.clipboard(vec![button(at, false)])
    }

    /// Turns the wheel by `lines` notches over `at`, then runs frames until
    /// egui has applied it all.
    fn wheel(&mut self, at: Pos2, lines: f32) {
        self.run(vec![Event::PointerMoved(at)]);
        self.run(vec![testing::wheel(lines)]);
        // egui spreads a notch over the frames of about a tenth of a second.
        for _ in 0..30 {
            self.run(vec![]);
        }
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

/// The solid border painted around the node filled with `fill` on the canvas.
fn border(shapes: &[Shape], fill: Color32) -> Stroke {
    let canvas = canvas(shapes);
    let rects = || {
        shapes.iter().filter_map(|shape| match shape {
            Shape::Rect(rect) if canvas.contains_rect(rect.rect) => Some(rect),
            _ => None,
        })
    };
    let nodes: Vec<_> = rects().filter(|rect| rect.fill == fill).collect();
    let [node] = nodes[..] else {
        panic!("expected one node filled with {fill:?}: {nodes:?}");
    };
    let borders: Vec<_> = rects()
        .filter(|rect| rect.rect == node.rect && !rect.stroke.is_empty())
        .map(|rect| rect.stroke)
        .collect();
    let [border] = borders[..] else {
        panic!("expected one border around {node:?}: {borders:?}");
    };
    border
}

/// How many dashes were painted on the canvas.
fn dashes(shapes: &[Shape]) -> usize {
    let canvas = canvas(shapes);
    shapes
        .iter()
        .map(|shape| match shape {
            Shape::Mesh(mesh) if canvas.contains_rect(mesh.calc_bounds()) => {
                mesh.vertices.len() / 4
            }
            _ => 0,
        })
        .sum()
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

    app.selected = Selection::node("n0".to_owned());

    let not_graphml = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/box.svg");
    let err = app.open(Path::new(not_graphml)).unwrap_err();
    assert!(matches!(err, graphml::Error::Parse { .. }), "{err:?}");

    let missing = Path::new("/nonexistent/drwr.graphml");
    let err = app.open(missing).unwrap_err();
    assert!(matches!(err, graphml::Error::Io(_)), "{err:?}");

    assert_eq!(app.graphml, before);
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
        "Id",            "n0",
        "Shape",         "rectangle",
        "X",             "100",
        "Y",             "50.5",
        "Width",         "150",
        "Height",        "26.67",
        "Fill",          "#FFCC00",
        "Gradient",      "#FFFFFF",
        "Border",        "None",
        "Border width",  "1",
        "Border style",  "Solid",
        "Corner radius", "0",
        "Label",         "...", "Start",
        "Font",          "Dialog",
        "Font style",    "Plain",
        "Font size",     "12",
        "Text color",    "#000000",
        "Position",      "Center",
        "Label",         "...", "Two\nlines",
        "Font",          "Dialog",
        "Font style",    "Plain",
        "Font size",     "12",
        "Text color",    "#000000",
        "Position",      "Center",
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
fn dragging_a_side_resizes_the_selected_node() {
    let mut harness = Harness::new();
    let id = document::add_node(&mut harness.app.graphml, &new_box());
    harness.click(vec2(150.0, 60.0));
    harness.drag(vec2(250.0, 90.0), vec2(300.0, 90.0));
    assert_eq!(harness.graphics(&id).geometry.width, 200.0);
    let side_panel = harness.side_panel();
    assert!(
        side_panel.windows(2).any(|row| row == ["Width", "200"]),
        "{side_panel:?}"
    );
}

#[test]
fn dragging_the_selected_node_moves_it() {
    let mut harness = Harness::new();
    let id = document::add_node(&mut harness.app.graphml, &new_box());
    harness.click(vec2(150.0, 60.0));
    harness.drag(vec2(175.0, 90.0), vec2(205.0, 130.0));
    let geometry = harness.graphics(&id).geometry;
    assert_eq!((geometry.x, geometry.y), (130.0, 90.0));
    let side_panel = harness.side_panel();
    assert!(
        side_panel
            .windows(4)
            .any(|rows| rows == ["X", "130", "Y", "90"]),
        "{side_panel:?}"
    );
}

#[test]
fn ctrl_arrows_move_the_selected_node() {
    let mut harness = Harness::new();
    let id = document::add_node(&mut harness.app.graphml, &labelled_box("Start"));
    let unmoved = harness.app.graphml.clone();
    harness.press(CTRL, Key::ArrowRight);
    assert_eq!(harness.app.graphml, unmoved);

    harness.click(vec2(150.0, 60.0));
    let before = harness.graphics(&id);
    for (key, x, y) in [
        (Key::ArrowRight, 101.0, 50.0),
        (Key::ArrowDown, 101.0, 51.0),
        (Key::ArrowLeft, 100.0, 51.0),
        (Key::ArrowUp, 100.0, 50.0),
        (Key::ArrowUp, 100.0, 49.0),
    ] {
        harness.press(CTRL, key);
        let geometry = harness.graphics(&id).geometry;
        assert_eq!((geometry.x, geometry.y), (x, y), "{key:?}");
    }
    // Whatever the tool.
    harness.press(Modifiers::NONE, Key::B);
    harness.press(CTRL, Key::ArrowLeft);

    let after = harness.graphics(&id);
    let geometry = yed::Rect {
        x: 99.0,
        y: 49.0,
        ..before.geometry
    };
    assert_eq!(after.geometry, geometry);
    // Placed relative to the node, they move with it unchanged.
    assert_eq!(after.labels, before.labels);
    let side_panel = harness.side_panel();
    assert!(
        side_panel
            .windows(4)
            .any(|rows| rows == ["X", "99", "Y", "49"]),
        "{side_panel:?}"
    );
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
        "Fill",          "#FF0000",
        "Gradient",      "None",
        "Border",        "#000000",
        "Border width",  "1",
        "Border style",  "Solid",
        "Corner radius", "0",
        "Label",         "...", "",
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
        let press = testing::key(Modifiers::NONE, key);
        harness.run(vec![press, Event::Text(text.to_owned())]);
    }
    harness.press(Modifiers::NONE, Key::Enter);

    assert_eq!(harness.app.tool, Tool::Cursor);
    let typed = yed::Color::rgb(0xAB, 0xCE, 0xAB);
    assert_eq!(harness.graphics(&id).fill.color, Some(typed));
}

/// yEd's default node fill, which [`new_box`] has.
const BOX_FILL: Color32 = Color32::from_rgb(0xFF, 0xCC, 0x00);

/// Adds the box from [`new_box`] and selects it, then clicks its border width
/// in the side panel. Returns the box's id.
fn click_border_width(harness: &mut Harness) -> String {
    let id = document::add_node(&mut harness.app.graphml, &new_box());
    harness.click(vec2(150.0, 60.0));
    harness.click_text("1");
    id
}

#[test]
fn typing_a_border_width_in_the_side_panel_sets_it() {
    let mut harness = Harness::new();
    let id = click_border_width(&mut harness);
    // The width is selected, so typing replaces it.
    harness.type_text("4.5");
    assert_eq!(harness.graphics(&id).border.width, 1.0);
    harness.press(Modifiers::NONE, Key::Enter);

    assert_eq!(harness.graphics(&id).border.width, 4.5);
    assert!(!harness.ctx.text_edit_focused());
    let shapes = harness.run(vec![]);
    assert_eq!(border(&shapes, BOX_FILL), Stroke::new(4.5, Color32::BLACK));
    let side_panel = harness.side_panel();
    assert!(
        side_panel
            .windows(2)
            .any(|row| row == ["Border width", "4.5"]),
        "{side_panel:?}"
    );
}

#[test]
fn dragging_the_border_width_changes_it_as_it_goes() {
    let mut harness = Harness::new();
    let id = document::add_node(&mut harness.app.graphml, &new_box());
    harness.click(vec2(150.0, 60.0));
    let field = harness.text_at("1");
    // A tenth per point, rightwards.
    let shapes = harness.drag_and_hold(field, field + vec2(20.0, 0.0));
    assert_eq!(harness.graphics(&id).border.width, 3.0);
    assert_eq!(border(&shapes, BOX_FILL).width, 3.0);
    harness.run(vec![button(field + vec2(20.0, 0.0), false)]);
    assert_eq!(harness.graphics(&id).border.width, 3.0);
}

#[test]
fn escape_in_the_border_width_field_keeps_the_width_and_the_tool() {
    let mut harness = Harness::new();
    let id = document::add_node(&mut harness.app.graphml, &new_box());
    harness.click(vec2(150.0, 60.0));
    harness.press(Modifiers::NONE, Key::B);
    harness.click_text("1");
    harness.type_text("4");
    harness.press(Modifiers::NONE, Key::Escape);
    harness.run(vec![]);

    assert_eq!(harness.graphics(&id).border.width, 1.0);
    assert_eq!(harness.app.tool, Tool::Box);
    assert!(!harness.ctx.text_edit_focused());
    // Once the field has let go of the keys.
    harness.press(Modifiers::NONE, Key::Escape);
    assert_eq!(harness.app.tool, Tool::Cursor);
}

#[test]
fn clicking_a_color_applies_the_border_width_typed_first() {
    let mut harness = Harness::new();
    let id = click_border_width(&mut harness);
    harness.type_text("4");
    harness.click_text("#FFCC00");
    assert_eq!(harness.graphics(&id).border.width, 4.0);
    assert!(harness.painted("Fill color"));
}

#[test]
fn the_dots_button_applies_the_border_width_typed_first() {
    let mut harness = Harness::new();
    let id = click_border_width(&mut harness);
    harness.type_text("4");
    harness.click_text("...");
    assert_eq!(harness.graphics(&id).border.width, 4.0);
    assert!(harness.app.label_dialog.is_open());
}

#[test]
fn entering_the_border_width_unchanged_keeps_it_exactly() {
    let mut harness = Harness::new();
    let mut graphics = new_box();
    graphics.border.width = 0.125;
    let id = document::add_node(&mut harness.app.graphml, &graphics);
    harness.click(vec2(150.0, 60.0));
    harness.click_text("0.125");
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(harness.graphics(&id).border.width, 0.125);
}

#[test]
fn typed_border_widths_are_kept_in_range() {
    for (typed, width) in [("-2", 0.0), ("1000", 100.0), ("inf", 100.0)] {
        let mut harness = Harness::new();
        let id = click_border_width(&mut harness);
        harness.type_text(typed);
        harness.press(Modifiers::NONE, Key::Enter);
        assert_eq!(harness.graphics(&id).border.width, width, "{typed}");
    }
}

#[test]
fn showing_a_border_width_out_of_range_keeps_it() {
    let mut harness = Harness::new();
    let mut graphics = new_box();
    graphics.border.width = 250.0;
    document::add_node(&mut harness.app.graphml, &graphics);
    let before = harness.app.graphml.clone();
    harness.click(vec2(150.0, 60.0));
    let side_panel = harness.side_panel();
    assert!(
        side_panel
            .windows(2)
            .any(|row| row == ["Border width", "250"]),
        "{side_panel:?}"
    );
    assert_eq!(harness.app.graphml, before);
}

/// The side panel's rows of `name` and the value right of it.
fn rows<'a>(side_panel: &'a [String], name: &str) -> Vec<&'a str> {
    let rows = side_panel.windows(2).filter(|row| row[0] == name);
    rows.map(|row| row[1].as_str()).collect()
}

#[test]
fn typing_a_corner_radius_in_the_side_panel_rounds_the_box() {
    let mut harness = Harness::new();
    let id = document::add_node(&mut harness.app.graphml, &new_box());
    harness.click(vec2(150.0, 60.0));
    assert_eq!(rows(&harness.side_panel(), "Corner radius"), ["0"]);
    harness.click_text("0");
    harness.type_text("10");
    harness.press(Modifiers::NONE, Key::Enter);

    let graphml = &harness.app.graphml;
    assert_eq!(document::corner_radius(graphml, &id), Some(10.0));
    let side_panel = harness.side_panel();
    assert_eq!(rows(&side_panel, "Corner radius"), ["10"]);
    assert_eq!(rows(&side_panel, "Shape"), ["roundrectangle"]);
    // A polygon now, not a rect.
    let shapes = harness.run(vec![]);
    let canvas = canvas(&shapes);
    let on_canvas = |shape: &&Shape| match shape {
        Shape::Rect(rect) => rect.fill == BOX_FILL && canvas.contains_rect(rect.rect),
        Shape::Path(path) => path.fill == BOX_FILL && canvas.contains(path.points[0]),
        _ => false,
    };
    let filled: Vec<_> = shapes.iter().filter(on_canvas).collect();
    assert!(matches!(filled[..], [Shape::Path(_)]), "{filled:?}");
}

#[test]
fn round_rectangles_read_with_yeds_corner_radius() {
    let mut harness = Harness::new();
    let mut graphics = new_box();
    graphics.kind = NodeKind::Shape(ShapeType::RoundRectangle);
    document::add_node(&mut harness.app.graphml, &graphics);
    harness.click(vec2(150.0, 60.0));
    assert_eq!(rows(&harness.side_panel(), "Corner radius"), ["4"]);
}

#[test]
fn only_boxes_have_a_corner_radius() {
    let mut harness = Harness::new();
    let mut graphics = new_box();
    graphics.kind = NodeKind::Shape(ShapeType::Ellipse);
    document::add_node(&mut harness.app.graphml, &graphics);
    harness.click(vec2(175.0, 90.0));
    let side_panel = harness.side_panel();
    assert_eq!(side_panel[..4], ["Id", "n0", "Shape", "ellipse"]);
    assert_eq!(rows(&side_panel, "Corner radius"), [""; 0]);
}

#[test]
fn typed_corner_radii_are_kept_in_range() {
    for (typed, radius) in [("-2", 0.0), ("5000", 1000.0), ("inf", 1000.0)] {
        let mut harness = Harness::new();
        let id = document::add_node(&mut harness.app.graphml, &new_box());
        harness.click(vec2(150.0, 60.0));
        harness.click_text("0");
        harness.type_text(typed);
        harness.press(Modifiers::NONE, Key::Enter);
        let read = document::corner_radius(&harness.app.graphml, &id);
        assert_eq!(read, Some(radius), "{typed}");
    }
}

#[test]
fn picking_a_border_style_in_the_side_panel_sets_it() {
    let mut harness = Harness::new();
    let id = document::add_node(&mut harness.app.graphml, &new_box());
    harness.click(vec2(150.0, 60.0));
    let mut shown = "Solid";
    for (name, line_type) in [
        ("Dashed", yed::LineType::Dashed),
        ("Dotted", yed::LineType::Dotted),
        ("Dash-dotted", yed::LineType::DashedDotted),
        ("Solid", yed::LineType::Line),
    ] {
        harness.click_text(shown);
        harness.click_text(name);
        assert_eq!(harness.graphics(&id).border.line_type, line_type);
        assert!(!Popup::is_any_open(&harness.ctx));
        let side_panel = harness.side_panel();
        assert!(
            side_panel
                .windows(2)
                .any(|row| row == ["Border style", name]),
            "{side_panel:?}"
        );
        let shapes = harness.run(vec![]);
        assert_eq!(dashes(&shapes) > 0, name != "Solid", "{name}");
        shown = name;
    }
    let shapes = harness.run(vec![]);
    assert_eq!(border(&shapes, BOX_FILL), Stroke::new(1.0, Color32::BLACK));
}

#[test]
fn picking_the_border_style_a_node_has_changes_nothing() {
    let mut harness = Harness::new();
    document::add_node(&mut harness.app.graphml, &new_box());
    harness.click(vec2(150.0, 60.0));
    let before = harness.app.graphml.clone();
    harness.click_text("Solid");
    // The list's, painted over the dropdown's own.
    let solids: Vec<_> = harness
        .run(vec![])
        .iter()
        .filter_map(|shape| match shape {
            Shape::Text(text) if text.galley.text() == "Solid" => {
                Some(text.pos + text.galley.rect.center().to_vec2())
            }
            _ => None,
        })
        .collect();
    let [_, listed] = solids[..] else {
        panic!("expected the dropdown and its list to show Solid: {solids:?}");
    };
    harness.click_at(listed);
    assert!(!Popup::is_any_open(&harness.ctx));
    assert_eq!(harness.app.graphml, before);
}

#[test]
fn escape_closes_the_border_style_dropdown_before_it_selects_the_cursor() {
    let mut harness = Harness::new();
    document::add_node(&mut harness.app.graphml, &new_box());
    harness.click(vec2(150.0, 60.0));
    harness.press(Modifiers::NONE, Key::B);
    harness.click_text("Solid");
    assert!(Popup::is_any_open(&harness.ctx));

    harness.press(Modifiers::NONE, Key::Escape);
    assert!(!Popup::is_any_open(&harness.ctx));
    assert_eq!(harness.app.tool, Tool::Box);

    harness.press(Modifiers::NONE, Key::Escape);
    assert_eq!(harness.app.tool, Tool::Cursor);
}

#[test]
fn clicking_the_border_style_applies_the_border_width_typed_first() {
    let mut harness = Harness::new();
    let id = click_border_width(&mut harness);
    harness.type_text("4");
    harness.click_text("Solid");
    assert_eq!(harness.graphics(&id).border.width, 4.0);
    assert!(Popup::is_any_open(&harness.ctx));
}

#[test]
fn unknown_border_styles_read_as_written_and_are_drawn_solid() {
    let mut harness = Harness::new();
    let mut graphics = new_box();
    graphics.border.line_type = yed::LineType::Other("wavy".to_owned());
    document::add_node(&mut harness.app.graphml, &graphics);
    let before = harness.app.graphml.clone();
    harness.click(vec2(150.0, 60.0));
    let side_panel = harness.side_panel();
    assert!(
        side_panel
            .windows(2)
            .any(|row| row == ["Border style", "wavy"]),
        "{side_panel:?}"
    );
    let shapes = harness.run(vec![]);
    assert_eq!(border(&shapes, BOX_FILL), Stroke::new(1.0, Color32::BLACK));
    assert_eq!(harness.app.graphml, before);
}

#[test]
fn open_closes_the_color_picker() {
    let mut app = DrwrApp::default();
    app.color_picker
        .open(ColorProperty::Fill, "Fill color", None, true);
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
    assert!(harness.app.selected.is_empty());
}

#[test]
fn a_node_without_labels_gets_one_from_the_side_panel() {
    let mut harness = Harness::new();
    let id = document::add_node(&mut harness.app.graphml, &new_box());
    harness.click(vec2(150.0, 60.0));
    let field = harness.empty_field();
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
        let press = testing::key(Modifiers::NONE, key);
        harness.run(vec![press, Event::Text(text.to_owned())]);
    }
    assert_eq!(harness.app.tool, Tool::Box);
    harness.press(Modifiers::NONE, Key::Escape);
    assert_eq!(harness.app.tool, Tool::Box);
    assert_eq!(harness.label(&id), "Start");
}

#[test]
fn ctrl_arrows_in_a_label_field_move_the_text_cursor() {
    let mut harness = Harness::new();
    let id = type_in_label_field(&mut harness, "Start");
    harness.type_text("One two");
    // Back a word.
    harness.press(CTRL, Key::ArrowLeft);
    harness.type_text("big ");
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(harness.label(&id), "One big two");
    assert_eq!(harness.graphics(&id).geometry, new_box().geometry);
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
fn ctrl_enter_edits_the_label_whatever_the_tool() {
    let mut harness = Harness::new();
    let id = document::add_node(&mut harness.app.graphml, &new_box());
    harness.click(vec2(150.0, 60.0));
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
        harness.press(CTRL, Key::Enter);
        harness.run(vec![]);
        assert!(harness.ctx.text_edit_focused(), "{tool:?}");
        let text = format!("{tool:?}");
        harness.type_text(&text);
        harness.press(Modifiers::NONE, Key::Enter);
        assert_eq!(harness.label(&id), text);
        assert_eq!(harness.app.tool, tool);
    }
}

#[test]
fn boxes_can_be_drawn_and_labelled_without_changing_tools() {
    let mut harness = Harness::new();
    harness.press(Modifiers::NONE, Key::B);
    for (corner, text) in [(vec2(20.0, 20.0), "One"), (vec2(20.0, 150.0), "Two")] {
        harness.drag(corner, corner + vec2(120.0, 60.0));
        harness.press(CTRL, Key::Enter);
        harness.run(vec![]);
        harness.type_text(text);
        harness.press(Modifiers::NONE, Key::Enter);
    }
    assert_eq!(harness.label("n0"), "One");
    assert_eq!(harness.label("n1"), "Two");
    assert_eq!(harness.app.tool, Tool::Box);
}

#[test]
fn circles_and_ellipses_are_drawn_with_their_tools() {
    let mut harness = Harness::new();
    harness.press(Modifiers::NONE, Key::C);
    harness.drag(vec2(20.0, 20.0), vec2(140.0, 80.0));
    harness.press(Modifiers::NONE, Key::E);
    harness.drag(vec2(20.0, 150.0), vec2(140.0, 210.0));

    let ellipse = NodeKind::Shape(ShapeType::Ellipse);
    let circle = harness.graphics("n0");
    assert_eq!(circle.kind, ellipse);
    assert_eq!(
        circle.geometry,
        yed::Rect {
            x: 20.0,
            y: 20.0,
            width: 120.0,
            height: 120.0,
        }
    );
    let oval = harness.graphics("n1");
    assert_eq!(oval.kind, ellipse);
    assert_eq!(
        oval.geometry,
        yed::Rect {
            x: 20.0,
            y: 150.0,
            width: 120.0,
            height: 60.0,
        }
    );
    assert_eq!(harness.app.selected_node(), Some("n1"));
    assert_eq!(harness.side_panel()[..4], ["Id", "n1", "Shape", "ellipse"]);
}

/// The light blue new nodes get.
const LIGHT_BLUE: yed::Color = yed::Color::rgb(0xAD, 0xD8, 0xE6);
const RED: yed::Color = yed::Color::rgb(0xFF, 0x00, 0x00);

impl Harness {
    /// Performs `action`, as the side panel does.
    fn perform(&mut self, action: Action) {
        self.app.perform(action, &self.ctx, &self.frame);
    }

    /// Draws a node with `tool` between two points relative to the canvas's
    /// top-left corner, returning its id.
    fn draw_node(&mut self, tool: Tool, from: Vec2, to: Vec2) -> String {
        self.app.tool = tool;
        self.drag(from, to);
        self.app
            .selected_node()
            .expect("a node was drawn")
            .to_owned()
    }

    /// Draws a line with `tool` from the node at one point to the node at
    /// another, both relative to the canvas's top-left corner, returning its
    /// index.
    fn draw_line(&mut self, tool: Tool, from: Vec2, to: Vec2) -> usize {
        self.app.tool = tool;
        self.drag(from, to);
        self.app.selected.edges().next().expect("a line was drawn")
    }

    /// Types `text` on the canvas as the first label of the selected node or
    /// line.
    fn type_label(&mut self, text: &str) {
        self.press(CTRL, Key::Enter);
        self.run(vec![]);
        self.type_text(text);
        self.press(Modifiers::NONE, Key::Enter);
    }
}

#[test]
fn new_nodes_get_the_fill_and_border_last_set() {
    let mut harness = Harness::new();
    click_border_width(&mut harness);
    harness.type_text("3");
    harness.click_text("#FFCC00");
    harness.run(vec![]);
    harness.run(vec![Event::Text("#FF0000".to_owned())]);
    harness.press(Modifiers::NONE, Key::Enter);
    for action in [
        Action::SetColor(ColorProperty::Gradient, Some(yed::Color::WHITE)),
        Action::SetColor(ColorProperty::Border, None),
        Action::SetLineType(yed::LineType::Dashed),
    ] {
        harness.perform(action);
    }

    let fill = yed::Fill {
        color: Some(RED),
        gradient: Some(yed::Color::WHITE),
    };
    let border = yed::Stroke {
        color: None,
        line_type: yed::LineType::Dashed,
        width: 3.0,
    };
    for (tool, x) in [
        (Tool::Box, 20.0),
        (Tool::Circle, 200.0),
        (Tool::Ellipse, 380.0),
    ] {
        let id = harness.draw_node(tool, vec2(x, 200.0), vec2(x + 120.0, 260.0));
        let graphics = harness.graphics(&id);
        assert_eq!(graphics.fill, fill, "{tool:?}");
        assert_eq!(graphics.border, border, "{tool:?}");
    }
}

#[test]
fn new_lines_get_the_line_last_set_and_new_nodes_keep_their_look() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let left = document::add_node(graphml, &new_box());
    let right = document::add_node(graphml, &box_at(300.0, 0.0));
    document::add_edge(graphml, &left, &right, &edge_through(&[]));
    harness.app.selected = Selection::edge(0);
    for action in [
        Action::SetColor(ColorProperty::Line, Some(RED)),
        Action::SetLineWidth(2.5),
        Action::SetLineType(yed::LineType::Dotted),
        // Arrowheads are up to the tool.
        Action::SetArrow(document::EdgeEnd::Source, true),
    ] {
        harness.perform(action);
    }

    let below = harness.draw_node(Tool::Box, vec2(100.0, 250.0), vec2(250.0, 330.0));
    let graphics = harness.graphics(&below);
    assert_eq!(graphics.fill.color, Some(LIGHT_BLUE));
    assert_eq!(graphics.border, yed::Stroke::default());
    let line = yed::Stroke {
        color: Some(RED),
        line_type: yed::LineType::Dotted,
        width: 2.5,
    };
    for (tool, target) in [
        (Tool::Line, yed::Arrow::None),
        (Tool::Arrow, yed::Arrow::Standard),
    ] {
        let index = harness.draw_line(tool, vec2(175.0, 290.0), vec2(175.0, 90.0));
        let edge = harness.edge(index);
        assert_eq!(edge.line, line, "{tool:?}");
        let arrows = yed::Arrows {
            source: yed::Arrow::None,
            target,
        };
        assert_eq!(edge.arrows, arrows, "{tool:?}");
    }
}

#[test]
fn line_rows_set_on_nodes_and_lines_together_restyle_new_ones_of_both() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let left = document::add_node(graphml, &new_box());
    let right = document::add_node(graphml, &box_at(300.0, 0.0));
    document::add_edge(graphml, &left, &right, &edge_through(&[]));
    harness.app.selected = Selection::of([node(&left), Item::Edge(0)]);
    harness.click_text("#000000");
    assert!(harness.painted("Line color"));
    harness.run(vec![Event::Text("#FF0000".to_owned())]);
    harness.press(Modifiers::NONE, Key::Enter);
    harness.click_number_field(0);
    harness.type_text("4");
    harness.press(Modifiers::NONE, Key::Enter);
    harness.click_text("Solid");
    harness.click_text("Dashed");

    let stroke = yed::Stroke {
        color: Some(RED),
        line_type: yed::LineType::Dashed,
        width: 4.0,
    };
    let below = harness.draw_node(Tool::Box, vec2(100.0, 250.0), vec2(250.0, 330.0));
    assert_eq!(harness.graphics(&below).border, stroke);
    let index = harness.draw_line(Tool::Line, vec2(175.0, 290.0), vec2(475.0, 90.0));
    assert_eq!(harness.edge(index).line, stroke);
}

#[test]
fn new_items_keep_the_defaults_of_what_was_never_set() {
    let mut harness = Harness::new();
    let id = document::add_node(&mut harness.app.graphml, &new_box());
    harness.app.selected = Selection::node(id);
    harness.perform(Action::SetLineWidth(2.0));

    let below = harness.draw_node(Tool::Box, vec2(100.0, 250.0), vec2(250.0, 330.0));
    let graphics = harness.graphics(&below);
    let fill = yed::Fill {
        color: Some(LIGHT_BLUE),
        gradient: None,
    };
    assert_eq!(graphics.fill, fill);
    let border = yed::Stroke {
        width: 2.0,
        ..yed::Stroke::default()
    };
    assert_eq!(graphics.border, border);
    // With square corners.
    assert_eq!(graphics.kind, NodeKind::Shape(ShapeType::Rectangle));
    let index = harness.draw_line(Tool::Line, vec2(175.0, 290.0), vec2(175.0, 90.0));
    assert_eq!(harness.edge(index).line, yed::Stroke::default());
    harness.type_label("New");
    let label = &harness.edge(index).labels[0];
    assert_eq!(label.font, yed::Font::default());
    assert_eq!(label.text_color, yed::Color::BLACK);
}

#[test]
fn new_labels_get_the_font_and_text_color_last_set() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let styled = document::add_node(graphml, &labelled_box("Styled"));
    let mut kept = labelled_box("Kept");
    kept.geometry.x = 400.0;
    let kept = document::add_node(graphml, &kept);
    harness.app.selected = Selection::node(styled.clone());
    let label = LabelRef {
        item: node(&styled),
        index: 0,
    };
    for action in [
        Action::SetFont(label.clone(), FontPart::Family("Monospaced".to_owned())),
        Action::SetFont(label.clone(), FontPart::Style(yed::FontStyle::Bold)),
        Action::SetFont(label, FontPart::Size(20)),
        Action::SetColor(ColorProperty::Text(0), Some(RED)),
    ] {
        harness.perform(action);
    }
    let font = yed::Font {
        family: "Monospaced".to_owned(),
        size: 20,
        style: yed::FontStyle::Bold,
    };

    // Typed in that font, too.
    let below = harness.draw_node(Tool::Box, vec2(100.0, 250.0), vec2(250.0, 330.0));
    harness.press(CTRL, Key::Enter);
    harness.run(vec![]);
    harness.type_text("New");
    let family = fonts::family("Monospaced", &yed::FontStyle::Bold);
    let typed = harness.canvas_text("New").font_id;
    assert_eq!(typed, egui::FontId::new(20.0, family));
    harness.press(Modifiers::NONE, Key::Enter);
    let new = &harness.graphics(&below).labels[0];
    assert_eq!(
        (new.text.as_str(), &new.font, new.text_color),
        ("New", &font, RED)
    );

    let index = harness.draw_line(Tool::Line, vec2(175.0, 290.0), vec2(175.0, 90.0));
    harness.type_label("Line");
    let new = &harness.edge(index).labels[0];
    assert_eq!(
        (new.text.as_str(), &new.font, new.text_color),
        ("Line", &font, RED)
    );

    // Labels there already keep theirs.
    harness.app.selected = Selection::node(kept.clone());
    harness.type_label("Retyped");
    let retyped = &harness.graphics(&kept).labels[0];
    assert_eq!(retyped.text, "Retyped");
    assert_eq!(retyped.font, yed::Font::default());
    assert_eq!(retyped.text_color, yed::Color::BLACK);
}

#[test]
fn labels_added_in_the_side_panel_or_its_dialog_get_the_font_last_set() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let styled = document::add_node(graphml, &labelled_box("Styled"));
    let in_field = document::add_node(graphml, &box_at(300.0, 0.0));
    let in_dialog = document::add_node(graphml, &box_at(600.0, 0.0));
    let label = LabelRef {
        item: node(&styled),
        index: 0,
    };
    harness.perform(Action::SetFont(label, FontPart::Size(20)));

    harness.app.selected = Selection::node(in_field.clone());
    let field = harness.empty_field();
    harness.click_at(field);
    harness.type_text("Field");
    harness.press(Modifiers::NONE, Key::Enter);
    harness.app.selected = Selection::node(in_dialog.clone());
    harness.click_text("...");
    harness.run(vec![]);
    harness.type_text("Dialog");
    harness.click_text("OK");

    for (id, text) in [(&in_field, "Field"), (&in_dialog, "Dialog")] {
        let label = &harness.graphics(id).labels[0];
        assert_eq!((label.text.as_str(), label.font.size), (text, 20));
    }
}

#[test]
fn only_new_boxes_get_the_corner_radius_last_set() {
    let mut harness = Harness::new();
    let id = document::add_node(&mut harness.app.graphml, &new_box());
    harness.app.selected = Selection::node(id.clone());
    harness.perform(Action::SetCornerRadius(10.0));
    for (tool, x) in [(Tool::Circle, 20.0), (Tool::Ellipse, 200.0)] {
        let drawn = harness.draw_node(tool, vec2(x, 250.0), vec2(x + 120.0, 310.0));
        assert_eq!(
            harness.graphics(&drawn).kind,
            NodeKind::Shape(ShapeType::Ellipse)
        );
        // Only its graphics, with no radius kept.
        let nodes = &harness.app.graphml.graphs[0].nodes;
        let node = nodes.iter().find(|node| node.id == drawn).unwrap();
        assert_eq!(node.data.len(), 1, "{tool:?}");
    }

    for (radius, shape, x) in [
        (10.0, ShapeType::RoundRectangle, 20.0),
        (4.0, ShapeType::RoundRectangle, 200.0),
        (0.0, ShapeType::Rectangle, 380.0),
    ] {
        harness.app.selected = Selection::node(id.clone());
        harness.perform(Action::SetCornerRadius(radius));
        let drawn = harness.draw_node(Tool::Box, vec2(x, 400.0), vec2(x + 120.0, 460.0));
        assert_eq!(harness.graphics(&drawn).kind, NodeKind::Shape(shape));
        let drawn_radius = document::corner_radius(&harness.app.graphml, &drawn);
        assert_eq!(drawn_radius, Some(radius));
    }
}

#[test]
fn pasting_reproduces_what_was_copied_whatever_was_set_since() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::of([node("n0"), node("n1"), Item::Edge(0)]);
    let nodes = [harness.graphics("n0"), harness.graphics("n1")];
    let edge = harness.edge(0);
    // Cut, so that the paste lands where they were.
    let (copied, _) = harness.clipboard(vec![Event::Cut]);

    // All there is to remember, set on what's left.
    harness.app.selected = Selection::of([node("n2"), node("n3"), Item::Edge(0)]);
    let label = LabelRef {
        item: node("n2"),
        index: 0,
    };
    for action in [
        Action::SetColor(ColorProperty::Fill, Some(RED)),
        Action::SetColor(ColorProperty::Gradient, Some(RED)),
        Action::SetColor(ColorProperty::Line, Some(RED)),
        Action::SetLineWidth(5.0),
        Action::SetLineType(yed::LineType::DashedDotted),
        Action::SetCornerRadius(10.0),
        Action::SetFont(label.clone(), FontPart::Family("Monospaced".to_owned())),
        Action::SetFont(label.clone(), FontPart::Style(yed::FontStyle::Italic)),
        Action::SetFont(label, FontPart::Size(30)),
        Action::SetColor(ColorProperty::Text(0), Some(RED)),
    ] {
        harness.perform(action);
    }
    assert_ne!(harness.app.style, Style::default());

    harness.run(vec![Event::Paste(copied[0].clone())]);
    let pasted = harness.app.selected.clone();
    assert!(pasted.contains_node("n0") && pasted.contains_node("n1"));
    let index = pasted.edges().next().expect("the line was pasted");
    assert_eq!([harness.graphics("n0"), harness.graphics("n1")], nodes);
    assert_eq!(harness.edge(index), edge);
}

#[test]
fn open_neither_restyles_the_file_nor_forgets_what_was_set() {
    let mut harness = Harness::new();
    let id = document::add_node(&mut harness.app.graphml, &new_box());
    harness.app.selected = Selection::node(id);
    harness.perform(Action::SetColor(ColorProperty::Fill, Some(RED)));
    harness.perform(Action::SetLineWidth(3.0));
    harness.app.open(Path::new(FIXTURE)).unwrap();
    assert_eq!(harness.app.graphml, GraphMl::read_file(FIXTURE).unwrap());

    // Below the file's nodes.
    let drawn = harness.draw_node(Tool::Box, vec2(20.0, 500.0), vec2(140.0, 560.0));
    let graphics = harness.graphics(&drawn);
    assert_eq!(graphics.fill.color, Some(RED));
    assert_eq!(graphics.border.width, 3.0);
}

#[test]
fn drawing_the_next_box_applies_the_label_typed() {
    let mut harness = Harness::new();
    harness.press(Modifiers::NONE, Key::B);
    harness.drag(vec2(20.0, 20.0), vec2(140.0, 80.0));
    harness.press(CTRL, Key::Enter);
    harness.run(vec![]);
    harness.type_text("One");
    harness.drag(vec2(20.0, 150.0), vec2(140.0, 210.0));
    assert_eq!(harness.label("n0"), "One");
    assert!(!harness.ctx.text_edit_focused());
    assert_eq!(harness.app.selected_node(), Some("n1"));
    assert_eq!(harness.label("n1"), "");
}

#[test]
fn drawing_the_next_box_applies_the_label_typed_in_the_side_panel() {
    let mut harness = Harness::new();
    let id = type_in_label_field(&mut harness, "Start");
    harness.type_text("Stop");
    harness.app.tool = Tool::Box;
    harness.drag(vec2(20.0, 150.0), vec2(140.0, 210.0));
    harness.run(vec![]);
    assert_eq!(harness.label(&id), "Stop");
    assert!(!harness.ctx.text_edit_focused());
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

/// Adds a box labelled "Start" and selects it. Its border has no color, so its
/// label's is the only black in the side panel. Returns the box's id.
fn select_labelled_box(harness: &mut Harness) -> String {
    let mut graphics = labelled_box("Start");
    graphics.border.color = None;
    let id = document::add_node(&mut harness.app.graphml, &graphics);
    harness.click(vec2(150.0, 60.0));
    id
}

#[test]
fn picking_a_font_in_the_side_panel_sets_it() {
    let mut harness = Harness::new();
    let id = select_labelled_box(&mut harness);
    harness.click_text("Dialog");
    harness.run(vec![]);
    // The search field has the focus.
    harness.type_text("mono");
    harness.press(Modifiers::NONE, Key::Enter);

    assert_eq!(harness.graphics(&id).labels[0].font.family, "Monospaced");
    assert!(!Popup::is_any_open(&harness.ctx));
    let family = fonts::family("Monospaced", &yed::FontStyle::Plain);
    assert_eq!(harness.canvas_text("Start").font_id.family, family);
    let side_panel = harness.side_panel();
    assert!(
        side_panel
            .windows(2)
            .any(|row| row == ["Font", "Monospaced"]),
        "{side_panel:?}"
    );
}

#[test]
fn searching_fonts_selects_no_tools() {
    let mut harness = Harness::new();
    let id = select_labelled_box(&mut harness);
    harness.press(Modifiers::NONE, Key::B);
    harness.click_text("Dialog");
    harness.run(vec![]);
    // Each key is a tool's shortcut.
    for (key, text) in [(Key::C, "c"), (Key::E, "e"), (Key::L, "l")] {
        let press = testing::key(Modifiers::NONE, key);
        harness.run(vec![press, Event::Text(text.to_owned())]);
    }
    assert_eq!(harness.app.tool, Tool::Box);

    harness.press(Modifiers::NONE, Key::Escape);
    assert!(!Popup::is_any_open(&harness.ctx));
    assert_eq!(harness.app.tool, Tool::Box);
    assert_eq!(harness.graphics(&id).labels[0].font.family, "Dialog");
    harness.press(Modifiers::NONE, Key::Escape);
    assert_eq!(harness.app.tool, Tool::Cursor);
}

#[test]
fn picking_a_font_style_in_the_side_panel_sets_it() {
    use yed::FontStyle::{Bold, BoldItalic, Italic, Plain};
    let mut harness = Harness::new();
    let id = select_labelled_box(&mut harness);
    let mut shown = "Plain";
    for (name, style) in [
        ("Bold", Bold),
        ("Italic", Italic),
        ("Bold italic", BoldItalic),
        ("Plain", Plain),
    ] {
        harness.click_text(shown);
        harness.click_text(name);
        assert_eq!(harness.graphics(&id).labels[0].font.style, style);
        assert!(!Popup::is_any_open(&harness.ctx));
        let family = fonts::family("Dialog", &style);
        assert_eq!(
            harness.canvas_text("Start").font_id.family,
            family,
            "{name}"
        );
        shown = name;
    }
}

#[test]
fn typing_a_font_size_in_the_side_panel_sets_it() {
    let mut harness = Harness::new();
    let id = select_labelled_box(&mut harness);
    harness.click_text("12");
    // The size is selected, so typing replaces it.
    harness.type_text("20");
    assert_eq!(harness.graphics(&id).labels[0].font.size, 12);
    harness.press(Modifiers::NONE, Key::Enter);

    assert_eq!(harness.graphics(&id).labels[0].font.size, 20);
    assert_eq!(harness.canvas_text("Start").font_id.size, 20.0);
    let side_panel = harness.side_panel();
    assert!(
        side_panel.windows(2).any(|row| row == ["Font size", "20"]),
        "{side_panel:?}"
    );
}

#[test]
fn escape_in_the_font_size_field_keeps_the_size_and_the_tool() {
    let mut harness = Harness::new();
    let id = select_labelled_box(&mut harness);
    harness.press(Modifiers::NONE, Key::B);
    harness.click_text("12");
    harness.type_text("20");
    harness.press(Modifiers::NONE, Key::Escape);
    harness.run(vec![]);

    assert_eq!(harness.graphics(&id).labels[0].font.size, 12);
    assert_eq!(harness.app.tool, Tool::Box);
    harness.press(Modifiers::NONE, Key::Escape);
    assert_eq!(harness.app.tool, Tool::Cursor);
}

#[test]
fn typed_font_sizes_are_kept_in_range() {
    for (typed, size) in [("0", 1), ("-3", 1), ("1000", 500), ("inf", 500)] {
        let mut harness = Harness::new();
        let id = select_labelled_box(&mut harness);
        harness.click_text("12");
        harness.type_text(typed);
        harness.press(Modifiers::NONE, Key::Enter);
        assert_eq!(harness.graphics(&id).labels[0].font.size, size, "{typed}");
    }
}

#[test]
fn font_rows_edit_their_own_label() {
    let mut harness = Harness::new();
    let mut graphics = new_box();
    graphics.labels = vec![yed::Label::new("One"), yed::Label::new("Two")];
    let id = document::add_node(&mut harness.app.graphml, &graphics);
    harness.click(vec2(150.0, 60.0));
    let sizes = harness.texts_at("12");
    let [_, second] = sizes[..] else {
        panic!("expected two font sizes: {sizes:?}");
    };
    harness.click_at(second);
    harness.type_text("20");
    harness.press(Modifiers::NONE, Key::Enter);

    let labels = harness.graphics(&id).labels;
    let sizes: Vec<_> = labels.iter().map(|label| label.font.size).collect();
    assert_eq!(sizes, [12, 20]);
}

/// Runs a frame, returning where the text `text` was painted on the canvas.
fn canvas_text_rect(harness: &mut Harness, text: &str) -> Rect {
    let shapes = harness.run(vec![]);
    let canvas = canvas(&shapes);
    let found = shapes.iter().find_map(|shape| match shape {
        Shape::Text(shape) if shape.galley.text() == text && canvas.contains(shape.pos) => {
            Some(shape.galley.rect.translate(shape.pos.to_vec2()))
        }
        _ => None,
    });
    found.unwrap_or_else(|| panic!("{text:?} wasn't painted on the canvas"))
}

#[test]
fn picking_a_label_position_in_the_side_panel_moves_the_label() {
    let mut harness = Harness::new();
    let id = select_labelled_box(&mut harness);
    let node = Rect::from_min_size(canvas(&harness.run(vec![])).min, vec2(150.0, 80.0))
        .translate(vec2(100.0, 50.0));
    let mut shown = "Center";
    for (name, position) in [
        ("North", yed::LabelPosition::North),
        ("Bottom right", yed::LabelPosition::BottomRight),
        ("Center", yed::LabelPosition::Center),
    ] {
        harness.click_text(shown);
        harness.click_text(name);
        assert_eq!(harness.graphics(&id).labels[0].position(), Some(position));
        assert!(!Popup::is_any_open(&harness.ctx));
        let side_panel = harness.side_panel();
        assert!(
            side_panel.windows(2).any(|row| row == ["Position", name]),
            "{side_panel:?}"
        );
        // yEd hasn't sized the new label, so it goes 4 from the node, and its
        // text 2 more, as yEd would put it.
        let text = canvas_text_rect(&mut harness, "Start");
        let (at, expected) = match position {
            yed::LabelPosition::North => (text.center_bottom(), node.center_top() - vec2(0.0, 6.0)),
            yed::LabelPosition::BottomRight => {
                (text.right_bottom(), node.right_bottom() - vec2(6.0, 6.0))
            }
            _ => (text.center(), node.center()),
        };
        assert!(
            (at - expected).length() < 0.01,
            "{name}: {at:?} != {expected:?}"
        );
        shown = name;
    }
}

#[test]
fn the_position_list_shows_every_position() {
    let mut harness = Harness::new();
    select_labelled_box(&mut harness);
    harness.click_text("Center");
    let listed: Vec<_> = harness
        .side_panel()
        .into_iter()
        .skip_while(|text| text != "Top left")
        .collect();
    #[rustfmt::skip]
    assert_eq!(listed, [
        "Top left", "Top", "Top right",
        "Left", "Center", "Right",
        "Bottom left", "Bottom", "Bottom right",
        "Northwest", "North", "Northeast",
        "West", "East",
        "Southwest", "South", "Southeast",
    ]);
    // Clicks reach the last one, so the list doesn't scroll.
    harness.click_text("Southeast");
    assert!(!Popup::is_any_open(&harness.ctx));
}

#[test]
fn labels_placed_elsewhere_read_as_custom() {
    let mut harness = Harness::new();
    let id = document::add_node(&mut harness.app.graphml, &labelled_box("Start"));
    let realizer = harness.app.graphml.graphs[0].nodes[0].data[0]
        .element_mut()
        .unwrap();
    let label = realizer.child_mut("NodeLabel").unwrap();
    label.set_attr("modelName", "free");
    label.set_attr("modelPosition", "anywhere");
    harness.click(vec2(150.0, 60.0));
    let side_panel = harness.side_panel();
    assert!(
        side_panel
            .windows(2)
            .any(|row| row == ["Position", "Custom"]),
        "{side_panel:?}"
    );

    harness.click_text("Custom");
    harness.click_text("Center");
    let label = &harness.graphics(&id).labels[0];
    assert_eq!(label.position(), Some(yed::LabelPosition::Center));
}

#[test]
fn clicking_the_text_color_picks_a_new_one() {
    let mut harness = Harness::new();
    let id = select_labelled_box(&mut harness);
    harness.click_text("#000000");
    assert!(harness.app.color_picker.is_open());
    // Labels always have a text color.
    assert!(!harness.painted("No color"));
    // The hex field has the focus, with the text color selected.
    harness.type_text("#FF0000");
    harness.press(Modifiers::NONE, Key::Enter);

    let red = yed::Color::rgb(0xFF, 0x00, 0x00);
    assert_eq!(harness.graphics(&id).labels[0].text_color, red);
    let painted = harness.canvas_text("Start").color;
    assert_eq!(painted, Color32::from_rgb(0xFF, 0x00, 0x00));
}

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
fn open_resets_the_zoom() {
    let mut app = DrwrApp::default();
    app.canvas.set_zoom(2.0);
    app.open(Path::new(FIXTURE)).unwrap();
    assert_eq!(app.canvas.zoom(), 1.0);
}

#[test]
fn middle_dragging_the_canvas_pans_it() {
    let mut harness = Harness::new();
    let id = document::add_node(&mut harness.app.graphml, &new_box());
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
    let id = document::add_node(&mut harness.app.graphml, &new_box());
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
    let id = document::add_node(&mut harness.app.graphml, &new_box());
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

/// Opens the yEd fixture: its edge e0 runs from n0 to n1 through two bends
/// and has a label, and e1 joins n2 and n3.
fn fixture_harness() -> Harness {
    let mut harness = Harness::new();
    harness.app.open(Path::new(FIXTURE)).unwrap();
    harness.run(vec![]);
    harness
}

impl Harness {
    /// The graphics of the edge at `index`.
    fn edge(&self, index: usize) -> yed::EdgeGraphics {
        document::edge_graphics_of(&self.app.graphml, index).expect("the edge has graphics")
    }

    /// The ids of the edges.
    fn edge_ids(&self) -> Vec<&str> {
        let edges = &self.app.graphml.graphs[0].edges;
        edges.iter().filter_map(|edge| edge.id.as_deref()).collect()
    }

    /// Runs a frame, returning the check boxes in the side panel, top down.
    fn checkboxes(&mut self) -> Vec<egui::accesskit::Node> {
        self.widgets_of(egui::accesskit::Role::CheckBox)
    }

    /// Runs a frame, returning the widgets in `role`, top down.
    fn widgets_of(&mut self, role: egui::accesskit::Role) -> Vec<egui::accesskit::Node> {
        self.ctx.enable_accesskit();
        let mut output = self
            .ctx
            .run_ui(RawInput::default(), |ui| self.app.ui(ui, &mut self.frame));
        let widgets = testing::widgets(&mut output);
        output.drop_without_applying_deltas();
        let mut found: Vec<_> = (widgets.into_iter())
            .filter(|widget| widget.role() == role)
            .collect();
        found.sort_by(|a, b| testing::center(a).y.total_cmp(&testing::center(b).y));
        found
    }

    /// Clicks the `index`th number field in the side panel, top down.
    fn click_number_field(&mut self, index: usize) {
        let fields = self.widgets_of(egui::accesskit::Role::SpinButton);
        self.click_at(testing::center(&fields[index]));
    }

    /// Runs a frame, returning the middle of the first label's field in the
    /// side panel, which must be empty.
    fn empty_field(&mut self) -> Pos2 {
        // Beside the field.
        let dots = self.text_at("...");
        let shapes = self.run(vec![]);
        let field = shapes.iter().find_map(|shape| match shape {
            Shape::Text(text) if text.galley.text().is_empty() => {
                let at = text.pos + text.galley.rect.center().to_vec2();
                (at.x < dots.x && (at.y - dots.y).abs() < 8.0).then_some(at)
            }
            _ => None,
        });
        field.expect("the first label's field is empty")
    }
}

fn bend(x: f64, y: f64) -> yed::Point {
    yed::Point { x, y }
}

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
    let unchanged = harness.app.graphml.clone();
    harness.press(Modifiers::NONE, Key::Delete);
    assert_eq!(harness.app.graphml, unchanged);

    harness.app.selected = Selection::node("n1");
    harness.press(Modifiers::NONE, Key::Delete);
    assert_eq!(document::graphics_of(&harness.app.graphml, "n1"), None);
    assert_eq!(harness.edge_ids(), ["e1"]);
    assert!(harness.app.selected.is_empty());
}

#[test]
fn the_side_panels_delete_button_deletes_the_selected_node() {
    let mut harness = fixture_harness();
    assert!(harness.widget("Delete").is_disabled());
    harness.app.selected = Selection::node("n1");
    harness.click_widget("Delete");
    assert_eq!(document::graphics_of(&harness.app.graphml, "n1"), None);
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
    let red = Some(yed::Color::rgb(0xFF, 0x00, 0x00));
    for action in [
        Action::SetLineWidth(2.5),
        Action::SetLineType(yed::LineType::Dotted),
        Action::SetColor(ColorProperty::Line, red),
    ] {
        harness.app.perform(action, &harness.ctx, &harness.frame);
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
    let graphml = &mut harness.app.graphml;
    let left = document::add_node(graphml, &new_box());
    let mut right = new_box();
    right.geometry.x += 300.0;
    let right = document::add_node(graphml, &right);
    harness.drag(vec2(150.0, 60.0), vec2(450.0, 60.0));

    let edge = &harness.app.graphml.graphs[0].edges[0];
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
    let graphml = &harness.app.graphml;
    let edge = &graphml.graphs[0].edges[index];
    let bounds = |id: &str| document::graphics_of(graphml, id).unwrap().geometry;
    yed::Route::new(
        &harness.edge(index).path,
        bounds(&edge.source),
        bounds(&edge.target),
    )
}

/// [`new_box`] moved right by `dx` and down by `dy`.
fn box_at(dx: f64, dy: f64) -> NodeGraphics {
    let mut graphics = new_box();
    graphics.geometry.x += dx;
    graphics.geometry.y += dy;
    graphics
}

/// An edge with yEd's look through `bends`.
fn edge_through(bends: &[(f64, f64)]) -> yed::EdgeGraphics {
    let mut graphics = yed::EdgeGraphics::new(yed::EdgeKind::PolyLine);
    graphics.path.points = bends.iter().map(|&(x, y)| bend(x, y)).collect();
    graphics
}

#[test]
fn the_side_panel_shows_what_the_selected_nodes_share() {
    let mut harness = Harness::new();
    let mut left = labelled_box("Same");
    left.border.width = 2.0;
    let mut right = labelled_box("Same");
    right.geometry.x = 400.0;
    right.geometry.width = 100.0;
    right.fill.color = Some(yed::Color::rgb(0xFF, 0x00, 0x00));
    right.labels.push(yed::Label::new("Second"));
    let graphml = &mut harness.app.graphml;
    document::add_node(graphml, &left);
    document::add_node(graphml, &right);
    harness.click(vec2(150.0, 60.0));
    harness.with_ctrl(|harness| harness.click(vec2(450.0, 60.0)));
    // Blank where they differ: X, Width, Fill and Border width.
    #[rustfmt::skip]
    assert_eq!(harness.side_panel(), [
        "Selected",      "2 nodes",
        "Shape",         "rectangle",
        "X",             "",
        "Y",             "50",
        "Width",         "",
        "Height",        "80",
        // A blank swatch.
        "Fill",
        "Gradient",      "None",
        "Border",        "#000000",
        "Border width",  "",
        "Border style",  "Solid",
        "Corner radius", "0",
        // Only the label both have.
        "Label",         "...", "Same",
        "Font",          "Dialog",
        "Font style",    "Plain",
        "Font size",     "12",
        "Text color",    "#000000",
        "Position",      "Center",
    ]);
}

#[test]
fn nodes_and_lines_selected_together_share_line_rows() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let left = document::add_node(graphml, &labelled_box("Same"));
    let right = document::add_node(graphml, &box_at(300.0, 0.0));
    let mut line = edge_through(&[]);
    line.line.width = 2.0;
    line.labels = vec![yed::Label::new_on_edge("Same")];
    document::add_edge(graphml, &left, &right, &line);
    harness.app.selected = Selection::of([Item::Node(left), Item::Edge(0)]);
    // No fill or label position, which only nodes have.
    #[rustfmt::skip]
    assert_eq!(harness.side_panel(), [
        "Selected",   "1 node, 1 line",
        "Line",       "#000000",
        "Line width", "",
        "Line style", "Solid",
        "Label",      "...", "Same",
        "Font",       "Dialog",
        "Font style", "Plain",
        "Font size",  "12",
        "Text color", "#000000",
    ]);
}

#[test]
fn the_side_panel_shows_what_the_selected_lines_share() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let left = document::add_node(graphml, &new_box());
    let right = document::add_node(graphml, &box_at(300.0, 0.0));
    let below = document::add_node(graphml, &box_at(0.0, 300.0));
    let mut line = edge_through(&[]);
    line.arrows.target = yed::Arrow::None;
    document::add_edge(graphml, &left, &right, &edge_through(&[]));
    document::add_edge(graphml, &left, &below, &line);
    harness.app.selected = Selection::of([Item::Edge(0), Item::Edge(1)]);
    #[rustfmt::skip]
    assert_eq!(harness.side_panel(), [
        "Selected",    "2 lines",
        "Source",      "n0",
        "Target",      "",
        "Line",        "#000000",
        "Line width",  "1",
        "Line style",  "Solid",
        // The check boxes' empty texts.
        "Start arrow", "",
        "End arrow",   "",
        "Label",       "...", "",
    ]);
    // Neither checked nor not where they differ, and then set on both.
    let [start, end] = harness.checkboxes().try_into().unwrap();
    assert_eq!(start.toggled(), Some(egui::accesskit::Toggled::False));
    assert_eq!(end.toggled(), Some(egui::accesskit::Toggled::Mixed));
    harness.click_at(testing::center(&end));
    for index in [0, 1] {
        assert_eq!(harness.edge(index).arrows.target, yed::Arrow::Standard);
    }
}

#[test]
fn values_set_in_the_side_panel_go_to_every_selected_item() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let mut thick = new_box();
    thick.border.width = 2.0;
    let thick = document::add_node(graphml, &thick);
    let right = document::add_node(graphml, &box_at(300.0, 0.0));
    let line = document::add_edge(graphml, &thick, &right, &edge_through(&[]));
    let items = [
        Item::Node(thick.clone()),
        Item::Node(right.clone()),
        Item::Edge(line),
    ];
    harness.app.selected = Selection::of(items);
    // Blank, as the widths differ. The first's width still goes to the rest.
    harness.click_number_field(0);
    harness.type_text("2");
    harness.press(Modifiers::NONE, Key::Enter);
    let widths = [
        harness.graphics(&thick).border.width,
        harness.graphics(&right).border.width,
        harness.edge(line).line.width,
    ];
    assert_eq!(widths, [2.0; 3]);
    assert_eq!(rows(&harness.side_panel(), "Line width"), ["2"]);

    harness.click_text("Solid");
    harness.click_text("Dotted");
    let dotted = yed::LineType::Dotted;
    assert_eq!(harness.graphics(&thick).border.line_type, dotted);
    assert_eq!(harness.graphics(&right).border.line_type, dotted);
    assert_eq!(harness.edge(line).line.line_type, dotted);
}

#[test]
fn colors_picked_in_the_side_panel_go_to_every_selected_node() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let left = document::add_node(graphml, &new_box());
    let mut right = box_at(300.0, 0.0);
    right.fill.color = Some(yed::Color::rgb(0x00, 0x00, 0xFF));
    let right = document::add_node(graphml, &right);
    harness.app.selected = Selection::of([Item::Node(left.clone()), Item::Node(right.clone())]);
    // The blank swatch beside "Fill".
    let fill = harness.text_at("Fill");
    let buttons = harness.widgets_of(egui::accesskit::Role::Button);
    let swatch = (buttons.iter().map(testing::center))
        .find(|at| at.x > fill.x && (at.y - fill.y).abs() < 8.0)
        .expect("a button is beside Fill");
    harness.click_at(swatch);
    assert!(harness.painted("Fill color"));
    harness.run(vec![Event::Text("#FF0000".to_owned())]);
    harness.press(Modifiers::NONE, Key::Enter);
    let red = Some(yed::Color::rgb(0xFF, 0x00, 0x00));
    assert_eq!(harness.graphics(&left).fill.color, red);
    assert_eq!(harness.graphics(&right).fill.color, red);
}

#[test]
fn a_blank_label_field_sets_every_label_only_once_typed_in() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let left = document::add_node(graphml, &labelled_box("Left"));
    let mut right = labelled_box("Right");
    right.geometry.x = 400.0;
    let right = document::add_node(graphml, &right);
    harness.app.selected = Selection::of([Item::Node(left.clone()), Item::Node(right.clone())]);
    let labels = |harness: &Harness| [harness.label(&left), harness.label(&right)];

    let field = harness.empty_field();
    harness.click_at(field);
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(labels(&harness), ["Left", "Right"]);
    harness.click_at(field);
    harness.type_text("Both");
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(labels(&harness), ["Both", "Both"]);
}

#[test]
fn the_label_dialog_sets_every_selected_label_once_changed() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let left = document::add_node(graphml, &labelled_box("Left"));
    // Without a label, which it gets.
    let right = document::add_node(graphml, &box_at(300.0, 0.0));
    harness.app.selected = Selection::of([Item::Node(left.clone()), Item::Node(right.clone())]);
    let labels = |harness: &Harness| [harness.label(&left), harness.label(&right)];

    harness.click_text("...");
    harness.run(vec![]);
    harness.click_text("OK");
    assert!(!harness.app.label_dialog.is_open());
    assert_eq!(labels(&harness), ["Left", ""]);
    harness.click_text("...");
    harness.run(vec![]);
    harness.type_text("Both");
    harness.click_text("OK");
    assert_eq!(labels(&harness), ["Both", "Both"]);
}

#[test]
fn font_changes_go_to_every_selected_label_keeping_the_rest_of_each_font() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let left = document::add_node(graphml, &labelled_box("Left"));
    let mut right = labelled_box("Right");
    right.geometry.x = 400.0;
    right.labels[0].font.family = "Monospaced".to_owned();
    right.labels[0].font.size = 20;
    let right = document::add_node(graphml, &right);
    harness.app.selected = Selection::of([Item::Node(left.clone()), Item::Node(right.clone())]);
    let side_panel = harness.side_panel();
    assert_eq!(rows(&side_panel, "Font"), [""]);
    assert_eq!(rows(&side_panel, "Font size"), [""]);

    // After the border width and corner radius.
    harness.click_number_field(2);
    harness.type_text("16");
    harness.press(Modifiers::NONE, Key::Enter);
    let fonts = [&left, &right].map(|id| harness.graphics(id).labels[0].font.clone());
    let fonts = fonts.map(|font| (font.family, font.size));
    assert_eq!(
        fonts,
        [("Dialog".to_owned(), 16), ("Monospaced".to_owned(), 16)]
    );
}

#[test]
fn only_boxes_together_have_a_corner_radius() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let mut ellipse = box_at(300.0, 0.0);
    ellipse.kind = NodeKind::Shape(ShapeType::Ellipse);
    let items = [new_box(), box_at(0.0, 200.0), ellipse]
        .map(|graphics| Item::Node(document::add_node(graphml, &graphics)));
    harness.app.selected = Selection::of(items[..2].iter().cloned());
    assert_eq!(rows(&harness.side_panel(), "Corner radius"), ["0"]);
    harness.app.selected = Selection::of(items);
    assert_eq!(rows(&harness.side_panel(), "Corner radius"), [""; 0]);
}

#[test]
fn ctrl_arrows_move_everything_selected() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let left = document::add_node(graphml, &new_box());
    let right = document::add_node(graphml, &box_at(300.0, 0.0));
    let below = document::add_node(graphml, &box_at(0.0, 300.0));
    // Between the two moved nodes, and to the one left behind.
    document::add_edge(graphml, &left, &right, &edge_through(&[(325.0, 20.0)]));
    document::add_edge(graphml, &right, &below, &edge_through(&[(600.0, 200.0)]));
    harness.app.selected = Selection::of([Item::Node(left.clone()), Item::Node(right.clone())]);
    harness.press(CTRL, Key::ArrowRight);

    let x = |id: &str| harness.graphics(id).geometry.x;
    assert_eq!([x(&left), x(&right), x(&below)], [101.0, 401.0, 100.0]);
    assert_eq!(harness.edge(0).path.points, [bend(326.0, 20.0)]);
    assert_eq!(harness.edge(1).path.points, [bend(600.0, 200.0)]);
}

#[test]
fn delete_removes_everything_selected() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let left = document::add_node(graphml, &new_box());
    let right = document::add_node(graphml, &box_at(300.0, 0.0));
    let below = document::add_node(graphml, &box_at(0.0, 300.0));
    for (source, target) in [(&left, &right), (&right, &below), (&left, &below)] {
        document::add_edge(graphml, source, target, &edge_through(&[]));
    }
    harness.app.selected = Selection::of([Item::Edge(0), Item::Edge(2)]);
    harness.press(Modifiers::NONE, Key::Delete);
    assert_eq!(harness.edge_ids(), ["e1"]);
    assert!(harness.app.selected.is_empty());

    harness.app.selected = Selection::of([Item::Edge(0), Item::Node(left.clone())]);
    harness.click_widget("Delete");
    assert_eq!(harness.edge_ids(), [""; 0]);
    assert_eq!(document::graphics_of(&harness.app.graphml, &left), None);
    assert!(document::graphics_of(&harness.app.graphml, &right).is_some());
}

/// Drags a box round the fixture's n0 and n1 and the bends of e0 between
/// them, and round n2 and n3, which the canvas doesn't draw.
fn drag_box_round_e0(harness: &mut Harness) {
    harness.drag(vec2(50.0, 5.0), vec2(400.0, 300.0));
}

#[test]
fn boxed_edges_are_counted_and_nudged_with_their_nodes() {
    let mut harness = fixture_harness();
    drag_box_round_e0(&mut harness);
    let boxed = Selection::of([node("n0"), node("n1"), Item::Edge(0)]);
    assert_eq!(harness.app.selected, boxed);
    assert_eq!(rows(&harness.side_panel(), "Selected"), ["2 nodes, 1 line"]);

    harness.press(CTRL, Key::ArrowRight);
    let x = |id: &str| harness.graphics(id).geometry.x;
    assert_eq!([x("n0"), x("n1")], [101.0, 301.0]);
    // Once, though e0 is selected and joins two moved nodes.
    assert_eq!(
        harness.edge(0).path.points,
        [bend(261.0, 20.0), bend(326.0, 20.0)]
    );
}

#[test]
fn delete_removes_the_boxed_nodes_and_edges() {
    let mut harness = fixture_harness();
    drag_box_round_e0(&mut harness);
    harness.press(Modifiers::NONE, Key::Delete);
    for id in ["n0", "n1"] {
        assert_eq!(document::graphics_of(&harness.app.graphml, id), None);
    }
    assert_eq!(harness.edge_ids(), ["e1"]);
    assert!(harness.app.selected.is_empty());
}

#[test]
fn boxed_edges_are_copied_and_pasted_with_their_nodes() {
    let mut harness = fixture_harness();
    drag_box_round_e0(&mut harness);
    let (copied, _) = harness.clipboard(vec![Event::Copy]);
    assert_eq!(parse_copied(copied.clone()).graphs[0].edges.len(), 1);
    harness.run(vec![Event::Paste(copied[0].clone())]);
    let pasted = Selection::of([node("n6"), node("n7"), Item::Edge(2)]);
    assert_eq!(harness.app.selected, pasted);
    assert_eq!(harness.edge_ids(), ["e0", "e1", "e2"]);
}

#[test]
fn font_rows_show_for_the_labels_that_some_selected_items_have() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
    let mut labelled = labelled_box("Box");
    labelled.labels[0].font.size = 20;
    let red = yed::Color::rgb(0xFF, 0x00, 0x00);
    labelled.labels[0].text_color = red;
    let left = document::add_node(graphml, &labelled);
    let right = document::add_node(graphml, &box_at(300.0, 0.0));
    document::add_edge(graphml, &left, &right, &edge_through(&[]));
    // The line, which has no label, first.
    let items = [
        Item::Edge(0),
        Item::Node(left.clone()),
        Item::Node(right.clone()),
    ];
    harness.app.selected = Selection::of(items);
    #[rustfmt::skip]
    assert_eq!(harness.side_panel(), [
        "Selected",   "2 nodes, 1 line",
        "Line",       "#000000",
        "Line width", "1",
        "Line style", "Solid",
        // Blank, as only the box has a label, whose font shows below.
        "Label",      "...", "",
        "Font",       "Dialog",
        "Font style", "Plain",
        "Font size",  "20",
        "Text color", "#FF0000",
    ]);

    // After the line width. The others get no labels.
    harness.click_number_field(1);
    harness.type_text("16");
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(harness.graphics(&left).labels[0].font.size, 16);
    assert_eq!(harness.graphics(&right).labels, []);
    assert_eq!(harness.edge(0).labels, []);
    // The picker starts on the label's color, so OK keeps it.
    harness.click_text("#FF0000");
    assert!(harness.painted("Text color"));
    harness.run(vec![]);
    harness.press(Modifiers::NONE, Key::Enter);
    assert!(!harness.app.color_picker.is_open());
    assert_eq!(harness.graphics(&left).labels[0].text_color, red);
}

fn node(id: &str) -> Item {
    Item::Node(id.to_owned())
}

/// The only text in `copied`, parsed.
fn parse_copied(copied: Vec<String>) -> GraphMl {
    let [text] = copied.as_slice() else {
        panic!("expected one copy: {copied:?}");
    };
    GraphMl::parse(text).unwrap()
}

#[test]
fn cut_copy_and_paste_sit_between_save_and_the_tools() {
    let mut harness = Harness::new();
    let labels = ["Save", "Cut", "Copy", "Paste", "Cursor"];
    let [save, cut, copy, paste, cursor] =
        labels.map(|label| testing::center(&harness.widget(label)).x);
    assert!(save < cut && cut < copy && copy < paste && paste < cursor);
    // Separators either side.
    assert!(cut - save > copy - cut + 4.0);
    assert!(cursor - paste > copy - cut + 4.0);
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
    assert_eq!(document::graphics_of(&harness.app.graphml, "n4"), None);

    harness.run(vec![Event::Paste(copied[0].clone())]);
    assert_eq!(harness.app.selected, Selection::node("n4"));
    assert_eq!(harness.graphics("n4"), before);
}

#[test]
fn cut_and_copy_with_only_lines_selected_do_nothing() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::edge(0);
    let before = harness.app.graphml.clone();
    let (copied, _) = harness.clipboard(vec![Event::Copy, Event::Cut]);
    assert_eq!(copied, [""; 0]);
    assert_eq!(harness.app.graphml, before);
    assert_eq!(harness.app.selected, Selection::edge(0));
}

#[test]
fn pasting_text_other_than_graphml_does_nothing() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::node("n0");
    let before = harness.app.graphml.clone();
    harness.run(vec![Event::Paste("Hello".to_owned())]);
    assert_eq!(harness.app.graphml, before);
    assert_eq!(harness.app.selected, Selection::node("n0"));
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
    assert_eq!(harness.app.graphml.graphs[0].nodes.len(), 1);
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
    assert_eq!(document::graphics_of(&harness.app.graphml, "n6"), None);
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
    assert!(items[0].y < items[1].y && items[1].y < items[2].y);
    let (copied, _) = harness.click_for_clipboard(items[1]);
    assert_eq!(parse_copied(copied).graphs[0].nodes[0].id, "n0");
    harness.run(vec![]);
    assert!(!Popup::is_any_open(&harness.ctx));
}

#[test]
fn the_cut_button_applies_the_label_typed_first() {
    let mut harness = Harness::new();
    let graphml = &mut harness.app.graphml;
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

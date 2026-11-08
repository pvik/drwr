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
        match &self.selected.as_ref()?.item {
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

    app.selected = Some(Selection::node("n0".to_owned()));

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
        selected: Some(Selection::node("n0")),
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

    harness.app.selected = Some(Selection::node("n0".to_owned()));
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
    app.label_dialog.open(label, "Label", "Start");
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
    assert_eq!(harness.app.selected, Some(Selection::node(id.clone())));
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
    assert_eq!(harness.app.selected, Some(Selection::node(id.clone())));
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
    assert_eq!(harness.app.selected, Some(Selection::node(id)));
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
    assert_eq!(harness.app.selected, Some(Selection::node(id)));
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
        self.ctx.enable_accesskit();
        let mut output = self
            .ctx
            .run_ui(RawInput::default(), |ui| self.app.ui(ui, &mut self.frame));
        let widgets = testing::widgets(&mut output);
        output.drop_without_applying_deltas();
        let mut boxes: Vec<_> = (widgets.into_iter())
            .filter(|widget| widget.role() == egui::accesskit::Role::CheckBox)
            .collect();
        boxes.sort_by(|a, b| testing::center(a).y.total_cmp(&testing::center(b).y));
        boxes
    }
}

fn bend(x: f64, y: f64) -> yed::Point {
    yed::Point { x, y }
}

#[test]
fn delete_removes_the_selected_bend_then_its_edge() {
    let mut harness = fixture_harness();
    harness.app.selected = Some(Selection {
        item: Item::Edge(0),
        bend: Some(1),
    });
    harness.press(Modifiers::NONE, Key::Delete);
    assert_eq!(harness.edge(0).path.points, [bend(260.0, 20.0)]);
    assert_eq!(harness.app.selected, Some(Selection::edge(0)));

    harness.press(Modifiers::NONE, Key::Delete);
    assert_eq!(harness.edge_ids(), ["e1"]);
    assert_eq!(harness.app.selected, None);
}

#[test]
fn delete_removes_the_selected_node_with_its_edges() {
    let mut harness = fixture_harness();
    let unchanged = harness.app.graphml.clone();
    harness.press(Modifiers::NONE, Key::Delete);
    assert_eq!(harness.app.graphml, unchanged);

    harness.app.selected = Some(Selection::node("n1"));
    harness.press(Modifiers::NONE, Key::Delete);
    assert_eq!(document::graphics_of(&harness.app.graphml, "n1"), None);
    assert_eq!(harness.edge_ids(), ["e1"]);
    assert_eq!(harness.app.selected, None);
}

#[test]
fn the_side_panel_shows_the_selected_edge() {
    let mut harness = fixture_harness();
    harness.app.selected = Some(Selection::edge(0));
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
    harness.app.selected = Some(Selection::edge(0));
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
    harness.app.selected = Some(Selection::edge(0));
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
    harness.app.selected = Some(Selection::edge(0));
    harness.press(CTRL, Key::ArrowRight);
    assert_eq!(
        harness.edge(0).path.points,
        [bend(261.0, 20.0), bend(326.0, 20.0)]
    );
    harness.app.selected = Some(Selection {
        item: Item::Edge(0),
        bend: Some(1),
    });
    harness.press(CTRL, Key::ArrowDown);
    assert_eq!(
        harness.edge(0).path.points,
        [bend(261.0, 20.0), bend(326.0, 21.0)]
    );
}

#[test]
fn ctrl_enter_edits_the_selected_edges_label_on_the_canvas() {
    let mut harness = fixture_harness();
    harness.app.selected = Some(Selection::edge(0));
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
    assert_eq!(harness.app.selected, Some(Selection::edge(0)));
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

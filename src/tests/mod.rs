mod clipboard;
mod edges;
mod files;
mod keys;
mod labels;
mod multi_selection;
mod nodes;
mod remembered_style;
mod undo;
mod view;

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

/// Drives the whole app, one frame at a time. Its file dialogs wait for
/// [`FileDialog::answer`], as tests never show rfd's.
struct Harness {
    ctx: egui::Context,
    frame: eframe::Frame,
    app: DrwrApp,
}

impl Harness {
    fn new() -> Self {
        let ctx = egui::Context::default();
        let app = DrwrApp::new(Fonts::default());
        app.init(&ctx);
        let mut harness = Harness {
            ctx,
            frame: eframe::Frame::_new_kittest(),
            app,
        };
        harness.run(vec![]);
        harness
    }

    /// Runs a frame with `input`, as eframe does.
    fn run_input(&mut self, input: RawInput) -> egui::FullOutput {
        self.ctx.run_ui(input, |ui| {
            self.app.logic(ui.ctx(), &mut self.frame);
            self.app.ui(ui, &mut self.frame);
        })
    }

    /// Runs a frame with `events`, returning what it painted.
    fn run(&mut self, events: Vec<Event>) -> Vec<Shape> {
        let input = RawInput {
            events,
            ..RawInput::default()
        };
        let mut output = self.run_input(input);
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
        document::graphics_of(self.app.doc.graphml(), id).expect("the node has graphics")
    }

    /// The text of the first label of the node with id `id`.
    fn label(&self, id: &str) -> String {
        let label = LabelRef {
            item: Item::Node(id.to_owned()),
            index: 0,
        };
        document::label_text(self.app.doc.graphml(), &label).expect("the node exists")
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

    /// Runs a frame, returning the widgets it showed.
    fn widgets(&mut self) -> Vec<egui::accesskit::Node> {
        self.ctx.enable_accesskit();
        let mut output = self.run_input(RawInput::default());
        let widgets = testing::widgets(&mut output);
        output.drop_without_applying_deltas();
        widgets
    }

    /// Runs a frame, returning the widget labelled `label`, e.g. a toolbar
    /// button.
    fn widget(&mut self, label: &str) -> egui::accesskit::Node {
        let widgets = self.widgets();
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
        let output = self.run_input(input);
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

    /// The graphics of the edge at `index`.
    fn edge(&self, index: usize) -> yed::EdgeGraphics {
        document::edge_graphics_of(self.app.doc.graphml(), index).expect("the edge has graphics")
    }

    /// The ids of the edges.
    fn edge_ids(&self) -> Vec<&str> {
        let edges = &self.app.doc.graphml().graphs[0].edges;
        edges.iter().filter_map(|edge| edge.id.as_deref()).collect()
    }

    /// Runs a frame, returning the check boxes in the side panel, top down.
    fn checkboxes(&mut self) -> Vec<egui::accesskit::Node> {
        self.widgets_of(egui::accesskit::Role::CheckBox)
    }

    /// Runs a frame, returning the widgets in `role`, top down.
    fn widgets_of(&mut self, role: egui::accesskit::Role) -> Vec<egui::accesskit::Node> {
        let mut found: Vec<_> = (self.widgets().into_iter())
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

    /// Opens the GraphML file at `path`.
    fn open(path: &Path) -> Self {
        let mut harness = Harness::new();
        harness.app.open(path).unwrap();
        harness.run(vec![]);
        harness
    }

    /// Picks `path` in the open file dialog, or cancels it, then runs a frame.
    fn answer(&mut self, path: Option<&Path>) {
        self.app.file_dialog.answer(path.map(Path::to_path_buf));
        self.run(vec![]);
    }

    /// Draws a box with the Box tool, leaving it selected.
    fn draw_box(&mut self) {
        self.press(Modifiers::NONE, Key::B);
        self.drag(vec2(20.0, 20.0), vec2(140.0, 80.0));
        self.press(Modifiers::NONE, Key::Escape);
    }

    /// Runs a frame with `input`, returning what it asked of the window.
    fn commands(&mut self, input: RawInput) -> Vec<egui::ViewportCommand> {
        let mut output = self.run_input(input);
        let root = output.viewport_output.remove(&egui::ViewportId::ROOT);
        output.drop_without_applying_deltas();
        root.map(|root| root.commands).unwrap_or_default()
    }

    /// Clicks `at`, returning what the release asked of the window.
    fn click_for_commands(&mut self, at: Pos2) -> Vec<egui::ViewportCommand> {
        self.run(vec![Event::PointerMoved(at)]);
        self.run(vec![button(at, true)]);
        self.commands(RawInput {
            events: vec![button(at, false)],
            ..RawInput::default()
        })
    }

    /// Clicks File > Quit, returning whether the app asked to close.
    fn quit(&mut self) -> bool {
        self.click_widget("File");
        let shortcut = shortcuts::text(&self.ctx, &Action::Quit).unwrap();
        let quit = testing::center(&self.widget(&format!("Quit {shortcut}")));
        let commands = self.click_for_commands(quit);
        commands.contains(&egui::ViewportCommand::Close)
    }

    /// Presses Ctrl+Q, returning whether the app asked to close.
    fn press_ctrl_q(&mut self) -> bool {
        let input = RawInput {
            events: vec![testing::key(CTRL, Key::Q)],
            ..RawInput::default()
        };
        self.commands(input).contains(&egui::ViewportCommand::Close)
    }

    /// Asks to close the window, as its close button does, returning whether
    /// the app kept it open.
    fn close_window(&mut self) -> bool {
        let mut input = RawInput::default();
        let root = input.viewports.entry(egui::ViewportId::ROOT).or_default();
        root.events.push(egui::ViewportEvent::Close);
        self.commands(input)
            .contains(&egui::ViewportCommand::CancelClose)
    }

    /// Clicks the open prompt's button labelled `label`, returning what the
    /// release asked of the window. The prompt sits below the toolbar, whose
    /// Save button has the same label as the unsaved-changes prompt's.
    fn answer_prompt(&mut self, label: &str) -> Vec<egui::ViewportCommand> {
        let lowest = (self.widgets().iter())
            .filter(|widget| widget.label() == Some(label))
            .map(testing::center)
            .max_by(|a, b| a.y.total_cmp(&b.y));
        let at = lowest.unwrap_or_else(|| panic!("no button is labelled {label:?}"));
        let commands = self.click_for_commands(at);
        // The dialog keeps the keys in the frame it closes in.
        self.run(vec![]);
        commands
    }

    /// Runs a frame, returning whether it asked whether to save changes.
    fn asks_to_save(&mut self) -> bool {
        self.run(vec![]).iter().any(|shape| {
            matches!(shape, Shape::Text(text) if text.galley.text().starts_with("Save changes to"))
        })
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

/// Adds the box from [`new_box`] and selects it, then clicks its border width
/// in the side panel. Returns the box's id.
fn click_border_width(harness: &mut Harness) -> String {
    let id = document::add_node(harness.app.doc.graphml_mut(), &new_box());
    harness.click(vec2(150.0, 60.0));
    harness.click_text("1");
    id
}

/// The side panel's rows of `name` and the value right of it.
fn rows<'a>(side_panel: &'a [String], name: &str) -> Vec<&'a str> {
    let rows = side_panel.windows(2).filter(|row| row[0] == name);
    rows.map(|row| row[1].as_str()).collect()
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
    let id = document::add_node(harness.app.doc.graphml_mut(), &labelled_box(text));
    harness.click(vec2(150.0, 60.0));
    // The side panel is painted first, so this is its field, not the canvas.
    harness.click_text(text);
    harness.press(CTRL, Key::A);
    id
}

const RED: yed::Color = yed::Color::rgb(0xFF, 0x00, 0x00);

/// Opens the yEd fixture: its edge e0 runs from n0 to n1 through two bends
/// and has a label, and e1 joins n2 and n3.
fn fixture_harness() -> Harness {
    Harness::open(Path::new(FIXTURE))
}

fn bend(x: f64, y: f64) -> yed::Point {
    yed::Point { x, y }
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

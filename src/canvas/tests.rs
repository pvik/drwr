use egui::{Key, Modifiers, RawInput, Shape, vec2};

use super::*;

/// yEd's default node fill, which new boxes get.
const BOX_FILL: Color32 = Color32::from_rgb(0xFF, 0xCC, 0x00);

/// Drives a canvas filling an 800×600 window, one frame at a time.
struct Harness {
    ctx: egui::Context,
    canvas: Canvas,
    tool: Tool,
    graphml: GraphMl,
    time: f64,
    /// The canvas's top-left corner on screen.
    origin: Pos2,
}

impl Harness {
    fn new(tool: Tool) -> Self {
        let mut harness = Harness {
            ctx: egui::Context::default(),
            canvas: Canvas::default(),
            tool,
            graphml: GraphMl::default(),
            time: 0.0,
            origin: Pos2::ZERO,
        };
        let shapes = harness.frame(vec![]);
        harness.origin = filled_rects(&shapes, BACKGROUND)[0].min;
        harness
    }

    /// Runs a frame with `events`, returning what it painted.
    fn frame(&mut self, events: Vec<Event>) -> Vec<Shape> {
        self.time += 0.1;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
            time: Some(self.time),
            events,
            ..RawInput::default()
        };
        let mut output = self.ctx.run_ui(input, |ui| {
            self.canvas.show(ui, self.tool, &mut self.graphml)
        });
        let shapes = std::mem::take(&mut output.shapes);
        output.drop_without_applying_deltas();
        shapes.into_iter().map(|clipped| clipped.shape).collect()
    }

    /// Drags between two points relative to the canvas, returning what the
    /// frame with the release painted.
    fn drag(&mut self, from: Vec2, to: Vec2) -> Vec<Shape> {
        let (from, to) = (self.origin + from, self.origin + to);
        self.frame(vec![Event::PointerMoved(from)]);
        self.frame(vec![button(from, true)]);
        self.frame(vec![Event::PointerMoved(to)]);
        self.frame(vec![button(to, false)])
    }

    /// The geometry of each box in the document.
    fn boxes(&self) -> Vec<yed::Rect> {
        document::node_graphics(&self.graphml)
            .filter(|graphics| graphics.kind == NodeKind::Shape(ShapeType::Rectangle))
            .map(|graphics| graphics.geometry)
            .collect()
    }
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn filled_rects(shapes: &[Shape], fill: Color32) -> Vec<Rect> {
    shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Rect(rect) if rect.fill == fill => Some(rect.rect),
            _ => None,
        })
        .collect()
}

fn geometry(x: f64, y: f64, width: f64, height: f64) -> yed::Rect {
    yed::Rect {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn dragging_with_the_box_tool_adds_a_box() {
    let mut harness = Harness::new(Tool::Box);
    let (from, to) = (vec2(100.0, 50.0), vec2(250.0, 130.0));
    let painted = harness.drag(from, to);

    assert_eq!(harness.boxes(), [geometry(100.0, 50.0, 150.0, 80.0)]);
    let on_screen = Rect::from_two_pos(harness.origin + from, harness.origin + to);
    assert_eq!(filled_rects(&painted, BOX_FILL), [on_screen]);
}

#[test]
fn boxes_can_be_dragged_out_in_any_direction() {
    let mut harness = Harness::new(Tool::Box);
    harness.drag(vec2(250.0, 130.0), vec2(100.0, 50.0));
    harness.drag(vec2(300.0, 60.0), vec2(400.0, 20.0));
    assert_eq!(
        harness.boxes(),
        [
            geometry(100.0, 50.0, 150.0, 80.0),
            geometry(300.0, 20.0, 100.0, 40.0),
        ]
    );
}

#[test]
fn the_box_shows_while_it_is_dragged_out() {
    let mut harness = Harness::new(Tool::Box);
    let (from, to) = (
        harness.origin + vec2(100.0, 50.0),
        harness.origin + vec2(250.0, 130.0),
    );
    harness.frame(vec![Event::PointerMoved(from)]);
    harness.frame(vec![button(from, true)]);
    let painted = harness.frame(vec![Event::PointerMoved(to)]);

    assert_eq!(
        filled_rects(&painted, BOX_FILL),
        [Rect::from_two_pos(from, to)]
    );
    assert_eq!(harness.boxes(), []);
}

#[test]
fn escape_cancels_the_box() {
    let mut harness = Harness::new(Tool::Box);
    let (from, to) = (
        harness.origin + vec2(100.0, 50.0),
        harness.origin + vec2(250.0, 130.0),
    );
    harness.frame(vec![Event::PointerMoved(from)]);
    harness.frame(vec![button(from, true)]);
    harness.frame(vec![Event::PointerMoved(to)]);
    let escape = Event::Key {
        key: Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    let painted = harness.frame(vec![escape]);
    harness.frame(vec![button(to, false)]);

    assert_eq!(filled_rects(&painted, BOX_FILL), []);
    assert_eq!(harness.boxes(), []);
}

#[test]
fn empty_boxes_are_dropped() {
    let mut harness = Harness::new(Tool::Box);
    let corner = vec2(100.0, 50.0);
    harness.drag(corner, corner);
    harness.drag(corner, corner + vec2(150.0, 0.0));
    harness.drag(corner, corner + vec2(0.0, 80.0));
    assert_eq!(harness.boxes(), []);
}

#[test]
fn corners_are_where_the_button_went_down_and_up() {
    let mut harness = Harness::new(Tool::Box);
    let (from, to) = (
        harness.origin + vec2(100.0, 50.0),
        harness.origin + vec2(250.0, 130.0),
    );
    harness.frame(vec![Event::PointerMoved(from)]);
    // The pointer moves on in the same frame as the press and the release.
    harness.frame(vec![
        button(from, true),
        Event::PointerMoved(from + vec2(20.0, 20.0)),
    ]);
    harness.frame(vec![
        Event::PointerMoved(to),
        button(to, false),
        Event::PointerMoved(to + vec2(30.0, 30.0)),
    ]);
    assert_eq!(harness.boxes(), [geometry(100.0, 50.0, 150.0, 80.0)]);
}

#[test]
fn corners_snap_to_whole_numbers() {
    let mut harness = Harness::new(Tool::Box);
    harness.drag(vec2(100.4, 49.6), vec2(250.7, 130.2));
    assert_eq!(harness.boxes(), [geometry(100.0, 50.0, 151.0, 80.0)]);
}

#[test]
fn other_tools_draw_no_boxes() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.drag(vec2(100.0, 50.0), vec2(250.0, 130.0));
    assert_eq!(harness.boxes(), []);
}

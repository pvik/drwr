use egui::{Key, Modifiers, RawInput, Shape, pos2, vec2};

use super::*;

/// yEd's default node fill, which new boxes get.
const BOX_FILL: Color32 = Color32::from_rgb(0xFF, 0xCC, 0x00);

/// Drives a canvas filling an 800×600 window, one frame at a time.
struct Harness {
    ctx: egui::Context,
    canvas: Canvas,
    tool: Tool,
    graphml: GraphMl,
    selected: Option<String>,
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
            selected: None,
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
            self.canvas
                .show(ui, self.tool, &mut self.graphml, &mut self.selected)
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

    /// Clicks a point relative to the canvas, returning what the frame with
    /// the release painted.
    fn click(&mut self, at: Vec2) -> Vec<Shape> {
        let at = self.origin + at;
        self.frame(vec![Event::PointerMoved(at)]);
        self.frame(vec![button(at, true)]);
        self.frame(vec![button(at, false)])
    }

    /// Adds a box between two corners in graph coordinates, returning its id.
    fn add_box(&mut self, corner: Pos2, opposite: Pos2) -> String {
        document::add_node(&mut self.graphml, &new_box(corner, opposite))
    }

    /// The geometry of each box in the document.
    fn boxes(&self) -> Vec<yed::Rect> {
        document::node_graphics(&self.graphml)
            .filter(|(_, graphics)| graphics.kind == NodeKind::Shape(ShapeType::Rectangle))
            .map(|(_, graphics)| graphics.geometry)
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

fn stroked_rects(shapes: &[Shape], color: Color32) -> Vec<Rect> {
    shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Rect(rect) if rect.stroke.color == color => Some(rect.rect),
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
fn switching_tools_mid_drag_drops_the_box() {
    let mut harness = Harness::new(Tool::Box);
    let (from, to) = (
        harness.origin + vec2(100.0, 50.0),
        harness.origin + vec2(250.0, 130.0),
    );
    harness.frame(vec![Event::PointerMoved(from)]);
    harness.frame(vec![button(from, true)]);
    harness.frame(vec![Event::PointerMoved(to)]);
    harness.tool = Tool::Circle;
    harness.frame(vec![]);
    harness.tool = Tool::Box;
    let painted = harness.frame(vec![]);
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

#[test]
fn drawn_boxes_are_selected() {
    let mut harness = Harness::new(Tool::Box);
    harness.drag(vec2(100.0, 50.0), vec2(250.0, 130.0));
    assert_eq!(harness.selected.as_deref(), Some("n0"));
    harness.drag(vec2(300.0, 60.0), vec2(400.0, 20.0));
    assert_eq!(harness.selected.as_deref(), Some("n1"));
}

#[test]
fn dropped_boxes_keep_the_selection() {
    let mut harness = Harness::new(Tool::Box);
    harness.drag(vec2(100.0, 50.0), vec2(250.0, 130.0));
    harness.drag(vec2(300.0, 60.0), vec2(400.0, 60.0));
    assert_eq!(harness.boxes().len(), 1);
    assert_eq!(harness.selected.as_deref(), Some("n0"));
}

#[test]
fn clicking_with_the_cursor_selects_the_node_clicked() {
    let mut harness = Harness::new(Tool::Cursor);
    let left = harness.add_box(pos2(100.0, 50.0), pos2(250.0, 130.0));
    let right = harness.add_box(pos2(300.0, 50.0), pos2(400.0, 130.0));

    harness.click(vec2(150.0, 100.0));
    assert_eq!(harness.selected, Some(left));
    harness.click(vec2(350.0, 100.0));
    assert_eq!(harness.selected, Some(right));
    harness.click(vec2(275.0, 100.0));
    assert_eq!(harness.selected, None);
}

#[test]
fn clicking_overlapping_nodes_selects_the_top_one() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.add_box(pos2(100.0, 50.0), pos2(250.0, 130.0));
    let top = harness.add_box(pos2(200.0, 100.0), pos2(300.0, 200.0));
    harness.click(vec2(225.0, 115.0));
    assert_eq!(harness.selected, Some(top));
}

#[test]
fn clicks_land_where_the_button_went_up() {
    let mut harness = Harness::new(Tool::Cursor);
    let id = harness.add_box(pos2(100.0, 50.0), pos2(250.0, 130.0));
    let at = harness.origin + vec2(240.0, 100.0);
    harness.frame(vec![Event::PointerMoved(at)]);
    harness.frame(vec![button(at, true)]);
    // The pointer moves off the node in the same frame as the release.
    harness.frame(vec![
        button(at, false),
        Event::PointerMoved(at + vec2(30.0, 0.0)),
    ]);
    assert_eq!(harness.selected, Some(id));
}

#[test]
fn nodes_the_canvas_cannot_draw_cannot_be_clicked() {
    let mut harness = Harness::new(Tool::Cursor);
    let mut svg = NodeGraphics::new(NodeKind::Other("SVGNode".to_owned()));
    svg.geometry = geometry(100.0, 50.0, 150.0, 80.0);
    document::add_node(&mut harness.graphml, &svg);
    harness.click(vec2(150.0, 100.0));
    assert_eq!(harness.selected, None);
}

#[test]
fn the_selected_node_is_outlined_on_top() {
    let mut harness = Harness::new(Tool::Box);
    let (from, to) = (vec2(100.0, 50.0), vec2(250.0, 130.0));
    harness.drag(from, to);
    harness.add_box(pos2(200.0, 100.0), pos2(300.0, 200.0));
    let painted = harness.frame(vec![]);

    let node = Rect::from_two_pos(harness.origin + from, harness.origin + to);
    assert_eq!(
        stroked_rects(&painted, SELECTION.color),
        [node.expand(SELECTION_GAP)]
    );
    let outline = painted
        .iter()
        .position(|shape| matches!(shape, Shape::Rect(rect) if rect.stroke == SELECTION));
    let top_node = painted
        .iter()
        .rposition(|shape| matches!(shape, Shape::Rect(rect) if rect.fill == BOX_FILL));
    assert!(outline.unwrap() > top_node.unwrap());

    harness.tool = Tool::Cursor;
    let painted = harness.click(vec2(500.0, 300.0));
    assert_eq!(stroked_rects(&painted, SELECTION.color), []);
}

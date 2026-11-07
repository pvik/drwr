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
    /// What the canvas asked for, in order.
    actions: Vec<Action>,
    /// The mouse cursor the last frame asked for.
    cursor: CursorIcon,
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
            actions: Vec::new(),
            cursor: CursorIcon::Default,
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
            let actions = self
                .canvas
                .show(ui, self.tool, &mut self.graphml, &mut self.selected);
            self.actions.extend(actions);
        });
        self.cursor = output.platform_output.cursor_icon;
        let shapes = std::mem::take(&mut output.shapes);
        output.drop_without_applying_deltas();
        shapes.into_iter().map(|clipped| clipped.shape).collect()
    }

    /// Drags between two points relative to the canvas, returning what the
    /// frame with the release painted.
    fn drag(&mut self, from: Vec2, to: Vec2) -> Vec<Shape> {
        self.drag_and_hold(from, to);
        self.frame(vec![button(self.origin + to, false)])
    }

    /// Like [`Self::drag`], but keeps the button down.
    fn drag_and_hold(&mut self, from: Vec2, to: Vec2) -> Vec<Shape> {
        let (from, to) = (self.origin + from, self.origin + to);
        self.frame(vec![Event::PointerMoved(from)]);
        self.frame(vec![button(from, true)]);
        self.frame(vec![Event::PointerMoved(to)])
    }

    /// Clicks a point relative to the canvas, returning what the frame with
    /// the release painted.
    fn click(&mut self, at: Vec2) -> Vec<Shape> {
        let at = self.origin + at;
        self.frame(vec![Event::PointerMoved(at)]);
        self.frame(vec![button(at, true)]);
        self.frame(vec![button(at, false)])
    }

    /// Double-clicks a point relative to the canvas.
    fn double_click(&mut self, at: Vec2) {
        let at = self.origin + at;
        self.frame(vec![Event::PointerMoved(at)]);
        for pressed in [true, false, true, false] {
            self.frame(vec![button(at, pressed)]);
        }
    }

    fn press(&mut self, modifiers: Modifiers, key: Key) -> Vec<Shape> {
        self.frame(vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }])
    }

    fn type_text(&mut self, text: &str) -> Vec<Shape> {
        self.frame(vec![Event::Text(text.to_owned())])
    }

    /// Adds a box between two corners in graph coordinates, returning its id.
    fn add_box(&mut self, corner: Pos2, opposite: Pos2) -> String {
        document::add_node(&mut self.graphml, &new_box(corner, opposite))
    }

    /// Adds the box in [`NODE`] with `labels`, returning its id.
    fn add_labelled_box(&mut self, labels: Vec<Label>) -> String {
        let mut graphics = new_box(NODE.min, NODE.max);
        graphics.labels = labels;
        document::add_node(&mut self.graphml, &graphics)
    }

    /// Adds the box in [`NODE`] and selects it, returning its id.
    fn add_selected_box(&mut self) -> String {
        let id = self.add_labelled_box(vec![]);
        self.selected = Some(id.clone());
        id
    }

    /// Edits the first label of the node with id `id`.
    fn edit(&mut self, id: &str) {
        let label = first_label(id);
        let text = document::label_text(&self.graphml, &label).unwrap();
        self.canvas.edit_label(label, text);
        // The editor takes the focus in its first frame, and Escape from the
        // next.
        self.frame(vec![]);
        self.frame(vec![]);
    }

    /// Where the node in [`NODE`] is on screen.
    fn node(&self) -> Rect {
        NODE.translate(self.origin.to_vec2())
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

/// A node's bounds in graph coordinates, for tests of its labels.
const NODE: Rect = Rect::from_min_max(pos2(100.0, 50.0), pos2(250.0, 130.0));

fn first_label(node: &str) -> LabelRef {
    LabelRef {
        node: node.to_owned(),
        index: 0,
    }
}

/// The texts painted, with where they were painted.
fn texts(shapes: &[Shape]) -> Vec<(&str, Rect)> {
    shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Text(text) => {
                let rect = text.galley.rect.translate(text.pos.to_vec2());
                Some((text.galley.text(), rect))
            }
            _ => None,
        })
        .collect()
}

/// Where the text `text` was painted, if it was, once.
fn painted(shapes: &[Shape], text: &str) -> Option<Rect> {
    let found: Vec<_> = texts(shapes)
        .into_iter()
        .filter(|(t, _)| *t == text)
        .collect();
    assert!(
        found.len() < 2,
        "{text:?} was painted {} times",
        found.len()
    );
    found.first().map(|(_, rect)| *rect)
}

/// The smallest rect painted around `inner`, i.e. the frame of the editor
/// showing it.
fn frame_around(shapes: &[Shape], inner: Rect) -> Rect {
    shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Rect(rect) if rect.rect.contains_rect(inner) => Some(rect.rect),
            _ => None,
        })
        .min_by(|a, b| a.area().total_cmp(&b.area()))
        .expect("a frame was painted around the text")
}

fn assert_near(a: Pos2, b: Pos2) {
    assert!((a - b).length() <= 1.0, "{a:?} isn't near {b:?}");
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

#[test]
fn dragging_a_side_or_corner_of_the_selected_node_resizes_it() {
    // The node spans (100, 50) to (250, 130).
    for (grab, resized) in [
        (vec2(100.0, 90.0), geometry(120.0, 50.0, 130.0, 80.0)),
        (vec2(250.0, 90.0), geometry(100.0, 50.0, 170.0, 80.0)),
        (vec2(175.0, 50.0), geometry(100.0, 60.0, 150.0, 70.0)),
        (vec2(175.0, 130.0), geometry(100.0, 50.0, 150.0, 90.0)),
        (vec2(100.0, 50.0), geometry(120.0, 60.0, 130.0, 70.0)),
        (vec2(250.0, 50.0), geometry(100.0, 60.0, 170.0, 70.0)),
        (vec2(100.0, 130.0), geometry(120.0, 50.0, 130.0, 90.0)),
        (vec2(250.0, 130.0), geometry(100.0, 50.0, 170.0, 90.0)),
    ] {
        let mut harness = Harness::new(Tool::Cursor);
        harness.add_selected_box();
        harness.drag(grab, grab + vec2(20.0, 10.0));
        assert_eq!(harness.boxes(), [resized], "{grab:?}");
    }
}

#[test]
fn sides_can_be_grabbed_a_little_either_side() {
    // The right side is at 250, and the outline ends 4 past it. Further in,
    // the drag moves the node instead.
    for (x, dragged) in [
        (244.0, geometry(120.0, 50.0, 150.0, 80.0)),
        (246.0, geometry(100.0, 50.0, 170.0, 80.0)),
        (254.0, geometry(100.0, 50.0, 170.0, 80.0)),
        (256.0, geometry(100.0, 50.0, 150.0, 80.0)),
    ] {
        let mut harness = Harness::new(Tool::Cursor);
        harness.add_selected_box();
        harness.drag(vec2(x, 90.0), vec2(x + 20.0, 90.0));
        assert_eq!(harness.boxes(), [dragged], "{x}");
    }
}

#[test]
fn the_node_is_resized_while_its_side_is_dragged() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.add_selected_box();
    let painted = harness.drag_and_hold(vec2(250.0, 90.0), vec2(300.0, 90.0));

    let resized = harness.node().with_max_x(harness.origin.x + 300.0);
    assert_eq!(filled_rects(&painted, BOX_FILL), [resized]);
    assert_eq!(
        stroked_rects(&painted, SELECTION.color),
        [resized.expand(SELECTION_GAP)]
    );
    // The pointer is off the side, but still dragging it.
    assert_eq!(harness.cursor, CursorIcon::ResizeHorizontal);
    assert_eq!(harness.boxes(), [geometry(100.0, 50.0, 150.0, 80.0)]);
}

/// Where drags grab the node in [`NODE`] to resize it from its right side, and
/// to move it from inside.
const GRABS: [Vec2; 2] = [vec2(250.0, 90.0), vec2(175.0, 90.0)];

#[test]
fn escape_cancels_the_resize_or_move() {
    for from in GRABS {
        let mut harness = Harness::new(Tool::Cursor);
        harness.add_selected_box();
        let to = from + vec2(50.0, 0.0);
        harness.drag_and_hold(from, to);
        let painted = harness.press(Modifiers::NONE, Key::Escape);
        harness.frame(vec![button(harness.origin + to, false)]);

        assert_eq!(
            filled_rects(&painted, BOX_FILL),
            [harness.node()],
            "{from:?}"
        );
        assert_eq!(
            harness.boxes(),
            [geometry(100.0, 50.0, 150.0, 80.0)],
            "{from:?}"
        );
    }
}

#[test]
fn switching_tools_mid_drag_drops_the_resize_or_move() {
    for from in GRABS {
        let mut harness = Harness::new(Tool::Cursor);
        harness.add_selected_box();
        let to = from + vec2(50.0, 0.0);
        harness.drag_and_hold(from, to);
        harness.tool = Tool::Box;
        harness.frame(vec![]);
        harness.tool = Tool::Cursor;
        let painted = harness.frame(vec![]);
        harness.frame(vec![button(harness.origin + to, false)]);

        assert_eq!(
            filled_rects(&painted, BOX_FILL),
            [harness.node()],
            "{from:?}"
        );
        assert_eq!(
            harness.boxes(),
            [geometry(100.0, 50.0, 150.0, 80.0)],
            "{from:?}"
        );
    }
}

#[test]
fn only_the_selected_node_can_be_resized() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.add_labelled_box(vec![]);
    harness.drag(vec2(250.0, 90.0), vec2(270.0, 90.0));
    assert_eq!(harness.boxes(), [geometry(100.0, 50.0, 150.0, 80.0)]);
}

#[test]
fn the_box_tool_draws_boxes_from_the_sides_of_the_selected_node() {
    let mut harness = Harness::new(Tool::Box);
    harness.add_selected_box();
    harness.drag(vec2(250.0, 90.0), vec2(300.0, 150.0));
    assert_eq!(
        harness.boxes(),
        [
            geometry(100.0, 50.0, 150.0, 80.0),
            geometry(250.0, 90.0, 50.0, 60.0),
        ]
    );
}

#[test]
fn clicking_a_side_keeps_the_selection() {
    let mut harness = Harness::new(Tool::Cursor);
    let id = harness.add_selected_box();
    // On the outline, which is outside the node.
    harness.click(vec2(254.0, 90.0));
    assert_eq!(harness.selected, Some(id));
    assert_eq!(harness.boxes(), [geometry(100.0, 50.0, 150.0, 80.0)]);
}

#[test]
fn resized_nodes_keep_a_minimum_size() {
    // Each corner is dragged past the opposite one.
    let mut harness = Harness::new(Tool::Cursor);
    harness.add_selected_box();
    harness.drag(vec2(250.0, 130.0), vec2(50.0, 20.0));
    assert_eq!(harness.boxes(), [geometry(100.0, 50.0, 1.0, 1.0)]);
    // The corners overlap at this size, with the bottom right one on top.
    harness.drag(vec2(101.0, 51.0), vec2(150.0, 90.0));
    assert_eq!(harness.boxes(), [geometry(100.0, 50.0, 50.0, 40.0)]);

    let mut harness = Harness::new(Tool::Cursor);
    harness.add_selected_box();
    harness.drag(vec2(100.0, 50.0), vec2(300.0, 200.0));
    assert_eq!(harness.boxes(), [geometry(249.0, 129.0, 1.0, 1.0)]);
}

#[test]
fn dragged_sides_move_by_whole_numbers() {
    let mut harness = Harness::new(Tool::Cursor);
    let mut graphics = new_box(NODE.min, NODE.max);
    graphics.geometry = geometry(100.5, 50.0, 150.25, 80.0);
    harness.selected = Some(document::add_node(&mut harness.graphml, &graphics));
    // From near the right side, at 250.75, by 20.5.
    harness.drag(vec2(250.4, 90.0), vec2(270.9, 90.0));
    assert_eq!(harness.boxes(), [geometry(100.5, 50.0, 171.25, 80.0)]);
}

#[test]
fn resizes_are_from_where_the_button_went_down_to_where_it_went_up() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.add_selected_box();
    let (from, to) = (
        harness.origin + vec2(250.0, 90.0),
        harness.origin + vec2(270.0, 90.0),
    );
    harness.frame(vec![Event::PointerMoved(from)]);
    // The pointer moves on in the same frame as the press and the release.
    harness.frame(vec![
        button(from, true),
        Event::PointerMoved(from + vec2(5.0, 0.0)),
    ]);
    harness.frame(vec![
        Event::PointerMoved(to),
        button(to, false),
        Event::PointerMoved(to + vec2(30.0, 0.0)),
    ]);
    assert_eq!(harness.boxes(), [geometry(100.0, 50.0, 170.0, 80.0)]);
}

#[test]
fn hovering_a_side_or_corner_shows_which_way_it_resizes() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.add_selected_box();
    harness.frame(vec![]);
    for (at, cursor) in [
        (vec2(100.0, 90.0), CursorIcon::ResizeHorizontal),
        (vec2(250.0, 90.0), CursorIcon::ResizeHorizontal),
        (vec2(175.0, 50.0), CursorIcon::ResizeVertical),
        (vec2(175.0, 130.0), CursorIcon::ResizeVertical),
        (vec2(100.0, 50.0), CursorIcon::ResizeNwSe),
        (vec2(250.0, 130.0), CursorIcon::ResizeNwSe),
        (vec2(250.0, 50.0), CursorIcon::ResizeNeSw),
        (vec2(100.0, 130.0), CursorIcon::ResizeNeSw),
    ] {
        harness.frame(vec![Event::PointerMoved(harness.origin + at)]);
        assert_eq!(harness.cursor, cursor, "{at:?}");
    }
}

#[test]
fn resizing_or_moving_applies_the_label_typed() {
    // From a corner, and from inside, clear of the editor across the middle.
    for (from, dragged) in [
        (vec2(250.0, 130.0), geometry(100.0, 50.0, 170.0, 90.0)),
        (vec2(175.0, 65.0), geometry(120.0, 60.0, 150.0, 80.0)),
    ] {
        let mut harness = Harness::new(Tool::Cursor);
        let id = harness.add_labelled_box(vec![Label::new("Start")]);
        harness.selected = Some(id.clone());
        harness.edit(&id);
        harness.type_text("Stop");
        harness.drag(from, from + vec2(20.0, 10.0));
        assert_eq!(
            harness.actions,
            [Action::SetLabel(first_label(&id), "Stop".to_owned())],
            "{from:?}"
        );
        assert!(harness.canvas.editor.is_none());
        assert_eq!(harness.boxes(), [dragged], "{from:?}");
    }
}

#[test]
fn labels_in_the_middle_stay_there_while_resizing() {
    let mut harness = Harness::new(Tool::Cursor);
    let mut middle = Label::new("Middle");
    // Where yEd puts it in the middle of the node.
    middle.bounds = geometry(45.0, 31.0, 60.0, 18.0);
    harness.selected = Some(harness.add_labelled_box(vec![middle]));
    let to = vec2(300.0, 90.0);
    let resized = harness.node().with_max_x(harness.origin.x + to.x);
    let middle = |shapes: &[Shape]| painted(shapes, "Middle").unwrap().center();

    let shapes = harness.drag_and_hold(vec2(250.0, 90.0), to);
    assert_near(middle(&shapes), resized.center());
    let shapes = harness.frame(vec![button(harness.origin + to, false)]);
    assert_near(middle(&shapes), resized.center());
}

#[test]
fn dragging_the_selected_node_moves_it() {
    let mut harness = Harness::new(Tool::Cursor);
    let id = harness.add_selected_box();
    harness.drag(vec2(175.0, 90.0), vec2(205.0, 130.0));
    assert_eq!(harness.boxes(), [geometry(130.0, 90.0, 150.0, 80.0)]);
    assert_eq!(harness.selected, Some(id));
}

#[test]
fn the_node_moves_with_its_labels_while_it_is_dragged() {
    let mut harness = Harness::new(Tool::Cursor);
    let mut label = Label::new("Placed");
    label.bounds = geometry(10.0, 5.0, 60.0, 20.0);
    harness.selected = Some(harness.add_labelled_box(vec![label]));
    harness.drag_and_hold(vec2(175.0, 90.0), vec2(185.0, 90.0));
    // Off where the node was, but still dragging it.
    let shapes = harness.frame(vec![Event::PointerMoved(
        harness.origin + vec2(375.0, 290.0),
    )]);

    let moved = harness.node().translate(vec2(200.0, 200.0));
    assert_eq!(filled_rects(&shapes, BOX_FILL), [moved]);
    assert_eq!(
        stroked_rects(&shapes, SELECTION.color),
        [moved.expand(SELECTION_GAP)]
    );
    let placed = painted(&shapes, "Placed").unwrap();
    assert_near(placed.center(), moved.min + vec2(40.0, 15.0));
    assert_eq!(harness.cursor, CursorIcon::Move);
    assert_eq!(harness.boxes(), [geometry(100.0, 50.0, 150.0, 80.0)]);
}

#[test]
fn only_drags_from_inside_the_selected_node_move_it() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.add_box(pos2(300.0, 50.0), pos2(400.0, 130.0));
    let id = harness.add_selected_box();
    // From the other node, and from the empty canvas.
    for from in [vec2(350.0, 90.0), vec2(500.0, 300.0)] {
        harness.drag(from, from + vec2(30.0, 40.0));
    }
    assert_eq!(
        harness.boxes(),
        [
            geometry(300.0, 50.0, 100.0, 80.0),
            geometry(100.0, 50.0, 150.0, 80.0),
        ]
    );
    assert_eq!(harness.selected, Some(id));
}

#[test]
fn the_selected_node_moves_even_from_under_another() {
    let mut harness = Harness::new(Tool::Cursor);
    let id = harness.add_selected_box();
    harness.add_box(pos2(150.0, 70.0), pos2(200.0, 110.0));
    harness.drag(vec2(175.0, 90.0), vec2(205.0, 130.0));
    assert_eq!(
        harness.boxes(),
        [
            geometry(130.0, 90.0, 150.0, 80.0),
            geometry(150.0, 70.0, 50.0, 40.0),
        ]
    );
    assert_eq!(harness.selected, Some(id));
}

#[test]
fn presses_that_barely_move_still_click() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.add_selected_box();
    let top = harness.add_box(pos2(150.0, 70.0), pos2(200.0, 110.0));
    harness.drag(vec2(175.0, 90.0), vec2(179.0, 90.0));
    assert_eq!(harness.selected, Some(top));
    assert_eq!(harness.boxes()[0], geometry(100.0, 50.0, 150.0, 80.0));
}

#[test]
fn moves_are_from_where_the_button_went_down_to_where_it_went_up() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.add_selected_box();
    let (from, to) = (
        harness.origin + vec2(175.0, 90.0),
        harness.origin + vec2(205.0, 130.0),
    );
    harness.frame(vec![Event::PointerMoved(from)]);
    // The pointer moves on in the same frame as the press and the release.
    harness.frame(vec![
        button(from, true),
        Event::PointerMoved(from + vec2(5.0, 0.0)),
    ]);
    // Far enough to tell the drag from a click.
    harness.frame(vec![Event::PointerMoved(from + vec2(10.0, 0.0))]);
    harness.frame(vec![
        Event::PointerMoved(to),
        button(to, false),
        Event::PointerMoved(to + vec2(30.0, 0.0)),
    ]);
    assert_eq!(harness.boxes(), [geometry(130.0, 90.0, 150.0, 80.0)]);
}

#[test]
fn nodes_move_by_whole_numbers() {
    let mut harness = Harness::new(Tool::Cursor);
    let mut graphics = new_box(NODE.min, NODE.max);
    graphics.geometry = geometry(100.5, 50.25, 150.0, 80.0);
    harness.selected = Some(document::add_node(&mut harness.graphml, &graphics));
    // By 30.5 across and 39.6 down.
    harness.drag(vec2(175.4, 90.2), vec2(205.9, 129.8));
    assert_eq!(harness.boxes(), [geometry(131.5, 90.25, 150.0, 80.0)]);
}

#[test]
fn hovering_inside_the_selected_node_shows_it_moves() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.add_box(pos2(300.0, 50.0), pos2(400.0, 130.0));
    harness.add_selected_box();
    harness.frame(vec![]);
    for (at, cursor) in [
        (vec2(175.0, 90.0), CursorIcon::Move),
        (vec2(350.0, 90.0), CursorIcon::Default),
        (vec2(500.0, 300.0), CursorIcon::Default),
    ] {
        harness.frame(vec![Event::PointerMoved(harness.origin + at)]);
        assert_eq!(harness.cursor, cursor, "{at:?}");
    }
}

#[test]
fn borders_are_painted_at_their_width_across_the_outline() {
    let mut harness = Harness::new(Tool::Cursor);
    let mut graphics = new_box(NODE.min, NODE.max);
    graphics.border.width = 6.5;
    graphics.border.color = Some(yed::Color::rgb(0xFF, 0x00, 0x00));
    document::add_node(&mut harness.graphml, &graphics);
    let shapes = harness.frame(vec![]);

    assert_eq!(filled_rects(&shapes, BOX_FILL), [harness.node()]);
    let borders: Vec<_> = shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Rect(rect) if rect.stroke.color == RED => Some(rect),
            _ => None,
        })
        .collect();
    let [border] = borders[..] else {
        panic!("expected one border: {borders:?}");
    };
    assert_eq!(border.rect, harness.node());
    assert_eq!(border.stroke, Stroke::new(6.5, RED));
    assert_eq!(border.stroke_kind, StrokeKind::Middle);
}

const RED: Color32 = Color32::from_rgb(0xFF, 0x00, 0x00);

/// Adds the box in [`NODE`] with a red border of `line_type` and `width`,
/// returning the rects painted in `color`, relative to the node.
fn dashes(line_type: yed::LineType, width: f64, color: Color32) -> Vec<Rect> {
    let mut harness = Harness::new(Tool::Cursor);
    let mut graphics = new_box(NODE.min, NODE.max);
    graphics.border.line_type = line_type;
    graphics.border.width = width;
    graphics.border.color = Some(yed::Color::rgb(0xFF, 0x00, 0x00));
    document::add_node(&mut harness.graphml, &graphics);
    let shapes = harness.frame(vec![]);

    assert!(stroked_rects(&shapes, RED).is_empty());
    assert_eq!(filled_rects(&shapes, BOX_FILL), [harness.node()]);
    let dashes = shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Mesh(mesh) => Some(mesh.vertices.as_chunks::<4>().0),
            _ => None,
        })
        .flatten()
        .filter(|quad| quad.iter().all(|vertex| vertex.color == color))
        .map(|quad| Rect::from_points(&quad.map(|vertex| vertex.pos)));
    // Relative to the node, for readable expectations.
    let node = harness.node().min.to_vec2();
    dashes.map(|dash| dash.translate(-node)).collect()
}

#[test]
fn dashed_borders_are_painted_in_their_width_and_color() {
    let dashes = dashes(yed::LineType::Dashed, 2.0, RED);
    // Clockwise from the top-left corner, as long as four widths and two
    // apart, across the outline.
    assert_eq!(
        dashes[..2],
        [
            Rect::from_min_max(pos2(-1.0, -1.0), pos2(7.0, 1.0)),
            Rect::from_min_max(pos2(11.0, -1.0), pos2(19.0, 1.0)),
        ]
    );
    // The last ends on the left side, below the first.
    assert_eq!(
        dashes.last(),
        Some(&Rect::from_min_max(pos2(-1.0, 1.0), pos2(1.0, 5.0)))
    );
    let outline = NODE.translate(-NODE.min.to_vec2());
    for dash in &dashes {
        let inside = outline.shrink(1.0).intersect(*dash);
        assert!(outline.expand(1.0).contains_rect(*dash), "{dash:?}");
        assert!(inside.width() <= 0.0 || inside.height() <= 0.0, "{dash:?}");
    }
    // 13 each on the top and bottom, which take the corners, and 7 on each
    // side, counting the parts of the dashes going round them.
    assert_eq!(dashes.len(), 40);
}

#[test]
fn dotted_borders_are_square_dots_a_width_apart() {
    // An odd width, whose solid outline egui puts on pixel centers.
    let dashes = dashes(yed::LineType::Dotted, 3.0, RED);
    assert_eq!(
        dashes[..2],
        [
            Rect::from_min_max(pos2(-1.0, -1.0), pos2(2.0, 2.0)),
            Rect::from_min_max(pos2(5.0, -1.0), pos2(8.0, 2.0)),
        ]
    );
}

#[test]
fn borders_thinner_than_a_point_are_dotted_a_point_apart_and_fainter() {
    let faint = RED.gamma_multiply(0.5);
    let dashes = dashes(yed::LineType::Dotted, 0.5, faint);
    assert_eq!(
        dashes[..2],
        [
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Rect::from_min_max(pos2(2.0, 0.0), pos2(3.0, 1.0)),
        ]
    );
}

#[test]
fn dashed_borders_without_a_color_are_not_painted() {
    let mut harness = Harness::new(Tool::Cursor);
    let mut graphics = new_box(NODE.min, NODE.max);
    graphics.border.line_type = yed::LineType::Dashed;
    graphics.border.color = None;
    document::add_node(&mut harness.graphml, &graphics);
    let shapes = harness.frame(vec![]);
    assert!(!shapes.iter().any(|shape| matches!(shape, Shape::Mesh(_))));
    assert_eq!(filled_rects(&shapes, BOX_FILL), [harness.node()]);
}

#[test]
fn labels_are_painted_where_yed_put_them() {
    let mut harness = Harness::new(Tool::Cursor);
    let new = Label::new("New");
    let mut placed = Label::new("Placed");
    placed.bounds = geometry(10.0, 5.0, 60.0, 20.0);
    placed.text_color = yed::Color::rgb(0xFF, 0x00, 0x00);
    placed.font.size = 20;
    let mut right = Label::new("Right");
    right.bounds = geometry(0.0, 60.0, 150.0, 20.0);
    right.alignment = Alignment::Right;
    let mut hidden = Label::new("Hidden");
    hidden.visible = false;
    harness.add_labelled_box(vec![new, placed, right, hidden]);
    let painted_now = harness.frame(vec![]);
    let node = harness.node();

    // New labels have no bounds yet, so they go in the middle of the node.
    let new = painted(&painted_now, "New").unwrap();
    assert_near(new.center(), node.center());
    let placed = painted(&painted_now, "Placed").unwrap();
    assert_near(placed.center(), node.min + vec2(40.0, 15.0));
    let right = painted(&painted_now, "Right").unwrap();
    assert_near(right.right_center(), node.min + vec2(150.0, 70.0));
    assert_eq!(painted(&painted_now, "Hidden"), None);

    let Some(Shape::Text(text)) = painted_now
        .iter()
        .find(|shape| matches!(shape, Shape::Text(text) if text.galley.text() == "Placed"))
    else {
        unreachable!();
    };
    let format = &text.galley.job.sections[0].format;
    assert_eq!(format.color, Color32::from_rgb(0xFF, 0x00, 0x00));
    assert_eq!(format.font_id.size, 20.0);
}

#[test]
fn labels_of_size_0_are_painted_at_size_1() {
    let mut harness = Harness::new(Tool::Cursor);
    let mut tiny = Label::new("Tiny");
    tiny.font.size = 0;
    let id = harness.add_labelled_box(vec![tiny]);
    assert!(painted(&harness.frame(vec![]), "Tiny").is_some());
    harness.edit(&id);
    assert!(painted(&harness.frame(vec![]), "Tiny").is_some());
}

#[test]
fn the_lines_of_a_label_line_up_by_its_alignment() {
    let mut harness = Harness::new(Tool::Cursor);
    let mut left = Label::new("Left\naligned label");
    left.alignment = Alignment::Left;
    harness.add_labelled_box(vec![left, Label::new("Centered\nlabel")]);
    let shapes = harness.frame(vec![]);
    let halign = |label: &str| {
        shapes.iter().find_map(|shape| match shape {
            Shape::Text(text) if text.galley.text() == label => Some(text.galley.job.halign),
            _ => None,
        })
    };
    assert_eq!(halign("Left\naligned label"), Some(Align::LEFT));
    assert_eq!(halign("Centered\nlabel"), Some(Align::Center));
}

#[test]
fn double_clicking_a_node_asks_to_edit_its_label() {
    let mut harness = Harness::new(Tool::Cursor);
    let id = harness.add_labelled_box(vec![]);
    harness.double_click(vec2(150.0, 60.0));
    assert_eq!(harness.actions, [Action::EditLabel]);
    assert_eq!(harness.selected, Some(id));

    harness.actions.clear();
    harness.double_click(vec2(400.0, 300.0));
    assert_eq!(harness.actions, []);
    assert_eq!(harness.selected, None);
}

#[test]
fn double_clicking_with_the_box_tool_edits_no_labels() {
    let mut harness = Harness::new(Tool::Box);
    harness.add_labelled_box(vec![]);
    harness.double_click(vec2(150.0, 60.0));
    assert_eq!(harness.actions, []);
}

#[test]
fn enter_applies_the_label_typed() {
    let mut harness = Harness::new(Tool::Cursor);
    let id = harness.add_labelled_box(vec![Label::new("Start")]);
    harness.edit(&id);
    // The label's text is selected, so typing replaces it.
    harness.type_text("Stop");
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(
        harness.actions,
        [Action::SetLabel(first_label(&id), "Stop".to_owned())]
    );
    assert!(harness.canvas.editor.is_none());
}

#[test]
fn shift_enter_adds_a_line() {
    let mut harness = Harness::new(Tool::Cursor);
    let id = harness.add_labelled_box(vec![Label::new("Start")]);
    harness.edit(&id);
    harness.type_text("One");
    harness.press(Modifiers::SHIFT, Key::Enter);
    harness.type_text("Two");
    assert_eq!(harness.actions, []);
    harness.press(Modifiers::NONE, Key::Enter);
    let applied = Action::SetLabel(first_label(&id), "One\nTwo".to_owned());
    assert_eq!(harness.actions, [applied]);
}

#[test]
fn escape_cancels_the_label_typed() {
    let mut harness = Harness::new(Tool::Cursor);
    let id = harness.add_labelled_box(vec![Label::new("Start")]);
    harness.edit(&id);
    harness.type_text("Stop");
    harness.press(Modifiers::NONE, Key::Escape);
    assert_eq!(harness.actions, []);
    assert!(harness.canvas.editor.is_none());
    let shapes = harness.frame(vec![]);
    assert_eq!(painted(&shapes, "Stop"), None);
    assert!(painted(&shapes, "Start").is_some());
}

#[test]
fn clicking_elsewhere_applies_the_label_typed() {
    let mut harness = Harness::new(Tool::Cursor);
    let id = harness.add_labelled_box(vec![Label::new("Start")]);
    harness.edit(&id);
    harness.type_text("Stop");
    harness.click(vec2(400.0, 300.0));
    assert_eq!(
        harness.actions,
        [Action::SetLabel(first_label(&id), "Stop".to_owned())]
    );
    assert_eq!(harness.selected, None);
}

#[test]
fn drawing_a_box_applies_the_label_typed() {
    let mut harness = Harness::new(Tool::Box);
    let id = harness.add_labelled_box(vec![Label::new("Start")]);
    harness.edit(&id);
    harness.type_text("Stop");
    harness.drag(vec2(300.0, 200.0), vec2(400.0, 260.0));
    assert_eq!(
        harness.actions,
        [Action::SetLabel(first_label(&id), "Stop".to_owned())]
    );
    assert!(harness.canvas.editor.is_none());
    assert_eq!(harness.boxes().len(), 2);
    assert_eq!(harness.selected.as_deref(), Some("n1"));
}

#[test]
fn dragging_in_the_editor_draws_or_moves_nothing() {
    for tool in [Tool::Box, Tool::Cursor] {
        let mut harness = Harness::new(tool);
        let id = harness.add_labelled_box(vec![Label::new("Start")]);
        harness.selected = Some(id.clone());
        harness.edit(&id);
        // Across the editor, which is as wide as the node and in its middle.
        let middle = NODE.center().to_vec2();
        harness.drag(middle - vec2(40.0, 0.0), middle + vec2(40.0, 0.0));
        assert_eq!(harness.actions, [], "{tool:?}");
        assert!(harness.canvas.editor.is_some());
        assert!(harness.ctx.text_edit_focused());
        assert_eq!(harness.boxes(), [geometry(100.0, 50.0, 150.0, 80.0)]);
    }
}

#[test]
fn the_label_edited_shows_only_in_its_editor() {
    let mut harness = Harness::new(Tool::Cursor);
    let id = harness.add_labelled_box(vec![Label::new("Start")]);
    harness.edit(&id);
    harness.type_text("Stop");
    let shapes = harness.frame(vec![]);
    assert_eq!(painted(&shapes, "Start"), None);

    // Over the label and at least as wide as its node.
    let stop = painted(&shapes, "Stop").expect("the editor shows the text typed");
    assert_near(stop.center(), harness.node().center());
    let editor = frame_around(&shapes, stop);
    assert!(editor.width() >= NODE.width(), "{editor:?}");
    assert!(editor.height() < NODE.height(), "{editor:?}");
    assert_near(editor.center(), harness.node().center());
}

#[test]
fn nodes_without_labels_get_one_typed() {
    let mut harness = Harness::new(Tool::Cursor);
    let id = harness.add_labelled_box(vec![]);
    harness.edit(&id);
    harness.type_text("Hello");
    let shapes = harness.frame(vec![]);
    assert_near(
        painted(&shapes, "Hello").unwrap().center(),
        harness.node().center(),
    );
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(
        harness.actions,
        [Action::SetLabel(first_label(&id), "Hello".to_owned())]
    );
}

#[test]
fn the_editor_closes_when_its_node_is_gone() {
    let mut harness = Harness::new(Tool::Cursor);
    let id = harness.add_labelled_box(vec![Label::new("Start")]);
    harness.edit(&id);
    harness.graphml = GraphMl::default();
    harness.frame(vec![]);
    assert!(harness.canvas.editor.is_none());
    assert_eq!(harness.actions, []);
}

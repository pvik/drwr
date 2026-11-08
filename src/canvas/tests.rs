use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use egui::epaint::{ClippedShape, ColorMode, Primitive, Vertex};
use egui::{Key, Modifiers, RawInput, Shape, ViewportId, pos2, vec2};

use super::*;
use crate::fonts::Fonts;
use crate::testing::{self, button, middle_button};

/// yEd's default node fill, which new boxes get.
const BOX_FILL: Color32 = Color32::from_rgb(0xFF, 0xCC, 0x00);

/// Drives a canvas filling an 800×600 window, one frame at a time.
struct Harness {
    ctx: egui::Context,
    canvas: Canvas,
    tool: Tool,
    graphml: GraphMl,
    selected: Option<Selection>,
    /// What the canvas asked for, in order.
    actions: Vec<Action>,
    /// The mouse cursor the last frame asked for.
    cursor: CursorIcon,
    /// How soon the last frame asked for the next.
    repaint_delay: Duration,
    time: f64,
    /// The canvas's top-left corner on screen.
    origin: Pos2,
}

impl Harness {
    fn new(tool: Tool) -> Self {
        let ctx = egui::Context::default();
        ctx.add_font_provider(Arc::new(Fonts::default()));
        let mut harness = Harness {
            ctx,
            canvas: Canvas::default(),
            tool,
            graphml: GraphMl::default(),
            selected: None,
            actions: Vec::new(),
            cursor: CursorIcon::Default,
            repaint_delay: Duration::MAX,
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
        self.repaint_delay = output.viewport_output[&ViewportId::ROOT].repaint_delay;
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

    /// Drags between two points relative to the canvas with the middle
    /// button, returning what the frame with the release painted.
    fn middle_drag(&mut self, from: Vec2, to: Vec2) -> Vec<Shape> {
        let (from, to) = (self.origin + from, self.origin + to);
        self.frame(vec![Event::PointerMoved(from)]);
        self.frame(vec![middle_button(from, true)]);
        self.frame(vec![Event::PointerMoved(to)]);
        self.frame(vec![middle_button(to, false)])
    }

    /// Turns the wheel by `lines` notches over a point relative to the
    /// canvas, returning what was painted once egui had applied it all.
    fn wheel(&mut self, at: Vec2, lines: f32) -> Vec<Shape> {
        self.frame(vec![
            Event::PointerMoved(self.origin + at),
            testing::wheel(lines),
        ]);
        // egui spreads a notch over a few frames.
        let mut shapes = Vec::new();
        for _ in 0..5 {
            shapes = self.frame(vec![]);
        }
        shapes
    }

    /// Zooms by `factor` about the canvas's top-left corner, as a pinch
    /// there does, so graph points end up `factor` times as far from it.
    fn zoom(&mut self, factor: f32) {
        self.frame(vec![Event::PointerMoved(self.origin), Event::Zoom(factor)]);
    }

    fn press(&mut self, modifiers: Modifiers, key: Key) -> Vec<Shape> {
        self.frame(vec![testing::key(modifiers, key)])
    }

    fn type_text(&mut self, text: &str) -> Vec<Shape> {
        self.frame(vec![Event::Text(text.to_owned())])
    }

    /// Adds the boxes in [`NODE`] and [`OTHER`], returning their ids.
    fn add_two_boxes(&mut self) -> (String, String) {
        let node = self.add_box(NODE.min, NODE.max);
        (node, self.add_box(OTHER.min, OTHER.max))
    }

    /// Adds an edge with yEd's look from the node with id `source` to the
    /// one with id `target`, through `bends` in graph coordinates, returning
    /// its index.
    fn add_edge(&mut self, source: &str, target: &str, bends: &[Pos2]) -> usize {
        let graphics = new_edge(Tool::Line.arrows().unwrap(), bends);
        document::add_edge(&mut self.graphml, source, target, &graphics)
    }

    /// The ids of the nodes each edge joins, and its graphics.
    fn edges(&self) -> Vec<(String, String, EdgeGraphics)> {
        let edges = document::edge_graphics(&self.graphml);
        let edges =
            edges.map(|(_, edge, graphics)| (edge.source.clone(), edge.target.clone(), graphics));
        edges.collect()
    }

    /// The bends of the edge at `index`.
    fn bends(&self, index: usize) -> Vec<yed::Point> {
        document::edge_graphics_of(&self.graphml, index)
            .unwrap()
            .path
            .points
    }

    /// The id of the selected node.
    fn selected_node(&self) -> Option<&str> {
        match &self.selected.as_ref()?.item {
            Item::Node(id) => Some(id),
            Item::Edge(_) => None,
        }
    }

    /// Adds a box between two corners in graph coordinates, returning its id.
    fn add_box(&mut self, corner: Pos2, opposite: Pos2) -> String {
        document::add_node(&mut self.graphml, &new_box(corner, opposite))
    }

    /// Adds an ellipse between two corners in graph coordinates, returning
    /// its id.
    fn add_ellipse(&mut self, corner: Pos2, opposite: Pos2) -> String {
        let graphics = new_shape(ShapeType::Ellipse, corner, opposite);
        document::add_node(&mut self.graphml, &graphics)
    }

    /// Adds the box in [`NODE`] with its corners rounded to `radius`,
    /// returning its id.
    fn add_rounded_box(&mut self, radius: f64) -> String {
        let id = self.add_box(NODE.min, NODE.max);
        document::set_corner_radius(&mut self.graphml, &id, radius);
        id
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
        self.selected = Some(Selection::node(id.clone()));
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

    /// Where the node in [`NODE`] is on screen once [`Self::zoom`]ed by
    /// `factor`.
    fn zoomed_node(&self, factor: f32) -> Rect {
        let corner = |pos: Pos2| self.origin + pos.to_vec2() * factor;
        Rect::from_min_max(corner(NODE.min), corner(NODE.max))
    }

    /// The geometry of each box in the document.
    fn boxes(&self) -> Vec<yed::Rect> {
        self.shapes(ShapeType::Rectangle)
    }

    /// The geometry of each ellipse in the document.
    fn ellipses(&self) -> Vec<yed::Rect> {
        self.shapes(ShapeType::Ellipse)
    }

    /// The geometry of each node of `shape` in the document.
    fn shapes(&self, shape: ShapeType) -> Vec<yed::Rect> {
        document::node_graphics(&self.graphml)
            .filter(|(_, graphics, _)| graphics.kind == NodeKind::Shape(shape.clone()))
            .map(|(_, graphics, _)| graphics.geometry)
            .collect()
    }
}

/// A box with yEd's default look between two corners in graph coordinates.
fn new_box(corner: Pos2, opposite: Pos2) -> NodeGraphics {
    new_shape(ShapeType::Rectangle, corner, opposite)
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

fn paths(shapes: &[Shape]) -> Vec<&PathShape> {
    shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Path(path) => Some(path),
            _ => None,
        })
        .collect()
}

/// The bounds of the polygons painted filled with `fill`, as ellipses are.
fn filled_ellipses(shapes: &[Shape], fill: Color32) -> Vec<Rect> {
    shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Path(path) if path.closed && path.fill == fill => {
                Some(Rect::from_points(&path.points))
            }
            _ => None,
        })
        .collect()
}

/// The vertices of the triangles `shapes` are drawn as, at 1 pixel a point.
fn tessellated(harness: &Harness, shapes: Vec<Shape>) -> Vec<Vertex> {
    let shapes = shapes
        .into_iter()
        .map(|shape| ClippedShape::new(Rect::EVERYTHING, shape))
        .collect();
    let primitives = harness.ctx.tessellate(shapes, 1.0);
    primitives
        .into_iter()
        .flat_map(|primitive| match primitive.primitive {
            Primitive::Mesh(mesh) => mesh.vertices,
            Primitive::Callback(_) => Vec::new(),
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
/// Right of [`NODE`], for tests of edges.
const OTHER: Rect = Rect::from_min_max(pos2(400.0, 50.0), pos2(550.0, 130.0));
/// Below [`OTHER`].
const THIRD: Rect = Rect::from_min_max(pos2(400.0, 250.0), pos2(550.0, 330.0));

fn first_label(node: &str) -> LabelRef {
    LabelRef {
        item: Item::Node(node.to_owned()),
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
    let escape = testing::key(Modifiers::NONE, Key::Escape);
    let painted = harness.frame(vec![escape]);
    harness.frame(vec![button(to, false)]);

    assert_eq!(filled_rects(&painted, BOX_FILL), []);
    assert_eq!(harness.boxes(), []);
}

#[test]
fn switching_tools_mid_drag_drops_the_shape() {
    // To tools that draw nodes, and to one that doesn't.
    for (tool, other) in [
        (Tool::Box, Tool::Circle),
        (Tool::Circle, Tool::Ellipse),
        (Tool::Ellipse, Tool::Line),
    ] {
        let mut harness = Harness::new(tool);
        let (from, to) = (
            harness.origin + vec2(100.0, 50.0),
            harness.origin + vec2(250.0, 130.0),
        );
        harness.frame(vec![Event::PointerMoved(from)]);
        harness.frame(vec![button(from, true)]);
        harness.frame(vec![Event::PointerMoved(to)]);
        harness.tool = other;
        harness.frame(vec![]);
        harness.tool = tool;
        let painted = harness.frame(vec![]);
        harness.frame(vec![button(to, false)]);

        assert_eq!(filled_rects(&painted, BOX_FILL), [], "{tool:?}");
        assert_eq!(filled_ellipses(&painted, BOX_FILL), [], "{tool:?}");
        assert_eq!(harness.boxes(), [], "{tool:?}");
        assert_eq!(harness.ellipses(), [], "{tool:?}");
    }
}

#[test]
fn dragging_with_the_ellipse_tool_adds_an_ellipse() {
    let mut harness = Harness::new(Tool::Ellipse);
    let (from, to) = (vec2(100.0, 50.0), vec2(250.0, 130.0));
    let painted = harness.drag(from, to);

    assert_eq!(harness.ellipses(), [geometry(100.0, 50.0, 150.0, 80.0)]);
    assert_eq!(harness.boxes(), []);
    assert_eq!(harness.selected_node(), Some("n0"));
    let on_screen = Rect::from_two_pos(harness.origin + from, harness.origin + to);
    assert_eq!(filled_ellipses(&painted, BOX_FILL), [on_screen]);
    assert_eq!(filled_rects(&painted, BOX_FILL), []);
}

#[test]
fn the_circle_tool_draws_circles_as_big_as_the_longer_side_of_the_drag() {
    let mut harness = Harness::new(Tool::Circle);
    // Right and down, up and left, right and up, then left and down.
    harness.drag(vec2(100.0, 50.0), vec2(250.0, 130.0));
    harness.drag(vec2(400.0, 300.0), vec2(350.0, 220.0));
    harness.drag(vec2(100.0, 300.0), vec2(180.0, 250.0));
    harness.drag(vec2(500.0, 100.0), vec2(470.0, 200.0));
    assert_eq!(
        harness.ellipses(),
        [
            geometry(100.0, 50.0, 150.0, 150.0),
            geometry(320.0, 220.0, 80.0, 80.0),
            geometry(100.0, 220.0, 80.0, 80.0),
            geometry(400.0, 100.0, 100.0, 100.0),
        ]
    );
}

#[test]
fn clicks_draw_nothing_even_if_they_slip() {
    for tool in [Tool::Box, Tool::Circle, Tool::Ellipse] {
        let mut harness = Harness::new(tool);
        // Across whole numbers, and as far as a click can go.
        harness.drag(vec2(300.4, 200.4), vec2(300.6, 200.6));
        harness.drag(vec2(300.0, 200.0), vec2(304.0, 204.0));
        assert_eq!(harness.boxes(), [], "{tool:?}");
        assert_eq!(harness.ellipses(), [], "{tool:?}");
    }
    // Any further is a drag.
    let mut harness = Harness::new(Tool::Circle);
    harness.drag(vec2(300.0, 200.0), vec2(307.0, 200.0));
    assert_eq!(harness.ellipses(), [geometry(300.0, 200.0, 7.0, 7.0)]);
}

#[test]
fn the_circle_shows_while_it_is_dragged_out() {
    let mut harness = Harness::new(Tool::Circle);
    let painted = harness.drag_and_hold(vec2(100.0, 50.0), vec2(250.0, 130.0));
    let on_screen = Rect::from_min_size(harness.origin + vec2(100.0, 50.0), Vec2::splat(150.0));
    assert_eq!(filled_ellipses(&painted, BOX_FILL), [on_screen]);
    assert_eq!(harness.ellipses(), []);
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
    assert_eq!(harness.selected_node(), Some("n0"));
    harness.drag(vec2(300.0, 60.0), vec2(400.0, 20.0));
    assert_eq!(harness.selected_node(), Some("n1"));
}

#[test]
fn dropped_boxes_keep_the_selection() {
    let mut harness = Harness::new(Tool::Box);
    harness.drag(vec2(100.0, 50.0), vec2(250.0, 130.0));
    harness.drag(vec2(300.0, 60.0), vec2(400.0, 60.0));
    assert_eq!(harness.boxes().len(), 1);
    assert_eq!(harness.selected_node(), Some("n0"));
}

#[test]
fn clicking_with_the_cursor_selects_the_node_clicked() {
    let mut harness = Harness::new(Tool::Cursor);
    let left = harness.add_box(pos2(100.0, 50.0), pos2(250.0, 130.0));
    let right = harness.add_box(pos2(300.0, 50.0), pos2(400.0, 130.0));

    harness.click(vec2(150.0, 100.0));
    assert_eq!(harness.selected, Some(Selection::node(left)));
    harness.click(vec2(350.0, 100.0));
    assert_eq!(harness.selected, Some(Selection::node(right)));
    harness.click(vec2(275.0, 100.0));
    assert_eq!(harness.selected, None);
}

#[test]
fn clicking_overlapping_nodes_selects_the_top_one() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.add_box(pos2(100.0, 50.0), pos2(250.0, 130.0));
    let top = harness.add_box(pos2(200.0, 100.0), pos2(300.0, 200.0));
    harness.click(vec2(225.0, 115.0));
    assert_eq!(harness.selected, Some(Selection::node(top)));
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
    assert_eq!(harness.selected, Some(Selection::node(id)));
}

#[test]
fn clicking_an_ellipse_selects_it_only_inside_its_outline() {
    let mut harness = Harness::new(Tool::Cursor);
    let id = harness.add_ellipse(NODE.min, NODE.max);
    // Near its right end, then in a corner of its bounds.
    harness.click(vec2(245.0, 90.0));
    assert_eq!(harness.selected, Some(Selection::node(id)));
    harness.click(vec2(110.0, 58.0));
    assert_eq!(harness.selected, None);
}

#[test]
fn ellipses_of_negative_size_cannot_be_clicked() {
    let mut harness = Harness::new(Tool::Cursor);
    let mut graphics = new_shape(ShapeType::Ellipse, NODE.min, NODE.max);
    graphics.geometry = geometry(300.0, 200.0, -150.0, -80.0);
    document::add_node(&mut harness.graphml, &graphics);
    harness.click(vec2(225.0, 160.0));
    assert_eq!(harness.selected, None);
}

#[test]
fn the_selected_ellipse_moves_only_from_inside_its_outline() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.selected = Some(Selection::node(harness.add_ellipse(NODE.min, NODE.max)));
    harness.frame(vec![]);
    // A corner of its bounds, clear of the handles.
    let corner = vec2(110.0, 58.0);
    harness.frame(vec![Event::PointerMoved(harness.origin + corner)]);
    assert_eq!(harness.cursor, CursorIcon::Default);
    harness.drag(corner, corner + vec2(30.0, 40.0));
    assert_eq!(harness.ellipses(), [geometry(100.0, 50.0, 150.0, 80.0)]);

    let middle = vec2(175.0, 90.0);
    harness.frame(vec![Event::PointerMoved(harness.origin + middle)]);
    assert_eq!(harness.cursor, CursorIcon::Move);
    harness.drag(middle, middle + vec2(30.0, 40.0));
    assert_eq!(harness.ellipses(), [geometry(130.0, 90.0, 150.0, 80.0)]);
}

#[test]
fn ellipses_are_resized_by_the_handles_on_their_bounds() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.selected = Some(Selection::node(harness.add_ellipse(NODE.min, NODE.max)));
    harness.drag(vec2(250.0, 90.0), vec2(270.0, 100.0));
    assert_eq!(harness.ellipses(), [geometry(100.0, 50.0, 170.0, 80.0)]);

    let painted = harness.frame(vec![]);
    let resized = harness.node().with_max_x(harness.origin.x + 270.0);
    assert_eq!(filled_ellipses(&painted, BOX_FILL), [resized]);
    assert_eq!(
        stroked_rects(&painted, SELECTION.color),
        [resized.expand(SELECTION_GAP)]
    );
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
    assert_eq!(harness.selected, Some(Selection::node(id)));
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
    harness.selected = Some(Selection::node(document::add_node(
        &mut harness.graphml,
        &graphics,
    )));
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
        harness.selected = Some(Selection::node(id.clone()));
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
    harness.selected = Some(Selection::node(harness.add_labelled_box(vec![middle])));
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
    assert_eq!(harness.selected, Some(Selection::node(id)));
}

#[test]
fn the_node_moves_with_its_labels_while_it_is_dragged() {
    let mut harness = Harness::new(Tool::Cursor);
    let mut label = Label::new("Placed");
    label.bounds = geometry(10.0, 5.0, 60.0, 20.0);
    harness.selected = Some(Selection::node(harness.add_labelled_box(vec![label])));
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
    assert_eq!(harness.selected, Some(Selection::node(id)));
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
    assert_eq!(harness.selected, Some(Selection::node(id)));
}

#[test]
fn presses_that_barely_move_still_click() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.add_selected_box();
    let top = harness.add_box(pos2(150.0, 70.0), pos2(200.0, 110.0));
    harness.drag(vec2(175.0, 90.0), vec2(179.0, 90.0));
    assert_eq!(harness.selected, Some(Selection::node(top)));
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
    harness.selected = Some(Selection::node(document::add_node(
        &mut harness.graphml,
        &graphics,
    )));
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

#[test]
fn ellipse_borders_are_painted_across_the_outline() {
    let mut harness = Harness::new(Tool::Cursor);
    let mut graphics = new_shape(ShapeType::Ellipse, NODE.min, NODE.max);
    graphics.border.width = 6.0;
    graphics.border.color = Some(yed::Color::rgb(0xFF, 0x00, 0x00));
    document::add_node(&mut harness.graphml, &graphics);
    let shapes = harness.frame(vec![]);
    let node = harness.node();

    assert_eq!(filled_ellipses(&shapes, BOX_FILL), [node]);
    assert_eq!(filled_rects(&shapes, BOX_FILL), []);
    // The fill and the border on the polygon its dashes would follow.
    let points = line_type::ellipse_points(node, 1.0);
    let fill = PathShape::convex_polygon(points.clone(), BOX_FILL, Stroke::NONE);
    let border = PathShape::closed_line(points, Stroke::new(6.0, RED));
    assert_eq!(paths(&shapes), [&fill, &border]);
}

#[test]
fn round_rectangles_are_painted_with_yeds_corners() {
    let mut harness = Harness::new(Tool::Cursor);
    let graphics = new_shape(ShapeType::RoundRectangle, NODE.min, NODE.max);
    document::add_node(&mut harness.graphml, &graphics);
    let shapes = harness.frame(vec![]);

    assert_eq!(filled_rects(&shapes, BOX_FILL), []);
    // The fill and the border on one polygon, as for ellipses.
    let stroke = Stroke::new(1.0, Color32::BLACK);
    let points = line_type::rounded_rect_points(harness.node(), 4.0, stroke, 1.0);
    let fill = PathShape::convex_polygon(points.clone(), BOX_FILL, Stroke::NONE);
    let border = PathShape::closed_line(points, stroke);
    assert_eq!(paths(&shapes), [&fill, &border]);
}

#[test]
fn corner_radii_scale_with_the_zoom() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.add_rounded_box(10.0);
    harness.zoom(2.0);
    let shapes = harness.frame(vec![]);

    let stroke = Stroke::new(2.0, Color32::BLACK);
    let node = harness.zoomed_node(2.0);
    let points = line_type::rounded_rect_points(node, 20.0, stroke, 1.0);
    assert_eq!(paths(&shapes)[0].points, points);
}

#[test]
fn clicks_on_a_rounded_corner_miss_the_box() {
    let mut harness = Harness::new(Tool::Cursor);
    let id = harness.add_rounded_box(20.0);
    let corner = NODE.min.to_vec2();
    // Outside the rounding, though inside the bounds.
    harness.click(corner + vec2(3.0, 3.0));
    assert_eq!(harness.selected, None);
    for at in [vec2(7.0, 7.0), vec2(0.5, 20.0), vec2(20.0, 0.5)] {
        harness.selected = None;
        harness.click(corner + at);
        assert_eq!(harness.selected_node(), Some(id.as_str()), "{at:?}");
    }
}

#[test]
fn the_selected_box_moves_only_from_inside_its_rounded_corners() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.selected = Some(Selection::node(harness.add_rounded_box(30.0)));
    harness.frame(vec![]);
    // Outside the rounding, clear of the handles.
    let corner = vec2(108.0, 58.0);
    harness.frame(vec![Event::PointerMoved(harness.origin + corner)]);
    assert_eq!(harness.cursor, CursorIcon::Default);
    harness.drag(corner, corner + vec2(30.0, 40.0));
    let boxes = harness.shapes(ShapeType::RoundRectangle);
    assert_eq!(boxes, [geometry(100.0, 50.0, 150.0, 80.0)]);

    let inside = vec2(112.0, 62.0);
    harness.frame(vec![Event::PointerMoved(harness.origin + inside)]);
    assert_eq!(harness.cursor, CursorIcon::Move);
    harness.drag(inside, inside + vec2(30.0, 40.0));
    let boxes = harness.shapes(ShapeType::RoundRectangle);
    assert_eq!(boxes, [geometry(130.0, 90.0, 150.0, 80.0)]);
}

#[test]
fn dashed_rounded_borders_run_round_the_corners_from_the_top_of_the_left_side() {
    let mut harness = Harness::new(Tool::Cursor);
    let mut graphics = new_box(NODE.min, NODE.max);
    graphics.border.line_type = yed::LineType::Dashed;
    graphics.border.width = 2.0;
    graphics.border.color = Some(yed::Color::rgb(0xFF, 0x00, 0x00));
    let id = document::add_node(&mut harness.graphml, &graphics);
    document::set_corner_radius(&mut harness.graphml, &id, 20.0);
    let shapes = harness.frame(vec![]);

    let outline = line_type::rounded_rect_points(harness.node(), 20.0, Stroke::new(2.0, RED), 1.0);
    let (straight, curved) = (quads(&shapes, RED), curved_dashes(&shapes, RED));
    // Four widths long and two apart, like Java's from the top of the left
    // side down.
    let start = outline[0];
    let first = Rect::from_min_max(start - vec2(1.0, 0.0), start + vec2(1.0, 8.0));
    assert_eq!(straight[0], first);
    let bounds = Rect::from_points(&outline).expand(1.0);
    for dash in &straight {
        assert!(bounds.contains_rect(*dash), "{dash:?}");
    }
    // Those reaching a corner bend round it, along the outline.
    assert!(!curved.is_empty());
    for &point in curved.iter().flatten() {
        along(&outline, point);
    }
    let count = (perimeter(&outline) / 12.0).ceil() as usize;
    assert_eq!(straight.len() + curved.len(), count);
}

#[test]
fn borders_nearly_as_wide_as_their_ellipse_keep_round_it() {
    for (size, width) in [(vec2(100.0, 30.0), 29.9), (vec2(150.0, 80.0), 79.0)] {
        let mut harness = Harness::new(Tool::Cursor);
        let mut graphics = new_shape(ShapeType::Ellipse, NODE.min, NODE.min + size);
        graphics.border.width = width;
        graphics.border.color = Some(yed::Color::rgb(0xFF, 0x00, 0x00));
        document::add_node(&mut harness.graphml, &graphics);
        let shapes = harness.frame(vec![]);

        let node = Rect::from_min_size(harness.node().min, size);
        // Half the width out from the outline, and a pixel to fade out.
        let border = node.expand(width as f32 / 2.0 + 1.0);
        let vertices = tessellated(&harness, shapes);
        assert!(vertices.iter().any(|vertex| vertex.color == RED));
        for vertex in vertices.iter().filter(|vertex| vertex.color == RED) {
            assert!(border.contains(vertex.pos), "{size:?}: {vertex:?}");
        }
    }
}

#[test]
fn flat_ellipses_show_their_border_in_every_line_type() {
    for line_type in line_type::ALL {
        let mut harness = Harness::new(Tool::Cursor);
        let mut graphics = new_shape(ShapeType::Ellipse, NODE.min, NODE.min + vec2(150.0, 0.0));
        graphics.border.line_type = line_type.clone();
        graphics.border.width = 2.0;
        graphics.border.color = Some(yed::Color::rgb(0xFF, 0x00, 0x00));
        document::add_node(&mut harness.graphml, &graphics);
        let shapes = harness.frame(vec![]);
        let red = shapes.iter().any(|shape| match shape {
            Shape::Rect(rect) => rect.stroke.color == RED,
            Shape::Mesh(mesh) => mesh.vertices.iter().any(|vertex| vertex.color == RED),
            _ => false,
        });
        assert!(red, "{line_type:?}");
    }
}

#[test]
fn ellipses_of_absurd_sizes_are_painted_in_few_triangles() {
    let mut harness = Harness::new(Tool::Cursor);
    for (size, width) in [(1e10, 1.0), (150.0, 1e10)] {
        for line_type in [yed::LineType::Line, yed::LineType::Dashed] {
            let mut graphics = new_shape(ShapeType::Ellipse, NODE.min, NODE.max);
            graphics.geometry.width = size;
            graphics.border.width = width;
            graphics.border.line_type = line_type;
            document::add_node(&mut harness.graphml, &graphics);
        }
    }
    let shapes = harness.frame(vec![]);
    assert!(tessellated(&harness, shapes).len() < 1_000_000);
}

/// Paints a node of `shape` in [`NODE`] with a red, dashed border of
/// `line_type` and `width`, zoomed by `zoom`. Returns what was painted, where
/// the node is, and the pixels per point.
fn dashed_border(
    shape: ShapeType,
    line_type: yed::LineType,
    width: f64,
    zoom: f32,
) -> (Vec<Shape>, Rect, f32) {
    let mut harness = Harness::new(Tool::Cursor);
    harness.ctx.set_zoom_factor(zoom);
    let shapes = harness.frame(vec![]);
    harness.origin = filled_rects(&shapes, BACKGROUND)[0].min;
    let mut graphics = new_shape(shape.clone(), NODE.min, NODE.max);
    graphics.border.line_type = line_type;
    graphics.border.width = width;
    graphics.border.color = Some(yed::Color::rgb(0xFF, 0x00, 0x00));
    document::add_node(&mut harness.graphml, &graphics);
    let shapes = harness.frame(vec![]);

    let solid = shapes.iter().any(|shape| match shape {
        Shape::Rect(rect) => rect.stroke.color == RED,
        Shape::Path(path) => path.stroke.color == ColorMode::Solid(RED),
        _ => false,
    });
    assert!(!solid, "the border was painted solid");
    let filled = match shape {
        ShapeType::Ellipse => filled_ellipses(&shapes, BOX_FILL),
        _ => filled_rects(&shapes, BOX_FILL),
    };
    assert_eq!(filled, [harness.node()]);
    (shapes, harness.node(), harness.ctx.pixels_per_point())
}

/// The bounds of the quads painted in `color`, which dots and dashes along
/// straight lines are.
fn quads(shapes: &[Shape], color: Color32) -> Vec<Rect> {
    shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Mesh(mesh) => Some(mesh.vertices.as_chunks::<4>().0),
            _ => None,
        })
        .flatten()
        .filter(|quad| quad.iter().all(|vertex| vertex.color == color))
        .map(|quad| Rect::from_points(&quad.map(|vertex| vertex.pos)))
        .collect()
}

/// The middles of the dashes painted in `color` along curves, each through
/// the corners it turns at.
fn curved_dashes(shapes: &[Shape], color: Color32) -> Vec<Vec<Pos2>> {
    let clear = Color32::TRANSPARENT;
    let mut dashes: Vec<Vec<Pos2>> = Vec::new();
    for shape in shapes {
        let Shape::Mesh(mesh) = shape else {
            continue;
        };
        // Four vertices across each point of a dash, which triangles join to
        // the next point's.
        let joined: HashSet<u32> = (mesh.indices.as_chunks::<3>().0.iter())
            .filter_map(|triangle| {
                let points = triangle.map(|index| index / 4);
                let (first, last) = (*points.iter().min()?, *points.iter().max()?);
                (last == first + 1).then_some(first)
            })
            .collect();
        for (point, quad) in (0..).zip(mesh.vertices.as_chunks::<4>().0) {
            if quad.map(|vertex| vertex.color) != [clear, color, color, clear] {
                continue;
            }
            let middle = quad[1].pos.lerp(quad[2].pos, 0.5);
            match dashes.last_mut() {
                Some(dash) if point > 0 && joined.contains(&(point - 1)) => dash.push(middle),
                _ => dashes.push(vec![middle]),
            }
        }
    }
    dashes
}

fn perimeter(outline: &[Pos2]) -> f32 {
    let next = outline.iter().cycle().skip(1);
    outline.iter().zip(next).map(|(a, b)| a.distance(*b)).sum()
}

/// The length of the line through `points`.
fn length(points: &[Pos2]) -> f32 {
    let segments = points.windows(2);
    segments.map(|pair| pair[0].distance(pair[1])).sum()
}

/// Whether `v` is in whole numbers, but for rounding errors.
fn whole(v: Vec2) -> bool {
    (v - v.round()).length() < 1e-3
}

/// How far round `outline`, a polygon, `point` is from its first corner.
fn along(outline: &[Pos2], point: Pos2) -> f32 {
    let mut distance = 0.0;
    for (&a, &b) in outline.iter().zip(outline.iter().cycle().skip(1)) {
        if point.distance(a) + point.distance(b) - a.distance(b) < 1e-3 {
            return distance + a.distance(point);
        }
        distance += a.distance(b);
    }
    panic!("{point:?} isn't on the outline");
}

/// The [`quads`] of a red, dashed border of `line_type` and `width` round a
/// node of `shape` in [`NODE`], painted in `color`, relative to the node.
fn dashes(shape: ShapeType, line_type: yed::LineType, width: f64, color: Color32) -> Vec<Rect> {
    let (shapes, node, _) = dashed_border(shape, line_type, width, 1.0);
    // Relative to the node, for readable expectations.
    let offset = node.min.to_vec2();
    let quads = quads(&shapes, color).into_iter();
    quads.map(|quad| quad.translate(-offset)).collect()
}

#[test]
fn dashed_borders_are_painted_in_their_width_and_color() {
    let dashes = dashes(ShapeType::Rectangle, yed::LineType::Dashed, 2.0, RED);
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
    let dashes = dashes(ShapeType::Rectangle, yed::LineType::Dotted, 3.0, RED);
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
    let dashes = dashes(ShapeType::Rectangle, yed::LineType::Dotted, 0.5, faint);
    assert_eq!(
        dashes[..2],
        [
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Rect::from_min_max(pos2(2.0, 0.0), pos2(3.0, 1.0)),
        ]
    );
}

#[test]
fn dashed_ellipses_are_dashed_along_their_outline() {
    // 10 wide, the outline ends 9.3 into the last dash, which stays a dash.
    for width in [2.0, 10.0] {
        let line_type = yed::LineType::Dashed;
        let (shapes, node, _) = dashed_border(ShapeType::Ellipse, line_type, width, 1.0);
        assert_eq!(quads(&shapes, RED), [], "{width}");
        let dashes = curved_dashes(&shapes, RED);
        let outline = line_type::ellipse_points(node, 1.0);
        let perimeter = perimeter(&outline);

        // Four widths long and two apart, from the right end down, until the
        // outline ends.
        let (dash, period) = (4.0 * width as f32, 6.0 * width as f32);
        assert_eq!(dashes.len() as f32, (perimeter / period).ceil(), "{width}");
        assert!(dashes[0][1].y > dashes[0][0].y, "{width}");
        for (i, points) in dashes.iter().enumerate() {
            let start = i as f32 * period;
            let at = along(&outline, points[0]);
            assert!((at - start).abs() < 0.01, "{width}: {i} at {at}");
            let (drawn, expected) = (length(points), dash.min(perimeter - start));
            assert!((drawn - expected).abs() < 0.01, "{width}: {i} is {drawn}");
            for &point in points {
                along(&outline, point);
            }
        }
    }
}

#[test]
fn dots_on_ellipses_are_squares_on_whole_pixels() {
    // Zoomed by egui's keys or the display's scale.
    for zoom in [0.9, 1.0, 1.25, 1.5, 2.0] {
        for width in [0.5, 1.0, 2.5, 3.0] {
            let line_type = yed::LineType::Dotted;
            let (shapes, node, ppp) = dashed_border(ShapeType::Ellipse, line_type, width, zoom);
            let width = width as f32;
            let color = RED.gamma_multiply((width * ppp).min(1.0));
            assert!(curved_dashes(&shapes, color).is_empty());
            let dots = quads(&shapes, color);

            // A unit apart, which is at least a point.
            let unit = width.max(1.0);
            let perimeter = perimeter(&line_type::ellipse_points(node, ppp));
            let expected = (perimeter / (2.0 * unit)).ceil();
            assert_eq!(dots.len() as f32, expected, "{zoom} {width}");
            // As thick as the line, which is at least a pixel.
            let side = (width * ppp).max(1.0).round();
            for dot in &dots {
                let (min, size) = (dot.min.to_vec2() * ppp, dot.size() * ppp);
                let message = format!("{zoom} {width}: {dot:?}");
                assert!(whole(min) && whole(size), "{message}");
                assert_eq!(size.round(), Vec2::splat(side), "{message}");
            }
        }
    }
    // The first, down from the right end.
    let dots = dashes(ShapeType::Ellipse, yed::LineType::Dotted, 3.0, RED);
    assert_eq!(
        dots[0],
        Rect::from_min_max(pos2(148.0, 40.0), pos2(151.0, 43.0))
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
fn new_labels_are_painted_where_their_position_puts_them() {
    let mut harness = Harness::new(Tool::Cursor);
    let positioned = |text, position| {
        let mut label = Label::new(text);
        label.set_position(position);
        label
    };
    let id = harness.add_labelled_box(vec![
        positioned("North", yed::LabelPosition::North),
        positioned("Top left", yed::LabelPosition::TopLeft),
    ]);
    let shapes = harness.frame(vec![]);
    let node = harness.node();

    // Their text 2 inside where yEd will put them, once it sizes them.
    let north = painted(&shapes, "North").unwrap();
    assert_near(north.center_bottom(), node.center_top() - vec2(0.0, 6.0));
    let top_left = painted(&shapes, "Top left").unwrap();
    assert_near(top_left.left_top(), node.left_top() + vec2(6.0, 6.0));

    // So is its editor, clear of the node.
    harness.edit(&id);
    let shapes = harness.frame(vec![]);
    let text = painted(&shapes, "North").unwrap();
    assert_near(text.center(), north.center());
    let editor = frame_around(&shapes, text);
    assert!(editor.max.y < node.min.y, "{editor:?}");
}

#[test]
fn labels_are_painted_in_their_font_family_and_style() {
    let mut harness = Harness::new(Tool::Cursor);
    let mut label = Label::new("Mono");
    label.font.family = "Noto Mono".to_owned();
    label.font.style = yed::FontStyle::Bold;
    let id = harness.add_labelled_box(vec![label]);
    let family = |shapes: &[Shape]| {
        shapes.iter().find_map(|shape| match shape {
            Shape::Text(text) if text.galley.text() == "Mono" => {
                assert!(text.galley.size().x > 0.0, "the label has no glyphs");
                Some(text.galley.job.sections[0].format.font_id.family.clone())
            }
            _ => None,
        })
    };
    let bold_mono = fonts::family("Noto Mono", &yed::FontStyle::Bold);
    assert_eq!(family(&harness.frame(vec![])), Some(bold_mono.clone()));
    // So is its editor.
    harness.edit(&id);
    assert_eq!(family(&harness.frame(vec![])), Some(bold_mono));
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
    assert_eq!(harness.selected, Some(Selection::node(id)));

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
    assert_eq!(harness.selected_node(), Some("n1"));
}

#[test]
fn dragging_in_the_editor_draws_or_moves_nothing() {
    for tool in [Tool::Box, Tool::Cursor] {
        let mut harness = Harness::new(tool);
        let id = harness.add_labelled_box(vec![Label::new("Start")]);
        harness.selected = Some(Selection::node(id.clone()));
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

/// The size of the font `text` was painted in.
fn font_size(shapes: &[Shape], text: &str) -> f32 {
    let size = shapes.iter().find_map(|shape| match shape {
        Shape::Text(shape) if shape.galley.text() == text => {
            Some(shape.galley.job.sections[0].format.font_id.size)
        }
        _ => None,
    });
    size.unwrap_or_else(|| panic!("{text:?} wasn't painted"))
}

#[test]
fn the_wheel_zooms_in_and_out_about_the_pointer() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.add_box(NODE.min, NODE.max);
    let at = vec2(400.0, 300.0);
    let painted = harness.wheel(at, 1.0);

    // As fast as egui zooms with Ctrl held: 40 points a notch, 200 an e-fold.
    let notch = 0.2_f32.exp();
    assert!((harness.canvas.zoom() - notch).abs() < 1e-4);
    // The point under the pointer stays put, and the node grows away from it.
    let pointer = harness.origin + at;
    let zoomed = |pos: Pos2| pointer + (pos - at.to_pos2()) * notch;
    let [node] = filled_rects(&painted, BOX_FILL)[..] else {
        panic!("expected one node");
    };
    assert_near(node.min, zoomed(NODE.min));
    assert_near(node.max, zoomed(NODE.max));

    let painted = harness.wheel(at, -1.0);
    assert!((harness.canvas.zoom() - 1.0).abs() < 1e-4);
    let [node] = filled_rects(&painted, BOX_FILL)[..] else {
        panic!("expected one node");
    };
    assert_near(node.min, harness.node().min);
    assert_near(node.max, harness.node().max);
}

#[test]
fn pinches_and_ctrl_wheel_zoom_too() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.zoom(1.5);
    assert_eq!(harness.canvas.zoom(), 1.5);

    let ctrl_wheel = Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: vec2(0.0, 1.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: testing::CTRL,
    };
    harness.frame(vec![ctrl_wheel]);
    for _ in 0..5 {
        harness.frame(vec![]);
    }
    assert!((harness.canvas.zoom() - 1.5 * 0.2_f32.exp()).abs() < 1e-4);
}

#[test]
fn zoom_stays_within_its_limits() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.zoom(100.0);
    assert_eq!(harness.canvas.zoom(), zoom::MAX);
    harness.zoom(1e-6);
    assert_eq!(harness.canvas.zoom(), zoom::MIN);
    harness.canvas.set_zoom(50.0);
    assert_eq!(harness.canvas.zoom(), zoom::MAX);
}

#[test]
fn the_wheel_zooms_only_over_the_canvas() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.frame(vec![Event::PointerMoved(harness.origin - vec2(2.0, 2.0))]);
    harness.frame(vec![testing::wheel(1.0)]);
    for _ in 0..5 {
        harness.frame(vec![]);
    }
    assert_eq!(harness.canvas.zoom(), 1.0);
}

#[test]
fn setting_the_zoom_keeps_the_middle_of_the_canvas_in_place() {
    let mut harness = Harness::new(Tool::Cursor);
    let canvas = filled_rects(&harness.frame(vec![]), BACKGROUND)[0];
    let middle = (canvas.size() / 2.0).to_pos2();
    harness.add_box(middle - vec2(20.0, 10.0), middle + vec2(20.0, 10.0));
    harness.canvas.set_zoom(2.0);
    let painted = harness.frame(vec![]);

    let [node] = filled_rects(&painted, BOX_FILL)[..] else {
        panic!("expected one node");
    };
    let zoomed = Rect::from_center_size(canvas.center(), vec2(80.0, 40.0));
    assert_near(node.min, zoomed.min);
    assert_near(node.max, zoomed.max);
}

#[test]
fn dragging_with_the_middle_button_pans_the_view() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.add_box(NODE.min, NODE.max);
    let (from, to) = (vec2(400.0, 300.0), vec2(430.0, 280.0));
    let painted = harness.middle_drag(from, to);

    assert_eq!(
        filled_rects(&painted, BOX_FILL),
        [harness.node().translate(to - from)]
    );
    // Only the view moved.
    assert_eq!(harness.boxes(), [geometry(100.0, 50.0, 150.0, 80.0)]);
}

#[test]
fn panning_by_a_distance_moves_the_view_that_far_on_screen() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.add_box(NODE.min, NODE.max);
    harness.canvas.pan_by(vec2(40.0, -20.0));
    let painted = harness.frame(vec![]);

    // The view moves right and up, so the graph shows left and down.
    let moved = vec2(-40.0, 20.0);
    assert_eq!(
        filled_rects(&painted, BOX_FILL),
        [harness.node().translate(moved)]
    );
    assert_eq!(harness.boxes(), [geometry(100.0, 50.0, 150.0, 80.0)]);

    // In screen points, whatever the zoom.
    harness.canvas.set_zoom(2.0);
    let zoomed = filled_rects(&harness.frame(vec![]), BOX_FILL)[0];
    harness.canvas.pan_by(vec2(40.0, -20.0));
    let panned = filled_rects(&harness.frame(vec![]), BOX_FILL)[0];
    assert_near(panned.min, zoomed.min + moved);
    assert_near(panned.max, zoomed.max + moved);
}

#[test]
fn the_view_follows_the_pointer_while_panning() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.add_box(NODE.min, NODE.max);
    let from = harness.origin + vec2(400.0, 300.0);
    harness.frame(vec![Event::PointerMoved(from)]);
    harness.frame(vec![middle_button(from, true)]);
    for step in 1..=3 {
        let moved = vec2(10.0, 5.0) * step as f32;
        let painted = harness.frame(vec![Event::PointerMoved(from + moved)]);
        assert_eq!(
            filled_rects(&painted, BOX_FILL),
            [harness.node().translate(moved)]
        );
        assert_eq!(harness.cursor, CursorIcon::Grabbing);
    }
}

#[test]
fn middle_drags_pan_whatever_they_start_on() {
    // Inside the selected node and on its right side, where primary drags
    // would move or resize it, and where the node tools would draw.
    for (tool, from) in [
        (Tool::Cursor, vec2(175.0, 90.0)),
        (Tool::Cursor, vec2(250.0, 90.0)),
        (Tool::Box, vec2(400.0, 300.0)),
        (Tool::Circle, vec2(400.0, 300.0)),
        (Tool::Line, vec2(400.0, 300.0)),
    ] {
        let mut harness = Harness::new(tool);
        harness.add_selected_box();
        let moved = vec2(30.0, 20.0);
        let painted = harness.middle_drag(from, from + moved);

        assert_eq!(
            filled_rects(&painted, BOX_FILL),
            [harness.node().translate(moved)],
            "{tool:?} {from:?}"
        );
        assert_eq!(
            harness.boxes(),
            [geometry(100.0, 50.0, 150.0, 80.0)],
            "{tool:?} {from:?}"
        );
        assert_eq!(harness.ellipses(), [], "{tool:?} {from:?}");
    }
}

#[test]
fn panning_keeps_the_label_being_typed() {
    let mut harness = Harness::new(Tool::Cursor);
    let id = harness.add_labelled_box(vec![Label::new("Start")]);
    harness.edit(&id);
    harness.type_text("Stop");
    let moved = vec2(30.0, 20.0);
    let shapes = harness.middle_drag(vec2(400.0, 300.0), vec2(400.0, 300.0) + moved);

    // Still in its editor, which moved with the node.
    let text = painted(&shapes, "Stop").unwrap();
    assert_near(text.center(), harness.node().center() + moved);
    assert_eq!(harness.actions, []);
    assert!(harness.canvas.editor.is_some());
}

#[test]
fn nodes_drawn_zoomed_and_panned_go_where_they_were_drawn() {
    let mut harness = Harness::new(Tool::Box);
    harness.zoom(2.0);
    harness.middle_drag(vec2(300.0, 300.0), vec2(400.0, 350.0));
    harness.drag(vec2(300.0, 200.0), vec2(500.0, 360.0));
    // The pan away, at 2 points a graph point.
    assert_eq!(harness.boxes(), [geometry(100.0, 75.0, 100.0, 80.0)]);
}

#[test]
fn clicks_select_the_node_shown_under_them_when_zoomed() {
    let mut harness = Harness::new(Tool::Cursor);
    let id = harness.add_box(NODE.min, NODE.max);
    harness.zoom(2.0);
    // The node shows from (200, 100) to (500, 260).
    harness.click(vec2(480.0, 250.0));
    assert_eq!(harness.selected, Some(Selection::node(id)));
    harness.click(vec2(510.0, 250.0));
    assert_eq!(harness.selected, None);
}

#[test]
fn zoomed_moves_and_resizes_go_by_graph_points() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.add_selected_box();
    harness.zoom(2.0);
    harness.drag(vec2(350.0, 180.0), vec2(390.0, 200.0));
    assert_eq!(harness.boxes(), [geometry(120.0, 60.0, 150.0, 80.0)]);
    // Its right side shows at 540 now, and is grabbed 4 points off it, as
    // when not zoomed.
    harness.drag(vec2(544.0, 220.0), vec2(574.0, 220.0));
    assert_eq!(harness.boxes(), [geometry(120.0, 60.0, 165.0, 80.0)]);
}

#[test]
fn zoomed_out_clicks_still_draw_nothing() {
    let mut harness = Harness::new(Tool::Box);
    harness.zoom(0.25);
    // 5 points on screen is 20 graph points, but still a click.
    harness.drag(vec2(100.0, 100.0), vec2(104.0, 103.0));
    assert_eq!(harness.boxes(), []);
    harness.drag(vec2(100.0, 100.0), vec2(110.0, 104.0));
    assert_eq!(harness.boxes(), [geometry(400.0, 400.0, 40.0, 16.0)]);
}

#[test]
fn borders_and_labels_scale_with_the_zoom() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.add_labelled_box(vec![Label::new("Start")]);
    harness.zoom(2.0);
    let shapes = harness.frame(vec![]);

    let node = harness.zoomed_node(2.0);
    assert_eq!(filled_rects(&shapes, BOX_FILL), [node]);
    let borders: Vec<f32> = shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Rect(rect) if rect.rect == node && !rect.stroke.is_empty() => {
                Some(rect.stroke.width)
            }
            _ => None,
        })
        .collect();
    assert_eq!(borders, [2.0]);
    assert_eq!(font_size(&shapes, "Start"), 24.0);
    assert_near(painted(&shapes, "Start").unwrap().center(), node.center());
}

#[test]
fn dashed_borders_scale_with_the_zoom() {
    let mut harness = Harness::new(Tool::Cursor);
    let mut graphics = new_box(NODE.min, NODE.max);
    graphics.border.line_type = yed::LineType::Dashed;
    graphics.border.color = Some(yed::Color::rgb(0xFF, 0x00, 0x00));
    document::add_node(&mut harness.graphml, &graphics);
    let dashes = quads(&harness.frame(vec![]), RED);
    harness.zoom(2.0);
    let zoomed = quads(&harness.frame(vec![]), RED);

    // As many, each twice as long and thick.
    assert_eq!(zoomed.len(), dashes.len());
    assert_eq!(zoomed[0].size(), dashes[0].size() * 2.0);
}

#[test]
fn the_selection_outline_keeps_its_size_when_zoomed() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.add_selected_box();
    harness.zoom(2.0);
    let shapes = harness.frame(vec![]);
    let outline = harness.zoomed_node(2.0).expand(SELECTION_GAP);
    assert_eq!(stroked_rects(&shapes, SELECTION.color), [outline]);
}

#[test]
fn the_label_editor_zooms_with_the_canvas() {
    let mut harness = Harness::new(Tool::Cursor);
    let id = harness.add_labelled_box(vec![Label::new("Start")]);
    harness.zoom(2.0);
    harness.edit(&id);
    let shapes = harness.frame(vec![]);
    assert_eq!(font_size(&shapes, "Start"), 24.0);
    let text = painted(&shapes, "Start").unwrap();
    assert_near(text.center(), harness.zoomed_node(2.0).center());
}

#[test]
fn labels_are_painted_however_far_the_view_zooms() {
    // Zoomed in, the text gets too big for egui's glyphs, so it stops growing.
    for (factor, size, painted_size) in [(zoom::MIN, 1, 0.1), (zoom::MAX, 500, MAX_TEXT_PIXELS)] {
        let mut harness = Harness::new(Tool::Cursor);
        let mut label = Label::new("Label");
        label.font.size = size;
        harness.add_labelled_box(vec![label]);
        harness.zoom(factor);
        let shapes = harness.frame(vec![]);
        assert_eq!(font_size(&shapes, "Label"), painted_size, "{factor}");
    }
}

#[test]
fn labels_keep_their_layout_while_the_zoom_changes() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.add_labelled_box(vec![Label::new("Start")]);
    let before = painted(&harness.frame(vec![]), "Start").unwrap();
    // As the toolbar does, before the canvas shows.
    harness.canvas.set_zoom(1.125);
    let shapes = harness.frame(vec![]);

    // Scaled, as laying text out anew each frame is slow.
    assert_eq!(font_size(&shapes, "Start"), 12.0);
    let text = painted(&shapes, "Start").unwrap();
    assert!((text.width() - before.width() * 1.125).abs() < 0.01);
    let middle = filled_rects(&shapes, BACKGROUND)[0].center();
    let zoomed = middle + (harness.node().center() - middle) * 1.125;
    assert_near(text.center(), zoomed);
    // Laid out anew once the zoom stops changing.
    assert_eq!(harness.repaint_delay, Duration::ZERO);
    let shapes = harness.frame(vec![]);
    assert_eq!(font_size(&shapes, "Start"), 13.5);
}

#[test]
fn labels_are_laid_out_anew_once_the_zoom_strays_far() {
    let mut harness = Harness::new(Tool::Cursor);
    harness.add_labelled_box(vec![Label::new("Start")]);
    harness.canvas.set_zoom(1.5);
    assert_eq!(font_size(&harness.frame(vec![]), "Start"), 18.0);
}

/// The lines painted, as the points of open paths in `color`.
fn lines(shapes: &[Shape], color: Color32) -> Vec<Vec<Pos2>> {
    let lines = paths(shapes).into_iter().filter(|path| !path.closed);
    let lines = lines.filter(|path| path.stroke.color == ColorMode::Solid(color));
    lines.map(|path| path.points.clone()).collect()
}

/// The arrowheads painted in `color`, each from its tip.
fn arrowheads(shapes: &[Shape], color: Color32) -> Vec<Vec<Pos2>> {
    let heads = paths(shapes)
        .into_iter()
        .filter(|path| path.closed && path.fill == color);
    heads.map(|path| path.points.clone()).collect()
}

fn assert_path_near(path: &[Pos2], expected: &[Pos2]) {
    assert_eq!(path.len(), expected.len(), "{path:?} isn't {expected:?}");
    for (point, expected) in path.iter().zip(expected) {
        assert_near(*point, *expected);
    }
}

fn point(x: f64, y: f64) -> yed::Point {
    yed::Point { x, y }
}

#[test]
fn dragging_with_the_line_tool_joins_two_nodes() {
    let mut harness = Harness::new(Tool::Line);
    let (node, other) = harness.add_two_boxes();
    harness.drag(vec2(175.0, 90.0), vec2(475.0, 90.0));

    let edges = harness.edges();
    let [(source, target, graphics)] = &edges[..] else {
        panic!("expected one edge: {edges:?}");
    };
    assert_eq!((source, target), (&node, &other));
    assert_eq!(graphics.path, yed::Path::default());
    assert_eq!(graphics.arrows, Tool::Line.arrows().unwrap());
    assert_eq!(graphics.arrows.target, yed::Arrow::None);
    assert_eq!(harness.selected, Some(Selection::edge(0)));
}

#[test]
fn the_arrow_tool_draws_edges_with_an_arrowhead_at_the_end() {
    let mut harness = Harness::new(Tool::Arrow);
    harness.add_two_boxes();
    let shapes = harness.drag(vec2(175.0, 90.0), vec2(475.0, 90.0));

    let arrows = &harness.edges()[0].2.arrows;
    assert_eq!(
        (arrows.source.clone(), arrows.target.clone()),
        (yed::Arrow::None, yed::Arrow::Standard)
    );
    // Its tip touches the target, and the line stops at its notch.
    let origin = harness.origin.to_vec2();
    let heads = arrowheads(&shapes, Color32::BLACK);
    let [head] = &heads[..] else {
        panic!("expected one arrowhead: {heads:?}");
    };
    assert_near(head[0], pos2(400.0, 90.5) + origin);
    let lines = lines(&shapes, Color32::BLACK);
    assert_path_near(
        &lines[0],
        &[pos2(250.0, 90.5) + origin, pos2(393.0, 90.5) + origin],
    );
}

#[test]
fn arrowheads_scale_with_the_line_width() {
    let mut harness = Harness::new(Tool::Cursor);
    let (node, other) = harness.add_two_boxes();
    let edge = harness.add_edge(&node, &other, &[]);
    document::edit_edge_graphics(&mut harness.graphml, edge, |graphics| {
        graphics.line.width = 3.0;
        graphics.arrows.target = yed::Arrow::Standard;
    });
    let shapes = harness.frame(vec![]);

    // Three times as big as on a line 1 point wide.
    let origin = harness.origin.to_vec2();
    let heads = arrowheads(&shapes, Color32::BLACK);
    let bounds = Rect::from_points(&heads[0]);
    assert_near(bounds.min, pos2(370.0, 78.5) + origin);
    assert_near(bounds.max, pos2(400.0, 102.5) + origin);
    assert_path_near(
        &lines(&shapes, Color32::BLACK)[0],
        &[pos2(250.0, 90.5) + origin, pos2(379.0, 90.5) + origin],
    );
}

#[test]
fn edges_shorter_than_an_arrowhead_keep_both_arrowheads() {
    let mut harness = Harness::new(Tool::Cursor);
    let node = harness.add_box(NODE.min, NODE.max);
    let other = harness.add_box(pos2(260.0, 50.0), pos2(410.0, 130.0));
    let edge = harness.add_edge(&node, &other, &[]);
    document::edit_edge_graphics(&mut harness.graphml, edge, |graphics| {
        graphics.line.width = 2.0;
        graphics.arrows.source = yed::Arrow::Standard;
        graphics.arrows.target = yed::Arrow::Standard;
    });
    let shapes = harness.frame(vec![]);

    let origin = harness.origin.to_vec2();
    let tips: Vec<Pos2> = (arrowheads(&shapes, Color32::BLACK).iter())
        .map(|head| head[0])
        .collect();
    assert_path_near(
        &tips,
        &[pos2(260.0, 90.0) + origin, pos2(250.0, 90.0) + origin],
    );
    assert_eq!(lines(&shapes, Color32::BLACK), Vec::<Vec<Pos2>>::new());
}

#[test]
fn clicks_draw_edges_with_bends_where_they_miss_the_nodes() {
    let mut harness = Harness::new(Tool::Line);
    let (node, other) = harness.add_two_boxes();
    harness.click(vec2(175.0, 90.0));
    harness.click(vec2(325.0, 250.0));
    assert_eq!(harness.edges(), []);
    harness.click(vec2(475.0, 90.0));

    let edges = harness.edges();
    assert_eq!((&edges[0].0, &edges[0].1), (&node, &other));
    assert_eq!(harness.bends(0), [point(325.0, 250.0)]);
}

#[test]
fn letting_go_of_a_drag_off_the_nodes_adds_a_bend_and_keeps_drawing() {
    let mut harness = Harness::new(Tool::Line);
    harness.add_two_boxes();
    harness.drag(vec2(175.0, 90.0), vec2(325.0, 250.0));
    assert_eq!(harness.edges(), []);
    harness.click(vec2(475.0, 90.0));
    assert_eq!(harness.bends(0), [point(325.0, 250.0)]);
}

#[test]
fn double_clicks_while_drawing_add_one_bend() {
    let mut harness = Harness::new(Tool::Line);
    harness.add_two_boxes();
    harness.click(vec2(175.0, 90.0));
    harness.double_click(vec2(325.0, 250.0));
    harness.click(vec2(475.0, 90.0));
    assert_eq!(harness.bends(0), [point(325.0, 250.0)]);
}

#[test]
fn edges_start_only_on_nodes() {
    let mut harness = Harness::new(Tool::Line);
    harness.add_two_boxes();
    harness.drag(vec2(325.0, 250.0), vec2(475.0, 90.0));
    harness.click(vec2(600.0, 90.0));
    assert_eq!(harness.edges(), []);
    assert_eq!(harness.selected, None);
}

#[test]
fn clicking_the_start_again_keeps_drawing_unless_it_loops_through_a_bend() {
    let mut harness = Harness::new(Tool::Line);
    let (node, _) = harness.add_two_boxes();
    harness.click(vec2(175.0, 90.0));
    harness.click(vec2(180.0, 95.0));
    assert_eq!(harness.edges(), []);
    harness.click(vec2(325.0, 250.0));
    harness.click(vec2(180.0, 95.0));

    let edges = harness.edges();
    assert_eq!((&edges[0].0, &edges[0].1), (&node, &node));
    assert_eq!(harness.bends(0), [point(325.0, 250.0)]);
}

#[test]
fn switching_tools_drops_the_edge_being_drawn() {
    let mut harness = Harness::new(Tool::Line);
    harness.add_two_boxes();
    harness.click(vec2(175.0, 90.0));
    harness.tool = Tool::Cursor;
    harness.frame(vec![]);
    harness.tool = Tool::Line;
    harness.click(vec2(475.0, 90.0));
    assert_eq!(harness.edges(), []);
}

#[test]
fn the_edge_shows_while_it_is_drawn_and_snaps_to_the_node_under_the_pointer() {
    let mut harness = Harness::new(Tool::Arrow);
    harness.add_two_boxes();
    let origin = harness.origin.to_vec2();
    harness.click(vec2(175.0, 90.0));
    let shapes = harness.frame(vec![Event::PointerMoved(pos2(325.0, 300.0) + origin)]);
    assert_eq!(lines(&shapes, Color32::BLACK).len(), 1);
    assert_eq!(arrowheads(&shapes, Color32::BLACK).len(), 1);

    let shapes = harness.frame(vec![Event::PointerMoved(pos2(500.0, 100.0) + origin)]);
    let heads = arrowheads(&shapes, Color32::BLACK);
    // At the target's side, level with the start.
    assert_near(heads[0][0], pos2(400.0, 90.5) + origin);
    let target = harness
        .node()
        .translate(OTHER.min - NODE.min)
        .expand(SELECTION_GAP);
    assert_eq!(stroked_rects(&shapes, SELECTION.color), [target]);
}

#[test]
fn edges_are_painted_between_the_outlines_of_their_nodes() {
    let mut harness = Harness::new(Tool::Cursor);
    let (node, other) = harness.add_two_boxes();
    let ellipse = harness.add_ellipse(pos2(300.0, 250.0), pos2(450.0, 330.0));
    harness.add_edge(&node, &other, &[]);
    harness.add_edge(&node, &ellipse, &[]);
    let shapes = harness.frame(vec![]);

    let origin = harness.origin.to_vec2();
    let lines = lines(&shapes, Color32::BLACK);
    assert_path_near(
        &lines[0],
        &[pos2(250.0, 90.5) + origin, pos2(400.0, 90.5) + origin],
    );
    // From the box's corner, diagonally to the ellipse's outline.
    assert_path_near(
        &lines[1],
        &[pos2(215.0, 130.0) + origin, pos2(339.7, 254.7) + origin],
    );
}

#[test]
fn clicking_near_an_edge_selects_it() {
    let mut harness = Harness::new(Tool::Cursor);
    let (node, other) = harness.add_two_boxes();
    harness.add_edge(&node, &other, &[]);
    harness.click(vec2(325.0, 93.0));
    assert_eq!(harness.selected, Some(Selection::edge(0)));
    harness.click(vec2(325.0, 100.0));
    assert_eq!(harness.selected, None);
    // Over the nodes it joins, near their sides.
    harness.click(vec2(247.0, 90.0));
    assert_eq!(harness.selected, Some(Selection::edge(0)));
    harness.click(vec2(240.0, 90.0));
    assert_eq!(harness.selected, Some(Selection::node(&node)));
}

#[test]
fn clicking_an_edge_label_selects_the_edge() {
    let mut harness = Harness::new(Tool::Cursor);
    let (node, other) = harness.add_two_boxes();
    let index = harness.add_edge(&node, &other, &[]);
    let route = Route::new(&yed::Path::default(), yed_rect(NODE), yed_rect(OTHER));
    let mut label = Label::new_on_edge("Label");
    label.place_on(&route, point(325.0, 50.0), 40.0, 20.0);
    document::edit_edge_graphics(&mut harness.graphml, index, |graphics| {
        graphics.labels.push(label);
    });
    harness.click(vec2(340.0, 45.0));
    assert_eq!(harness.selected, Some(Selection::edge(0)));
}

#[test]
fn the_selected_edge_shows_handles_on_its_ends_and_bends() {
    let mut harness = Harness::new(Tool::Cursor);
    let (node, other) = harness.add_two_boxes();
    harness.add_edge(&node, &other, &[pos2(325.0, 250.0)]);
    harness.selected = Some(Selection::edge(0));
    let shapes = harness.frame(vec![]);

    let origin = harness.origin.to_vec2();
    let handle = Rect::from_center_size(pos2(325.0, 250.0) + origin, Vec2::splat(HANDLE_SIZE));
    assert_eq!(stroked_rects(&shapes, SELECTION.color), [handle]);
    let ends: Vec<_> = (shapes.iter())
        .filter_map(|shape| match shape {
            Shape::Circle(circle) if circle.stroke.color == SELECTION.color => Some(circle.center),
            _ => None,
        })
        .collect();
    assert_eq!(ends.len(), 2);
    // A halo under the line.
    assert_eq!(lines(&shapes, HALO.color).len(), 1);
}

#[test]
fn dragging_the_line_of_the_selected_edge_adds_a_bend_there() {
    let mut harness = Harness::new(Tool::Cursor);
    let (node, other) = harness.add_two_boxes();
    harness.add_edge(&node, &other, &[]);
    harness.selected = Some(Selection::edge(0));
    harness.drag(vec2(325.0, 91.0), vec2(325.0, 200.0));

    assert_eq!(harness.bends(0), [point(325.0, 200.0)]);
    let selection = Selection {
        item: Item::Edge(0),
        bend: Some(0),
    };
    assert_eq!(harness.selected, Some(selection));
}

#[test]
fn bends_are_added_between_the_bends_either_side() {
    let mut harness = Harness::new(Tool::Cursor);
    let (node, other) = harness.add_two_boxes();
    harness.add_edge(&node, &other, &[pos2(175.0, 250.0), pos2(475.0, 250.0)]);
    harness.selected = Some(Selection::edge(0));
    harness.drag(vec2(325.0, 250.0), vec2(325.0, 300.0));
    let bends = [
        point(175.0, 250.0),
        point(325.0, 300.0),
        point(475.0, 250.0),
    ];
    assert_eq!(harness.bends(0), bends);
}

#[test]
fn dragging_a_bend_moves_it_and_clicking_one_selects_it() {
    let mut harness = Harness::new(Tool::Cursor);
    let (node, other) = harness.add_two_boxes();
    harness.add_edge(&node, &other, &[pos2(325.0, 250.0), pos2(325.5, 300.0)]);
    harness.selected = Some(Selection::edge(0));
    harness.click(vec2(325.0, 300.0));
    assert_eq!(harness.selected.as_ref().unwrap().bend, Some(1));

    harness.drag(vec2(326.0, 252.0), vec2(366.0, 232.0));
    assert_eq!(harness.bends(0), [point(365.0, 230.0), point(325.5, 300.0)]);
    assert_eq!(harness.selected.as_ref().unwrap().bend, Some(0));
}

#[test]
fn the_edge_follows_a_bend_while_it_is_dragged() {
    let mut harness = Harness::new(Tool::Cursor);
    let (node, other) = harness.add_two_boxes();
    harness.add_edge(&node, &other, &[pos2(325.0, 250.0)]);
    harness.selected = Some(Selection::edge(0));
    let shapes = harness.drag_and_hold(vec2(325.0, 250.0), vec2(325.0, 300.0));

    let line = &lines(&shapes, Color32::BLACK)[0];
    assert_near(line[1], pos2(325.0, 300.0) + harness.origin.to_vec2());
    assert_eq!(harness.bends(0), [point(325.0, 250.0)]);
}

#[test]
fn escape_cancels_the_drag_of_a_bend() {
    let mut harness = Harness::new(Tool::Cursor);
    let (node, other) = harness.add_two_boxes();
    harness.add_edge(&node, &other, &[pos2(325.0, 250.0)]);
    harness.selected = Some(Selection::edge(0));
    harness.drag_and_hold(vec2(325.0, 250.0), vec2(325.0, 300.0));
    harness.press(Modifiers::NONE, Key::Escape);
    harness.frame(vec![button(harness.origin + vec2(325.0, 300.0), false)]);
    assert_eq!(harness.bends(0), [point(325.0, 250.0)]);
}

#[test]
fn dragging_an_end_onto_another_node_moves_the_edge_there() {
    let mut harness = Harness::new(Tool::Cursor);
    let (node, other) = harness.add_two_boxes();
    let third = harness.add_box(THIRD.min, THIRD.max);
    let index = harness.add_edge(&node, &other, &[]);
    document::edit_edge_graphics(&mut harness.graphml, index, |graphics| {
        graphics.path.target = point(10.0, 0.0);
    });
    harness.selected = Some(Selection::edge(0));
    // Off the nodes, the end goes back, and on its node it stays as it was.
    harness.drag(vec2(400.0, 90.0), vec2(325.0, 200.0));
    harness.drag(vec2(400.0, 90.0), vec2(500.0, 100.0));
    assert_eq!(harness.edges()[0].1, other);
    assert_eq!(harness.edges()[0].2.path.target, point(10.0, 0.0));

    harness.drag(vec2(400.0, 90.0), vec2(475.0, 300.0));
    let edges = harness.edges();
    assert_eq!((&edges[0].0, &edges[0].1), (&node, &third));
    // To the node's middle.
    assert_eq!(edges[0].2.path.target, point(0.0, 0.0));
}

#[test]
fn dragging_an_edge_label_moves_it_beside_the_edge() {
    let mut harness = Harness::new(Tool::Cursor);
    let (node, other) = harness.add_two_boxes();
    let index = harness.add_edge(&node, &other, &[]);
    document::edit_edge_graphics(&mut harness.graphml, index, |graphics| {
        graphics.labels.push(Label::new_on_edge("Label"));
    });
    harness.selected = Some(Selection::edge(0));
    let before = painted(&harness.frame(vec![]), "Label").unwrap();
    harness.drag(vec2(325.0, 90.0), vec2(345.0, 130.0));

    let after = painted(&harness.frame(vec![]), "Label").unwrap();
    assert_near(after.center(), before.center() + vec2(20.0, 40.0));
    // It stays where it was put relative to the edge as the edge moves.
    let route = Route::new(&yed::Path::default(), yed_rect(NODE), yed_rect(OTHER));
    let label = &harness.edges()[0].2.labels[0];
    let bounds = label.bounds;
    let placed = label
        .placed_on(&route, bounds.width, bounds.height)
        .unwrap();
    assert_eq!(
        (
            placed.x + placed.width / 2.0,
            placed.y + placed.height / 2.0
        ),
        (345.0, 130.0)
    );
}

#[test]
fn edges_follow_a_node_while_it_moves() {
    let mut harness = Harness::new(Tool::Cursor);
    let (node, other) = harness.add_two_boxes();
    harness.add_edge(&node, &other, &[]);
    harness.selected = Some(Selection::node(&node));
    let shapes = harness.drag_and_hold(vec2(175.0, 90.0), vec2(175.0, 190.0));

    // From the moved node's right side, up to the other's left side.
    let origin = harness.origin.to_vec2();
    let line = &lines(&shapes, Color32::BLACK)[0];
    assert_path_near(
        line,
        &[pos2(250.0, 165.0) + origin, pos2(400.0, 115.0) + origin],
    );
}

#[test]
fn double_clicking_an_edge_edits_its_label() {
    let mut harness = Harness::new(Tool::Cursor);
    let (node, other) = harness.add_two_boxes();
    harness.add_edge(&node, &other, &[]);
    harness.double_click(vec2(325.0, 90.0));
    assert_eq!(harness.selected, Some(Selection::edge(0)));
    assert_eq!(harness.actions, [Action::EditLabel]);
}

#[test]
fn new_edge_labels_are_edited_on_the_middle_of_the_edge() {
    let mut harness = Harness::new(Tool::Cursor);
    let (node, other) = harness.add_two_boxes();
    harness.add_edge(&node, &other, &[]);
    let label = LabelRef {
        item: Item::Edge(0),
        index: 0,
    };
    harness.canvas.edit_label(label.clone(), String::new());
    harness.frame(vec![]);
    harness.type_text("Edge");
    let shapes = harness.frame(vec![]);
    let text = painted(&shapes, "Edge").unwrap();
    assert_near(text.center(), pos2(325.0, 90.0) + harness.origin.to_vec2());
    harness.press(Modifiers::NONE, Key::Enter);
    assert_eq!(
        harness.actions,
        [Action::SetLabel(label, "Edge".to_owned())]
    );
}

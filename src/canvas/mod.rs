mod drawing;
mod edge;
mod grid;
mod paint;
mod shown;
mod textures;

use std::borrow::Cow;
use std::sync::Arc;

use egui::emath::TSTransform;
use egui::{
    Align, Align2, Color32, CursorIcon, Event, Id, Margin, PointerButton, Pos2, Rangef, Rect,
    Response, Sense, Stroke, UiBuilder, Vec2, Visuals, vec2,
};
use graphml::GraphMl;
use graphml::yed::{
    self, Arrows, EdgeGraphics, EdgeKind, NodeGraphics, NodeKind, Route, ShapeType,
};
use serde::{Deserialize, Serialize};

use crate::action::Action;
use crate::document::{self, EdgeEnd, Item, LabelRef};
use crate::history::Document;
use crate::scene::SceneEdge;
use crate::selection::Selection;
use crate::snap::{Axis, Feature, Guide, Side, Targets};
use crate::style::Style;
use crate::text_field::{self, End};
use crate::tool::{NodeShape, Tool};
use crate::zoom;

pub use drawing::{Drawing, Mark, Pen, Text};
use paint::{halign, label_font, label_rect};
pub use shown::Outline;
use shown::{Anchor, EdgeLook, Shown, egui_pos, yed_point};
use textures::Textures;

/// White like paper, even in the dark theme.
const BACKGROUND: Color32 = Color32::WHITE;
/// New nodes' fill, CSS's `lightblue`.
const NEW_NODE_FILL: yed::Color = yed::Color::rgb(0xAD, 0xD8, 0xE6);

const SELECTION: Stroke = Stroke {
    width: 2.0,
    color: Color32::from_rgb(0x00, 0x78, 0xD4),
};
/// Between the selected node's bounds and its outline.
const SELECTION_GAP: f32 = 2.0;
/// Inside the box a drag from the empty canvas draws to select nodes and
/// edges.
const SELECTION_BOX_FILL: Color32 = Color32::from_rgba_unmultiplied_const(0x00, 0x78, 0xD4, 0x20);
/// Under the selected edges, this wide either side of their lines.
const HALO: Stroke = Stroke {
    width: 3.0,
    color: Color32::from_rgba_premultiplied(0x00, 0x30, 0x55, 0x66),
};
/// How wide the handles on the selected edge's bends and ends are.
const HANDLE_SIZE: f32 = 7.0;
/// The guides showing what a drag lines up with: magenta, as in drawing apps.
const GUIDE: Color32 = Color32::from_rgb(0xE6, 0x00, 0x8A);
/// How far guides reach past what they span, in screen points.
const GUIDE_OVERHANG: f32 = 8.0;

/// The sides and corners of the selected node that can be dragged, e.g.
/// `RIGHT_CENTER` for its right side.
const HANDLES: [Align2; 8] = [
    Align2::LEFT_TOP,
    Align2::CENTER_TOP,
    Align2::RIGHT_TOP,
    Align2::LEFT_CENTER,
    Align2::RIGHT_CENTER,
    Align2::LEFT_BOTTOM,
    Align2::CENTER_BOTTOM,
    Align2::RIGHT_BOTTOM,
];
/// How far inside or outside a side of the selected node a drag grabs it,
/// which takes in its outline.
const GRAB: f32 = 5.0;
const MIN_SIZE: f64 = 1.0;
/// How near on screen, in points, what's dragged snaps onto a line.
const SNAP_REACH: f32 = 6.0;

/// Between a label's text and its bounds, as yEd measures labels.
const LABEL_PADDING: f32 = 2.0;
/// The largest text the canvas draws, in pixels, as egui draws no glyph over
/// 1024 pixels.
const MAX_TEXT_PIXELS: f32 = 1000.0;
/// How far labels are scaled while the zoom changes before they're laid out
/// anew, which is too slow to do every frame.
const MAX_TEXT_SCALE: f32 = 1.25;

const EDITOR_MARGIN: Margin = Margin::symmetric(4, 2);
const EDITOR_MIN_WIDTH: f32 = 60.0;

/// What the View menu turns on for the canvas.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    pub grid_lines: bool,
    /// Whether drags snap onto the sides and middles of other nodes, with
    /// guides showing what lines up, unless Alt is held.
    pub snap_lines: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            grid_lines: false,
            snap_lines: true,
        }
    }
}

/// The drawing canvas, which shows the graph zoomed and panned.
#[derive(Default)]
pub struct Canvas {
    view: View,
    /// As last shown with.
    options: Options,
    /// The size it was last shown at, to zoom about its middle.
    size: Vec2,
    /// The zoom it was last shown at.
    shown_zoom: f32,
    /// The zoom labels are laid out at, which lags while the zoom changes.
    text_zoom: f32,
    /// While the middle button pans the view, where the pointer was last.
    panning: Option<Pos2>,
    shape_drag: Option<ShapeDrag>,
    resize: Option<Resize>,
    move_drag: Option<MoveDrag>,
    box_drag: Option<BoxDrag>,
    edge_draw: Option<EdgeDraw>,
    edge_drag: Option<EdgeDrag>,
    editor: Option<LabelEditor>,
    textures: Textures,
    /// Whether drags snap this frame.
    snapping: bool,
    /// What this frame's drag lines up with.
    guides: Vec<Guide>,
}

/// Where the graph shows on the canvas.
#[derive(Clone, Copy)]
struct View {
    /// In screen points per graph point.
    zoom: f32,
    /// Where the graph's origin is, from the canvas's top-left corner.
    offset: Vec2,
}

impl Default for View {
    fn default() -> Self {
        View {
            zoom: 1.0,
            offset: Vec2::ZERO,
        }
    }
}

impl View {
    /// Zooms to `zoom`, within limits, keeping the graph point at `anchor`,
    /// from the canvas's top-left corner, where it is.
    fn zoom_to(&mut self, zoom: f32, anchor: Vec2) {
        let zoom = zoom.clamp(zoom::MIN, zoom::MAX);
        if zoom != self.zoom {
            self.offset = anchor - (anchor - self.offset) * (zoom / self.zoom);
            self.zoom = zoom;
        }
    }

    /// Takes graph coordinates to screen ones, on a canvas at `rect`.
    fn to_screen(self, rect: Rect) -> TSTransform {
        TSTransform::new(rect.min.to_vec2() + self.offset, self.zoom)
    }
}

/// A node being dragged out by the tool that draws it.
#[derive(Clone, Copy)]
struct ShapeDrag {
    tool: Tool,
    /// The corner it was started from, in graph coordinates.
    from: Pos2,
}

/// A drag of one of the [`HANDLES`] of the only selected node, which
/// resizes it.
#[derive(Clone, Copy)]
struct Resize {
    handle: Align2,
    /// Where the drag started, in graph coordinates.
    from: Pos2,
}

/// A drag of the selected nodes and edges from one of them, which moves them
/// all.
#[derive(Clone, Copy)]
struct MoveDrag {
    /// Where the drag started, in graph coordinates.
    from: Pos2,
}

/// A box being dragged out from the empty canvas, which selects the nodes
/// and edges inside it.
#[derive(Clone, Copy)]
struct BoxDrag {
    /// The corner it started from, in graph coordinates, unrounded.
    from: Pos2,
    /// Whether what it selects is added to the selection, rather than
    /// replacing it.
    add: bool,
}

/// An edge being drawn by the tool that draws it, from a node through bends
/// to the pointer.
struct EdgeDraw {
    tool: Tool,
    /// The id of the node it starts from.
    source: String,
    /// In graph coordinates.
    bends: Vec<yed::Point>,
}

/// A drag of part of the selected edge.
#[derive(Clone, Copy)]
struct EdgeDrag {
    part: EdgePart,
    /// Where the drag started, in graph coordinates.
    from: Pos2,
}

/// A part of the selected edge that can be dragged.
#[derive(Clone, Copy, Debug, PartialEq)]
enum EdgePart {
    /// The bend at this index, by its handle.
    Bend(usize),
    /// Its line, which adds a bend at this index where the drag ends.
    Line(usize),
    /// An end, by the handle on its port, which moves the port within its
    /// node or onto another.
    End(EdgeEnd),
    /// The label at this index.
    Label(usize),
}

/// What a label is placed on: a node, by its bounds, or an edge, along its
/// route.
enum Owner {
    Node(yed::Rect),
    Edge(Route),
}

/// A label being edited on the canvas, over where it's painted.
struct LabelEditor {
    label: LabelRef,
    text: String,
    /// Whether the editor has yet to take the focus.
    focus: bool,
}

/// What the input this frame shows over the scene.
struct Preview {
    /// The node being drawn.
    drawing: Option<NodeGraphics>,
    drawn_edge: Option<EdgeLook<'static>>,
    /// The only selected edge, while it's being dragged.
    dragged_edge: Option<EdgeLook<'static>>,
    /// The box being dragged out on screen, and whether what it selects is
    /// added to the selection.
    boxed: Option<(Rect, bool)>,
}

impl Canvas {
    /// Shows the canvas in the space left over by the other panels, with
    /// what `options` turns on, drawing new nodes, edges and labels in
    /// `style`. Returns the actions asked for.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        tool: Tool,
        doc: &mut Document,
        selected: &mut Selection,
        style: &Style,
        options: Options,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        self.options = options;
        egui::CentralPanel::default().show(ui, |ui| {
            let sense = match tool {
                Tool::Cursor => Sense::click_and_drag(),
                _ if tool.node_shape().is_some() => Sense::drag(),
                // Edges are drawn by clicks as well as drags.
                _ => Sense::click_and_drag(),
            };
            let (response, painter) = ui.allocate_painter(ui.available_size(), sense);
            // Unlike a click, a drag leaves the focus in a label being typed,
            // which would then stay open while the next node is drawn.
            if response.drag_started_by(PointerButton::Primary) {
                ui.memory_mut(|memory| memory.stop_text_input());
            }
            let rect = response.rect;
            self.size = rect.size();
            self.zoom_with_wheel(ui, rect);
            self.pan(ui, rect);
            let to_screen = self.view.to_screen(rect);
            let text_zoom = self.text_zoom(ui.ctx());
            // Alt skips snapping, e.g. to place a node just beside a line.
            self.snapping = self.options.snap_lines && !ui.input(|input| input.modifiers.alt);
            self.guides.clear();
            // A shortcut can switch tools mid-drag, which drops the shape.
            self.shape_drag.take_if(|drag| drag.tool != tool);
            self.edge_draw.take_if(|draw| draw.tool != tool);
            if tool != Tool::Cursor {
                // And the move, resize or selection box.
                (self.resize, self.move_drag, self.box_drag) = (None, None, None);
                self.edge_drag = None;
            }

            // A copy, so that edits can change the document while it's shown.
            let scene = doc.scene().clone();
            let shown = Shown {
                scene: &scene,
                painter: &painter,
                to_screen,
                text_zoom,
            };
            let mut double_clicked = false;
            let mut drawn_edge = None;
            let drawing = match tool {
                Tool::Cursor => {
                    let ctrl = ui.input(|input| input.modifiers.command);
                    select_clicked(&response, shown, selected, ctrl);
                    self.start_drag(&response, shown, selected, ctrl);
                    // Not with Ctrl, whose clicks only add to or take from the selection.
                    double_clicked = response.double_clicked_by(PointerButton::Primary)
                        && !ctrl
                        && !selected.is_empty();
                    // Over anything a drag moves. The canvas counts as hovered
                    // while it's dragged, so not while it draws a box.
                    let hovered = (response.hover_pos())
                        .filter(|_| self.box_drag.is_none())
                        .and_then(|pos| shown.item_at(pos, None));
                    let dragging = self.move_drag.is_some() || self.edge_drag.is_some();
                    if dragging || hovered.is_some() {
                        ui.ctx().set_cursor_icon(CursorIcon::Move);
                    }
                    None
                }
                _ if tool.arrows().is_some() => {
                    drawn_edge = self.draw_edge(&response, shown, doc, selected, tool, style);
                    None
                }
                _ => self.draw_node(response.clone(), shown, doc, selected, tool, style),
            };
            let (mut resized, mut dragged_edge, mut moved, mut boxed) = (None, None, None, None);
            if tool == Tool::Cursor {
                match selected.single().cloned() {
                    Some(Item::Node(id)) => {
                        self.edge_drag = None;
                        let geometry = self.resize_node(ui, shown, doc, &id);
                        resized = geometry.map(|geometry| (id, geometry));
                    }
                    Some(Item::Edge(_)) => {
                        self.resize = None;
                        dragged_edge = self.drag_edge(ui, response.clone(), shown, doc, selected);
                    }
                    None => (self.resize, self.edge_drag) = (None, None),
                }
                moved = self.move_selection(ui, &response, shown, doc, selected);
                boxed = self.select_in_box(ui, &response, shown, selected);
            }

            // What a release did shows in the same frame.
            let released = ui.input(primary_release).is_some();
            let mut scene = if released { doc.scene().clone() } else { scene };
            // Drags show on the copy, sharing the graphics of the rest.
            if let Some((id, geometry)) = resized
                && let Some(node) = scene.node(&id)
            {
                let node = Arc::make_mut(&mut scene.nodes[node]);
                document::resize(&mut node.graphics, geometry);
            }
            if let Some([dx, dy]) = moved {
                let edges = document::bends_moved_with(doc.graphml(), selected.items());
                scene.translate(selected, &edges, dx, dy);
            }
            let shown = Shown {
                scene: &scene,
                painter: &painter,
                to_screen,
                text_zoom,
            };
            let preview = Preview {
                drawing,
                drawn_edge,
                dragged_edge,
                boxed,
            };
            self.paint_scene(ui, rect, shown, selected, style, preview);
            self.textures.sweep();

            // Over the nodes. An edit it applies goes first, in case the
            // double click is on the same label.
            actions.extend(self.show_editor(ui, shown, style));
            if double_clicked {
                actions.push(Action::EditLabel);
            }
            // Last, so it wins over the cursors of the nodes and handles.
            if self.panning.is_some() {
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            }
        });
        actions
    }

    /// The zoom level, in screen points per graph point.
    pub fn zoom(&self) -> f32 {
        self.view.zoom
    }

    /// Zooms to `zoom`, within limits, about the middle of the canvas.
    pub fn set_zoom(&mut self, zoom: f32) {
        self.view.zoom_to(zoom, self.size / 2.0);
    }

    /// The point of the graph in the middle of the canvas.
    pub fn view_center(&self) -> Pos2 {
        ((self.size / 2.0 - self.view.offset) / self.view.zoom).to_pos2()
    }

    /// Moves the view right and down over the graph by `delta`, in screen
    /// points.
    pub fn pan_by(&mut self, delta: Vec2) {
        self.view.offset -= delta;
    }

    /// Zooms about the pointer as the wheel turns, or a touchpad pinches, over
    /// the canvas at `rect`.
    fn zoom_with_wheel(&mut self, ui: &egui::Ui, rect: Rect) {
        // As fast as Ctrl+wheel, which egui reports as zooming already.
        let speed = ui.ctx().options(|o| o.input_options.scroll_zoom_speed);
        let (pointer, factor) = ui.input(|input| {
            let scrolled = input.smooth_scroll_delta.y;
            let factor = input.zoom_delta() * (speed * scrolled).exp();
            (input.pointer.interact_pos(), factor)
        });
        if let Some(pointer) = pointer
            && factor != 1.0
            && ui.rect_contains_pointer(rect)
        {
            let zoom = self.view.zoom * factor;
            self.view.zoom_to(zoom, pointer - rect.min);
        }
    }

    /// The zoom to lay labels out at: the zoom, unless it changed since the
    /// canvas was last shown and is still near the one they're laid out at.
    fn text_zoom(&mut self, ctx: &egui::Context) -> f32 {
        let zoom = self.view.zoom;
        let zooming = std::mem::replace(&mut self.shown_zoom, zoom) != zoom;
        let scale = zoom / self.text_zoom;
        if !zooming || !(1.0 / MAX_TEXT_SCALE..=MAX_TEXT_SCALE).contains(&scale) {
            self.text_zoom = zoom;
        }
        if zooming {
            // The toolbar showed the old zoom, and labels catch up once it
            // stops changing.
            ctx.request_repaint();
        }
        self.text_zoom
    }

    /// Pans the view while the middle button drags from the canvas at `rect`,
    /// whatever on it the drag starts from.
    fn pan(&mut self, ui: &egui::Ui, rect: Rect) {
        let (pressed, released, latest, down) = ui.input(|input| {
            (
                button_event(input, PointerButton::Middle, true),
                button_event(input, PointerButton::Middle, false),
                input.pointer.latest_pos(),
                input.pointer.button_down(PointerButton::Middle),
            )
        });
        if pressed.is_some() && ui.rect_contains_pointer(rect) {
            self.panning = pressed;
        }
        let Some(last) = self.panning else {
            return;
        };
        if let Some(to) = released.or(latest) {
            self.view.offset += to - last;
            self.panning = Some(to);
        }
        if released.is_some() || !down {
            self.panning = None;
        }
    }

    /// Edits `label`, whose text is `text`, over where it's painted.
    pub fn edit_label(&mut self, label: LabelRef, text: String) {
        self.editor = Some(LabelEditor {
            label,
            text,
            focus: true,
        });
    }

    /// Stops editing a label on the canvas, dropping what was typed.
    pub fn cancel_label_edit(&mut self) {
        self.editor = None;
    }

    /// Drops the drags, the edge being drawn and the label being edited, which
    /// are of the document as it was.
    pub fn cancel_edits(&mut self) {
        self.shape_drag = None;
        self.resize = None;
        self.move_drag = None;
        self.box_drag = None;
        self.edge_draw = None;
        self.edge_drag = None;
        self.editor = None;
    }

    /// Shows the label being edited, if there is one, a new one in `style`'s
    /// font. Returns the edit once Enter or a click elsewhere applies it.
    fn show_editor(&mut self, ui: &mut egui::Ui, shown: Shown, style: &Style) -> Option<Action> {
        let item = self.editor.as_ref()?.label.item.clone();
        let to_screen = shown.to_screen;
        let scene = shown.scene;
        // The labels, what they're placed on, and how wide the editor is at
        // least: as wide as a node.
        let found = match &item {
            Item::Node(id) => scene.node(id).map(|index| {
                let labels = &scene.nodes[index].graphics.labels;
                let width = shown.node_rect(index).width();
                (labels, Owner::Node(shown.geometry(index)), width)
            }),
            Item::Edge(index) => scene.edge(*index).map(|edge| {
                let route = shown.route(&EdgeLook::of(edge));
                (&edge.graphics.labels, Owner::Edge(route), 0.0)
            }),
        };
        let Some((labels, owner, min_width)) = found else {
            self.editor = None;
            return None;
        };
        let editor = self.editor.as_mut()?;
        let label = match labels.get(editor.label.index) {
            Some(label) => label.clone(),
            None => document::new_label(scene.graphics(&item)?, "", style),
        };
        let font = label_font(&label, to_screen.scaling, ui.pixels_per_point());
        let align = Align2([halign(&label.alignment), Align::Center]);
        let text = ui
            .painter()
            .layout_no_wrap(editor.text.clone(), font.clone(), Color32::PLACEHOLDER)
            .size();
        // An em to spare, so the next letter typed doesn't wrap.
        let width = text.x + font.size + EDITOR_MARGIN.sum().x;
        let size = vec2(
            width.max(min_width).max(EDITOR_MIN_WIDTH),
            text.y + EDITOR_MARGIN.sum().y,
        );
        let label_rect = label_rect(to_screen, &owner, &label, text);
        let rect = align.align_size_within_rect(size, label_rect);

        let id = Id::unique("label_editor");
        let output = ui
            .scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                // White like the canvas, even in the dark theme.
                ui.style_mut().visuals = Visuals::light();
                text_field::label_edit(&mut editor.text)
                    .id(id)
                    .font(font)
                    .margin(EDITOR_MARGIN)
                    .desired_width(rect.width())
                    .align(align)
                    .show(ui)
            })
            .inner;
        if std::mem::take(&mut editor.focus) {
            text_field::focus_all(ui.ctx(), output, &editor.text);
            return None;
        }
        let response = &output.response.response;
        match text_field::end(ui, response) {
            Some(End::Cancel) => {
                self.editor = None;
                return None;
            }
            None if response.has_focus() => return None,
            Some(End::Apply) | None => {}
        }
        let editor = self.editor.take()?;
        Some(Action::SetLabel(editor.label, editor.text))
    }

    /// Drags out a node in `style` with the primary button, if `tool` draws
    /// nodes, adding it to `doc` and selecting it on release. Returns the
    /// node while it's being drawn.
    fn draw_node(
        &mut self,
        response: Response,
        shown: Shown,
        doc: &mut Document,
        selected: &mut Selection,
        tool: Tool,
        style: &Style,
    ) -> Option<NodeGraphics> {
        let shape = tool.node_shape()?;
        let response = response.on_hover_cursor(CursorIcon::Crosshair);
        let to_graph = |pos: Pos2| graph_point(shown.to_screen, pos);
        if response.drag_started_by(PointerButton::Primary) {
            let from = response.ctx.input(|i| i.pointer.press_origin());
            self.shape_drag = from.map(|from| ShapeDrag {
                tool,
                from: to_graph(from),
            });
        }
        let drawing = !response.drag_stopped();
        let (drag, end) = if drawing {
            (self.shape_drag, response.interact_pointer_pos())
        } else {
            (self.shape_drag.take(), response.ctx.input(primary_release))
        };
        let (drag, end) = drag.zip(end.map(to_graph))?;
        // Within egui's click distance on screen a press is a click, which
        // draws nothing, even if it slips across a whole number.
        let max_click_dist = response.ctx.options(|o| o.input_options.max_click_dist);
        if drag.from.distance(end) * shown.to_screen.scaling <= max_click_dist {
            return None;
        }
        let (geometry, guides) = self.snap_shape(shown, &shape, drag.from, end);
        let graphics = new_node(&shape, geometry, style);
        if drawing {
            self.guides = guides;
            return Some(graphics);
        }
        if geometry.width > 0.0 && geometry.height > 0.0 {
            let graphml = doc.graphml_mut();
            let id = document::add_node(graphml, &graphics);
            // Boxes only: it leaves other shapes as they are.
            if let Some(radius) = style.corner_radius() {
                document::set_corner_radius(graphml, &id, radius);
            }
            *selected = Selection::node(id);
        }
        None
    }

    /// The bounds of a node of `shape` dragged out from `from` to `to`, in
    /// graph points: as wide as high if it's square, by the longer side of
    /// the drag. Its sides are moved to line up with those of nodes nearby,
    /// with the guides showing what lines up.
    fn snap_shape(
        &self,
        shown: Shown,
        shape: &NodeShape,
        from: Pos2,
        to: Pos2,
    ) -> (yed::Rect, Vec<Guide>) {
        let mut from = [from.x, from.y].map(f64::from);
        let pointer = [to.x, to.y].map(f64::from);
        let corner = |from: [f64; 2]| {
            let mut size = [0, 1].map(|i| pointer[i] - from[i]);
            if shape.square {
                let side = size[0].abs().max(size[1].abs());
                size = size.map(|size| side.copysign(size));
            }
            [0, 1].map(|i| from[i] + size[i])
        };
        let mut to = corner(from);
        let Some(targets) = self.targets(shown, |_| false) else {
            return (between(from, to), Vec::new());
        };
        // How far the sides at `corner` would move, the box spanning to `other`.
        let shifts = |corner: [f64; 2], other: [f64; 2]| {
            let rect = between(corner, other);
            Axis::BOTH.map(|axis| {
                let i = axis as usize;
                let side = if corner[i] <= other[i] {
                    Side::Start
                } else {
                    Side::End
                };
                targets.snap(&[Feature::side(&rect, axis, side)])[i]
            })
        };
        let shift = shifts(from, to);
        for i in 0..2 {
            from[i] += shift[i].unwrap_or_default();
        }
        to = corner(from);
        let shift = shifts(to, from);
        // The size across axis `i` with `to` moved by `shift`, unless too small.
        let size = |i: usize, to: [f64; 2], shift: f64| {
            let size = to[i] + shift - from[i];
            (size.abs() >= MIN_SIZE).then_some(size)
        };
        if shape.square {
            // By the side that moves least, the other keeping up.
            let nearest = (0..2)
                .filter_map(|i| Some((i, shift[i]?)))
                .min_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()));
            if let Some(side) = nearest.and_then(|(i, shift)| size(i, to, shift)) {
                to = [0, 1].map(|i| from[i] + side.abs().copysign(to[i] - from[i]));
            }
        } else {
            for i in 0..2 {
                if let Some(size) = shift[i].and_then(|shift| size(i, to, shift)) {
                    to[i] = from[i] + size;
                }
            }
        }
        let rect = between(from, to);
        let sides = Axis::BOTH.map(|axis| [Side::Start, Side::End].map(|side| (axis, side)));
        let features: Vec<_> = (sides.into_iter().flatten())
            .map(|(axis, side)| Feature::side(&rect, axis, side))
            .collect();
        (rect, targets.guides(&features))
    }

    /// What drags snap onto this frame, unless snapping is off or Alt is
    /// held: the nodes drawn on the canvas, but those `skip` picks by index.
    fn targets(&self, shown: Shown, skip: impl Fn(usize) -> bool) -> Option<Targets> {
        if !self.snapping {
            return None;
        }
        let zoom = f64::from(shown.to_screen.scaling);
        let mut targets = Targets::new(f64::from(SNAP_REACH) / zoom);
        let visible = shown.painter.clip_rect();
        for node in 0..shown.scene.nodes.len() {
            if !skip(node) && shown.node_rect(node).intersects(visible) {
                targets.add_node(shown.geometry(node));
            }
        }
        Some(targets)
    }

    /// `point`, in graph points, moved to line up with a side or middle of a
    /// node drawn on the canvas, or with one of `neighbours`, with the guides
    /// showing what lines up. Only the middles of the node at index
    /// `middles` count, e.g. of the node a port is on.
    fn snap_point(
        &self,
        shown: Shown,
        point: yed::Point,
        neighbours: &[yed::Point],
        middles: Option<usize>,
    ) -> (yed::Point, Vec<Guide>) {
        let Some(mut targets) = self.targets(shown, |node| Some(node) == middles) else {
            return (point, Vec::new());
        };
        if let Some(node) = middles {
            targets.add_middles(shown.geometry(node));
        }
        for &neighbour in neighbours {
            targets.add_point(neighbour);
        }
        let [dx, dy] = targets.snap(&Feature::point(point));
        let snapped = yed::Point {
            x: point.x + dx.unwrap_or_default(),
            y: point.y + dy.unwrap_or_default(),
        };
        (snapped, targets.guides(&Feature::point(snapped)))
    }

    /// Draws an edge in `style` with the primary button, if `tool` draws
    /// edges: from a node, through bends where it's let go elsewhere, to the
    /// node it's let go on, by a drag or by clicks. Adds it to `doc` and
    /// selects it once it ends on a node. Returns it while it's being drawn.
    fn draw_edge(
        &mut self,
        response: &Response,
        shown: Shown,
        doc: &mut Document,
        selected: &mut Selection,
        tool: Tool,
        style: &Style,
    ) -> Option<EdgeLook<'static>> {
        let arrows = tool.arrows()?;
        let response = response.clone().on_hover_cursor(CursorIcon::Crosshair);
        let scene = shown.scene;
        // Its node may have gone, e.g. by Delete.
        self.edge_draw
            .take_if(|draw| scene.node(&draw.source).is_none());
        let released = (response.clicked_by(PointerButton::Primary)
            || response.drag_stopped_by(PointerButton::Primary))
        .then(|| response.ctx.input(primary_release))
        .flatten();
        // Where a release off the nodes adds a bend, lined up with a node or
        // with the bend or node before.
        let bend = released.filter(|&pos| shown.node_at(pos, None).is_none());
        let bend = bend.zip(self.edge_draw.as_ref()).and_then(|(pos, draw)| {
            let at = yed_point(graph_point(shown.to_screen, pos));
            let before = self.point_before(shown, draw)?;
            Some(self.snap_point(shown, at, &[before], None).0)
        });
        match &mut self.edge_draw {
            None => {
                let start = if response.drag_started_by(PointerButton::Primary) {
                    response.ctx.input(|input| input.pointer.press_origin())
                } else {
                    released
                };
                if let Some(source) = start.and_then(|pos| shown.node_at(pos, None)) {
                    self.edge_draw = Some(EdgeDraw {
                        tool,
                        source: scene.nodes[source].id.clone(),
                        bends: Vec::new(),
                    });
                }
            }
            Some(draw) => {
                if let Some(pos) = released {
                    match shown.node_at(pos, None) {
                        // As where it started, or a click on its node.
                        Some(node)
                            if scene.nodes[node].id == draw.source && draw.bends.is_empty() => {}
                        Some(node) => {
                            let graphics = new_edge(arrows, &draw.bends, style);
                            let target = &scene.nodes[node].id;
                            let graphml = doc.graphml_mut();
                            let index =
                                document::add_edge(graphml, &draw.source, target, &graphics);
                            *selected = Selection::edge(index);
                            self.edge_draw = None;
                            return None;
                        }
                        None => {
                            // The second click of a double click adds none.
                            if let Some(bend) = bend
                                && draw.bends.last() != Some(&bend)
                            {
                                draw.bends.push(bend);
                            }
                        }
                    }
                }
            }
        }
        let draw = self.edge_draw.as_ref()?;
        let source = scene.node(&draw.source)?;
        let pointer = response.ctx.input(|input| input.pointer.latest_pos())?;
        // It can loop back to its node only through a bend.
        let target = match shown.node_at(pointer, None) {
            Some(node) if node != source || !draw.bends.is_empty() => Anchor::Node(node),
            _ => {
                let at = yed_point(graph_point(shown.to_screen, pointer));
                let before = self.point_before(shown, draw)?;
                let (at, guides) = self.snap_point(shown, at, &[before], None);
                self.guides = guides;
                Anchor::Point(egui_pos(at))
            }
        };
        Some(EdgeLook {
            graphics: Cow::Owned(new_edge(arrows, &draw.bends, style)),
            source: Anchor::Node(source),
            target,
        })
    }

    /// Where the edge being `draw`n runs to the pointer from, in graph
    /// points: its last bend, or else the middle of its node.
    fn point_before(&self, shown: Shown, draw: &EdgeDraw) -> Option<yed::Point> {
        let source = || Some(shown.center(shown.scene.node(&draw.source)?));
        draw.bends.last().copied().or_else(source)
    }

    /// Lets the parts of the only selected edge be dragged with the primary
    /// button: its bends and ends by their handles, its labels, and its line,
    /// which adds a bend. A click on a bend selects it. Changes the document
    /// on release, and returns the edge so far while it's being dragged.
    fn drag_edge(
        &mut self,
        ui: &egui::Ui,
        canvas: Response,
        shown: Shown,
        doc: &mut Document,
        selection: &mut Selection,
    ) -> Option<EdgeLook<'static>> {
        let Some(&Item::Edge(index)) = selection.single() else {
            return None;
        };
        let Some(edge) = shown.scene.edge(index) else {
            self.edge_drag = None;
            return None;
        };
        let look = EdgeLook::of(edge);
        let route = shown.unclipped(&look);
        let to_graph = |pos: Pos2| graph_point(shown.to_screen, pos);
        // The handles go over the canvas, so they take the drags and clicks
        // on them.
        let bends = route[1..route.len() - 1]
            .iter()
            .enumerate()
            .map(|(bend, &at)| {
                let rect = Rect::from_center_size(at, Vec2::splat(2.0 * GRAB));
                let id = Id::unique(("bend_handle", bend));
                let response = ui.interact(rect, id, Sense::click_and_drag());
                (
                    Some(EdgePart::Bend(bend)),
                    response.on_hover_cursor(CursorIcon::Move),
                )
            });
        let ends = [
            (EdgeEnd::Source, route[0]),
            (EdgeEnd::Target, route[route.len() - 1]),
        ];
        let ends = ends.map(|(end, at)| {
            let rect = Rect::from_center_size(at, Vec2::splat(2.0 * GRAB));
            let response = ui.interact(rect, Id::unique(("port_handle", end)), Sense::drag());
            (
                Some(EdgePart::End(end)),
                response.on_hover_cursor(CursorIcon::Move),
            )
        });
        let parts: Vec<_> = std::iter::once((None, canvas))
            .chain(bends.collect::<Vec<_>>())
            .chain(ends)
            .collect();
        for (part, response) in &parts {
            if let Some(EdgePart::Bend(bend)) = part
                && (response.clicked_by(PointerButton::Primary)
                    || response.drag_started_by(PointerButton::Primary))
            {
                selection.select_bend(*bend);
            }
            if response.drag_started_by(PointerButton::Primary) {
                // Like drawing a box, this applies a label being typed.
                ui.memory_mut(|memory| memory.stop_text_input());
                let from = ui.input(|input| input.pointer.press_origin());
                let part = from.and_then(|from| part.or_else(|| shown.edge_part_at(edge, from)));
                self.edge_drag = part.zip(from).map(|(part, from)| EdgeDrag {
                    part,
                    from: to_graph(from),
                });
            }
        }
        let drag = self.edge_drag?;
        let handle = match drag.part {
            EdgePart::Line(_) | EdgePart::Label(_) => None,
            part => Some(part),
        };
        let (_, response) = parts.iter().find(|(part, _)| *part == handle)?;
        if !response.drag_stopped() {
            let pointer = response.interact_pointer_pos()?;
            let (look, guides) = self.dragged_edge(edge, drag, pointer, shown);
            self.guides = guides;
            return Some(look);
        }
        self.edge_drag = None;
        if let Some(pointer) = ui.input(primary_release)
            && to_graph(pointer) != drag.from
        {
            let (moved, _) = self.dragged_edge(edge, drag, pointer, shown);
            commit_edge(doc.graphml_mut(), shown, edge, &moved, drag, selection);
        }
        None
    }

    /// How `edge` looks with `drag` taken to `pointer` on screen, with the
    /// guides showing what a dragged bend or port lines up with.
    fn dragged_edge(
        &self,
        edge: &SceneEdge,
        drag: EdgeDrag,
        pointer: Pos2,
        shown: Shown,
    ) -> (EdgeLook<'static>, Vec<Guide>) {
        let mut look = EdgeLook::of(edge).into_owned();
        let to = graph_point(shown.to_screen, pointer);
        let delta = to - drag.from;
        let (dx, dy) = (f64::from(delta.x), f64::from(delta.y));
        let old = shown.route(&look);
        // From port to port, in graph points.
        let route = shown.route_points(&look);
        let mut guides = Vec::new();
        let graphics = look.graphics.to_mut();
        let path = &mut graphics.path;
        match drag.part {
            EdgePart::Bend(bend) => {
                if let Some(point) = path.points.get_mut(bend) {
                    let moved = yed::Point {
                        x: point.x + dx,
                        y: point.y + dy,
                    };
                    let neighbours = [route[bend], route[bend + 2]];
                    (*point, guides) = self.snap_point(shown, moved, &neighbours, None);
                }
            }
            EdgePart::Line(segment) => {
                let segment = segment.min(path.points.len());
                let neighbours = [route[segment], route[segment + 1]];
                let (point, lined_up) = self.snap_point(shown, yed_point(to), &neighbours, None);
                path.points.insert(segment, point);
                guides = lined_up;
                let new = shown.route(&look);
                document::keep_labels(look.graphics.to_mut(), &old, &new);
            }
            EdgePart::End(end) => {
                let bends = !path.points.is_empty();
                let (port, anchor, node, other, neighbour) = match end {
                    EdgeEnd::Source => (
                        &mut path.source,
                        &mut look.source,
                        edge.source,
                        edge.target,
                        route[1],
                    ),
                    EdgeEnd::Target => (
                        &mut path.target,
                        &mut look.target,
                        edge.target,
                        edge.source,
                        route[route.len() - 2],
                    ),
                };
                let center = shown.center(node);
                let mut moved = yed::Point {
                    x: center.x + port.x + dx,
                    y: center.y + port.y + dy,
                };
                // Like a new edge, it loops back to its other node only through
                // a bend.
                let land = |at: yed::Point| {
                    let onto = shown.node_at(shown.to_screen * egui_pos(at), None);
                    onto.filter(|&onto| onto == node || onto != other || bends)
                };
                let landing = land(moved);
                // Onto the middles of the node it's on, where new edges' ports
                // are, as long as it stays on that node.
                let (snapped, lined_up) = self.snap_point(shown, moved, &[neighbour], landing);
                if land(snapped) == landing {
                    (moved, guides) = (snapped, lined_up);
                }
                match landing {
                    Some(onto) => {
                        let center = shown.center(onto);
                        let offset = yed::Point {
                            x: moved.x - center.x,
                            y: moved.y - center.y,
                        };
                        (*port, *anchor) = (offset, Anchor::Node(onto));
                    }
                    None => *anchor = Anchor::Point(egui_pos(moved)),
                }
            }
            EdgePart::Label(index) => {
                if let Some(label) = graphics.labels.get_mut(index) {
                    let bounds = shown.edge_label_bounds(&old, label);
                    let center = yed::Point {
                        x: bounds.x + bounds.width / 2.0 + f64::from(delta.x),
                        y: bounds.y + bounds.height / 2.0 + f64::from(delta.y),
                    };
                    label.place_on(&old, center, bounds.width, bounds.height);
                }
            }
        }
        (look, guides)
    }

    /// Starts what a drag of the primary button from the canvas does with the
    /// Cursor tool: from one of the selected items it moves them all, and
    /// from another item it selects that one first. From the empty canvas it
    /// draws a box, which selects the nodes and edges inside.
    /// [`Self::drag_edge`] takes drags of the line or labels of the only
    /// selected edge. `ctrl` adds to the selection rather than replacing it.
    fn start_drag(
        &mut self,
        canvas: &Response,
        shown: Shown,
        selected: &mut Selection,
        ctrl: bool,
    ) {
        if !canvas.drag_started_by(PointerButton::Primary) {
            return;
        }
        let Some(from) = canvas.ctx.input(|input| input.pointer.press_origin()) else {
            return;
        };
        // The selected items win over others on top, as they're the ones moved.
        let mut item = shown.item_at(from, Some(selected));
        if item.is_none() {
            item = shown.item_at(from, None);
            if let Some(item) = &item {
                if ctrl {
                    selected.add(item.clone());
                } else {
                    *selected = Selection::of([item.clone()]);
                }
            }
        }
        match item {
            None => {
                let from = shown.to_screen.inverse() * from;
                self.box_drag = Some(BoxDrag { from, add: ctrl });
            }
            Some(Item::Edge(_)) if selected.single().is_some() => {}
            Some(_) => {
                let from = graph_point(shown.to_screen, from);
                self.move_drag = Some(MoveDrag { from });
            }
        }
    }

    /// Moves the selected nodes and edges while a drag that
    /// [`Self::start_drag`] started goes on, changing the document on release.
    /// Returns how far right and down they've moved so far, in graph points.
    fn move_selection(
        &mut self,
        ui: &egui::Ui,
        canvas: &Response,
        shown: Shown,
        doc: &mut Document,
        selected: &Selection,
    ) -> Option<[f64; 2]> {
        let drag = self.move_drag?;
        let to_graph = |pos: Pos2| graph_point(shown.to_screen, pos);
        if !canvas.drag_stopped() {
            let to = canvas.interact_pointer_pos().map(to_graph)?;
            let (delta, guides) = self.snap_move(shown, selected, to - drag.from);
            self.guides = guides;
            return Some(delta);
        }
        self.move_drag = None;
        if let Some(to) = ui.input(primary_release).map(to_graph) {
            let ([dx, dy], _) = self.snap_move(shown, selected, to - drag.from);
            // Ending where it started changes nothing.
            if dx != 0.0 || dy != 0.0 {
                document::move_items(doc.graphml_mut(), selected.items(), dx, dy);
            }
        }
        None
    }

    /// How far right and down to move the `selected` nodes for a drag by
    /// `delta`, in graph points, so that one lines up with a node left in
    /// place, with the guides showing what lines up. With no nodes selected,
    /// the selected edges' bends line up instead.
    fn snap_move(&self, shown: Shown, selected: &Selection, delta: Vec2) -> ([f64; 2], Vec<Guide>) {
        let mut delta = [delta.x, delta.y].map(f64::from);
        let scene = shown.scene;
        let moving = |node: usize| selected.contains_node(&scene.nodes[node].id);
        let Some(targets) = self.targets(shown, moving) else {
            return (delta, Vec::new());
        };
        let features = |[dx, dy]: [f64; 2]| {
            let nodes = (0..scene.nodes.len()).filter(|&node| moving(node));
            let lines = nodes.flat_map(|node| {
                let mut rect = shown.geometry(node);
                (rect.x, rect.y) = (rect.x + dx, rect.y + dy);
                Feature::box_lines(&rect)
            });
            let mut features: Vec<_> = lines.collect();
            if features.is_empty() {
                let edges = (scene.edges.iter()).filter(|edge| selected.contains_edge(edge.index));
                let bends = edges.flat_map(|edge| edge.graphics.path.points.iter());
                let moved = |bend: &yed::Point| yed::Point {
                    x: bend.x + dx,
                    y: bend.y + dy,
                };
                features = bends.flat_map(|bend| Feature::point(moved(bend))).collect();
            }
            features
        };
        let shift = targets.snap(&features(delta));
        for (delta, shift) in delta.iter_mut().zip(shift) {
            *delta += shift.unwrap_or_default();
        }
        (delta, targets.guides(&features(delta)))
    }

    /// Selects the nodes inside the box that a drag [`Self::start_drag`]
    /// started draws, on release, with the edges between them whose bends
    /// are inside too. Returns the box so far on screen, and whether what it
    /// selects is to be added to the selection.
    fn select_in_box(
        &mut self,
        ui: &egui::Ui,
        canvas: &Response,
        shown: Shown,
        selected: &mut Selection,
    ) -> Option<(Rect, bool)> {
        let drag = self.box_drag?;
        let from = shown.to_screen * drag.from;
        if !canvas.drag_stopped() {
            let to = canvas.interact_pointer_pos()?;
            return Some((Rect::from_two_pos(from, to), drag.add));
        }
        self.box_drag = None;
        if let Some(to) = ui.input(primary_release) {
            let rect = Rect::from_two_pos(from, to);
            if !drag.add {
                *selected = Selection::default();
            }
            for (index, node) in shown.scene.nodes.iter().enumerate() {
                if shown.in_box(index, rect) {
                    selected.add(Item::Node(node.id.clone()));
                }
            }
            for edge in &shown.scene.edges {
                if shown.edge_in_box(edge, rect) {
                    selected.add(Item::Edge(edge.index));
                }
            }
        }
        None
    }

    /// Lets the node with id `id` be resized by dragging its [`HANDLES`] with
    /// the primary button. Changes its bounds on release, and returns them so
    /// far while it's being resized.
    fn resize_node(
        &mut self,
        ui: &egui::Ui,
        shown: Shown,
        doc: &mut Document,
        id: &str,
    ) -> Option<yed::Rect> {
        let index = shown.scene.node(id)?;
        let node = shown.node_rect(index);
        let geometry = shown.geometry(index);
        let to_graph = |pos: Pos2| graph_point(shown.to_screen, pos);
        let handles = HANDLES.map(|handle| {
            let rect = handle_rect(node, handle);
            let response = ui
                .interact(rect, Id::unique(("resize_handle", handle)), Sense::drag())
                .on_hover_cursor(resize_cursor(handle));
            (handle, response)
        });
        let mut resized = None;
        for (handle, response) in handles {
            if response.drag_started_by(PointerButton::Primary) {
                ui.memory_mut(|memory| memory.stop_text_input());
                let from = ui.input(|input| input.pointer.press_origin());
                self.resize = from.map(|from| Resize {
                    handle,
                    from: to_graph(from),
                });
            }
            let Some(drag) = self.resize.filter(|drag| drag.handle == handle) else {
                continue;
            };
            // Like yEd, which keeps the proportions while Shift is held.
            let keep = ui.input(|input| input.modifiers.shift);
            if !response.drag_stopped() {
                if let Some(to) = response.interact_pointer_pos().map(to_graph) {
                    let (new, guides) = self.snap_resize(shown, index, drag, to, keep);
                    (resized, self.guides) = (Some(new), guides);
                }
                continue;
            }
            self.resize = None;
            if let Some(to) = ui.input(primary_release).map(to_graph) {
                let (new, _) = self.snap_resize(shown, index, drag, to, keep);
                if new != geometry {
                    document::edit_graphics(doc.graphml_mut(), id, |graphics| {
                        document::resize(graphics, new);
                    });
                }
            }
        }
        resized
    }

    /// The bounds `drag` to `to` gives the node at index `node`, kept in
    /// proportion by a corner if `keep` says so, its dragged sides moved to
    /// line up with those of nodes nearby, with the guides showing what lines
    /// up.
    fn snap_resize(
        &self,
        shown: Shown,
        node: usize,
        drag: Resize,
        to: Pos2,
        keep: bool,
    ) -> (yed::Rect, Vec<Guide>) {
        let old = shown.geometry(node);
        let new = drag.geometry(old, to, keep);
        let Some(targets) = self.targets(shown, |other| other == node) else {
            return (new, Vec::new());
        };
        let sides = drag.sides();
        let features = |rect: &yed::Rect| -> Vec<_> {
            (sides.iter())
                .map(|&(axis, side)| Feature::side(rect, axis, side))
                .collect()
        };
        let snapped = drag.moved(old, new, targets.snap(&features(&new)), keep);
        (snapped, targets.guides(&features(&snapped)))
    }
}

impl Resize {
    /// The sides of the node that the handle drags.
    fn sides(&self) -> Vec<(Axis, Side)> {
        let sides = Axis::BOTH.into_iter().zip(self.handle.0);
        let sides = sides.filter_map(|(axis, align)| match align {
            Align::Min => Some((axis, Side::Start)),
            Align::Center => None,
            Align::Max => Some((axis, Side::End)),
        });
        sides.collect()
    }

    /// `new`, the bounds this drag gives `old`, with the sides it drags moved
    /// right and down by `shift`, but to no less than [`MIN_SIZE`]. Kept in
    /// proportion by a corner if `keep` says so, by the side that moves least.
    fn moved(
        &self,
        old: yed::Rect,
        new: yed::Rect,
        shift: [Option<f64>; 2],
        keep: bool,
    ) -> yed::Rect {
        // How long `new` is across `axis` with its side there moved by `shift`.
        let grown = |axis: Axis, shift: f64| {
            let side = self.handle.0[axis as usize];
            let size = extent(&new, axis) + if side == Align::Min { -shift } else { shift };
            (size >= MIN_SIZE).then_some(size)
        };
        let shifts = Axis::BOTH.into_iter().zip(shift);
        let shifts = shifts.filter_map(|(axis, shift)| Some((axis, shift?)));
        if keep && self.keeps(old) {
            let nearest = shifts.min_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()));
            let scale =
                nearest.and_then(|(axis, shift)| Some(grown(axis, shift)? / extent(&old, axis)));
            return scale.map_or(new, |scale| self.scaled(old, scale));
        }
        let mut rect = new;
        for (axis, shift) in shifts {
            let Some(size) = grown(axis, shift) else {
                continue;
            };
            let (start, extent) = match axis {
                Axis::X => (&mut rect.x, &mut rect.width),
                Axis::Y => (&mut rect.y, &mut rect.height),
            };
            if self.handle.0[axis as usize] == Align::Min {
                *start += shift;
            }
            *extent = size;
        }
        rect
    }

    /// Whether the handle keeps `old` in proportion while Shift is held: if
    /// it's a corner, and `old` has a size.
    fn keeps(&self, old: yed::Rect) -> bool {
        let [x_side, y_side] = self.handle.0;
        let corner = x_side != Align::Center && y_side != Align::Center;
        corner && old.width > 0.0 && old.height > 0.0
    }

    /// The bounds `old` would have with the handle dragged to `to`, in graph
    /// coordinates, kept in proportion by a corner if `keep` says so.
    fn geometry(&self, old: yed::Rect, to: Pos2, keep: bool) -> yed::Rect {
        let delta = to - self.from;
        let [x_side, y_side] = self.handle.0;
        let (x, width) = drag_side(old.x, old.width, x_side, delta.x.into());
        let (y, height) = drag_side(old.y, old.height, y_side, delta.y.into());
        if !(keep && self.keeps(old)) {
            return yed::Rect {
                x,
                y,
                width,
                height,
            };
        }
        // As far as the side dragged further, so it keeps up with the pointer.
        self.scaled(old, (width / old.width).max(height / old.height))
    }

    /// `old` scaled by `scale` from the corner opposite the handle, which
    /// stays put, but to no less than [`MIN_SIZE`].
    fn scaled(&self, old: yed::Rect, scale: f64) -> yed::Rect {
        let [x_side, y_side] = self.handle.0;
        let scale = scale.max(MIN_SIZE / old.width.min(old.height));
        let (width, height) = (old.width * scale, old.height * scale);
        let start = |start: f64, old: f64, new: f64, side: Align| match side {
            Align::Min => start + old - new,
            _ => start,
        };
        yed::Rect {
            x: start(old.x, old.width, width, x_side),
            y: start(old.y, old.height, height, y_side),
            width,
            height,
        }
    }
}

/// Moves the `side` of the span from `start` of length `size` by `delta`,
/// keeping it at least [`MIN_SIZE`] long. Returns its new start and length.
fn drag_side(start: f64, size: f64, side: Align, delta: f64) -> (f64, f64) {
    match side {
        Align::Min => {
            let delta = delta.min(size - MIN_SIZE);
            (start + delta, size - delta)
        }
        Align::Center => (start, size),
        Align::Max => (start, (size + delta).max(MIN_SIZE)),
    }
}

/// Where a drag grabs `handle` of `node`: within [`GRAB`] of its sides, with
/// the corners taking the ends of the sides.
fn handle_rect(node: Rect, handle: Align2) -> Rect {
    let range = |side, min: f32, max: f32| match side {
        Align::Min => Rangef::new(min - GRAB, min + GRAB),
        Align::Center => Rangef::new(min + GRAB, max - GRAB),
        Align::Max => Rangef::new(max - GRAB, max + GRAB),
    };
    Rect::from_x_y_ranges(
        range(handle.x(), node.min.x, node.max.x),
        range(handle.y(), node.min.y, node.max.y),
    )
}

fn resize_cursor(handle: Align2) -> CursorIcon {
    match handle.0 {
        [Align::Center, _] => CursorIcon::ResizeVertical,
        [_, Align::Center] => CursorIcon::ResizeHorizontal,
        [x, y] if x == y => CursorIcon::ResizeNwSe,
        _ => CursorIcon::ResizeNeSw,
    }
}

/// Selects the topmost node or edge a primary click lands on, or nothing if
/// it misses them all. With `ctrl`, the click adds the item to the selection,
/// or takes it out, and a miss keeps the selection.
fn select_clicked(response: &Response, shown: Shown, selected: &mut Selection, ctrl: bool) {
    // `clicked()` would count Enter and Space too, which have no position.
    if !response.clicked_by(PointerButton::Primary) {
        return;
    }
    let Some(pos) = response.ctx.input(primary_release) else {
        return;
    };
    match (shown.item_at(pos, None), ctrl) {
        (Some(item), true) => selected.toggle(item),
        (Some(item), false) => *selected = Selection::of([item]),
        (None, true) => {}
        (None, false) => *selected = Selection::default(),
    }
}

/// Makes the change to `edge` in `graphml` that `drag` made, which left it
/// as `moved`, and selects a bend it adds.
fn commit_edge(
    graphml: &mut GraphMl,
    shown: Shown,
    edge: &SceneEdge,
    moved: &EdgeLook,
    drag: EdgeDrag,
    selection: &mut Selection,
) {
    match drag.part {
        EdgePart::Bend(_) | EdgePart::Line(_) | EdgePart::Label(_) => {
            let edited = moved.graphics.as_ref();
            document::edit_edge_graphics(graphml, edge.index, |graphics| {
                graphics.path = edited.path.clone();
                graphics.labels = edited.labels.clone();
            });
            if let EdgePart::Line(bend) = drag.part {
                selection.select_bend(bend);
            }
        }
        EdgePart::End(end) => {
            let path = &moved.graphics.path;
            let port = match end {
                EdgeEnd::Source => path.source,
                EdgeEnd::Target => path.target,
            };
            // Off the nodes, it goes back.
            if let Anchor::Node(node) = moved.anchor(end) {
                let id = &shown.scene.nodes[node].id;
                document::move_port(graphml, edge.index, end, id, port);
            }
        }
    }
}

/// A new edge in `style`, with `arrows` and `bends` in graph coordinates.
fn new_edge(arrows: Arrows, bends: &[yed::Point], style: &Style) -> EdgeGraphics {
    let mut graphics = EdgeGraphics::new(EdgeKind::PolyLine);
    graphics.arrows = arrows;
    graphics.path.points = bends.to_vec();
    style.apply_to_edge(&mut graphics);
    graphics
}

/// Where the primary button was released this frame. A drag that stops
/// without a release was stopped by Escape, which cancels it.
fn primary_release(input: &egui::InputState) -> Option<Pos2> {
    button_event(input, PointerButton::Primary, false)
}

/// Where `button` was pressed, or released, this frame. The pointer may have
/// moved on since, in the same frame.
fn button_event(input: &egui::InputState, button: PointerButton, pressed: bool) -> Option<Pos2> {
    input.events.iter().find_map(|event| match *event {
        Event::PointerButton {
            pos,
            button: b,
            pressed: p,
            ..
        } if b == button && p == pressed => Some(pos),
        _ => None,
    })
}

/// The point in graph coordinates at `pos` on screen, in whole numbers,
/// which keep the saved coordinates tidy.
fn graph_point(to_screen: TSTransform, pos: Pos2) -> Pos2 {
    (to_screen.inverse() * pos).round()
}

/// A new node of `shape` in `style`, with bounds `geometry`.
fn new_node(shape: &NodeShape, geometry: yed::Rect, style: &Style) -> NodeGraphics {
    let mut graphics = new_shape(shape.shape.clone(), geometry);
    style.apply_to_node(&mut graphics);
    graphics
}

/// A `shape` with yEd's default look but filled with [`NEW_NODE_FILL`], with
/// bounds `geometry`.
fn new_shape(shape: ShapeType, geometry: yed::Rect) -> NodeGraphics {
    let mut graphics = NodeGraphics::new(NodeKind::Shape(shape));
    graphics.geometry = geometry;
    graphics.fill.color = Some(NEW_NODE_FILL);
    graphics
}

/// How long `rect` is across `axis`.
fn extent(rect: &yed::Rect, axis: Axis) -> f64 {
    match axis {
        Axis::X => rect.width,
        Axis::Y => rect.height,
    }
}

/// The box between two corners, in graph points.
fn between(corner: [f64; 2], opposite: [f64; 2]) -> yed::Rect {
    yed::Rect {
        x: corner[0].min(opposite[0]),
        y: corner[1].min(opposite[1]),
        width: (opposite[0] - corner[0]).abs(),
        height: (opposite[1] - corner[1]).abs(),
    }
}

#[cfg(test)]
mod tests;

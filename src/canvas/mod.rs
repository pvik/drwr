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

use crate::action::Action;
use crate::document::{self, EdgeEnd, Item, LabelRef};
use crate::history::Document;
use crate::scene::SceneEdge;
use crate::selection::Selection;
use crate::style::Style;
use crate::text_field::{self, End};
use crate::tool::{NodeShape, Tool};
use crate::zoom;

pub use drawing::{Drawing, Mark, Pen, Text};
use paint::{halign, label_font, label_rect};
pub use shown::Outline;
use shown::{Anchor, EdgeLook, Shown, egui_pos, yed_point, yed_rect};
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
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Options {
    pub grid_lines: bool,
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
    bends: Vec<Pos2>,
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
                _ => self.draw_node(response.clone(), to_screen, doc, selected, tool, style),
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
                moved = self.move_selection(ui, &response, to_screen, doc, selected);
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
            if let Some(delta) = moved {
                let edges = document::bends_moved_with(doc.graphml(), selected.items());
                scene.translate(selected, &edges, delta);
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
        to_screen: TSTransform,
        doc: &mut Document,
        selected: &mut Selection,
        tool: Tool,
        style: &Style,
    ) -> Option<NodeGraphics> {
        let shape = tool.node_shape()?;
        let response = response.on_hover_cursor(CursorIcon::Crosshair);
        let to_graph = |pos: Pos2| graph_point(to_screen, pos);
        // Within egui's click distance on screen a press is a click, which
        // draws nothing, even if it slips across a whole number.
        let max_click_dist = response.ctx.options(|o| o.input_options.max_click_dist);
        let node = |(drag, end): (ShapeDrag, Pos2)| {
            let distance = drag.from.distance(end) * to_screen.scaling;
            (distance > max_click_dist).then(|| new_node(&shape, drag.from, end, style))
        };
        if response.drag_started_by(PointerButton::Primary) {
            let from = response.ctx.input(|i| i.pointer.press_origin());
            self.shape_drag = from.map(|from| ShapeDrag {
                tool,
                from: to_graph(from),
            });
        }
        if !response.drag_stopped() {
            let end = response.interact_pointer_pos().map(to_graph);
            return self.shape_drag.zip(end).and_then(node);
        }

        let end = response.ctx.input(primary_release).map(to_graph);
        let graphics = self.shape_drag.take().zip(end).and_then(node);
        if let Some(graphics) = graphics
            && graphics.geometry.width > 0.0
            && graphics.geometry.height > 0.0
        {
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
                            let bend = graph_point(shown.to_screen, pos);
                            // The second click of a double click adds none.
                            if draw.bends.last() != Some(&bend) {
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
            _ => Anchor::Point(graph_point(shown.to_screen, pointer)),
        };
        Some(EdgeLook {
            graphics: Cow::Owned(new_edge(arrows, &draw.bends, style)),
            source: Anchor::Node(source),
            target,
        })
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
            return Some(self.dragged_edge(edge, drag, pointer, shown));
        }
        self.edge_drag = None;
        if let Some(pointer) = ui.input(primary_release)
            && to_graph(pointer) != drag.from
        {
            let moved = self.dragged_edge(edge, drag, pointer, shown);
            commit_edge(doc.graphml_mut(), shown, edge, &moved, drag, selection);
        }
        None
    }

    /// How `edge` looks with `drag` taken to `pointer` on screen.
    fn dragged_edge(
        &self,
        edge: &SceneEdge,
        drag: EdgeDrag,
        pointer: Pos2,
        shown: Shown,
    ) -> EdgeLook<'static> {
        let mut look = EdgeLook::of(edge).into_owned();
        let to = graph_point(shown.to_screen, pointer);
        let delta = to - drag.from;
        let old = shown.route(&look);
        let graphics = look.graphics.to_mut();
        let path = &mut graphics.path;
        match drag.part {
            EdgePart::Bend(bend) => {
                if let Some(point) = path.points.get_mut(bend) {
                    point.x += f64::from(delta.x);
                    point.y += f64::from(delta.y);
                }
            }
            EdgePart::Line(bend) => {
                let point = yed_point(to);
                path.points.insert(bend.min(path.points.len()), point);
                let new = shown.route(&look);
                document::keep_labels(look.graphics.to_mut(), &old, &new);
            }
            EdgePart::End(end) => {
                let bends = !path.points.is_empty();
                let (port, anchor, node, other) = match end {
                    EdgeEnd::Source => {
                        (&mut path.source, &mut look.source, edge.source, edge.target)
                    }
                    EdgeEnd::Target => {
                        (&mut path.target, &mut look.target, edge.target, edge.source)
                    }
                };
                let center = shown.center(node);
                let moved = yed::Point {
                    x: center.x + port.x + f64::from(delta.x),
                    y: center.y + port.y + f64::from(delta.y),
                };
                let at = egui_pos(moved);
                // Like a new edge, it loops back to its other node only through
                // a bend.
                let onto = shown
                    .node_at(shown.to_screen * at, None)
                    .filter(|&onto| onto == node || onto != other || bends);
                match onto {
                    Some(onto) => {
                        let center = shown.center(onto);
                        let mut offset = yed::Point {
                            x: moved.x - center.x,
                            y: moved.y - center.y,
                        };
                        // Near the middle it snaps there, where new edges' ports are.
                        let zoom = f64::from(shown.to_screen.scaling);
                        if offset.x.hypot(offset.y) * zoom <= f64::from(GRAB) {
                            offset = yed::Point::default();
                        }
                        (*port, *anchor) = (offset, Anchor::Node(onto));
                    }
                    None => *anchor = Anchor::Point(at),
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
        look
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
    /// Returns how far they've moved so far, in graph points.
    fn move_selection(
        &mut self,
        ui: &egui::Ui,
        canvas: &Response,
        to_screen: TSTransform,
        doc: &mut Document,
        selected: &Selection,
    ) -> Option<Vec2> {
        let drag = self.move_drag?;
        let to_graph = |pos: Pos2| graph_point(to_screen, pos);
        if !canvas.drag_stopped() {
            let to = canvas.interact_pointer_pos().map(to_graph)?;
            return Some(to - drag.from);
        }
        self.move_drag = None;
        if let Some(to) = ui.input(primary_release).map(to_graph)
            && to != drag.from
        {
            let delta = to - drag.from;
            let (dx, dy) = (delta.x.into(), delta.y.into());
            document::move_items(doc.graphml_mut(), selected.items(), dx, dy);
        }
        None
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
                let to = response.interact_pointer_pos().map(to_graph);
                resized = to.map(|to| drag.geometry(geometry, to, keep));
                continue;
            }
            self.resize = None;
            if let Some(to) = ui.input(primary_release).map(to_graph) {
                let new = drag.geometry(geometry, to, keep);
                if new != geometry {
                    document::edit_graphics(doc.graphml_mut(), id, |graphics| {
                        document::resize(graphics, new);
                    });
                }
            }
        }
        resized
    }
}

impl Resize {
    /// The bounds `old` would have with the handle dragged to `to`, in graph
    /// coordinates, kept in proportion by a corner if `keep` says so.
    fn geometry(&self, old: yed::Rect, to: Pos2, keep: bool) -> yed::Rect {
        let delta = to - self.from;
        let [x_side, y_side] = self.handle.0;
        let (x, width) = drag_side(old.x, old.width, x_side, delta.x.into());
        let (y, height) = drag_side(old.y, old.height, y_side, delta.y.into());
        let corner = x_side != Align::Center && y_side != Align::Center;
        if !(keep && corner && old.width > 0.0 && old.height > 0.0) {
            return yed::Rect {
                x,
                y,
                width,
                height,
            };
        }
        // As far as the side dragged further, so it keeps up with the pointer.
        let scale = (width / old.width).max(height / old.height);
        let scale = scale.max(MIN_SIZE / old.width.min(old.height));
        let (width, height) = (old.width * scale, old.height * scale);
        // From the opposite corner, which stays put.
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
fn new_edge(arrows: Arrows, bends: &[Pos2], style: &Style) -> EdgeGraphics {
    let mut graphics = EdgeGraphics::new(EdgeKind::PolyLine);
    graphics.arrows = arrows;
    graphics.path.points = bends.iter().copied().map(yed_point).collect();
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

/// A new node of `shape` in `style`, dragged out between two corners in graph
/// coordinates.
fn new_node(shape: &NodeShape, corner: Pos2, opposite: Pos2, style: &Style) -> NodeGraphics {
    let mut size = opposite - corner;
    if shape.square {
        // The longer side of the drag, so the node keeps up with the pointer.
        let side = size.abs().max_elem();
        size = vec2(side.copysign(size.x), side.copysign(size.y));
    }
    let mut graphics = new_shape(shape.shape.clone(), corner, corner + size);
    style.apply_to_node(&mut graphics);
    graphics
}

/// A `shape` with yEd's default look but filled with [`NEW_NODE_FILL`],
/// spanning two corners in graph coordinates.
fn new_shape(shape: ShapeType, corner: Pos2, opposite: Pos2) -> NodeGraphics {
    let mut graphics = NodeGraphics::new(NodeKind::Shape(shape));
    graphics.geometry = yed_rect(Rect::from_two_pos(corner, opposite));
    graphics.fill.color = Some(NEW_NODE_FILL);
    graphics
}

#[cfg(test)]
mod tests;

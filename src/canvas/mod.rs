mod edge;

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use egui::emath::TSTransform;
use egui::epaint::{CircleShape, PathShape, TextShape};
use egui::text::LayoutJob;
use egui::{
    Align, Align2, Color32, CursorIcon, Event, FontId, Galley, Id, Margin, Painter, PointerButton,
    Pos2, Rangef, Rect, Response, Sense, Stroke, StrokeKind, UiBuilder, Vec2, Visuals, pos2, vec2,
};
use graphml::GraphMl;
use graphml::yed::{
    self, Alignment, Arrows, EdgeGraphics, EdgeKind, Label, NodeGraphics, NodeKind, Route,
    ShapeType,
};

use crate::action::Action;
use crate::color_picker::color32;
use crate::document::{self, EdgeEnd, Item, LabelRef};
use crate::fonts;
use crate::line_type;
use crate::selection::Selection;
use crate::text_field::{self, End};
use crate::tool::{NodeShape, Tool};
use crate::zoom;

/// White like paper, even in the dark theme.
const BACKGROUND: Color32 = Color32::WHITE;

const SELECTION: Stroke = Stroke {
    width: 2.0,
    color: Color32::from_rgb(0x00, 0x78, 0xD4),
};
/// Between the selected node's bounds and its outline.
const SELECTION_GAP: f32 = 2.0;
/// Under the selected edge, this wide either side of its line.
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

/// The drawing canvas, which shows the graph zoomed and panned.
#[derive(Default)]
pub struct Canvas {
    view: View,
    /// The size it was last shown at, to zoom about its middle.
    size: Vec2,
    /// The zoom it was last shown at.
    shown_zoom: f32,
    /// The zoom labels are laid out at, which lags while the zoom changes.
    text_zoom: f32,
    /// While the middle button pans the view, where the pointer was last.
    panning: Option<Pos2>,
    shape_drag: Option<ShapeDrag>,
    node_drag: Option<NodeDrag>,
    edge_draw: Option<EdgeDraw>,
    edge_drag: Option<EdgeDrag>,
    editor: Option<LabelEditor>,
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

/// A drag of the selected node, from inside it or by one of its [`HANDLES`].
#[derive(Clone, Copy)]
struct NodeDrag {
    /// `None` when dragged from inside, which moves the node.
    handle: Option<Align2>,
    /// Where the drag started, in graph coordinates.
    from: Pos2,
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

/// The top-level graph's nodes and edges, as the canvas shows them.
struct Scene {
    nodes: Vec<SceneNode>,
    /// Those whose nodes are both in `nodes`.
    edges: Vec<SceneEdge>,
}

struct SceneNode {
    id: String,
    graphics: NodeGraphics,
    /// The corner radius of a box.
    radius: Option<f64>,
}

struct SceneEdge {
    /// Its index in the document.
    index: usize,
    /// The indexes of its nodes in the scene.
    source: usize,
    target: usize,
    graphics: EdgeGraphics,
}

/// Where an end of an edge is: on a node of the scene, by its index, or at a
/// point in graph coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Anchor {
    Node(usize),
    Point(Pos2),
}

/// An edge as it's drawn, which can differ from the document's while it's
/// being drawn or dragged.
#[derive(Clone)]
struct EdgeLook<'a> {
    graphics: Cow<'a, EdgeGraphics>,
    source: Anchor,
    target: Anchor,
}

/// The scene as it's shown this frame.
#[derive(Clone, Copy)]
struct Shown<'a> {
    scene: &'a Scene,
    to_screen: TSTransform,
    /// A node being moved or resized, by its index, and its bounds so far.
    dragged: Option<(usize, yed::Rect)>,
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

impl Canvas {
    /// Shows the canvas in the space left over by the other panels. Returns
    /// the actions asked for.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        tool: Tool,
        graphml: &mut GraphMl,
        selected: &mut Option<Selection>,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        egui::CentralPanel::default().show(ui, |ui| {
            let sense = match tool {
                Tool::Cursor => Sense::click_and_drag(),
                _ if tool.node_shape().is_some() => Sense::drag(),
                // Edges are drawn by clicks as well as drags.
                _ if tool.arrows().is_some() => Sense::click_and_drag(),
                _ => Sense::hover(),
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

            let scene = Scene::read(graphml);
            let shown = Shown {
                scene: &scene,
                to_screen,
                dragged: None,
            };
            let mut double_clicked = false;
            let mut drawn_edge = None;
            let drawing = match tool {
                Tool::Cursor => {
                    select_clicked(&response, &painter, shown, text_zoom, selected);
                    double_clicked =
                        response.double_clicked_by(PointerButton::Primary) && selected.is_some();
                    None
                }
                _ if tool.arrows().is_some() => {
                    drawn_edge = self.draw_edge(&response, shown, graphml, selected, tool);
                    None
                }
                _ => self.draw_node(response.clone(), to_screen, graphml, selected, tool),
            };
            let (mut dragged, mut dragged_edge) = (None, None);
            match (tool, selected.as_mut()) {
                (Tool::Cursor, Some(selection)) => match selection.item.clone() {
                    Item::Node(id) => {
                        self.edge_drag = None;
                        let geometry = self.drag_node(ui, response, to_screen, graphml, &id);
                        dragged = geometry.map(|geometry| (id, geometry));
                    }
                    Item::Edge(_) => {
                        self.node_drag = None;
                        dragged_edge = self.drag_edge(
                            ui, response, &painter, shown, text_zoom, graphml, selection,
                        );
                    }
                },
                // Switching tools mid-drag drops the move or resize, as it does
                // the shape.
                _ => {
                    self.node_drag = None;
                    self.edge_drag = None;
                }
            }

            // What a release did shows in the same frame.
            let released = ui.input(primary_release).is_some();
            let scene = if released {
                Scene::read(graphml)
            } else {
                scene
            };
            let dragged = dragged.and_then(|(id, geometry)| Some((scene.node(&id)?, geometry)));
            let shown = Shown {
                scene: &scene,
                to_screen,
                dragged,
            };
            painter.rect_filled(rect, 0.0, BACKGROUND);
            let editing = self.editor.as_ref().map(|editor| &editor.label);
            // The label being edited shows in its editor instead.
            let hidden = |item: &Item| {
                let label = editing.filter(|label| label.item == *item);
                label.map(|label| label.index)
            };
            let mut outline = None;
            for (index, node) in scene.nodes.iter().enumerate() {
                let item = Item::Node(node.id.clone());
                let mut resized = None;
                if let Some((dragged, geometry)) = shown.dragged
                    && dragged == index
                {
                    let mut graphics = node.graphics.clone();
                    document::resize(&mut graphics, geometry);
                    resized = Some(graphics);
                }
                let graphics = resized.as_ref().unwrap_or(&node.graphics);
                if selected
                    .as_ref()
                    .is_some_and(|selection| selection.item == item)
                {
                    outline = Some(screen_rect(to_screen, graphics).expand(SELECTION_GAP));
                }
                paint(
                    &painter,
                    to_screen,
                    text_zoom,
                    graphics,
                    node.radius,
                    hidden(&item),
                );
            }
            let selected_edge = selected
                .as_ref()
                .and_then(|selection| match selection.item {
                    Item::Edge(index) => Some(index),
                    Item::Node(_) => None,
                });
            for edge in &scene.edges {
                let item = Item::Edge(edge.index);
                let chosen = selected_edge == Some(edge.index);
                let look = match dragged_edge.as_ref().filter(|_| chosen) {
                    Some(look) => look.clone(),
                    None => EdgeLook::of(edge),
                };
                paint_edge(&painter, shown, text_zoom, &look, chosen, hidden(&item));
            }
            if let Some(look) = &drawn_edge {
                paint_edge(&painter, shown, text_zoom, look, false, None);
            }
            // The node the edge being drawn, or the port being dragged, would
            // end on.
            let ending = match (&drawn_edge, self.edge_drag) {
                (Some(look), _) => Some(look.target),
                (
                    None,
                    Some(EdgeDrag {
                        part: EdgePart::End(end),
                        ..
                    }),
                ) => dragged_edge.as_ref().map(|look| look.anchor(end)),
                _ => None,
            };
            if let Some(Anchor::Node(node)) = ending {
                let rect = shown.node_rect(node).expand(SELECTION_GAP);
                painter.rect_stroke(rect, 0.0, SELECTION, StrokeKind::Outside);
            }
            if let Some(graphics) = drawing {
                // New boxes have square corners.
                paint(&painter, to_screen, text_zoom, &graphics, None, None);
            }
            // Over the nodes, so one on top can't hide it.
            if let Some(outline) = outline {
                painter.rect_stroke(outline, 0.0, SELECTION, StrokeKind::Outside);
            }
            if let Some(selection) = selected.as_ref()
                && let Some(index) = selected_edge
                && let Some(edge) = scene.edge(index)
            {
                let look = dragged_edge.clone().unwrap_or_else(|| EdgeLook::of(edge));
                paint_handles(&painter, shown, &look, selection.bend);
            }
            // Outlined, since the light theme's panels are nearly white too.
            let border = ui.visuals().widgets.noninteractive.bg_stroke;
            painter.rect_stroke(rect, 0.0, border, StrokeKind::Inside);

            // Over the nodes. An edit it applies goes first, in case the
            // double click is on the same label.
            actions.extend(self.show_editor(ui, shown));
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

    /// Shows the label being edited, if there is one. Returns the edit once
    /// Enter or a click elsewhere applies it.
    fn show_editor(&mut self, ui: &mut egui::Ui, shown: Shown) -> Option<Action> {
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
        let label = labels.get(editor.label.index).cloned();
        let label = label.unwrap_or_else(|| match editor.label.item {
            Item::Node(_) => Label::new(""),
            Item::Edge(_) => Label::new_on_edge(""),
        });
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

    /// Drags out a node with the primary button, if `tool` draws nodes,
    /// adding it to `graphml` and selecting it on release. Returns the node
    /// while it's being drawn.
    fn draw_node(
        &mut self,
        response: Response,
        to_screen: TSTransform,
        graphml: &mut GraphMl,
        selected: &mut Option<Selection>,
        tool: Tool,
    ) -> Option<NodeGraphics> {
        let shape = tool.node_shape()?;
        let response = response.on_hover_cursor(CursorIcon::Crosshair);
        let to_graph = |pos: Pos2| graph_point(to_screen, pos);
        // Within egui's click distance on screen a press is a click, which
        // draws nothing, even if it slips across a whole number.
        let max_click_dist = response.ctx.options(|o| o.input_options.max_click_dist);
        let node = |(drag, end): (ShapeDrag, Pos2)| {
            let distance = drag.from.distance(end) * to_screen.scaling;
            (distance > max_click_dist).then(|| new_node(&shape, drag.from, end))
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

        // Without a release, Escape stopped the drag, which cancels the node.
        let end = response.ctx.input(primary_release).map(to_graph);
        let graphics = self.shape_drag.take().zip(end).and_then(node);
        if let Some(graphics) = graphics
            && graphics.geometry.width > 0.0
            && graphics.geometry.height > 0.0
        {
            *selected = Some(Selection::node(document::add_node(graphml, &graphics)));
        }
        None
    }

    /// Draws an edge with the primary button, if `tool` draws edges: from a
    /// node, through bends where it's let go elsewhere, to the node it's let
    /// go on, by a drag or by clicks. Adds it to `graphml` and selects it once
    /// it ends on a node. Returns it while it's being drawn.
    fn draw_edge(
        &mut self,
        response: &Response,
        shown: Shown,
        graphml: &mut GraphMl,
        selected: &mut Option<Selection>,
        tool: Tool,
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
                if let Some(source) = start.and_then(|pos| shown.node_at(pos)) {
                    self.edge_draw = Some(EdgeDraw {
                        tool,
                        source: scene.nodes[source].id.clone(),
                        bends: Vec::new(),
                    });
                }
            }
            Some(draw) => {
                if let Some(pos) = released {
                    match shown.node_at(pos) {
                        // As where it started, or a click on its node.
                        Some(node)
                            if scene.nodes[node].id == draw.source && draw.bends.is_empty() => {}
                        Some(node) => {
                            let graphics = new_edge(arrows, &draw.bends);
                            let target = &scene.nodes[node].id;
                            let index =
                                document::add_edge(graphml, &draw.source, target, &graphics);
                            *selected = Some(Selection::edge(index));
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
        let target = match shown.node_at(pointer) {
            Some(node) if node != source || !draw.bends.is_empty() => Anchor::Node(node),
            _ => Anchor::Point(graph_point(shown.to_screen, pointer)),
        };
        Some(EdgeLook {
            graphics: Cow::Owned(new_edge(arrows, &draw.bends)),
            source: Anchor::Node(source),
            target,
        })
    }

    /// Lets the parts of the selected edge be dragged with the primary
    /// button: its bends and ends by their handles, its labels, and its line,
    /// which adds a bend. A click on a bend selects it. Changes the document
    /// on release, and returns the edge so far while it's being dragged.
    #[allow(clippy::too_many_arguments)]
    fn drag_edge(
        &mut self,
        ui: &egui::Ui,
        canvas: Response,
        painter: &Painter,
        shown: Shown,
        text_zoom: f32,
        graphml: &mut GraphMl,
        selection: &mut Selection,
    ) -> Option<EdgeLook<'static>> {
        let Item::Edge(index) = selection.item else {
            return None;
        };
        let Some(edge) = shown.scene.edge(index) else {
            self.edge_drag = None;
            return None;
        };
        let look = EdgeLook::of(edge);
        let path = shown.path(&look);
        let route = shown.unclipped(&look);
        let to_graph = |pos: Pos2| graph_point(shown.to_screen, pos);
        // The part of the edge, other than a handle, at `pos` on screen.
        let owner = Owner::Edge(shown.route(&look));
        let part_at = |pos: Pos2| {
            let labels = edge.graphics.labels.iter().enumerate().rev();
            let label = labels
                .filter(|(_, label)| label.visible)
                .find(|(_, label)| {
                    label_layout(painter, shown.to_screen, text_zoom, &owner, label)
                        .1
                        .contains(pos)
                });
            if let Some((label, _)) = label {
                return Some(EdgePart::Label(label));
            }
            let near = edge::nearest_segment(&path, pos).is_some_and(|(_, d)| d <= GRAB);
            // The bend goes on the segment of the whole route nearest `pos`.
            near.then(|| edge::nearest_segment(&route, pos))
                .flatten()
                .map(|(segment, _)| EdgePart::Line(segment))
        };
        if self.edge_drag.is_some() || canvas.hover_pos().is_some_and(|pos| part_at(pos).is_some())
        {
            ui.ctx().set_cursor_icon(CursorIcon::Move);
        }
        // The handles go over the canvas, so they take the drags and clicks
        // on them.
        let bends = edge
            .graphics
            .path
            .points
            .iter()
            .enumerate()
            .map(|(bend, point)| {
                let at = shown.to_screen * pos2(point.x as f32, point.y as f32);
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
                selection.bend = Some(*bend);
            }
            if response.drag_started_by(PointerButton::Primary) {
                // Like drawing a box, this applies a label being typed.
                ui.memory_mut(|memory| memory.stop_text_input());
                let from = ui.input(|input| input.pointer.press_origin());
                let part = from.and_then(|from| part.or_else(|| part_at(from)));
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
            return Some(self.dragged_edge(edge, drag, pointer, painter, shown, text_zoom));
        }
        self.edge_drag = None;
        // Without a release, Escape stopped the drag, which cancels it.
        if let Some(pointer) = ui.input(primary_release)
            && to_graph(pointer) != drag.from
        {
            let moved = self.dragged_edge(edge, drag, pointer, painter, shown, text_zoom);
            commit_edge(graphml, shown, edge, &moved, drag, selection);
        }
        None
    }

    /// How `edge` looks with `drag` taken to `pointer` on screen.
    fn dragged_edge(
        &self,
        edge: &SceneEdge,
        drag: EdgeDrag,
        pointer: Pos2,
        painter: &Painter,
        shown: Shown,
        text_zoom: f32,
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
                let point = yed::Point {
                    x: to.x.into(),
                    y: to.y.into(),
                };
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
                let at = pos2(moved.x as f32, moved.y as f32);
                // Like a new edge, it loops back to its other node only through
                // a bend.
                let onto = shown
                    .node_at(shown.to_screen * at)
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
                    let bounds = edge_label_bounds(painter, text_zoom, &old, label);
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

    /// Lets the node with id `id` be dragged with the primary button, from
    /// inside on `canvas` to move it, or by its [`HANDLES`] to resize it.
    /// Changes its bounds on release, and returns them so far while it's
    /// being dragged.
    fn drag_node(
        &mut self,
        ui: &egui::Ui,
        canvas: Response,
        to_screen: TSTransform,
        graphml: &mut GraphMl,
        id: &str,
    ) -> Option<yed::Rect> {
        let graphics = document::graphics_of(graphml, id)?;
        let corner_radius = document::corner_radius(graphml, id);
        let node = screen_rect(to_screen, &graphics);
        let geometry = graphics.geometry;
        let to_graph = |pos: Pos2| graph_point(to_screen, pos);
        let inside = |pos: Pos2| hit(to_screen, &graphics, corner_radius, pos);
        let moving = self.node_drag.is_some_and(|drag| drag.handle.is_none());
        if moving || canvas.hover_pos().is_some_and(inside) {
            ui.ctx().set_cursor_icon(CursorIcon::Move);
        }
        // The handles go over the canvas, so they take the drags and clicks
        // on them.
        let handles = HANDLES.map(|handle| {
            let rect = handle_rect(node, handle);
            let response = ui
                .interact(rect, Id::unique(("resize_handle", handle)), Sense::drag())
                .on_hover_cursor(resize_cursor(handle));
            (Some(handle), response)
        });
        let mut dragged = None;
        for (handle, response) in std::iter::once((None, canvas)).chain(handles) {
            if response.drag_started_by(PointerButton::Primary) {
                // Like drawing a box, this applies a label being typed.
                ui.memory_mut(|memory| memory.stop_text_input());
                let from = ui.input(|input| input.pointer.press_origin());
                // Drags on the canvas move the node only from inside it.
                let from = from.filter(|&from| handle.is_some() || inside(from));
                self.node_drag = from.map(|from| NodeDrag {
                    handle,
                    from: to_graph(from),
                });
            }
            let Some(drag) = self.node_drag.filter(|drag| drag.handle == handle) else {
                continue;
            };
            if !response.drag_stopped() {
                let to = response.interact_pointer_pos().map(to_graph);
                dragged = to.map(|to| drag.geometry(geometry, to));
                continue;
            }
            self.node_drag = None;
            // Without a release, Escape stopped the drag, which cancels it.
            if let Some(to) = ui.input(primary_release).map(to_graph) {
                let new = drag.geometry(geometry, to);
                if new != geometry {
                    document::edit_graphics(graphml, id, |graphics| {
                        document::resize(graphics, new);
                    });
                }
            }
        }
        dragged
    }
}

impl NodeDrag {
    /// The bounds `old` would have with the node or handle dragged to `to`, in
    /// graph coordinates.
    fn geometry(&self, old: yed::Rect, to: Pos2) -> yed::Rect {
        let delta = to - self.from;
        let Some(handle) = self.handle else {
            return yed::Rect {
                x: old.x + f64::from(delta.x),
                y: old.y + f64::from(delta.y),
                ..old
            };
        };
        let (x, width) = drag_side(old.x, old.width, handle.x(), delta.x.into());
        let (y, height) = drag_side(old.y, old.height, handle.y(), delta.y.into());
        yed::Rect {
            x,
            y,
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
/// it misses them all. Edges are painted over the nodes.
fn select_clicked(
    response: &Response,
    painter: &Painter,
    shown: Shown,
    text_zoom: f32,
    selected: &mut Option<Selection>,
) {
    // `clicked()` would count Enter and Space too, which have no position.
    if !response.clicked_by(PointerButton::Primary) {
        return;
    }
    let Some(pos) = response.ctx.input(primary_release) else {
        return;
    };
    let edge = shown.scene.edges.iter().rev().find(|edge| {
        let look = EdgeLook::of(edge);
        let owner = Owner::Edge(shown.route(&look));
        let labels = edge.graphics.labels.iter().filter(|label| label.visible);
        labels
            .map(|label| label_layout(painter, shown.to_screen, text_zoom, &owner, label).1)
            .any(|rect| rect.contains(pos))
            || edge::nearest_segment(&shown.path(&look), pos).is_some_and(|(_, d)| d <= GRAB)
    });
    *selected = match edge {
        Some(edge) => Some(Selection::edge(edge.index)),
        None => shown
            .node_at(pos)
            .map(|node| Selection::node(&shown.scene.nodes[node].id)),
    };
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
                selection.bend = Some(bend);
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

/// A new edge with yEd's default look, with `arrows` and `bends` in graph
/// coordinates.
fn new_edge(arrows: Arrows, bends: &[Pos2]) -> EdgeGraphics {
    let mut graphics = EdgeGraphics::new(EdgeKind::PolyLine);
    graphics.arrows = arrows;
    let bends = bends.iter().map(|bend| yed::Point {
        x: bend.x.into(),
        y: bend.y.into(),
    });
    graphics.path.points = bends.collect();
    graphics
}

impl Scene {
    fn read(graphml: &GraphMl) -> Self {
        let nodes: Vec<SceneNode> = document::node_graphics(graphml)
            .map(|(id, graphics, radius)| SceneNode {
                id: id.to_owned(),
                graphics,
                radius,
            })
            .collect();
        let ids: HashMap<&str, usize> = (nodes.iter().enumerate())
            .map(|(index, node)| (node.id.as_str(), index))
            .collect();
        let edges = document::edge_graphics(graphml)
            .filter_map(|(index, edge, graphics)| {
                Some(SceneEdge {
                    index,
                    source: *ids.get(edge.source.as_str())?,
                    target: *ids.get(edge.target.as_str())?,
                    graphics,
                })
            })
            .collect();
        Scene { nodes, edges }
    }

    /// The index of the node with id `id`.
    fn node(&self, id: &str) -> Option<usize> {
        self.nodes.iter().position(|node| node.id == id)
    }

    /// The edge at `index` in the document.
    fn edge(&self, index: usize) -> Option<&SceneEdge> {
        self.edges.iter().find(|edge| edge.index == index)
    }
}

impl<'a> EdgeLook<'a> {
    /// `edge` as the document has it.
    fn of(edge: &'a SceneEdge) -> Self {
        EdgeLook {
            graphics: Cow::Borrowed(&edge.graphics),
            source: Anchor::Node(edge.source),
            target: Anchor::Node(edge.target),
        }
    }

    fn into_owned(self) -> EdgeLook<'static> {
        EdgeLook {
            graphics: Cow::Owned(self.graphics.into_owned()),
            source: self.source,
            target: self.target,
        }
    }

    fn anchor(&self, end: EdgeEnd) -> Anchor {
        match end {
            EdgeEnd::Source => self.source,
            EdgeEnd::Target => self.target,
        }
    }
}

impl Shown<'_> {
    /// The bounds of the node at index `node`, as dragged so far.
    fn geometry(&self, node: usize) -> yed::Rect {
        match self.dragged {
            Some((dragged, geometry)) if dragged == node => geometry,
            _ => self.scene.nodes[node].graphics.geometry,
        }
    }

    /// Where the bounds of the node at index `node` are on screen.
    fn node_rect(&self, node: usize) -> Rect {
        egui_rect(self.geometry(node), self.to_screen)
    }

    /// The middle of the node at index `node`, in graph coordinates.
    fn center(&self, node: usize) -> yed::Point {
        let node = self.geometry(node);
        yed::Point {
            x: node.x + node.width / 2.0,
            y: node.y + node.height / 2.0,
        }
    }

    /// Whether `pos` on screen is inside the node at index `node`, or its
    /// bounds if the canvas can't draw it.
    fn inside(&self, node: usize, pos: Pos2) -> bool {
        let SceneNode {
            graphics, radius, ..
        } = &self.scene.nodes[node];
        let rect = self.node_rect(node);
        inside(shape(graphics, *radius), rect, self.to_screen.scaling, pos)
    }

    /// The index of the topmost node the canvas draws at `pos` on screen.
    fn node_at(&self, pos: Pos2) -> Option<usize> {
        let nodes = self.scene.nodes.iter().enumerate().rev();
        let drawn = nodes.filter(|(_, node)| shape(&node.graphics, node.radius).is_some());
        drawn
            .map(|(index, _)| index)
            .find(|&index| self.inside(index, pos))
    }

    /// Where `look` runs on screen, from port to port through its bends.
    fn unclipped(&self, look: &EdgeLook) -> Vec<Pos2> {
        let port = |anchor: Anchor, offset: yed::Point| match anchor {
            Anchor::Node(node) => {
                let center = self.center(node);
                pos2((center.x + offset.x) as f32, (center.y + offset.y) as f32)
            }
            Anchor::Point(point) => point,
        };
        let path = &look.graphics.path;
        let bends = path
            .points
            .iter()
            .map(|point| pos2(point.x as f32, point.y as f32));
        std::iter::once(port(look.source, path.source))
            .chain(bends)
            .chain(std::iter::once(port(look.target, path.target)))
            .map(|point| self.to_screen * point)
            .collect()
    }

    /// Where `look` is painted on screen: cut where it leaves its nodes.
    fn path(&self, look: &EdgeLook) -> Vec<Pos2> {
        let zoom = self.to_screen.scaling;
        let inside = |anchor: Anchor| {
            let node = match anchor {
                Anchor::Node(node) => {
                    let SceneNode {
                        graphics, radius, ..
                    } = &self.scene.nodes[node];
                    Some((shape(graphics, *radius), self.node_rect(node)))
                }
                Anchor::Point(_) => None,
            };
            move |pos| node.is_some_and(|(outline, rect)| inside(outline, rect, zoom, pos))
        };
        edge::clip(
            self.unclipped(look),
            inside(look.source),
            inside(look.target),
        )
    }

    /// The route yEd places the labels of `look` along.
    fn route(&self, look: &EdgeLook) -> Route {
        let bounds = |anchor: Anchor| match anchor {
            Anchor::Node(node) => self.geometry(node),
            Anchor::Point(point) => yed::Rect {
                x: point.x.into(),
                y: point.y.into(),
                width: 0.0,
                height: 0.0,
            },
        };
        Route::new(
            &look.graphics.path,
            bounds(look.source),
            bounds(look.target),
        )
    }
}

/// Paints an edge as `look` has it, with its labels but the one at index
/// `hidden`, over a halo if it's `chosen`.
fn paint_edge(
    painter: &Painter,
    shown: Shown,
    text_zoom: f32,
    look: &EdgeLook,
    chosen: bool,
    hidden: Option<usize>,
) {
    let path = shown.path(look);
    let line = &look.graphics.line;
    let width = line.width as f32 * shown.to_screen.scaling;
    if chosen {
        let halo = Stroke::new(width + 2.0 * HALO.width, HALO.color);
        painter.add(PathShape::line(path.clone(), halo));
    }
    let stroke = line
        .color
        .map_or(Stroke::NONE, |color| Stroke::new(width, color32(color)));
    let arrows = &look.graphics.arrows;
    edge::paint(painter, path, stroke, &line.line_type, arrows);
    let owner = Owner::Edge(shown.route(look));
    for (index, label) in look.graphics.labels.iter().enumerate() {
        if label.visible && hidden != Some(index) {
            paint_label(painter, shown.to_screen, text_zoom, &owner, label);
        }
    }
}

/// Paints the handles on the ports and bends of the selected edge, as `look`
/// has it, filling the one on the bend at index `bend`.
fn paint_handles(painter: &Painter, shown: Shown, look: &EdgeLook, bend: Option<usize>) {
    let route = shown.unclipped(look);
    let outline = Stroke::new(1.0, SELECTION.color);
    for port in [route[0], route[route.len() - 1]] {
        painter.add(CircleShape {
            center: port,
            radius: HANDLE_SIZE / 2.0,
            fill: Color32::WHITE,
            stroke: outline,
        });
    }
    for (index, point) in look.graphics.path.points.iter().enumerate() {
        let at = shown.to_screen * pos2(point.x as f32, point.y as f32);
        let fill = if bend == Some(index) {
            SELECTION.color
        } else {
            Color32::WHITE
        };
        let rect = Rect::from_center_size(at, Vec2::splat(HANDLE_SIZE));
        painter.rect(rect, 0.0, fill, outline, StrokeKind::Inside);
    }
}

/// Where the primary button was released this frame.
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

/// A node of `shape` with yEd's default look, dragged out between two
/// corners in graph coordinates.
fn new_node(shape: &NodeShape, corner: Pos2, opposite: Pos2) -> NodeGraphics {
    let mut size = opposite - corner;
    if shape.square {
        // The longer side of the drag, so the node keeps up with the pointer.
        let side = size.abs().max_elem();
        size = vec2(side.copysign(size.x), side.copysign(size.y));
    }
    new_shape(shape.shape.clone(), corner, corner + size)
}

/// A `shape` with yEd's default look, spanning two corners in graph
/// coordinates.
fn new_shape(shape: ShapeType, corner: Pos2, opposite: Pos2) -> NodeGraphics {
    let mut graphics = NodeGraphics::new(NodeKind::Shape(shape));
    graphics.geometry = yed_rect(Rect::from_two_pos(corner, opposite));
    graphics
}

/// The shapes the canvas can draw nodes as.
#[derive(Clone, Copy)]
enum Outline {
    /// A box, with its corners rounded to this radius in graph points.
    Rect(f32),
    Ellipse,
}

/// The shape the canvas draws a node as, if it's a kind it can draw yet.
/// `corner_radius` rounds the corners of a box; see
/// [`document::corner_radius`].
fn shape(graphics: &NodeGraphics, corner_radius: Option<f64>) -> Option<Outline> {
    match &graphics.kind {
        NodeKind::Shape(ShapeType::Rectangle | ShapeType::RoundRectangle) => {
            Some(Outline::Rect(corner_radius.unwrap_or_default() as f32))
        }
        NodeKind::Shape(ShapeType::Ellipse) => Some(Outline::Ellipse),
        _ => None,
    }
}

/// Whether a point on screen is on a node the canvas draws.
fn hit(
    to_screen: TSTransform,
    graphics: &NodeGraphics,
    corner_radius: Option<f64>,
    pos: Pos2,
) -> bool {
    let outline = shape(graphics, corner_radius);
    let rect = screen_rect(to_screen, graphics);
    outline.is_some() && inside(outline, rect, to_screen.scaling, pos)
}

/// Whether a point is inside `outline` filling `rect`, or in `rect` for a
/// node the canvas can't draw, at `zoom` screen points a graph point.
fn inside(outline: Option<Outline>, rect: Rect, zoom: f32, pos: Pos2) -> bool {
    match outline {
        Some(Outline::Rect(radius)) => in_rounded_rect(rect, radius * zoom, pos),
        Some(Outline::Ellipse) => in_ellipse(rect, pos),
        None => rect.contains(pos),
    }
}

/// Whether a point is in `rect` with its corners rounded to `radius`, but by
/// no more than half of each side, as painted.
fn in_rounded_rect(rect: Rect, radius: f32, pos: Pos2) -> bool {
    let radius = Vec2::splat(radius).min(rect.size() / 2.0);
    if radius.min_elem() <= 0.0 {
        return rect.contains(pos);
    }
    // From the center of the corner it's in, in radii.
    let offset = (pos - rect.shrink2(radius).clamp(pos)) / radius;
    rect.contains(pos) && offset.length_sq() <= 1.0
}

/// Whether a point is in the ellipse filling `rect`. An inverted `rect` has
/// none, as none is painted.
fn in_ellipse(rect: Rect, pos: Pos2) -> bool {
    let offset = (pos - rect.center()) / (rect.size() / 2.0);
    rect.contains(pos) && offset.length_sq() <= 1.0
}

/// Where a node's bounds are on screen.
fn screen_rect(to_screen: TSTransform, graphics: &NodeGraphics) -> Rect {
    egui_rect(graphics.geometry, to_screen)
}

/// A yEd rect as an egui one, taken by `transform`.
fn egui_rect(rect: yed::Rect, transform: TSTransform) -> Rect {
    let min = transform * pos2(rect.x as f32, rect.y as f32);
    let size = vec2(rect.width as f32, rect.height as f32) * transform.scaling;
    Rect::from_min_size(min, size)
}

/// An egui rect as a yEd one.
fn yed_rect(rect: Rect) -> yed::Rect {
    yed::Rect {
        x: rect.min.x.into(),
        y: rect.min.y.into(),
        width: rect.width().into(),
        height: rect.height().into(),
    }
}

/// Paints a node with its labels, except the one at index `hidden`, unless
/// it's a kind the canvas can't draw yet. `corner_radius` rounds the corners
/// of a box.
fn paint(
    painter: &Painter,
    to_screen: TSTransform,
    text_zoom: f32,
    graphics: &NodeGraphics,
    corner_radius: Option<f64>,
    hidden: Option<usize>,
) {
    let Some(outline) = shape(graphics, corner_radius) else {
        return;
    };
    let rect = screen_rect(to_screen, graphics);
    let fill = graphics.fill.color.map_or(Color32::TRANSPARENT, color32);
    let border = &graphics.border;
    let stroke = border.color.map_or(Stroke::NONE, |color| {
        Stroke::new(border.width as f32 * to_screen.scaling, color32(color))
    });
    // Like yEd, which centers borders on the outline, over the fill.
    match outline {
        Outline::Rect(radius) => {
            let radius = radius * to_screen.scaling;
            let ppp = painter.pixels_per_point();
            let points = line_type::rounded_rect_points(rect, radius, stroke, ppp);
            if points.is_empty() {
                painter.rect_filled(rect, 0.0, fill);
                line_type::paint_rect_outline(painter, rect, stroke, &border.line_type);
            } else {
                // On the border's polygon, so their corners line up.
                let filled = PathShape::convex_polygon(points.clone(), fill, Stroke::NONE);
                painter.add(filled);
                line_type::paint_outline(painter, points, stroke, &border.line_type);
            }
        }
        Outline::Ellipse => {
            let points = line_type::ellipse_points(rect, painter.pixels_per_point());
            painter.add(PathShape::convex_polygon(points, fill, Stroke::NONE));
            line_type::paint_ellipse_outline(painter, rect, stroke, &border.line_type);
        }
    }

    let owner = Owner::Node(graphics.geometry);
    for (index, label) in graphics.labels.iter().enumerate() {
        if label.visible && hidden != Some(index) {
            paint_label(painter, to_screen, text_zoom, &owner, label);
        }
    }
}

/// Paints a label of `owner` where yEd puts it, lined up by its alignment,
/// in its font and color. It's laid out at `text_zoom`, then scaled to the
/// zoom.
fn paint_label(
    painter: &Painter,
    to_screen: TSTransform,
    text_zoom: f32,
    owner: &Owner,
    label: &Label,
) {
    let color = color32(label.text_color);
    let (galley, label_rect) = label_layout(painter, to_screen, text_zoom, owner, label);
    let align = Align2([galley.job.halign, Align::Center]);
    let scale = to_screen.scaling / text_zoom;
    let size = galley.size() * scale;
    let rect = align.align_size_within_rect(size, label_rect);
    let pos = rect.min - galley.rect.min.to_vec2() * scale;
    if scale == 1.0 {
        painter.galley(pos, galley, color);
    } else {
        let mut text = TextShape::new(pos, galley, color);
        // About `pos`.
        text.transform(TSTransform::new(pos.to_vec2() * (1.0 - scale), scale));
        painter.add(text);
    }
}

/// A label's text laid out at `text_zoom`, and where on screen the label
/// goes on `owner`.
fn label_layout(
    painter: &Painter,
    to_screen: TSTransform,
    text_zoom: f32,
    owner: &Owner,
    label: &Label,
) -> (Arc<Galley>, Rect) {
    let font = label_font(label, text_zoom, painter.pixels_per_point());
    let color = color32(label.text_color);
    let mut job = LayoutJob::simple(label.text.clone(), font, color, f32::INFINITY);
    job.halign = halign(&label.alignment);
    let galley = painter.layout_job(job);
    let size = galley.size() * (to_screen.scaling / text_zoom);
    let rect = label_rect(to_screen, owner, label, size);
    (galley, rect)
}

/// Where a label of `owner`, with text `size` big on screen, goes on screen:
/// where yEd puts it, or for a new label, which yEd hasn't sized yet, where
/// its position puts it.
fn label_rect(to_screen: TSTransform, owner: &Owner, label: &Label, size: Vec2) -> Rect {
    let text = size / to_screen.scaling;
    let node = match owner {
        Owner::Node(node) => *node,
        Owner::Edge(route) => return egui_rect(placed_on(route, label, text), to_screen),
    };
    let mut bounds = label.bounds;
    if !(bounds.width > 0.0 && bounds.height > 0.0) {
        let size = text + Vec2::splat(2.0 * LABEL_PADDING);
        let geometry = yed::Rect {
            width: node.width,
            height: node.height,
            ..yed::Rect::default()
        };
        match label.placed(&geometry, size.x.into(), size.y.into()) {
            Some(placed) => bounds = placed,
            None => return egui_rect(node, to_screen),
        }
    }
    // Relative to the node.
    let node_min = to_screen * pos2(node.x as f32, node.y as f32);
    let to_node = TSTransform::new(node_min.to_vec2(), to_screen.scaling);
    egui_rect(bounds, to_node)
}

/// Where an edge label with text `text` big goes along `route`, both in
/// graph points: where yEd puts it, or where it last did if its model is
/// one this library doesn't read.
fn placed_on(route: &Route, label: &Label, text: Vec2) -> yed::Rect {
    let bounds = label.bounds;
    let (width, height) = if bounds.width > 0.0 && bounds.height > 0.0 {
        (bounds.width, bounds.height)
    } else {
        let size = text + Vec2::splat(2.0 * LABEL_PADDING);
        (size.x.into(), size.y.into())
    };
    label.placed_on(route, width, height).unwrap_or_else(|| {
        let start = route.points[0];
        yed::Rect {
            x: start.x + bounds.x,
            y: start.y + bounds.y,
            width,
            height,
        }
    })
}

/// Where an edge label goes along `route`, in graph coordinates.
fn edge_label_bounds(painter: &Painter, text_zoom: f32, route: &Route, label: &Label) -> yed::Rect {
    let font = label_font(label, text_zoom, painter.pixels_per_point());
    let text = painter.layout_no_wrap(label.text.clone(), font, Color32::PLACEHOLDER);
    placed_on(route, label, text.size() / text_zoom)
}

/// A label's font at `zoom`. Its size counts as at least a point, since egui
/// can't draw text of size 0, and is capped at what egui can draw.
fn label_font(label: &Label, zoom: f32, pixels_per_point: f32) -> FontId {
    let font = &label.font;
    let size = font.size.max(1) as f32 * zoom;
    FontId::new(
        size.min(MAX_TEXT_PIXELS / pixels_per_point),
        fonts::family(&font.family, &font.style),
    )
}

/// How a label's lines line up.
fn halign(alignment: &Alignment) -> Align {
    match alignment {
        Alignment::Left => Align::LEFT,
        Alignment::Right => Align::RIGHT,
        _ => Align::Center,
    }
}

#[cfg(test)]
mod tests;

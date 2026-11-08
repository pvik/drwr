use egui::emath::TSTransform;
use egui::epaint::{PathShape, TextShape};
use egui::text::LayoutJob;
use egui::{
    Align, Align2, Color32, CursorIcon, Event, FontId, Id, Margin, Painter, PointerButton, Pos2,
    Rangef, Rect, Response, Sense, Stroke, StrokeKind, UiBuilder, Vec2, Visuals, pos2, vec2,
};
use graphml::GraphMl;
use graphml::yed::{self, Alignment, Label, NodeGraphics, NodeKind, ShapeType};

use crate::action::Action;
use crate::color_picker::color32;
use crate::document::{self, LabelRef};
use crate::fonts;
use crate::line_type;
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

/// A label being edited on the canvas, over where it's painted.
struct LabelEditor {
    label: LabelRef,
    text: String,
    /// Whether the editor has yet to take the focus.
    focus: bool,
}

impl Canvas {
    /// Shows the canvas in the space left over by the other panels.
    /// `selected` is the id of the selected node. Returns the actions asked
    /// for.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        tool: Tool,
        graphml: &mut GraphMl,
        selected: &mut Option<String>,
    ) -> Vec<Action> {
        let mut actions = Vec::new();
        egui::CentralPanel::default().show(ui, |ui| {
            let sense = match tool {
                Tool::Cursor => Sense::click_and_drag(),
                _ if tool.node_shape().is_some() => Sense::drag(),
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
            let mut double_clicked = false;
            let drawing = match tool {
                Tool::Cursor => {
                    select_clicked(&response, to_screen, graphml, selected);
                    double_clicked =
                        response.double_clicked_by(PointerButton::Primary) && selected.is_some();
                    None
                }
                _ => self.draw_node(response.clone(), to_screen, graphml, selected, tool),
            };
            let dragged = match (tool, selected.as_deref()) {
                (Tool::Cursor, Some(id)) => self.drag_node(ui, response, to_screen, graphml, id),
                // Switching tools mid-drag drops the move or resize, as it does
                // the shape.
                _ => {
                    self.node_drag = None;
                    None
                }
            };

            painter.rect_filled(rect, 0.0, BACKGROUND);
            let editing = self.editor.as_ref().map(|editor| &editor.label);
            let mut outline = None;
            for (id, mut graphics, radius) in document::node_graphics(graphml) {
                if selected.as_deref() == Some(id) {
                    if let Some(geometry) = dragged {
                        document::resize(&mut graphics, geometry);
                    }
                    outline = Some(screen_rect(to_screen, &graphics).expand(SELECTION_GAP));
                }
                // The label being edited shows in its editor instead.
                let hidden = editing.filter(|label| label.node == id);
                let hidden = hidden.map(|label| label.index);
                paint(&painter, to_screen, text_zoom, &graphics, radius, hidden);
            }
            if let Some(graphics) = drawing {
                // New boxes have square corners.
                paint(&painter, to_screen, text_zoom, &graphics, None, None);
            }
            // Over the nodes, so one on top can't hide it.
            if let Some(outline) = outline {
                painter.rect_stroke(outline, 0.0, SELECTION, StrokeKind::Outside);
            }
            // Outlined, since the light theme's panels are nearly white too.
            let border = ui.visuals().widgets.noninteractive.bg_stroke;
            painter.rect_stroke(rect, 0.0, border, StrokeKind::Inside);

            // Over the nodes. An edit it applies goes first, in case the
            // double click is on the same label.
            actions.extend(self.show_editor(ui, to_screen, graphml));
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
    fn show_editor(
        &mut self,
        ui: &mut egui::Ui,
        to_screen: TSTransform,
        graphml: &GraphMl,
    ) -> Option<Action> {
        let editor = self.editor.as_mut()?;
        let Some(graphics) = document::graphics_of(graphml, &editor.label.node) else {
            self.editor = None;
            return None;
        };
        let label = graphics.labels.get(editor.label.index).cloned();
        let label = label.unwrap_or_else(|| Label::new(""));
        let font = label_font(&label, to_screen.scaling, ui.pixels_per_point());
        let align = Align2([halign(&label.alignment), Align::Center]);
        let text = ui
            .painter()
            .layout_no_wrap(editor.text.clone(), font.clone(), Color32::PLACEHOLDER)
            .size();
        let node = screen_rect(to_screen, &graphics);
        // An em to spare, so the next letter typed doesn't wrap.
        let width = text.x + font.size + EDITOR_MARGIN.sum().x;
        let size = vec2(
            width.max(node.width()).max(EDITOR_MIN_WIDTH),
            text.y + EDITOR_MARGIN.sum().y,
        );
        let label_rect = label_rect(to_screen, graphics.geometry, &label, text);
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
        selected: &mut Option<String>,
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
            *selected = Some(document::add_node(graphml, &graphics));
        }
        None
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

/// Selects the topmost node a primary click lands on, or nothing if it
/// misses them all.
fn select_clicked(
    response: &Response,
    to_screen: TSTransform,
    graphml: &GraphMl,
    selected: &mut Option<String>,
) {
    // `clicked()` would count Enter and Space too, which have no position.
    if !response.clicked_by(PointerButton::Primary) {
        return;
    }
    let Some(pos) = response.ctx.input(primary_release) else {
        return;
    };
    // Nodes are painted in order, so the last one hit is on top.
    *selected = document::node_graphics(graphml)
        .filter(|(_, graphics, corner_radius)| hit(to_screen, graphics, *corner_radius, pos))
        .last()
        .map(|(id, ..)| id.to_owned());
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
    let rect = screen_rect(to_screen, graphics);
    match shape(graphics, corner_radius) {
        Some(Outline::Rect(radius)) => in_rounded_rect(rect, radius * to_screen.scaling, pos),
        Some(Outline::Ellipse) => in_ellipse(rect, pos),
        None => false,
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

    for (index, label) in graphics.labels.iter().enumerate() {
        if label.visible && hidden != Some(index) {
            paint_label(painter, to_screen, text_zoom, graphics.geometry, label);
        }
    }
}

/// Paints a label of the node at `node` where yEd last put it, lined up by
/// its alignment, in its font and color. It's laid out at `text_zoom`, then
/// scaled to the zoom.
fn paint_label(
    painter: &Painter,
    to_screen: TSTransform,
    text_zoom: f32,
    node: yed::Rect,
    label: &Label,
) {
    let color = color32(label.text_color);
    let font = label_font(label, text_zoom, painter.pixels_per_point());
    let mut job = LayoutJob::simple(label.text.clone(), font, color, f32::INFINITY);
    job.halign = halign(&label.alignment);
    let align = Align2([job.halign, Align::Center]);
    let galley = painter.layout_job(job);
    let scale = to_screen.scaling / text_zoom;
    let size = galley.size() * scale;
    let rect = align.align_size_within_rect(size, label_rect(to_screen, node, label, size));
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

/// Where a label of the node at `node`, with text `size` big on screen, goes
/// on screen: where yEd last put it, or for a new label, which yEd hasn't
/// sized yet, where its position puts it.
fn label_rect(to_screen: TSTransform, node: yed::Rect, label: &Label, size: Vec2) -> Rect {
    let mut bounds = label.bounds;
    if !(bounds.width > 0.0 && bounds.height > 0.0) {
        let size = size / to_screen.scaling + Vec2::splat(2.0 * LABEL_PADDING);
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

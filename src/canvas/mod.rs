use egui::text::{CCursorRange, LayoutJob};
use egui::{
    Align, Align2, Color32, CursorIcon, Event, FontId, Id, Margin, Painter, PointerButton, Pos2,
    Rangef, Rect, Response, Sense, Stroke, StrokeKind, UiBuilder, Vec2, Visuals, vec2,
};
use graphml::GraphMl;
use graphml::yed::{self, Alignment, Label, NodeGraphics, NodeKind, ShapeType};

use crate::action::Action;
use crate::color_picker::color32;
use crate::document::{self, LabelRef};
use crate::fonts;
use crate::label_field::{self, End};
use crate::line_type;
use crate::tool::Tool;

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

const EDITOR_MARGIN: Margin = Margin::symmetric(4, 2);
const EDITOR_MIN_WIDTH: f32 = 60.0;

/// The drawing canvas. Graph coordinates start at its top-left corner.
#[derive(Default)]
pub struct Canvas {
    /// The corner the box being drawn was started from, in graph coordinates.
    box_start: Option<Pos2>,
    node_drag: Option<NodeDrag>,
    editor: Option<LabelEditor>,
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
                Tool::Box => Sense::drag(),
                _ => Sense::hover(),
            };
            let (response, painter) = ui.allocate_painter(ui.available_size(), sense);
            // Unlike a click, a drag leaves the focus in a label being typed,
            // which would then stay open while the next box is drawn.
            if response.drag_started() {
                ui.memory_mut(|memory| memory.stop_text_input());
            }
            let rect = response.rect;
            let origin = rect.min.to_vec2();
            if tool != Tool::Box {
                // A shortcut can switch tools mid-drag, which drops the box.
                self.box_start = None;
            }
            let mut double_clicked = false;
            let drawing = match tool {
                Tool::Cursor => {
                    select_clicked(&response, origin, graphml, selected);
                    double_clicked =
                        response.double_clicked_by(PointerButton::Primary) && selected.is_some();
                    None
                }
                Tool::Box => self.draw_box(response.clone(), origin, graphml, selected),
                _ => None,
            };
            let dragged = match (tool, selected.as_deref()) {
                (Tool::Cursor, Some(id)) => self.drag_node(ui, response, origin, graphml, id),
                // Switching tools mid-drag drops the move or resize, as it does
                // the box.
                _ => {
                    self.node_drag = None;
                    None
                }
            };

            painter.rect_filled(rect, 0.0, BACKGROUND);
            let editing = self.editor.as_ref().map(|editor| &editor.label);
            let mut outline = None;
            for (id, mut graphics) in document::node_graphics(graphml) {
                if selected.as_deref() == Some(id) {
                    if let Some(geometry) = dragged {
                        document::resize(&mut graphics, geometry);
                    }
                    outline = Some(screen_rect(origin, &graphics).expand(SELECTION_GAP));
                }
                // The label being edited shows in its editor instead.
                let hidden = editing.filter(|label| label.node == id);
                paint(&painter, origin, &graphics, hidden.map(|label| label.index));
            }
            if let Some(graphics) = drawing {
                paint(&painter, origin, &graphics, None);
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
            actions.extend(self.show_editor(ui, origin, graphml));
            if double_clicked {
                actions.push(Action::EditLabel);
            }
        });
        actions
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
        origin: Vec2,
        graphml: &GraphMl,
    ) -> Option<Action> {
        let editor = self.editor.as_mut()?;
        let Some(graphics) = document::graphics_of(graphml, &editor.label.node) else {
            self.editor = None;
            return None;
        };
        let label = graphics.labels.get(editor.label.index).cloned();
        let label = label.unwrap_or_else(|| Label::new(""));
        let font = label_font(&label);
        let align = Align2([halign(&label.alignment), Align::Center]);
        let text = ui
            .painter()
            .layout_no_wrap(editor.text.clone(), font.clone(), Color32::PLACEHOLDER)
            .size();
        let node = screen_rect(origin, &graphics);
        // An em to spare, so the next letter typed doesn't wrap.
        let width = text.x + font.size + EDITOR_MARGIN.sum().x;
        let size = vec2(
            width.max(node.width()).max(EDITOR_MIN_WIDTH),
            text.y + EDITOR_MARGIN.sum().y,
        );
        let rect = align.align_size_within_rect(size, label_rect(node, &label));

        let id = Id::unique("label_editor");
        let mut output = ui
            .scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                // White like the canvas, even in the dark theme.
                ui.style_mut().visuals = Visuals::light();
                label_field::text_edit(&mut editor.text)
                    .id(id)
                    .font(font)
                    .margin(EDITOR_MARGIN)
                    .desired_width(rect.width())
                    .align(align)
                    .show(ui)
            })
            .inner;
        // Selected, so typing replaces it.
        if std::mem::take(&mut editor.focus) {
            output.response.response.request_focus();
            let all = CCursorRange::select_all_str(&editor.text);
            output.state.cursor.set_char_range(Some(all));
            output.state.store(ui.ctx(), id);
            return None;
        }
        let response = &output.response.response;
        match label_field::end(ui, response) {
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

    /// Drags out a box with the primary button, adding it to `graphml` and
    /// selecting it on release. Returns the box while it's being drawn.
    fn draw_box(
        &mut self,
        response: Response,
        origin: Vec2,
        graphml: &mut GraphMl,
        selected: &mut Option<String>,
    ) -> Option<NodeGraphics> {
        let response = response.on_hover_cursor(CursorIcon::Crosshair);
        // Whole numbers keep the saved coordinates tidy.
        let to_graph = |pos: Pos2| (pos - origin).round();
        if response.drag_started_by(PointerButton::Primary) {
            self.box_start = response
                .ctx
                .input(|i| i.pointer.press_origin())
                .map(to_graph);
        }
        if !response.drag_stopped() {
            let end = response.interact_pointer_pos().map(to_graph);
            return self
                .box_start
                .zip(end)
                .map(|(start, end)| new_box(start, end));
        }

        // Without a release, Escape stopped the drag, which cancels the box.
        let end = response.ctx.input(primary_release).map(to_graph);
        if let Some((start, end)) = self.box_start.take().zip(end) {
            let graphics = new_box(start, end);
            if graphics.geometry.width > 0.0 && graphics.geometry.height > 0.0 {
                *selected = Some(document::add_node(graphml, &graphics));
            }
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
        origin: Vec2,
        graphml: &mut GraphMl,
        id: &str,
    ) -> Option<yed::Rect> {
        let graphics = document::graphics_of(graphml, id)?;
        let node = screen_rect(origin, &graphics);
        let geometry = graphics.geometry;
        // Whole numbers, as when drawing a box.
        let to_graph = |pos: Pos2| (pos - origin).round();
        let moving = self.node_drag.is_some_and(|drag| drag.handle.is_none());
        if moving || canvas.hover_pos().is_some_and(|pos| node.contains(pos)) {
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
                let from = from.filter(|&from| handle.is_some() || node.contains(from));
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
    origin: Vec2,
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
        .filter(|(_, graphics)| drawable(graphics) && screen_rect(origin, graphics).contains(pos))
        .last()
        .map(|(id, _)| id.to_owned());
}

/// Where the primary button was released this frame. The pointer may have
/// moved on since, in the same frame.
fn primary_release(input: &egui::InputState) -> Option<Pos2> {
    input.events.iter().find_map(|event| match event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            ..
        } => Some(*pos),
        _ => None,
    })
}

/// A box with yEd's default look, spanning two corners in graph coordinates.
fn new_box(corner: Pos2, opposite: Pos2) -> NodeGraphics {
    let rect = Rect::from_two_pos(corner, opposite);
    let mut graphics = NodeGraphics::new(NodeKind::Shape(ShapeType::Rectangle));
    graphics.geometry = yed::Rect {
        x: rect.min.x.into(),
        y: rect.min.y.into(),
        width: rect.width().into(),
        height: rect.height().into(),
    };
    graphics
}

/// Whether the canvas can draw this kind of node yet.
fn drawable(graphics: &NodeGraphics) -> bool {
    graphics.kind == NodeKind::Shape(ShapeType::Rectangle)
}

/// Where a node's bounds are on screen.
fn screen_rect(origin: Vec2, graphics: &NodeGraphics) -> Rect {
    let yed::Rect {
        x,
        y,
        width,
        height,
    } = graphics.geometry;
    Rect::from_min_size(
        Pos2::new(x as f32, y as f32) + origin,
        Vec2::new(width as f32, height as f32),
    )
}

/// Paints a node with its labels, except the one at index `hidden`, unless
/// it's a kind the canvas can't draw yet.
fn paint(painter: &Painter, origin: Vec2, graphics: &NodeGraphics, hidden: Option<usize>) {
    if !drawable(graphics) {
        return;
    }
    let rect = screen_rect(origin, graphics);
    let fill = graphics.fill.color.map_or(Color32::TRANSPARENT, color32);
    let border = &graphics.border;
    let stroke = border.color.map_or(Stroke::NONE, |color| {
        Stroke::new(border.width as f32, color32(color))
    });
    // Like yEd, which centers borders on the outline, over the fill.
    painter.rect_filled(rect, 0.0, fill);
    line_type::paint_outline(painter, rect, stroke, &border.line_type);

    for (index, label) in graphics.labels.iter().enumerate() {
        if label.visible && hidden != Some(index) {
            paint_label(painter, rect, label);
        }
    }
}

/// Paints a label where yEd last put it, lined up by its alignment, in its
/// font and color.
fn paint_label(painter: &Painter, node: Rect, label: &Label) {
    let color = color32(label.text_color);
    let mut job = LayoutJob::simple(label.text.clone(), label_font(label), color, f32::INFINITY);
    job.halign = halign(&label.alignment);
    let align = Align2([job.halign, Align::Center]);
    let galley = painter.layout_job(job);
    let rect = align.align_size_within_rect(galley.size(), label_rect(node, label));
    painter.galley(rect.min - galley.rect.min.to_vec2(), galley, color);
}

/// Where yEd last put a label on screen, or the whole node for a new label.
fn label_rect(node: Rect, label: &Label) -> Rect {
    let yed::Rect {
        x,
        y,
        width,
        height,
    } = label.bounds;
    if width > 0.0 && height > 0.0 {
        let min = node.min + vec2(x as f32, y as f32);
        Rect::from_min_size(min, vec2(width as f32, height as f32))
    } else {
        node
    }
}

/// At least a point in size, since egui can't draw text of size 0.
fn label_font(label: &Label) -> FontId {
    let font = &label.font;
    FontId::new(
        font.size.max(1) as f32,
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

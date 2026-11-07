use egui::text::{CCursorRange, LayoutJob};
use egui::{
    Align, Align2, Color32, CursorIcon, Event, FontId, Id, Margin, Painter, PointerButton, Pos2,
    Rect, Response, Sense, Stroke, StrokeKind, UiBuilder, Vec2, Visuals, vec2,
};
use graphml::GraphMl;
use graphml::yed::{self, Alignment, Label, NodeGraphics, NodeKind, ShapeType};

use crate::action::Action;
use crate::color_picker::color32;
use crate::document::{self, LabelRef};
use crate::label_field::{self, End};
use crate::tool::Tool;

/// White like paper, even in the dark theme.
const BACKGROUND: Color32 = Color32::WHITE;

const SELECTION: Stroke = Stroke {
    width: 2.0,
    color: Color32::from_rgb(0x00, 0x78, 0xD4),
};
/// Between the selected node's bounds and its outline.
const SELECTION_GAP: f32 = 2.0;

const EDITOR_MARGIN: Margin = Margin::symmetric(4, 2);
const EDITOR_MIN_WIDTH: f32 = 60.0;

/// The drawing canvas. Graph coordinates start at its top-left corner.
#[derive(Default)]
pub struct Canvas {
    /// The corner the box being drawn was started from, in graph coordinates.
    box_start: Option<Pos2>,
    editor: Option<LabelEditor>,
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
                Tool::Cursor => Sense::click(),
                Tool::Box => Sense::drag(),
                _ => Sense::hover(),
            };
            let (response, painter) = ui.allocate_painter(ui.available_size(), sense);
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
                Tool::Box => self.draw_box(response, origin, graphml, selected),
                _ => None,
            };

            painter.rect_filled(rect, 0.0, BACKGROUND);
            let editing = self.editor.as_ref().map(|editor| &editor.label);
            for (id, graphics) in document::node_graphics(graphml) {
                // The label being edited shows in its editor instead.
                let hidden = editing.filter(|label| label.node == id);
                paint(&painter, origin, &graphics, hidden.map(|label| label.index));
            }
            if let Some(graphics) = drawing {
                paint(&painter, origin, &graphics, None);
            }
            // Over the nodes, so one on top can't hide it.
            if let Some(graphics) = selected
                .as_deref()
                .and_then(|id| document::graphics_of(graphml, id))
            {
                let outline = screen_rect(origin, &graphics).expand(SELECTION_GAP);
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
    // yEd centers borders on the outline.
    painter.rect(rect, 0.0, fill, stroke, StrokeKind::Middle);

    for (index, label) in graphics.labels.iter().enumerate() {
        if label.visible && hidden != Some(index) {
            paint_label(painter, rect, label);
        }
    }
}

/// Paints a label where yEd last put it, lined up by its alignment, in its
/// color and size but not yet its font family or style.
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
    FontId::proportional(label.font.size.max(1) as f32)
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

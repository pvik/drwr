use egui::{
    Color32, CursorIcon, Event, Painter, PointerButton, Pos2, Rect, Response, Sense, Stroke,
    StrokeKind, Vec2,
};
use graphml::GraphMl;
use graphml::yed::{self, NodeGraphics, NodeKind, ShapeType};

use crate::color_picker::color32;
use crate::document;
use crate::tool::Tool;

/// White like paper, even in the dark theme.
const BACKGROUND: Color32 = Color32::WHITE;

const SELECTION: Stroke = Stroke {
    width: 2.0,
    color: Color32::from_rgb(0x00, 0x78, 0xD4),
};
/// Between the selected node's bounds and its outline.
const SELECTION_GAP: f32 = 2.0;

/// The drawing canvas. Graph coordinates start at its top-left corner.
#[derive(Default)]
pub struct Canvas {
    /// The corner the box being drawn was started from, in graph coordinates.
    box_start: Option<Pos2>,
}

impl Canvas {
    /// Shows the canvas in the space left over by the other panels.
    /// `selected` is the id of the selected node.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        tool: Tool,
        graphml: &mut GraphMl,
        selected: &mut Option<String>,
    ) {
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
            let drawing = match tool {
                Tool::Cursor => {
                    select_clicked(&response, origin, graphml, selected);
                    None
                }
                Tool::Box => self.draw_box(response, origin, graphml, selected),
                _ => None,
            };

            painter.rect_filled(rect, 0.0, BACKGROUND);
            let nodes = document::node_graphics(graphml).map(|(_, graphics)| graphics);
            for graphics in nodes.chain(drawing) {
                paint(&painter, origin, &graphics);
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
        });
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

/// Paints a node, unless it's a kind the canvas can't draw yet.
fn paint(painter: &Painter, origin: Vec2, graphics: &NodeGraphics) {
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
}

#[cfg(test)]
mod tests;

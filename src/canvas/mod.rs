use egui::{
    Color32, CursorIcon, Event, Painter, PointerButton, Pos2, Rect, Response, Sense, Stroke,
    StrokeKind, Vec2,
};
use graphml::GraphMl;
use graphml::yed::{self, NodeGraphics, NodeKind, ShapeType};

use crate::document;
use crate::tool::Tool;

/// White like paper, even in the dark theme.
const BACKGROUND: Color32 = Color32::WHITE;

/// The drawing canvas. Graph coordinates start at its top-left corner.
#[derive(Default)]
pub struct Canvas {
    /// The corner the box being drawn was started from, in graph coordinates.
    box_start: Option<Pos2>,
}

impl Canvas {
    /// Shows the canvas in the space left over by the other panels.
    pub fn show(&mut self, ui: &mut egui::Ui, tool: Tool, graphml: &mut GraphMl) {
        egui::CentralPanel::default().show(ui, |ui| {
            let sense = match tool {
                Tool::Box => Sense::drag(),
                _ => Sense::hover(),
            };
            let (response, painter) = ui.allocate_painter(ui.available_size(), sense);
            let rect = response.rect;
            let origin = rect.min.to_vec2();
            let drawing = match tool {
                Tool::Box => self.draw_box(response, origin, graphml),
                _ => None,
            };

            painter.rect_filled(rect, 0.0, BACKGROUND);
            for graphics in document::node_graphics(graphml).chain(drawing) {
                paint(&painter, origin, &graphics);
            }
            // Outlined, since the light theme's panels are nearly white too.
            let border = ui.visuals().widgets.noninteractive.bg_stroke;
            painter.rect_stroke(rect, 0.0, border, StrokeKind::Inside);
        });
    }

    /// Drags out a box with the primary button, adding it to `graphml` on
    /// release. Returns the box while it's being drawn.
    fn draw_box(
        &mut self,
        response: Response,
        origin: Vec2,
        graphml: &mut GraphMl,
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
                document::add_node(graphml, &graphics);
            }
        }
        None
    }
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

/// Paints a node, unless it's a kind the canvas can't draw yet.
fn paint(painter: &Painter, origin: Vec2, graphics: &NodeGraphics) {
    if graphics.kind != NodeKind::Shape(ShapeType::Rectangle) {
        return;
    }
    let yed::Rect {
        x,
        y,
        width,
        height,
    } = graphics.geometry;
    let rect = Rect::from_min_size(
        Pos2::new(x as f32, y as f32) + origin,
        Vec2::new(width as f32, height as f32),
    );
    let fill = graphics.fill.color.map_or(Color32::TRANSPARENT, color32);
    let border = &graphics.border;
    let stroke = border.color.map_or(Stroke::NONE, |color| {
        Stroke::new(border.width as f32, color32(color))
    });
    // yEd centers borders on the outline.
    painter.rect(rect, 0.0, fill, stroke, StrokeKind::Middle);
}

fn color32(color: yed::Color) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

#[cfg(test)]
mod tests;

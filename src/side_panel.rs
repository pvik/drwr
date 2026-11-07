use egui::{Atom, Button, Grid, Label, Response, Stroke, TextStyle, Ui, Vec2, emath};
use graphml::GraphMl;
use graphml::yed::{Color, NodeGraphics, NodeKind};

use crate::action::Action;
use crate::color_picker;
use crate::document::{self, ColorProperty};

/// Shows a resizable panel right of the canvas with the properties of the
/// node whose id is `selected`. Returns the action clicked.
pub fn show(ui: &mut Ui, graphml: &GraphMl, selected: Option<&str>) -> Option<Action> {
    let mut clicked = None;
    egui::Panel::right("side_panel").show(ui, |ui| {
        // Otherwise the panel shrinks to fit its contents, even after resizing.
        ui.take_available_width();
        let node = selected.and_then(|id| Some((id, document::graphics_of(graphml, id)?)));
        match node {
            Some((id, graphics)) => clicked = properties(ui, id, &graphics),
            None => {
                ui.weak("Nothing selected");
            }
        }
    });
    clicked
}

fn properties(ui: &mut Ui, id: &str, graphics: &NodeGraphics) -> Option<Action> {
    Grid::new("properties")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            let shape = match &graphics.kind {
                NodeKind::Shape(shape) => shape.as_str(),
                NodeKind::Generic(configuration) => configuration,
                NodeKind::Other(name) => name,
            };
            let geometry = &graphics.geometry;
            let border = &graphics.border;
            row(ui, "Id", id);
            row(ui, "Shape", shape);
            row(ui, "X", &number(geometry.x));
            row(ui, "Y", &number(geometry.y));
            row(ui, "Width", &number(geometry.width));
            row(ui, "Height", &number(geometry.height));
            let mut clicked = None;
            for property in ColorProperty::ALL {
                ui.label(property.name());
                if color_button(ui, property.get(graphics)).clicked() {
                    clicked = Some(Action::EditColor(property));
                }
                ui.end_row();
            }
            row(ui, "Border width", &number(border.width));
            row(ui, "Border style", border.line_type.as_str());
            for label in &graphics.labels {
                row(ui, "Label", &label.text);
            }
            clicked
        })
        .inner
}

/// Adds a row, truncating a long value, which then shows in full on hover.
fn row(ui: &mut Ui, name: &str, value: &str) {
    ui.label(name);
    ui.add(Label::new(value).truncate());
    ui.end_row();
}

/// A button showing a swatch of `color` and its hex code.
fn color_button(ui: &mut Ui, color: Option<Color>) -> Response {
    let size = Vec2::splat(ui.text_style_height(&TextStyle::Body));
    let swatch = Atom::paint(size, move |ui, args| {
        let outline = Stroke::new(1.0, args.fallback_text_color);
        color_picker::paint_swatch(ui.painter(), args.rect, color, outline);
    });
    let text = color.map_or("None".to_owned(), |color| color.to_string());
    ui.add(Button::new((swatch, text)))
}

/// Up to two decimals, so yEd's 327.33333333333337 reads as 327.33.
fn number(value: f64) -> String {
    emath::format_with_decimals_in_range(value, 0..=2)
}

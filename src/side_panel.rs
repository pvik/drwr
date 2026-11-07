use egui::{Grid, Label, Ui, emath};
use graphml::GraphMl;
use graphml::yed::{Color, NodeGraphics, NodeKind};

use crate::document;

/// Shows a resizable panel right of the canvas with the properties of the
/// node whose id is `selected`.
pub fn show(ui: &mut Ui, graphml: &GraphMl, selected: Option<&str>) {
    egui::Panel::right("side_panel").show(ui, |ui| {
        // Otherwise the panel shrinks to fit its contents, even after resizing.
        ui.take_available_width();
        let node = selected.and_then(|id| Some((id, document::graphics_of(graphml, id)?)));
        match node {
            Some((id, graphics)) => properties(ui, id, &graphics),
            None => {
                ui.weak("Nothing selected");
            }
        }
    });
}

fn properties(ui: &mut Ui, id: &str, graphics: &NodeGraphics) {
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
            row(ui, "Fill", &color(graphics.fill.color));
            if let Some(gradient) = graphics.fill.gradient {
                row(ui, "Gradient", &gradient.to_string());
            }
            row(ui, "Border", &color(border.color));
            row(ui, "Border width", &number(border.width));
            row(ui, "Border style", border.line_type.as_str());
            for label in &graphics.labels {
                row(ui, "Label", &label.text);
            }
        });
}

/// Adds a row, truncating a long value, which then shows in full on hover.
fn row(ui: &mut Ui, name: &str, value: &str) {
    ui.label(name);
    ui.add(Label::new(value).truncate());
    ui.end_row();
}

/// Up to two decimals, so yEd's 327.33333333333337 reads as 327.33.
fn number(value: f64) -> String {
    emath::format_with_decimals_in_range(value, 0..=2)
}

fn color(color: Option<Color>) -> String {
    color.map_or("None".to_owned(), |color| color.to_string())
}

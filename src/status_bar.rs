use std::path::Path;

use egui::{FontSelection, Label, WidgetText};

use crate::tool::Tool;

/// What stands in for the file of a graph that isn't saved.
pub const NOT_SAVED: &str = "<Not Saved>";

/// Shows the status bar along the bottom of the window: what `tool` does,
/// unless the drawing is `exporting`, and the file, with a '*' after it if
/// the graph is `modified`.
pub fn show(ui: &mut egui::Ui, tool: Tool, exporting: bool, file: Option<&Path>, modified: bool) {
    egui::Panel::bottom("status_bar").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(if exporting {
                "Exporting…"
            } else {
                status(tool)
            });
            ui.separator();
            let file = file.map_or(NOT_SAVED.into(), Path::to_string_lossy);
            let marker = WidgetText::from(if modified { "*" } else { "" }).into_galley(
                ui,
                None,
                f32::INFINITY,
                FontSelection::Default,
            );
            // A truncated path shows in full on hover, and the marker stays.
            ui.spacing_mut().item_spacing.x = 0.0;
            let width = ui.available_width() - marker.size().x;
            ui.scope(|ui| {
                ui.set_max_width(width);
                ui.add(Label::new(file).truncate());
            });
            ui.label(marker);
        });
    });
}

fn status(tool: Tool) -> &'static str {
    match tool {
        Tool::Cursor => "Ready",
        Tool::Box => "Drawing a box",
        Tool::Circle => "Drawing a circle",
        Tool::Ellipse => "Drawing an ellipse",
        Tool::Line => "Drawing a line",
        Tool::Arrow => "Drawing an arrow",
        Tool::TextBox => "Drawing a text box",
    }
}

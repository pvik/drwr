use std::path::Path;

use crate::tool::Tool;

/// Shows the status bar along the bottom of the window.
pub fn show(ui: &mut egui::Ui, tool: Tool, file: Option<&Path>) {
    egui::Panel::bottom("status_bar").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(status(tool));
            ui.separator();
            // A truncated path shows in full on hover.
            let file = file.map_or("<Not Saved>".into(), Path::to_string_lossy);
            ui.add(egui::Label::new(file).truncate());
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

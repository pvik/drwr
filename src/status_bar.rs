use crate::tool::Tool;

/// Shows the status bar along the bottom of the window.
pub fn show(ui: &mut egui::Ui, tool: Tool) {
    egui::Panel::bottom("status_bar").show(ui, |ui| {
        ui.label(status(tool));
    });
}

fn status(tool: Tool) -> &'static str {
    match tool {
        Tool::Cursor => "Ready",
        Tool::Box => "Drawing a box",
        Tool::Circle => "Drawing a circle",
        Tool::Ellipse => "Drawing an ellipse",
        Tool::TextBox => "Drawing a text box",
    }
}

use crate::tool::Tool;

/// Shows the status bar along the bottom of the window. It says what the
/// selected `tool` is drawing, or "Ready" while the cursor is selected.
///
/// egui lays out panels in the order they are added, so call this before the
/// [`egui::CentralPanel`].
pub fn show(ui: &mut egui::Ui, tool: Tool) {
    egui::Panel::bottom("status_bar").show(ui, |ui| {
        ui.label(status(tool));
    });
}

/// The status bar's text while `tool` is selected.
fn status(tool: Tool) -> &'static str {
    match tool {
        Tool::Cursor => "Ready",
        Tool::Box => "Drawing a box",
        Tool::Circle => "Drawing a circle",
        Tool::Ellipse => "Drawing an ellipse",
        Tool::TextBox => "Drawing a text box",
    }
}

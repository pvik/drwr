/// Shows the side panel to the right of the canvas. It is meant to hold the
/// properties of the selected objects, and is empty for now.
///
/// The panel can be resized by dragging its left edge.
///
/// egui lays out panels in the order they are added, so call this after
/// [`crate::toolbar::show`] and [`crate::status_bar::show`], which keeps the
/// panel between them, and before the [`egui::CentralPanel`].
pub fn show(ui: &mut egui::Ui) {
    egui::Panel::right("side_panel").show(ui, |ui| {
        // A panel keeps the width of its contents from the previous frame, so
        // an empty one would shrink to its minimum width, and wouldn't stay
        // wider when dragged. Taking up the whole width keeps the width the
        // panel was given.
        ui.take_available_width();
    });
}

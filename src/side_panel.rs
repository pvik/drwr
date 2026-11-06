/// Shows a resizable panel right of the canvas, meant for the selected
/// objects' properties. Empty for now.
pub fn show(ui: &mut egui::Ui) {
    egui::Panel::right("side_panel").show(ui, |ui| {
        // An empty panel shrinks to its minimum width, even after resizing.
        ui.take_available_width();
    });
}

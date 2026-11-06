/// Shows the status bar along the bottom of the window.
///
/// egui lays out panels in the order they are added, so call this before the
/// [`egui::CentralPanel`].
pub fn show(ui: &mut egui::Ui) {
    egui::Panel::bottom("status_bar").show(ui, |ui| {
        // Placeholder until there is some status to report.
        ui.label("Ready");
    });
}

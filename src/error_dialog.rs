/// Shows `error` in a modal dialog until it's dismissed.
pub fn show(ui: &mut egui::Ui, error: &mut Option<String>) {
    let Some(message) = error else { return };
    let modal = egui::Modal::new(egui::Id::unique("error_dialog")).show(ui.ctx(), |ui| {
        ui.set_max_width(400.0);
        ui.label(message.as_str());
        if ui.button("OK").clicked() {
            ui.close();
        }
    });
    if modal.should_close() {
        *error = None;
    }
}

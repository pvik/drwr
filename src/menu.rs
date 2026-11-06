/// Shows the application's top menu bar.
///
/// egui lays out panels in the order they are added, so call this before the
/// [`egui::CentralPanel`].
pub fn show(ui: &mut egui::Ui) {
    egui::Panel::top("menu_bar").show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Quit").clicked() {
                    ui.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
    });
}

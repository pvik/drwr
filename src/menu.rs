use crate::open_dialog::OpenDialog;

/// Shows the menu bar at the top of the window.
pub fn show(ui: &mut egui::Ui, frame: &eframe::Frame, open_dialog: &mut OpenDialog) {
    egui::Panel::top("menu_bar").show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New").clicked() {
                    // TODO: replace the graph with an empty, unsaved one.
                }
                let open = egui::Button::new("Open");
                if ui.add_enabled(!open_dialog.is_open(), open).clicked() {
                    open_dialog.open(ui.ctx(), frame);
                }
                if ui.button("Save").clicked() {
                    // TODO: save the graph, asking for a file if it's unsaved.
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    ui.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
    });
}

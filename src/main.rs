mod menu;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions::default();

    // The app name doubles as the default window title and the Wayland app id.
    eframe::run_native(
        "drwr",
        options,
        Box::new(|_cc| Ok(Box::new(DrwrApp::default()))),
    )
}

#[derive(Default)]
struct DrwrApp {}

impl eframe::App for DrwrApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        menu::show(ui);

        // The central panel must be added last, after all other panels.
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Hello, world!");
        });
    }
}
